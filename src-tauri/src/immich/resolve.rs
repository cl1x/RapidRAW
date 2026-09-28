//! Turns the assets of an Immich album into local paths RapidRAW can edit.

use futures::stream::{self, StreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::client::{Asset, ImmichClient};
use super::config::ImmichConfig;
use super::registry::RemoteImage;

/// Parallel requests while looking up RAW originals.
const LOOKUP_CONCURRENCY: usize = 12;

#[derive(Debug, Clone)]
pub struct Resolved {
    pub path: PathBuf,
    pub image: RemoteImage,
    pub file_modified_at: Option<String>,
}

/// RAW lookups per listed asset id, kept for the session. An empty list means
/// the asset has no RAW, which is the common case and worth remembering too.
static RAW_MATCHES: Lazy<Mutex<HashMap<String, Vec<Asset>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn forget_raw_matches() {
    RAW_MATCHES.lock().unwrap().clear();
}

/// The file name an exported image most likely came from.
///
/// RapidRAW's default export template is `{original_filename}_edited`, and a
/// clashing name gets a counter appended (`_edited_2`). Anything else is
/// taken as is, which also pairs in-camera JPEGs with their RAW.
pub fn source_stem(file_name: &str) -> String {
    static EXPORT_SUFFIX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)^(.+?)_edited(?:_\d+)?$").unwrap());

    let stem = Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    EXPORT_SUFFIX
        .captures(&stem)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or(stem)
}

fn is_raw(asset: &Asset) -> bool {
    crate::formats::is_raw_file(&asset.original_file_name)
}

fn file_stem(file_name: &str) -> String {
    Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default()
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

/// Where an asset's original is cached: one folder per asset, so the file
/// keeps its name and its sidecar sits next to it.
pub fn cache_path(asset: &Asset, cache_dir: &Path) -> PathBuf {
    cache_dir
        .join(&asset.id)
        .join(safe_file_name(&asset.original_file_name))
}

async fn find_raws(client: &ImmichClient, listed: &Asset) -> Vec<Asset> {
    let known = RAW_MATCHES.lock().unwrap().get(&listed.id).cloned();
    if let Some(known) = known {
        return known;
    }

    let stem = source_stem(&listed.original_file_name);
    let wanted = stem.to_lowercase();
    // The search matches substrings, so filter for the exact stem.
    let found: Vec<Asset> = same_stem(client, &stem)
        .await
        .into_iter()
        .filter(|a| a.id != listed.id && is_raw(a) && file_stem(&a.original_file_name) == wanted)
        .collect();

    RAW_MATCHES
        .lock()
        .unwrap()
        .insert(listed.id.clone(), found.clone());
    found
}

/// Several RAWs may share a name, e.g. the same file uploaded twice. One that
/// is not in the trash wins, and among those one that is already cached.
fn choose<'a>(listed: &'a Asset, raw_candidates: &'a [Asset], cache_dir: &Path) -> &'a Asset {
    raw_candidates
        .iter()
        .filter(|raw| !raw.is_trashed)
        .max_by_key(|raw| cache_path(raw, cache_dir).exists())
        .unwrap_or(listed)
}

/// Up to this many images, the RAW behind every JPEG is looked up. Beyond it
/// only exports are, since a search per image would take minutes.
const FULL_LOOKUP_LIMIT: usize = 2000;

/// Which images to list. Maps onto Immich's metadata search; everything left
/// empty is not filtered on.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Filter {
    pub album_id: Option<String>,
    /// Only images that are in no album.
    pub not_in_album: bool,
    /// First and last day, as `YYYY-MM-DD`; both days are included.
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

/// Midnight (UTC) of `day` plus `offset` days, as Immich expects it.
fn day_start(day: Option<&str>, offset: i64) -> Option<String> {
    let date = chrono::NaiveDate::parse_from_str(day?.trim(), "%Y-%m-%d").ok()?;
    let date = date.checked_add_signed(chrono::Duration::days(offset))?;
    Some(format!("{}T00:00:00.000Z", date.format("%Y-%m-%d")))
}

/// Assets named like `stem` or like an export of it (`stem_edited.jpg`).
async fn same_stem(client: &ImmichClient, stem: &str) -> Vec<Asset> {
    let wanted = stem.to_lowercase();
    client
        .search(json!({ "originalFileName": stem, "type": "IMAGE", "size": 200 }))
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|a| source_stem(&a.original_file_name).to_lowercase() == wanted)
        .collect()
}

