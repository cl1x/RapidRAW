//! Immich integration: browse the albums of an Immich server in the library
//! and edit their originals.
//!
//! Everything goes through the Immich API; the server is the only place the
//! photos and their edits live. Locally there is just a cache, so the editor,
//! sidecars, presets and export keep working on files as everywhere else:
//!
//! * An album's images are shown with Immich's previews right away. The
//!   original is downloaded when an image is opened, into a cache folder that
//!   is trimmed to a size limit. Until then it counts as a cloud placeholder,
//!   the same way files still in iCloud are handled.
//! * Edits are stored in Immich as asset metadata (see `sync`), so they are
//!   the same on every machine.
//! * For exported JPEGs (`name_edited.jpg`), the RAW they came from is opened
//!   instead, if Immich has it.
//! * Exports of Immich images can be uploaded back into the album they were
//!   opened from and stacked on top of their RAW.
//!
//! The rest of the app talks to this module only through the few functions
//! below; see their call sites for the hooks.

mod client;
pub mod commands;
mod config;
mod registry;
mod resolve;
mod sync;

use once_cell::sync::Lazy;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use client::ImmichClient;
use config::ImmichConfig;

const THUMBNAIL_SMALL: u32 = 480;
const THUMBNAIL_MEDIUM: u32 = 1280;
/// How often local edits are checked and sent to the server.
const SYNC_INTERVAL: Duration = Duration::from_secs(5);

struct Session {
    config: ImmichConfig,
    client: Arc<ImmichClient>,
    cache_dir: PathBuf,
}

static SESSION: Lazy<RwLock<Option<Arc<Session>>>> = Lazy::new(|| RwLock::new(None));

/// Client and settings, created on first use and dropped when the settings
/// change.
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
    start_sync_loop(app_handle);
    prune_cache_later(&session);
    Ok(session)
}

fn reset_session() {
    *SESSION.write().unwrap() = None;
    registry::clear();
    resolve::forget_raw_matches();
}

fn cache_dir(app_handle: &AppHandle, config: &ImmichConfig) -> Result<PathBuf, String> {
    match config.cache_dir.as_deref().map(str::trim) {
        Some(dir) if !dir.is_empty() => Ok(PathBuf::from(dir)),
        _ => config::default_cache_dir(app_handle),
    }
}

// ---------------------------------------------------------------------------
// Hooks called from the rest of the app
// ---------------------------------------------------------------------------

/// True for an Immich asset that has not been downloaded yet. Folded into
/// `file_management::is_cloud_placeholder`, so the library marks it the same
/// way as a file that is still in iCloud.
pub fn is_placeholder(path: &Path) -> bool {
    registry::is_placeholder(path)
}

/// Makes sure the original behind `path` is on disk and its edits are the
/// latest from the server. Does nothing for paths that are not from Immich.
pub async fn ensure_local(app_handle: &AppHandle, path: &Path) -> Result<(), String> {
    let Some(entry) = registry::get(path) else {
        return Ok(());
    };
    let session = session(app_handle)?;

    if !path.exists() {
        download(app_handle, &session, &entry.asset_id, path).await?;
        prune_cache_later(&session);
    }
    // Edits may have been made on another machine. Not being able to fetch
    // them must not keep an image from opening, though.
    if let Err(e) = sync::pull(&session.client, &entry.asset_id, path).await {
        log::warn!("Could not fetch edits of {} from Immich: {e}", path.display());
    }
    Ok(())
}

async fn download(
    app_handle: &AppHandle,
    session: &Session,
    asset_id: &str,
    path: &Path,
) -> Result<(), String> {
    // One download per file, even if the editor and an export ask at once.
    let lock = {
        static LOCKS: Lazy<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
            Lazy::new(|| Mutex::new(HashMap::new()));
        LOCKS
            .lock()
            .unwrap()
            .entry(path.to_path_buf())
            .or_default()
            .clone()
    };
    let _guard = lock.lock().await;
    if path.exists() {
        return Ok(());
    }

    let parent = path.parent().ok_or("Invalid cache path")?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|e| format!("Cannot create '{}': {e}", parent.display()))?;

    let _ = app_handle.emit("immich-download", json!({ "path": path, "state": "started" }));
    // Download next to the target and rename, so an interrupted download
    // never leaves a half file that looks complete.
    let partial = parent.join(format!(".{asset_id}.part"));
    let result = session
        .client
        .download_original(asset_id, &partial)
        .await
        .and_then(|_| std::fs::rename(&partial, path).map_err(|e| e.to_string()));
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    let _ = app_handle.emit(
        "immich-download",
        json!({
            "path": path,
            "state": if result.is_ok() { "done" } else { "error" },
            "error": result.as_ref().err(),
        }),
    );
    result
}

