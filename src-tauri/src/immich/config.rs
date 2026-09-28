use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ImmichConfig {
    pub server_url: String,
    pub api_key: String,
    /// Open the RAW behind an exported JPEG instead of the JPEG itself.
    pub prefer_raw: bool,
    /// Upload exports of Immich images back to the album they came from.
    pub upload_exports: bool,
    /// Move the previously uploaded export of the same image to Immich's trash.
    pub replace_previous_export: bool,
    /// Where originals are cached. Defaults to the app cache folder.
    pub cache_dir: Option<String>,
    /// Downloaded originals beyond this size are removed, oldest first.
    pub cache_limit_gb: u32,
}

impl Default for ImmichConfig {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            api_key: String::new(),
            prefer_raw: true,
            upload_exports: true,
            replace_previous_export: false,
            cache_dir: None,
            cache_limit_gb: 20,
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
    config_path(app_handle)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn save(app_handle: &AppHandle, config: &ImmichConfig) -> Result<(), String> {
    let path = config_path(app_handle)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    restrict_permissions(&path);
    Ok(())
}

/// The file holds an API key, so only the owner may read it.
#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}
