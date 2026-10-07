//! Maps Immich listings to cache paths. An image stacked with a RAW - how
//! exports are uploaded - resolves to the RAW.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::client::{Asset, ImmichClient, Stack};
use super::config::ImmichSettings;
use super::registry::{self, RemoteImage};

#[derive(Debug, Clone)]
pub struct Resolved {
    pub path: PathBuf,
    pub image: RemoteImage,
    pub file_modified_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Filter {
    pub album_id: Option<String>,
    pub not_in_album: bool,
    /// `YYYY-MM-DD`, both days included.
    pub taken_from: Option<String>,
    pub taken_until: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub person_ids: Vec<String>,
    pub favorites_only: bool,
}

impl Filter {
    fn to_query(&self) -> Value {
        let mut query = json!({ "type": "IMAGE" });
        let set = |query: &mut Value, key: &str, value: &Option<String>| {
            if let Some(v) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                query[key] = json!(v);
            }
        };
        if let Some(album) = &self.album_id {
            query["albumIds"] = json!([album]);
        }
        if self.not_in_album {
            query["isNotInAlbum"] = json!(true);
        }
        if let Some(from) = day_start(self.taken_from.as_deref(), 0) {
            query["takenAfter"] = json!(from);
        }
        if let Some(until) = day_start(self.taken_until.as_deref(), 1) {
            query["takenBefore"] = json!(until);
        }
        set(&mut query, "country", &self.country);
        set(&mut query, "city", &self.city);
        set(&mut query, "make", &self.make);
        set(&mut query, "model", &self.model);
        if !self.person_ids.is_empty() {
            query["personIds"] = json!(self.person_ids);
        }
        if self.favorites_only {
            query["isFavorite"] = json!(true);
        }
        query
    }
}

fn day_start(day: Option<&str>, offset: i64) -> Option<String> {
    let date = chrono::NaiveDate::parse_from_str(day?.trim(), "%Y-%m-%d").ok()?;
    let date = date.checked_add_signed(chrono::Duration::days(offset))?;
    Some(format!("{}T00:00:00.000Z", date.format("%Y-%m-%d")))
}

fn is_raw(asset: &Asset) -> bool {
    crate::formats::is_raw_file(&asset.original_file_name)
}

