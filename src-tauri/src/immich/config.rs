use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use super::secrets;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ImmichConfig {
    pub server_url: String,
    pub api_key: String,
    /// Open the RAW stacked under an image instead of the image itself.
    pub prefer_raw: bool,
    /// Upload exports of Immich images back to the album they came from.
    pub upload_exports: bool,
    /// Move the previously uploaded export of the same image to Immich's trash.
    pub replace_previous_export: bool,
    /// Take the RAW out of the album once its export is in it. Immich does not
    /// collapse stacks inside albums, so the album would show both.
    pub raw_leaves_album: bool,
    /// Where originals are cached. Defaults to the app cache folder.
    pub cache_dir: Option<String>,
    /// Downloaded originals beyond this size are removed, oldest first.
    pub cache_limit_gb: u32,
    /// Whether the API key is in the system's credential store rather than in
    /// this file. Set when saving; shown in the settings.
    pub key_in_credential_store: bool,
}

impl Default for ImmichConfig {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            api_key: String::new(),
            prefer_raw: true,
            upload_exports: true,
            replace_previous_export: false,
            raw_leaves_album: true,
            cache_dir: None,
            cache_limit_gb: 20,
            key_in_credential_store: false,
        }
    }
}

impl ImmichConfig {
    pub fn is_configured(&self) -> bool {
        !self.server_url.trim().is_empty() && !self.api_key.trim().is_empty()
    }
}

fn config_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(dir.join("immich.json"))
}

pub fn default_cache_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let dir = app_handle
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?;
    Ok(dir.join("immich"))
}

pub fn load(app_handle: &AppHandle) -> ImmichConfig {
    let mut config: ImmichConfig = config_path(app_handle)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default();
    if config.key_in_credential_store {
        config.api_key = secrets::load().unwrap_or_default();
    }
    config
}

/// Saves the settings. The API key goes into the credential store if there
/// is one, and only otherwise into the file.
pub fn save(app_handle: &AppHandle, config: &ImmichConfig) -> Result<(), String> {
    let path = config_path(app_handle)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut stored = config.clone();
    stored.key_in_credential_store = secrets::save(&config.api_key) && !config.api_key.is_empty();
    if stored.key_in_credential_store {
        stored.api_key.clear();
    }
    let json = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    restrict_permissions(&path);
    Ok(())
}

/// The file may hold the API key, so only the owner may read it.
#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}