pub async fn listing(
    client: &ImmichClient,
    config: &ImmichConfig,
    filter: &Filter,
    cache_dir: &Path,
) -> Result<Vec<Resolved>, String> {
    let mut assets: Vec<Asset> = client
        .search(filter.to_query())
        .await?
        .into_iter()
        .filter(|a| a.kind == "IMAGE")
        .collect();

    if filter.not_in_album {
        assets = without_developed_raws(client, assets).await;
    }

    // Look up RAWs for everything that is not one already. A listing that
    // holds both the RAW and its export shows the RAW once. Everything else
    // is shown and edited as it is.
    let full_lookup = assets.len() <= FULL_LOOKUP_LIMIT;
    let raws: HashMap<String, Vec<Asset>> = if config.prefer_raw {
        // Owned items: a stream over borrowed assets cannot be proven `Send`
        // for every lifetime, which Tauri requires of async commands.
        let lookups: Vec<Asset> = assets
            .iter()
            .filter(|a| !is_raw(a) && (full_lookup || is_export(a)))
            .cloned()
            .collect();
        stream::iter(lookups)
            .map(|asset| async move {
                let raws = find_raws(client, &asset).await;
                (asset.id, raws)
            })
            .buffer_unordered(LOOKUP_CONCURRENCY)
            .collect()
            .await
    } else {
        HashMap::new()
    };

    let mut seen = HashSet::new();
    let mut resolved = Vec::with_capacity(assets.len());
    for listed in &assets {
        let raw_candidates = raws.get(&listed.id).map(Vec::as_slice).unwrap_or_default();
        let source = choose(listed, raw_candidates, cache_dir);
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
                album_id: filter.album_id.clone(),
            },
        });
    }
    Ok(resolved)
}

fn is_export(asset: &Asset) -> bool {
    let stem = Path::new(&asset.original_file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    source_stem(&asset.original_file_name) != stem
}

/// A RAW is usually kept out of albums once its export is in one. Such a RAW
/// is sorted, not forgotten, so it is left out of the images in no album.
async fn without_developed_raws(client: &ImmichClient, assets: Vec<Asset>) -> Vec<Asset> {
    let unassigned: HashSet<String> = assets.iter().map(|a| a.id.clone()).collect();
    let raws: Vec<Asset> = assets.iter().filter(|a| is_raw(a)).cloned().collect();
    let unassigned = &unassigned;
    let developed: HashSet<String> = stream::iter(raws)
        .map(|raw| async move {
            let stem = Path::new(&raw.original_file_name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let in_album = same_stem(client, &stem)
                .await
                .iter()
                .any(|a| !is_raw(a) && !unassigned.contains(&a.id));
            in_album.then_some(raw.id)
        })
        .buffer_unordered(LOOKUP_CONCURRENCY)
        .filter_map(|id| async move { id })
        .collect()
        .await;
    assets
        .into_iter()
        .filter(|a| !developed.contains(&a.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_rapidraw_export_suffix() {
        assert_eq!(source_stem("DSC02193_edited.jpg"), "DSC02193");
        assert_eq!(source_stem("DSC02193_Edited_2.jpg"), "DSC02193");
        assert_eq!(source_stem("IMG_1234.JPG"), "IMG_1234");
        assert_eq!(
            source_stem("holiday_edited_final.jpg"),
            "holiday_edited_final"
        );
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
