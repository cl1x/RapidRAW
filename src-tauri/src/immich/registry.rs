//! Which local paths belong to Immich assets.
//!
//! Filled whenever an Immich album is listed. The rest of the app only ever
//! sees ordinary file paths; the hooks in `mod.rs` look paths up here to decide
//! whether a file still has to be downloaded, whose edits to sync, or whether
//! an export should be uploaded.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Clone)]
pub struct RemoteImage {
    /// The asset whose original lives at this path - the RAW, if one was found.
    pub asset_id: String,
    /// The asset that appears in the album, e.g. a previously exported JPEG.
    /// Equal to `asset_id` when the album contains the original itself.
    pub listed_asset_id: String,
    /// The album the image was opened from.
    pub album_id: Option<String>,
}

static ENTRIES: Lazy<RwLock<HashMap<PathBuf, RemoteImage>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub fn insert(path: PathBuf, image: RemoteImage) {
    ENTRIES.write().unwrap().insert(path, image);
}

pub fn get(path: &Path) -> Option<RemoteImage> {
    ENTRIES.read().unwrap().get(path).cloned()
}

pub fn all() -> Vec<(PathBuf, RemoteImage)> {
    ENTRIES
        .read()
        .unwrap()
        .iter()
        .map(|(path, image)| (path.clone(), image.clone()))
        .collect()
}

/// Called after an export was uploaded, so the next export of the same image
/// replaces the new one.
pub fn set_listed_asset(path: &Path, listed_asset_id: &str) {
    if let Some(entry) = ENTRIES.write().unwrap().get_mut(path) {
        entry.listed_asset_id = listed_asset_id.to_string();
    }
}

/// An Immich asset that is known but not downloaded yet.
pub fn is_placeholder(path: &Path) -> bool {
    let known = ENTRIES
        .read()
        .map(|entries| entries.contains_key(path))
        .unwrap_or(false);
    known && !path.exists()
}

pub fn remove(path: &Path) {
    ENTRIES.write().unwrap().remove(path);
}

pub fn clear() {
    ENTRIES.write().unwrap().clear();
}
