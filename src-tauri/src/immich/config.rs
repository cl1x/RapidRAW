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
    #[serde(alias = "preferRaw")]
    pub open_stacked_raw: bool,
    /// The library reads the EXIF data of every listed image, which gets slow
    /// for large listings.
    pub listing_limit: u32,
    pub sync_edits: bool,
    pub upload_exports: bool,
    pub cache_dir: Option<String>,
    pub cache_limit_gb: u32,
    /// Set when saving; the key is then not stored in this file.
    pub key_in_credential_store: bool,
}

impl Default for ImmichConfig {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            api_key: String::new(),
            open_stacked_raw: true,
            listing_limit: 2000,
            sync_edits: true,
            upload_exports: true,
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

    /// Whether switching to `other` may change which files belong to which
    /// assets: another server, account or cache folder.
    pub fn same_library(&self, other: &ImmichConfig) -> bool {
        self.server_url.trim() == other.server_url.trim()
            && self.api_key.trim() == other.api_key.trim()
            && self.cache_dir == other.cache_dir
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

#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}