/// Keeps file names from the server from escaping the cache folder.
fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim_start_matches('.');
    if trimmed.is_empty() {
        "image".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn cache_path(asset: &Asset, cache_dir: &Path) -> PathBuf {
    cache_dir
        .join(&asset.id)
        .join(safe_file_name(&asset.original_file_name))
}

struct StackIndex {
    stacks: Vec<Stack>,
    by_asset: HashMap<String, usize>,
}

impl StackIndex {
    fn new(stacks: Vec<Stack>) -> Self {
        let by_asset = stacks
            .iter()
            .enumerate()
            .flat_map(|(i, s)| s.assets.iter().map(move |a| (a.id.clone(), i)))
            .collect();
        Self { stacks, by_asset }
    }

    /// Another image of the same stack that is on top of `asset`, if any.
    fn primary_of(&self, asset: &Asset) -> Option<&str> {
        let stack = &self.stacks[*self.by_asset.get(&asset.id)?];
        (stack.primary_asset_id != asset.id).then_some(stack.primary_asset_id.as_str())
    }

    fn companions<'a>(&'a self, asset: &'a Asset) -> impl Iterator<Item = &'a Asset> {
        self.by_asset
            .get(&asset.id)
            .map(|&i| self.stacks[i].assets.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(move |a| a.id != asset.id && !a.is_trashed)
    }

    fn source<'a>(&'a self, listed: &'a Asset, cache_dir: &Path) -> &'a Asset {
        if is_raw(listed) {
            return listed;
        }
        self.companions(listed)
            .filter(|a| is_raw(a))
            .max_by_key(|raw| cache_path(raw, cache_dir).exists())
            .unwrap_or(listed)
    }
}

pub async fn listing(
    client: &ImmichClient,
    settings: &ImmichSettings,
    filter: &Filter,
    cache_dir: &Path,
) -> Result<(Vec<Resolved>, bool), String> {
    let mut query = filter.to_query();
    query["order"] = json!("desc");
    let limit = settings.listing_limit.clamp(100, 50_000) as usize;
    let (found, truncated) = client.search_up_to(query, Some(limit)).await?;
    let assets: Vec<Asset> = found
        .into_iter()
        .filter(|a| a.kind == "IMAGE" && !registry::is_trashing(&a.id))
        .collect();

    // One request for all stacks is far cheaper than one per image.
    let stacks = if settings.open_stacked_raw || filter.not_in_album {
        StackIndex::new(client.stacks().await?)
    } else {
        StackIndex::new(Vec::new())
    };
    let assets = if filter.not_in_album {
        without_sorted_stacks(assets, &stacks)
    } else {
        assets
    };
    let resolved = resolve(
        &assets,
        &stacks,
        settings.open_stacked_raw,
        filter.album_id.as_deref(),
        cache_dir,
    );
    Ok((resolved, truncated))
}

fn resolve(
    assets: &[Asset],
    stacks: &StackIndex,
    open_stacked_raw: bool,
    album_id: Option<&str>,
    cache_dir: &Path,
) -> Vec<Resolved> {
    let listed_ids: HashSet<&str> = assets.iter().map(|a| a.id.as_str()).collect();
    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(assets.len());
    for listed in assets {
        // One entry per stack: its top image stands for it.
        if stacks
            .primary_of(listed)
            .is_some_and(|primary| listed_ids.contains(primary))
        {
            continue;
        }
        let source = if open_stacked_raw {
            stacks.source(listed, cache_dir)
        } else {
            listed
        };
        if registry::is_trashing(&source.id) {
            continue;
        }
        let path = cache_path(source, cache_dir);
        if !crate::formats::is_supported_image_file(&path) || !seen.insert(path.clone()) {
            continue;
        }
        resolved.push(Resolved {
            path,
            file_modified_at: source.file_modified_at.clone(),
            image: RemoteImage {
                asset_id: source.id.clone(),
                listed_asset_id: listed.id.clone(),
                album_id: album_id.map(str::to_string),
            },
        });
    }
    resolved
}

/// Albums hold only the top of a stack, so a stack counts as sorted as soon
/// as any of its images is in an album.
fn without_sorted_stacks(assets: Vec<Asset>, stacks: &StackIndex) -> Vec<Asset> {
    let unassigned: HashSet<&str> = assets.iter().map(|a| a.id.as_str()).collect();
    let sorted: HashSet<String> = assets
        .iter()
        .filter(|a| {
            stacks
                .companions(a)
                .any(|c| !unassigned.contains(c.id.as_str()))
        })
        .map(|a| a.id.clone())
        .collect();
    assets
        .into_iter()
        .filter(|a| !sorted.contains(&a.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(id: &str, name: &str) -> Asset {
        Asset {
            id: id.into(),
            kind: "IMAGE".into(),
            original_file_name: name.into(),
            file_modified_at: None,
            is_trashed: false,
        }
    }

    fn stack(assets: Vec<Asset>) -> Stack {
        Stack {
            primary_asset_id: assets[0].id.clone(),
            assets,
        }
    }

    #[test]
    fn opens_the_raw_stacked_under_an_export() {
        let jpeg = asset("j", "DSC1_edited.jpg");
        let stacks = StackIndex::new(vec![stack(vec![jpeg.clone(), asset("r", "DSC1.ARW")])]);
        let resolved = resolve(&[jpeg], &stacks, true, Some("album"), Path::new("/cache"));
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].image.asset_id, "r");
        assert_eq!(resolved[0].image.listed_asset_id, "j");
        assert_eq!(resolved[0].path, Path::new("/cache/r/DSC1.ARW"));
    }

    #[test]
    fn ignores_trashed_raws_and_unstacked_images() {
        let jpeg = asset("j", "DSC1_edited.jpg");
        let mut trashed = asset("r", "DSC1.ARW");
        trashed.is_trashed = true;
        let phone = asset("p", "IMG_0001.jpg");
        let stacks = StackIndex::new(vec![stack(vec![jpeg.clone(), trashed])]);
        let resolved = resolve(&[jpeg, phone], &stacks, true, None, Path::new("/c"));
        let ids: Vec<_> = resolved.iter().map(|r| r.image.asset_id.as_str()).collect();
        assert_eq!(ids, ["j", "p"]);
    }

    #[test]
    fn shows_a_stack_once_as_raw_or_as_its_top_image() {
        let jpeg = asset("j", "DSC1_edited.jpg");
        let raw = asset("r", "DSC1.ARW");
        let stacks = StackIndex::new(vec![stack(vec![jpeg.clone(), raw.clone()])]);
        let listing = [raw, jpeg];
        let as_raw = resolve(&listing, &stacks, true, None, Path::new("/c"));
        let as_top = resolve(&listing, &stacks, false, None, Path::new("/c"));
        assert_eq!(as_raw.len(), 1);
        assert_eq!(as_raw[0].image.asset_id, "r");
        assert_eq!(as_top.len(), 1);
        assert_eq!(as_top[0].image.asset_id, "j");
    }

    #[test]
    fn stacks_with_an_image_in_an_album_count_as_sorted() {
        // "j" and "b1" are in albums, so they are not part of the listing.
        let stacks = StackIndex::new(vec![
            stack(vec![asset("j", "DSC1_edited.jpg"), asset("r", "DSC1.ARW")]),
            stack(vec![asset("b1", "DSC3.jpg"), asset("b2", "DSC4.jpg")]),
            stack(vec![
                asset("o1", "DSC5_edited.jpg"),
                asset("o2", "DSC5.ARW"),
            ]),
        ]);
        let listing = ["r", "b2", "o1", "o2", "u"].map(|id| asset(id, "x.jpg"));
        let kept = without_sorted_stacks(listing.to_vec(), &stacks);
        let ids: Vec<_> = kept.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, ["o1", "o2", "u"]);
    }

    #[test]
    fn filter_days_are_inclusive() {
        let filter = Filter {
            taken_from: Some("2025-10-14".into()),
            taken_until: Some("2025-10-28".into()),
            country: Some("  ".into()),
            ..Default::default()
        };
        let query = filter.to_query();
        assert_eq!(query["takenAfter"], "2025-10-14T00:00:00.000Z");
        assert_eq!(query["takenBefore"], "2025-10-29T00:00:00.000Z");
        assert!(query.get("country").is_none());
        assert!(query.get("albumIds").is_none());
    }

    #[test]
    fn cache_names_stay_inside_the_folder() {
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_file_name("DSC1.ARW"), "DSC1.ARW");
    }
}
