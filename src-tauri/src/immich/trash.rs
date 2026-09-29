use serde_json::json;
use tauri::{AppHandle, Emitter};

use super::{registry, session};

/// Moves Immich images to Immich's trash - the opened image and the one listed
/// for it, and with `whole_stack` everything else in their stacks - and
/// returns the remaining paths for the caller to delete. The request runs in the background; meanwhile the images
/// are left out of listings.
pub fn trash_remote(
    app_handle: &AppHandle,
    paths: Vec<String>,
    whole_stack: bool,
) -> Result<Vec<String>, String> {
    let (remote, local): (Vec<_>, Vec<_>) = paths.into_iter().partition(|p| {
        !p.contains("?vc=")
            && registry::get(&crate::file_management::parse_virtual_path(p).0).is_some()
    });
    if remote.is_empty() {
        return Ok(local);
    }
    let session = session(app_handle)?;

    let mut ids = Vec::new();
    let mut sources = Vec::new();
    for path in &remote {
        let (source, _) = crate::file_management::parse_virtual_path(path);
        let Some(entry) = registry::get(&source) else {
            continue;
        };
        for id in [entry.asset_id, entry.listed_asset_id] {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        registry::remove(&source);
        sources.push(source);
    }
    registry::mark_trashing(&ids);

    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let mut ids = ids;
        if whole_stack {
            for stack in session.client.stacks().await.unwrap_or_default() {
                if stack.assets.iter().any(|a| ids.contains(&a.id)) {
                    for asset in stack.assets {
                        if !ids.contains(&asset.id) {
                            ids.push(asset.id);
                        }
                    }
                }
            }
        }
        let result = session.client.trash(&ids).await;
        registry::done_trashing(&ids);
        match &result {
            Ok(()) => {
                for source in &sources {
                    // Only ever remove an asset folder inside the cache.
                    if let Some(folder) = source.parent()
                        && folder.parent() == Some(session.cache_dir.as_path())
                    {
                        let _ = std::fs::remove_dir_all(folder);
                    }
                }
            }
            Err(e) => {
                log::error!("Could not move images to Immich's trash: {e}");
                let _ = app_handle.emit("immich-trash", json!({ "state": "error", "error": e }));
            }
        }
        let _ = app_handle.emit("immich-library-changed", json!({}));
    });
    Ok(local)
}
