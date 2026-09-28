use reqwest::multipart::{Form, Part};
use reqwest::{Client, Method, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub original_file_name: String,
    #[serde(default)]
    pub file_modified_at: Option<String>,
    #[serde(default)]
    pub is_trashed: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub album_name: String,
    #[serde(default)]
    pub asset_count: u64,
    #[serde(default)]
    pub album_thumbnail_asset_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub shared: bool,
}

/// For a `duplicate`, Immich returns the id of the existing asset.
#[derive(Deserialize, Debug, Clone)]
pub struct UploadResult {
    pub id: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetadataEntry {
    pub key: String,
    pub value: Value,
    pub updated_at: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct User {
    pub name: String,
    pub email: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Stack {
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimelineMonth {
    pub time_bucket: String,
    pub count: u64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Person {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeoplePage {
    people: Vec<Person>,
    #[serde(default)]
    has_next_page: bool,
}

#[derive(Deserialize)]
struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchPage {
    items: Vec<Asset>,
    next_page: Option<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    assets: SearchPage,
}

pub struct ImmichClient {
    base: String,
    api_key: String,
    http: Client,
}

impl ImmichClient {
    pub fn new(server_url: &str, api_key: &str) -> Result<Self, String> {
        let base = server_url.trim().trim_end_matches('/').to_string();
        if !base.starts_with("http://") && !base.starts_with("https://") {
            return Err("The server address must start with http:// or https://".to_string());
        }
        let http = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base,
            api_key: api_key.trim().to_string(),
            http,
        })
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}/api{}", self.base, path))
            .header("x-api-key", &self.api_key)
            .header("Accept", "application/json")
    }

    async fn send(&self, builder: RequestBuilder, what: &str) -> Result<Response, String> {
        let response = builder
            .send()
            .await
            .map_err(|e| format!("Immich is not reachable ({what}): {e}"))?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let detail = response.text().await.unwrap_or_default();
        let detail: String = detail.chars().take(300).collect();
        Err(format!(
            "Immich rejected {what} (HTTP {}): {detail}",
            status.as_u16()
        ))
    }

    async fn json<T: DeserializeOwned>(
        &self,
        builder: RequestBuilder,
        what: &str,
    ) -> Result<T, String> {
        self.send(builder, what)
            .await?
            .json::<T>()
            .await
            .map_err(|e| format!("Unexpected answer from Immich ({what}): {e}"))
    }

    pub async fn version(&self) -> Result<String, String> {
        let v: Version = self
            .json(
                self.request(Method::GET, "/server/version"),
                "server version",
            )
            .await?;
        Ok(format!("{}.{}.{}", v.major, v.minor, v.patch))
    }

    pub async fn me(&self) -> Result<User, String> {
        self.json(self.request(Method::GET, "/users/me"), "user")
            .await
    }

    pub async fn albums(&self) -> Result<Vec<Album>, String> {
        self.json(self.request(Method::GET, "/albums"), "albums")
            .await
    }

    pub async fn albums_containing(&self, asset_id: &str) -> Result<Vec<Album>, String> {
        self.json(
            self.request(Method::GET, &format!("/albums?assetId={asset_id}")),
            "albums of asset",
        )
        .await
    }

    pub async fn suggestions(
        &self,
        kind: &str,
        country: Option<&str>,
        make: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let mut path = format!("/search/suggestions?type={}", encode(kind));
        if let Some(country) = country {
            path.push_str(&format!("&country={}", encode(country)));
        }
        if let Some(make) = make {
            path.push_str(&format!("&make={}", encode(make)));
        }
        let values: Vec<Option<String>> = self
            .json(self.request(Method::GET, &path), "suggestions")
            .await?;
        Ok(values
            .into_iter()
            .flatten()
            .filter(|v| !v.is_empty())
            .collect())
    }

    pub async fn named_people(&self) -> Result<Vec<Person>, String> {
        let mut people = Vec::new();
        let mut page = 1u32;
        loop {
            let result: PeoplePage = self
                .json(
                    self.request(Method::GET, &format!("/people?page={page}&size=500")),
                    "people",
                )
                .await?;
            people.extend(
                result
                    .people
                    .into_iter()
                    .filter(|p| !p.name.trim().is_empty()),
            );
            if !result.has_next_page {
                break;
            }
            page += 1;
        }
        people.sort_by_key(|p| p.name.to_lowercase());
        Ok(people)
    }

    /// The second value tells whether there were more than `limit` hits.
    pub async fn search_up_to(
        &self,
        query: Value,
        limit: Option<usize>,
    ) -> Result<(Vec<Asset>, bool), String> {
        let mut found = Vec::new();
        let mut page = 1u32;
        loop {
            let mut body = query.clone();
            body["page"] = json!(page);
            if body.get("size").is_none() {
                body["size"] = json!(1000);
            }
            let result: SearchResponse = self
                .json(
                    self.request(Method::POST, "/search/metadata").json(&body),
                    "search",
                )
                .await?;
            found.extend(result.assets.items);
            let next = result.assets.next_page.and_then(|p| p.parse().ok());
            if let Some(limit) = limit
                && found.len() >= limit
            {
                let more = found.len() > limit || next.is_some();
                found.truncate(limit);
                return Ok((found, more));
            }
            match next {
                Some(next) if next > page => page = next,
                _ => return Ok((found, false)),
            }
        }
    }

    pub async fn stacks(&self) -> Result<Vec<Stack>, String> {
        self.json(self.request(Method::GET, "/stacks"), "stacks")
            .await
    }

    pub async fn timeline_months(&self) -> Result<Vec<TimelineMonth>, String> {
        self.json(
            self.request(Method::GET, "/timeline/buckets?visibility=timeline"),
            "timeline",
        )
        .await
    }

    pub async fn download_original(&self, asset_id: &str, target: &Path) -> Result<(), String> {
        let mut response = self
            .send(
                self.request(Method::GET, &format!("/assets/{asset_id}/original")),
                "original download",
            )
            .await?;
        let mut file = tokio::fs::File::create(target)
            .await
            .map_err(|e| format!("Cannot write '{}': {e}", target.display()))?;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Download interrupted: {e}"))?
        {
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        file.flush().await.map_err(|e| e.to_string())
    }

    pub async fn thumbnail(&self, asset_id: &str, size: &str) -> Result<Vec<u8>, String> {
        let response = self
            .send(
                self.request(
                    Method::GET,
                    &format!("/assets/{asset_id}/thumbnail?size={size}"),
                ),
                "thumbnail",
            )
            .await?;
        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| e.to_string())
    }

    pub async fn upload(
        &self,
        path: &Path,
        created_at: &str,
        modified_at: &str,
    ) -> Result<UploadResult, String> {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "export.jpg".to_string());
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|e| format!("Cannot read '{}': {e}", path.display()))?;
        let part = Part::bytes(bytes)
            .file_name(file_name.clone())
            .mime_str(mime_for(&file_name))
            .map_err(|e| e.to_string())?;
        let form = Form::new()
            .text("fileCreatedAt", created_at.to_string())
            .text("fileModifiedAt", modified_at.to_string())
            .text("filename", file_name)
            .part("assetData", part);
        self.json(
            self.request(Method::POST, "/assets").multipart(form),
            "upload",
        )
        .await
    }

    pub async fn metadata(
        &self,
        asset_id: &str,
        key: &str,
    ) -> Result<Option<MetadataEntry>, String> {
        let response = self
            .request(Method::GET, &format!("/assets/{asset_id}/metadata/{key}"))
            .send()
            .await
            .map_err(|e| format!("Immich is not reachable (edits): {e}"))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            // Immich 3.2 answers a missing key with 400 "... not found".
            if status == reqwest::StatusCode::BAD_REQUEST && detail.contains("not found") {
                return Ok(None);
            }
            return Err(format!(
                "Immich rejected reading edits (HTTP {})",
                status.as_u16()
            ));
        }
        response
            .json::<MetadataEntry>()
            .await
            .map(Some)
            .map_err(|e| format!("Unexpected answer from Immich (edits): {e}"))
    }

