use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};

use super::client::{Album, ImmichClient, Person, TimelineMonth};
use super::config::{self, AlbumSort, ApiKeyInfo};
use super::{MIN_VERSION, MIN_VERSION_FOR_EDITS, registry, reset_session, resolve, session};
use crate::file_management::ImageFile;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub version: String,
    pub user_name: String,
    pub user_email: String,
    pub supports_edits: bool,
}

#[tauri::command]
pub fn immich_get_api_key(app_handle: AppHandle) -> ApiKeyInfo {
    config::load_api_key(&app_handle)
}

#[tauri::command]
pub fn immich_set_api_key(api_key: String, app_handle: AppHandle) -> Result<ApiKeyInfo, String> {
    let info = config::save_api_key(&app_handle, &api_key)?;
    reset_session(true);
    Ok(info)
}

#[tauri::command]
pub async fn immich_test_connection(
    server_url: String,
    api_key: String,
) -> Result<ConnectionInfo, String> {
    let client = ImmichClient::new(&server_url, &api_key)?;
    let version = client.version_numbers().await?;
    let version_text = format!("{}.{}.{}", version.0, version.1, version.2);
    if version < MIN_VERSION {
        return Err(format!(
            "Immich {version_text} is too old. RapidRAW needs Immich {}.{}.{} or newer.",
            MIN_VERSION.0, MIN_VERSION.1, MIN_VERSION.2
        ));
    }
    let user = client.me().await?;
    Ok(ConnectionInfo {
        version: version_text,
        user_name: user.name,
        user_email: user.email,
        supports_edits: version >= MIN_VERSION_FOR_EDITS,
    })
}

#[tauri::command]
pub async fn immich_list_albums(app_handle: AppHandle) -> Result<Vec<Album>, String> {
    let session = session(&app_handle)?;
    let mut albums = session.client.albums().await?;
    albums.sort_by_key(|a| a.album_name.to_lowercase());
    // ISO dates sort as text; albums without photos have none and go last.
    match session.settings.album_sort {
        AlbumSort::Name => {}
        AlbumSort::Newest => albums.sort_by(|a, b| b.end_date.cmp(&a.end_date)),
        AlbumSort::Oldest => albums.sort_by(|a, b| match (&a.start_date, &b.start_date) {
            (Some(x), Some(y)) => x.cmp(y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }),
    }
    Ok(albums)
}

#[tauri::command]
pub async fn immich_suggestions(
    kind: String,
    country: Option<String>,
    make: Option<String>,
    app_handle: AppHandle,
) -> Result<Vec<String>, String> {
    let session = session(&app_handle)?;
    let mut values = session
        .client
        .suggestions(&kind, country.as_deref(), make.as_deref())
        .await?;
    values.sort_by_key(|v| v.to_lowercase());
    Ok(values)
}

#[tauri::command]
pub async fn immich_timeline(app_handle: AppHandle) -> Result<Vec<TimelineMonth>, String> {
    session(&app_handle)?.client.timeline_months().await
}

#[tauri::command]
pub async fn immich_list_people(app_handle: AppHandle) -> Result<Vec<Person>, String> {
    session(&app_handle)?.client.named_people().await
}

#[tauri::command]
pub async fn immich_get_images(
    filter: resolve::Filter,
    app_handle: AppHandle,
) -> Result<Vec<ImageFile>, String> {
    let session = session(&app_handle)?;
    let (resolved, truncated) = resolve::listing(
        &session.client,
        &session.settings,
        &filter,
        &session.cache_dir,
    )
    .await?;
    if truncated {
        let _ = app_handle.emit(
            "immich-listing-truncated",
            json!({ "limit": session.settings.listing_limit.clamp(100, 50_000) }),
        );
    }

    let mut local_paths = Vec::new();
    let mut placeholders = Vec::new();
    for item in resolved {
        let path = item.path.to_string_lossy().into_owned();
        if item.path.exists() {
            local_paths.push(path);
        } else {
            placeholders.push(placeholder_file(&path, item.file_modified_at.as_deref()));
        }
        registry::insert(item.path, item.image);
    }

    let app = app_handle.clone();
    let mut files = tauri::async_runtime::spawn_blocking(move || {
        crate::file_management::get_album_images(local_paths, app)
    })
    .await
    .map_err(|e| e.to_string())??;
    files.extend(placeholders);
    Ok(files)
}

fn placeholder_file(path: &str, file_modified_at: Option<&str>) -> ImageFile {
    let modified = file_modified_at
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.timestamp().max(0) as u64)
        .unwrap_or(0);
    ImageFile::placeholder(path.to_string(), modified)
}
