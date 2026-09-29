use serde_json::json;
use std::path::Path;
use tauri::{AppHandle, Emitter};

use super::{Session, registry, session, sync};

/// Must match `IMMICH_ALBUM_PREFIX` in the frontend.
const ALBUM_PREFIX: &str = "immich:";

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
        if session.config.sync_edits
            && let Err(e) = sync::push(&session.client, &entry.asset_id, &source).await
        {
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

/// The export goes on top of the stack of its original and, if so set, into
/// the album the image was opened from. A previous export of the same image is
/// then replaced in its albums but stays in the stack.
async fn upload_export(
    session: &Session,
    source: &Path,
    entry: &registry::RemoteImage,
    output: &Path,
) -> Result<(), String> {
    let client = &session.client;
    let modified = file_time(output);
    let new_id = client.upload(output, &modified, &modified).await?.id;
    if new_id == entry.asset_id {
        return Ok(());
    }

    // The listed asset is the previous export unless it is the original itself.
    let previous = (entry.listed_asset_id != entry.asset_id && entry.listed_asset_id != new_id)
        .then(|| entry.listed_asset_id.clone());

    let mut albums: Vec<String> = Vec::new();
    let mut replaced_in = Vec::new();
    if session.config.exports_to_album {
        albums.extend(entry.album_id.iter().cloned());
    }
    if let (true, Some(previous)) = (session.config.exports_to_album, &previous) {
        for album in client.albums_containing(previous).await.unwrap_or_default() {
            if !albums.contains(&album.id) {
                albums.push(album.id.clone());
            }
            replaced_in.push(album.id);
        }
    }
    for album in &albums {
        client.add_to_album(album, &[new_id.clone()]).await?;
    }
    client
        .create_stack(&[new_id.clone(), entry.asset_id.clone()])
        .await?;
    if let Some(previous) = previous {
        for album in &replaced_in {
            client.remove_from_album(album, &[previous.clone()]).await?;
        }
    }
    registry::set_listed_asset(source, &new_id);
    Ok(())
}

/// `None` if `album_id` is not an Immich album. Local files are uploaded
/// first; progress is reported through `immich-transfer`.
pub fn add_to_album(
    app_handle: &AppHandle,
    album_id: &str,
    paths: &[String],
) -> Option<Result<(), String>> {
    let target = album_id.strip_prefix(ALBUM_PREFIX)?;
    // Pseudo albums such as "unassigned" have no UUID and only upload.
    let album = uuid::Uuid::parse_str(target)
        .ok()
        .map(|_| target.to_string());
    let session = match session(app_handle) {
        Ok(session) => session,
        Err(e) => return Some(Err(e)),
    };
    let app_handle = app_handle.clone();
    let paths = paths.to_vec();
    tauri::async_runtime::spawn(async move {
        let total = paths.len();
        let mut ids = Vec::new();
        let mut failed = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            let _ = app_handle.emit(
                "immich-transfer",
                json!({ "state": "progress", "current": index, "total": total }),
            );
            match asset_for(&session, path, album.as_deref()).await {
                Ok(id) => ids.push(id),
                Err(e) => {
                    log::error!("Could not add {path} to Immich: {e}");
                    failed.push(e);
                }
            }
        }
        if let Some(album) = &album
            && !ids.is_empty()
            && let Err(e) = session.client.add_to_album(album, &ids).await
        {
            failed.push(e);
        }
        let _ = app_handle.emit(
            "immich-transfer",
            json!({
                "state": "done",
                "added": ids.len(),
                "failed": failed.len(),
                "error": failed.first(),
            }),
        );
        let _ = app_handle.emit("immich-library-changed", json!({}));
    });
    Some(Ok(()))
}

async fn asset_for(session: &Session, path: &str, album: Option<&str>) -> Result<String, String> {
    let (source, sidecar) = crate::file_management::parse_virtual_path(path);
    if let Some(entry) = registry::get(&source) {
        return Ok(entry.listed_asset_id);
    }
    if path.contains("?vc=") || !source.is_file() {
        return Err(format!(
            "{} is not a file that can be uploaded",
            source.display()
        ));
    }

    let modified = file_time(&source);
    let id = session
        .client
        .upload(&source, &modified, &modified)
        .await?
        .id;

    // A copy in the cache belongs to the uploaded asset, so its edits sync
    // with Immich. The local file is left as it is.
    let file_name = source
        .file_name()
        .map(|n| n.to_owned())
        .ok_or("Invalid file name")?;
    let target = session.cache_dir.join(&id).join(&file_name);
    std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::copy(&source, &target)
        .map_err(|e| format!("Cannot copy {}: {e}", source.display()))?;
    if sidecar.exists() {
        let _ = std::fs::copy(&sidecar, sync::sidecar_of(&target));
    }

    registry::insert(
        target.clone(),
        registry::RemoteImage {
            asset_id: id.clone(),
            listed_asset_id: id.clone(),
            album_id: album.map(str::to_string),
        },
    );
    if session.config.sync_edits {
        sync::push(&session.client, &id, &target).await?;
    }
    Ok(id)
}

fn file_time(path: &Path) -> String {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(chrono::DateTime::<chrono::Utc>::from)
        .unwrap_or_else(|_| chrono::Utc::now())
        .to_rfc3339()
}