    pub async fn set_metadata(
        &self,
        asset_id: &str,
        key: &str,
        value: &Value,
    ) -> Result<String, String> {
        let entries: Vec<MetadataEntry> = self
            .json(
                self.request(Method::PUT, &format!("/assets/{asset_id}/metadata"))
                    .json(&json!({ "items": [{ "key": key, "value": value }] })),
                "saving edits",
            )
            .await?;
        entries
            .into_iter()
            .find(|e| e.key == key)
            .map(|e| e.updated_at)
            .ok_or_else(|| "Immich did not confirm the saved edits".to_string())
    }

    pub async fn add_to_album(&self, album_id: &str, asset_ids: &[String]) -> Result<(), String> {
        self.send(
            self.request(Method::PUT, &format!("/albums/{album_id}/assets"))
                .json(&json!({ "ids": asset_ids })),
            "adding to album",
        )
        .await
        .map(|_| ())
    }

    pub async fn remove_from_album(
        &self,
        album_id: &str,
        asset_ids: &[String],
    ) -> Result<(), String> {
        self.send(
            self.request(Method::DELETE, &format!("/albums/{album_id}/assets"))
                .json(&json!({ "ids": asset_ids })),
            "removing from album",
        )
        .await
        .map(|_| ())
    }

    /// The first id becomes the primary asset; existing stacks are merged.
    pub async fn create_stack(&self, asset_ids: &[String]) -> Result<(), String> {
        self.send(
            self.request(Method::POST, "/stacks")
                .json(&json!({ "assetIds": asset_ids })),
            "stacking",
        )
        .await
        .map(|_| ())
    }

    pub async fn trash(&self, asset_ids: &[String]) -> Result<(), String> {
        self.send(
            self.request(Method::DELETE, "/assets")
                .json(&json!({ "ids": asset_ids, "force": false })),
            "moving to trash",
        )
        .await
        .map(|_| ())
    }
}

fn mime_for(file_name: &str) -> &'static str {
    let ext = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "tif" | "tiff" => "image/tiff",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "jxl" => "image/jxl",
        "heic" | "heif" => "image/heic",
        _ => "application/octet-stream",
    }
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
