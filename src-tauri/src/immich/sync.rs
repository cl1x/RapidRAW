//! Keeps RapidRAW's sidecar of an Immich image in Immich itself, as asset
//! metadata, so edits follow the image to every machine.
//!
//! The sidecar in the cache folder stays the working copy; RapidRAW reads and
//! writes it as always. Next to it, `.immich-sync.json` remembers which state
//! was last exchanged with the server. That is enough to tell a local change
//! (sidecar modified since) from a remote one (newer update time on the server).
//! If both changed, the local edit wins - it is the one the user just made.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::client::ImmichClient;

/// Metadata key the sidecar is stored under.
pub const EDITS_KEY: &str = "rapidraw";

const STATE_FILE: &str = ".immich-sync.json";

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SyncState {
    /// Modification time of the sidecar when it was last pulled or pushed.
    local_modified_ms: Option<u64>,
    /// Update time of the server entry when it was last pulled or pushed.
    remote_updated_at: Option<String>,
}

pub fn sidecar_of(source: &Path) -> PathBuf {
    crate::file_management::parse_virtual_path(&source.to_string_lossy()).1
}

fn state_file(source: &Path) -> PathBuf {
    source.with_file_name(STATE_FILE)
}

fn load_state(source: &Path) -> SyncState {
    fs::read_to_string(state_file(source))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(source: &Path, state: &SyncState) {
    if let Ok(json) = serde_json::to_string(state) {
        let _ = fs::write(state_file(source), json);
    }
}

fn modified_ms(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// True if the sidecar changed since the last exchange with the server.
pub fn has_local_changes(source: &Path) -> bool {
    match modified_ms(&sidecar_of(source)) {
        Some(modified) => load_state(source).local_modified_ms != Some(modified),
        None => false,
    }
}

/// Fetches edits made elsewhere, unless there are unsent local ones.
pub async fn pull(client: &ImmichClient, asset_id: &str, source: &Path) -> Result<(), String> {
    let Some(remote) = client.metadata(asset_id, EDITS_KEY).await? else {
        return Ok(());
    };
    let mut state = load_state(source);
    if state.remote_updated_at.as_deref() == Some(remote.updated_at.as_str()) {
        return Ok(());
    }
    if has_local_changes(source) {
        log::info!("Keeping local edits of {} over newer ones in Immich", source.display());
        return Ok(());
    }

    let sidecar = sidecar_of(source);
    if let Some(parent) = sidecar.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&remote.value).map_err(|e| e.to_string())?;
    fs::write(&sidecar, json).map_err(|e| e.to_string())?;

    state.local_modified_ms = modified_ms(&sidecar);
    state.remote_updated_at = Some(remote.updated_at);
    save_state(source, &state);
    Ok(())
}

/// Sends the sidecar to the server if it changed. Returns whether it did.
pub async fn push(client: &ImmichClient, asset_id: &str, source: &Path) -> Result<bool, String> {
    let sidecar = sidecar_of(source);
    let Some(modified) = modified_ms(&sidecar) else {
        return Ok(false);
    };
    let mut state = load_state(source);
    if state.local_modified_ms == Some(modified) {
        return Ok(false);
    }

    let content = fs::read_to_string(&sidecar).map_err(|e| e.to_string())?;
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        // Caught mid-write; the next round picks it up.
        return Ok(false);
    };
    if !value.is_object() {
        return Ok(false);
    }
    let updated_at = client.set_metadata(asset_id, EDITS_KEY, &value).await?;

    state.local_modified_ms = Some(modified);
    state.remote_updated_at = Some(updated_at);
    save_state(source, &state);
    Ok(true)
}

/// Removes cached originals, oldest download first, until the cache is below
/// `limit_bytes`. Sidecars stay, and so does every original whose edits have
/// not reached the server yet or that was fetched in the last hour.
pub fn prune_cache(cache_dir: &Path, limit_bytes: u64) {
    let Ok(folders) = fs::read_dir(cache_dir) else {
        return;
    };
    let mut originals = Vec::new();
    let mut total = 0u64;
    for folder in folders.filter_map(Result::ok) {
        let Ok(files) = fs::read_dir(folder.path()) else {
            continue;
        };
        for file in files.filter_map(Result::ok) {
            let path = file.path();
            let name = file.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.ends_with(".rrdata") || name.ends_with(".rrexif") {
                continue;
            }
            let Ok(meta) = file.metadata() else { continue };
            total += meta.len();
            originals.push((modified_ms(&path).unwrap_or(0), meta.len(), path));
        }
    }
    if total <= limit_bytes {
        return;
    }

    let an_hour_ago = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
        .saturating_sub(3_600_000);
    originals.sort_by_key(|(modified, ..)| *modified);
    for (modified, size, path) in originals {
        if total <= limit_bytes {
            break;
        }
        if modified > an_hour_ago || has_local_changes(&path) {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}
