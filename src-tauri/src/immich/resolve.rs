//! Turns the assets of an Immich album into local paths RapidRAW can edit.

use futures::stream::{self, StreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::json;
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
    let candidates = client
        .search(json!({
            "originalFileName": stem,
            "type": "IMAGE",
            "size": 200,
        }))
        .await
        .unwrap_or_default();

    let found: Vec<Asset> = candidates
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

pub async fn album(
    client: &ImmichClient,
    config: &ImmichConfig,
    album_id: &str,
    cache_dir: &Path,
) -> Result<Vec<Resolved>, String> {
    let assets: Vec<Asset> = client
        .search(json!({ "albumIds": [album_id], "type": "IMAGE" }))
        .await?
        .into_iter()
        .filter(|a| a.kind == "IMAGE")
        .collect();

    // Look up RAWs for everything that is not one already. An album that
    // holds both the RAW and its export shows the RAW once.
    let raws: HashMap<String, Vec<Asset>> = if config.prefer_raw {
        stream::iter(assets.iter().filter(|a| !is_raw(a)))
            .map(|asset| async move { (asset.id.clone(), find_raws(client, asset).await) })
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
                album_id: Some(album_id.to_string()),
            },
        });
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_rapidraw_export_suffix() {
        assert_eq!(source_stem("DSC02193_edited.jpg"), "DSC02193");
        assert_eq!(source_stem("DSC02193_Edited_2.jpg"), "DSC02193");
        assert_eq!(source_stem("IMG_1234.JPG"), "IMG_1234");
        assert_eq!(source_stem("holiday_edited_final.jpg"), "holiday_edited_final");
    }

    #[test]
    fn cache_names_stay_inside_the_folder() {
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_file_name("DSC1.ARW"), "DSC1.ARW");
    }
}
