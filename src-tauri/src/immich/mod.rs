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
use config::ImmichConfig;

pub use files::{ensure_local, ensure_local_all};
pub use thumbnails::placeholder_thumbnail;
pub use trash::trash_remote;
pub use uploads::{add_to_album, on_exported};

struct Session {
    config: ImmichConfig,
    client: Arc<ImmichClient>,
    cache_dir: PathBuf,
}

static SESSION: Lazy<RwLock<Option<Arc<Session>>>> = Lazy::new(|| RwLock::new(None));

fn session(app_handle: &AppHandle) -> Result<Arc<Session>, String> {
    if let Some(session) = SESSION.read().unwrap().as_ref() {
        return Ok(session.clone());
    }
    let config = config::load(app_handle);
    if !config.is_configured() {
        return Err("Immich is not set up yet. Add the server in Settings → Immich.".to_string());
    }
    let client = Arc::new(ImmichClient::new(&config.server_url, &config.api_key)?);
    let cache_dir = cache_dir(app_handle, &config)?;
    let session = Arc::new(Session {
        config,
        client,
        cache_dir,
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

fn cache_dir(app_handle: &AppHandle, config: &ImmichConfig) -> Result<PathBuf, String> {
    match config.cache_dir.as_deref().map(str::trim) {
        Some(dir) if !dir.is_empty() => Ok(PathBuf::from(dir)),
        _ => config::default_cache_dir(app_handle),
    }
}

pub fn is_placeholder(path: &Path) -> bool {
    registry::is_placeholder(path)
}
