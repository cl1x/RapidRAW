//! Immich integration. Immich assets are mapped onto paths in a local cache, so
//! the rest of the app keeps working on files; the functions exported here are
//! called from the few places where that is not enough.

mod client;
pub mod commands;
mod config;
mod files;
mod registry;
mod resolve;
mod secrets;
mod sync;
mod thumbnails;
mod trash;
mod uploads;

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::AppHandle;

use client::ImmichClient;

pub use config::ImmichSettings;
pub use files::{ensure_local, ensure_local_all};
pub use thumbnails::placeholder_thumbnail;
pub use trash::trash_remote;
pub use uploads::{add_to_album, on_exported};

const MIN_VERSION: (u32, u32, u32) = (2, 0, 0);
const MIN_VERSION_FOR_EDITS: (u32, u32, u32) = (2, 5, 0);

struct Session {
    settings: ImmichSettings,
    client: Arc<ImmichClient>,
    cache_dir: PathBuf,
    version: tokio::sync::OnceCell<(u32, u32, u32)>,
}

impl Session {
    async fn syncs_edits(&self) -> bool {
        if !self.settings.sync_edits {
            return false;
        }
        self.version
            .get_or_try_init(|| self.client.version_numbers())
            .await
            .is_ok_and(|version| *version >= MIN_VERSION_FOR_EDITS)
    }
}

static SESSION: Lazy<RwLock<Option<Arc<Session>>>> = Lazy::new(|| RwLock::new(None));

fn session(app_handle: &AppHandle) -> Result<Arc<Session>, String> {
    if let Some(session) = SESSION.read().unwrap().as_ref() {
        return Ok(session.clone());
    }
    let settings = config::load(app_handle);
    let api_key = config::load_api_key(app_handle).api_key;
    if settings.server_url.trim().is_empty() || api_key.is_empty() {
        return Err("Immich is not set up yet. Add the server in Settings → Immich.".to_string());
    }
    let client = Arc::new(ImmichClient::new(&settings.server_url, &api_key)?);
    let cache_dir = cache_dir(app_handle, &settings)?;
    let session = Arc::new(Session {
        settings,
        client,
        cache_dir,
        version: tokio::sync::OnceCell::new(),
    });
    *SESSION.write().unwrap() = Some(session.clone());
    sync::start_loop(app_handle);
    files::prune_cache_later(&session);
    Ok(session)
}

/// Rebuilds the session with the saved settings on next use. The images known
/// so far stay known unless they may now live on another server or in another
/// cache folder.
fn reset_session(forget_images: bool) {
    *SESSION.write().unwrap() = None;
    if forget_images {
        registry::clear();
    }
}

pub fn apply_settings(settings: &ImmichSettings) {
    let current = SESSION.read().unwrap().as_ref().map(|s| s.settings.clone());
    if let Some(current) = current
        && current != *settings
    {
        reset_session(!current.same_library(settings));
    }
}

fn cache_dir(app_handle: &AppHandle, settings: &ImmichSettings) -> Result<PathBuf, String> {
    match settings.cache_dir.as_deref().map(str::trim) {
        Some(dir) if !dir.is_empty() => Ok(PathBuf::from(dir)),
        _ => config::default_cache_dir(app_handle),
    }
}

pub fn is_placeholder(path: &Path) -> bool {
    registry::is_placeholder(path)
}