/// Sends changed sidecars of Immich images to the server every few seconds.
/// Only looks at images listed this session; their sync state on disk makes
/// sure edits from an earlier session are sent once the album is opened again.
fn start_sync_loop(app_handle: &AppHandle) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(SYNC_INTERVAL).await;
            let Ok(session) = session(&app_handle) else {
                continue;
            };
            for (path, entry) in registry::all() {
                if !sync::has_local_changes(&path) {
                    continue;
                }
                match sync::push(&session.client, &entry.asset_id, &path).await {
                    Ok(true) => {
                        let _ = app_handle.emit("immich-edits-saved", json!({ "path": path }));
                    }
                    Ok(false) => {}
                    Err(e) => {
                        // Most likely offline; try again next round.
                        log::debug!("Could not send edits of {}: {e}", path.display());
                        break;
                    }
                }
            }
        }
    });
}

fn prune_cache_later(session: &Arc<Session>) {
    let dir = session.cache_dir.clone();
    let limit = u64::from(session.config.cache_limit_gb) * 1024 * 1024 * 1024;
    tauri::async_runtime::spawn_blocking(move || sync::prune_cache(&dir, limit));
}

pub async fn ensure_local_all(app_handle: &AppHandle, paths: &[String]) -> Result<(), String> {
    for path in paths {
        let (source, _) = crate::file_management::parse_virtual_path(path);
        ensure_local(app_handle, &source).await?;
    }
    Ok(())
}

/// Thumbnails for placeholders come from Immich, since there is no local
/// file to render yet.
///
/// Returns `None` if `path` is not an Immich placeholder, so the caller
/// carries on as usual. Otherwise returns the cached thumbnail files, or
/// `Some(None)` while they are still being fetched; a `thumbnail-generated`
/// event follows once they are there.
pub fn placeholder_thumbnail(
    app_handle: &AppHandle,
    path_str: &str,
    thumb_cache_dir: &Path,
) -> Option<Option<(String, String)>> {
    let (source, _) = crate::file_management::parse_virtual_path(path_str);
    if !is_placeholder(&source) {
        return None;
    }
    // The listed asset is usually the developed export, so the library shows
    // the edited look rather than the flat RAW.
    let asset_id = registry::get(&source)?.listed_asset_id;
    let small = thumb_cache_dir.join(format!("immich_{asset_id}_small.jpg"));
    let medium = thumb_cache_dir.join(format!("immich_{asset_id}_medium.jpg"));
    let as_strings = |small: &Path, medium: &Path| {
        (
            small.to_string_lossy().into_owned(),
            medium.to_string_lossy().into_owned(),
        )
    };
    if small.exists() && medium.exists() {
        return Some(Some(as_strings(&small, &medium)));
    }

    static IN_FLIGHT: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));
    if !IN_FLIGHT.lock().unwrap().insert(asset_id.clone()) {
        return Some(None);
    }

    let app_handle = app_handle.clone();
    let path_str = path_str.to_string();
    tauri::async_runtime::spawn(async move {
        let result = fetch_thumbnail(&app_handle, &asset_id, &small, &medium).await;
        IN_FLIGHT.lock().unwrap().remove(&asset_id);
        match result {
            Ok(()) => {
                let (small, medium) = as_strings(&small, &medium);
                let _ = app_handle.emit(
                    "thumbnail-generated",
                    json!({
                        "path": path_str,
                        "thumbnailPath": small,
                        "previewPath": medium,
                        "rating": 0,
                        "is_edited": false,
                    }),
                );
            }
            Err(e) => log::warn!("Immich thumbnail for {asset_id} failed: {e}"),
        }
    });
    Some(None)
}

