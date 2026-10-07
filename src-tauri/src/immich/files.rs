use once_cell::sync::Lazy;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use super::{Session, registry, session, sync};

pub async fn ensure_local(app_handle: &AppHandle, path: &Path) -> Result<(), String> {
    let Some(entry) = registry::get(path) else {
        return Ok(());
    };
    let session = session(app_handle)?;

    if !path.exists() {
        download(app_handle, &session, &entry.asset_id, path).await?;
        prune_cache_later(&session);
    }
    // Failing to fetch newer edits must not keep the image from opening.
    if session.syncs_edits().await
        && let Err(e) = sync::pull(&session.client, &entry.asset_id, path).await
    {
        log::warn!(
            "Could not fetch edits of {} from Immich: {e}",
            path.display()
        );
    }
    Ok(())
}

pub async fn ensure_local_all(app_handle: &AppHandle, paths: &[String]) -> Result<(), String> {
    for path in paths {
        let (source, _) = crate::file_management::parse_virtual_path(path);
        ensure_local(app_handle, &source).await?;
    }
    Ok(())
}

async fn download(
    app_handle: &AppHandle,
    session: &Session,
    asset_id: &str,
    path: &Path,
) -> Result<(), String> {
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

    let _ = app_handle.emit(
        "immich-download",
        json!({ "path": path, "state": "started" }),
    );
    // Renamed when complete, so an interrupted download never looks finished.
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

pub fn prune_cache_later(session: &Arc<Session>) {
    let dir = session.cache_dir.clone();
    let limit = u64::from(session.settings.cache_limit_gb) * 1024 * 1024 * 1024;
    tauri::async_runtime::spawn_blocking(move || sync::prune_cache(&dir, limit));
}