async fn fetch_thumbnail(
    app_handle: &AppHandle,
    asset_id: &str,
    small: &Path,
    medium: &Path,
) -> Result<(), String> {
    let session = session(app_handle)?;
    let bytes = session.client.thumbnail(asset_id, "preview").await?;
    let small = small.to_path_buf();
    let medium = medium.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        for (target, size) in [(&small, THUMBNAIL_SMALL), (&medium, THUMBNAIL_MEDIUM)] {
            let resized = if image.width().max(image.height()) > size {
                image.thumbnail(size, size)
            } else {
                image.clone()
            };
            let mut encoded = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 85)
                .encode_image(&resized.to_rgb8())
                .map_err(|e| e.to_string())?;
            std::fs::write(target, encoded).map_err(|e| e.to_string())?;
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Called for every successfully exported image. Uploads the export if its
/// source came from Immich and uploading is switched on; runs in the
/// background and reports through `immich-upload` events.
pub fn on_exported(app_handle: &AppHandle, source_path: &str, output_path: &Path) {
    let (source, _) = crate::file_management::parse_virtual_path(source_path);
    let Some(entry) = registry::get(&source) else {
        return;
    };
    let Ok(session) = session(app_handle) else {
        return;
    };
    if !session.config.upload_exports {
        return;
    }

    let app_handle = app_handle.clone();
    let output = output_path.to_path_buf();
    tauri::async_runtime::spawn(async move {
        let file_name = output
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let _ = app_handle.emit(
            "immich-upload",
            json!({ "state": "started", "fileName": file_name }),
        );
        // Edits first, so the server never has an export without them.
        if let Err(e) = sync::push(&session.client, &entry.asset_id, &source).await {
            log::warn!("Could not send edits of {}: {e}", source.display());
        }
        let result = upload_export(&session, &source, &entry, &output).await;
        let _ = app_handle.emit(
            "immich-upload",
            match &result {
                Ok(()) => json!({ "state": "done", "fileName": file_name }),
                Err(e) => json!({ "state": "error", "fileName": file_name, "error": e }),
            },
        );
        if let Err(e) = result {
            log::error!("Immich upload of {} failed: {e}", output.display());
        }
    });
}

async fn upload_export(
    session: &Session,
    source: &Path,
    entry: &registry::RemoteImage,
    output: &Path,
) -> Result<(), String> {
    let client = &session.client;
    // Export already set the file time to the capture time, if it could.
    let modified = std::fs::metadata(output)
        .and_then(|m| m.modified())
        .map(chrono::DateTime::<chrono::Utc>::from)
        .unwrap_or_else(|_| chrono::Utc::now())
        .to_rfc3339();
    let uploaded = client.upload(output, &modified, &modified).await?;
    let new_id = uploaded.id;

    // The previous export is the asset that was listed in the album, as long
    // as it is not the original itself.
    let previous = (entry.listed_asset_id != entry.asset_id && entry.listed_asset_id != new_id)
        .then(|| entry.listed_asset_id.clone());
    let replace = session.config.replace_previous_export && previous.is_some();

    let mut albums: Vec<String> = entry.album_id.iter().cloned().collect();
    if let (true, Some(previous)) = (replace, &previous) {
        for album in client.albums_containing(previous).await.unwrap_or_default() {
            if !albums.contains(&album.id) {
                albums.push(album.id);
            }
        }
    }
    for album in &albums {
        client.add_to_album(album, &[new_id.clone()]).await?;
    }

    // Export on top, original underneath - the way Immich shows RAW+JPEG.
    if entry.asset_id != new_id {
        client
            .create_stack(&[new_id.clone(), entry.asset_id.clone()])
            .await?;
    }

    if let (true, Some(previous)) = (replace, previous) {
        client.trash(&[previous]).await?;
    }
    // The next export of this image replaces this one.
    if new_id != entry.asset_id {
        registry::set_listed_asset(source, &new_id);
    }
    Ok(())
}
