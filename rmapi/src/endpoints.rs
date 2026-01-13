use crate::error::Error;
use base64::Engine;
use const_format::formatcp;
use log;
use reqwest::{self, Body};
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use uuid::Uuid;

const AUTH_API_URL_ROOT: &str = "https://webapp-prod.cloud.remarkable.engineering";
const AUTH_API_VERSION: &str = "2";
const NEW_CLIENT_URL: &str =
    formatcp!("{AUTH_API_URL_ROOT}/token/json/{AUTH_API_VERSION}/device/new");
const NEW_TOKEN_URL: &str = formatcp!("{AUTH_API_URL_ROOT}/token/json/{AUTH_API_VERSION}/user/new");

const SERVICE_DISCOVERY_API_URL_ROOT: &str =
    "https://service-manager-production-dot-remarkable-production.appspot.com";
const STORAGE_API_VERSION: &str = "1";
const STORAGE_DISCOVERY_API_URL: &str = formatcp!(
    "{SERVICE_DISCOVERY_API_URL_ROOT}/service/json/{STORAGE_API_VERSION}/document-storage"
);
const GROUP_AUTH: &str = "auth0%7C5a68dc51cb30df1234567890";
const STORAGE_DISCOVERY_API_VERSION: &str = "2";

pub const STORAGE_API_URL_ROOT: &str = "https://internal.cloud.remarkable.com";
pub const WEBAPP_API_URL_ROOT: &str = "https://web.eu.tectonic.remarkable.com";

#[derive(Debug, Serialize, Deserialize)]
pub struct V4Metadata {
    #[serde(rename = "visibleName", default)]
    pub visible_name: String,
    #[serde(rename = "type", default)]
    pub doc_type: String,
    #[serde(default)]
    pub parent: String,
    #[serde(rename = "lastModified", default)]
    pub last_modified: String,
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct V4Entry {
    pub hash: String,
    pub doc_type: String,
    pub doc_id: String,
    pub subfiles: u32,
    pub size: u64,
}

const DOC_UPLOAD_ENDPOINT: &str = "doc/v2/files";
pub const ROOT_SYNC_ENDPOINT: &str = "sync/v4/root";
// const FILE_SYNC_ENDPOINT: &str = "sync/v3/files";

// const ITEM_LIST_ENDPOINT: &str = "document-storage/json/2/docs";
// const ITEM_ENDPOINT: &str = "document-storage/json/2/";
// const UPLOAD_REQUEST_ENDPOINT: &str = "document-storage/json/2/upload/request";
// const UPLOAD_STATUS_ENDPOINT: &str = "document-storage/json/2/upload/update-status";
// const DELETE_ENDPOINT: &str = "/document-storage/json/2/delete";

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct ClientRegistation {
    code: String,
    deviceDesc: String,
    deviceID: String,
}

/// Registers a new client with the reMarkable cloud service.
///
/// This function takes a registration code and sends a request to the reMarkable
/// authentication API to register a new client device. It generates a new UUID
/// for the device ID.
///
/// # Arguments
///
/// * `code` - A string that holds the registration code provided by reMarkable.
///
/// # Returns
///
/// * `Result<String, Error>` - Returns Ok with the response text on success,
///   or an Error if the registration process fails.
///
/// # Errors
///
/// This function will return an error if:
/// * The HTTP request fails
/// * The server responds with an error status
/// * The response cannot be parsed

pub async fn register_client(code: &str) -> Result<String, Error> {
    log::info!("Registering client with code: {}", code);
    let registration_info = ClientRegistation {
        code: code.to_string(),
        deviceDesc: "desktop-windows".to_string(),
        deviceID: Uuid::new_v4().to_string(),
    };

    let client = reqwest::Client::new();
    let response = client
        .post(NEW_CLIENT_URL)
        .header("Content-Type", "application/json")
        .json(&registration_info)
        .send()
        .await?;

    log::debug!("{:?}", response);

    match response.error_for_status() {
        Ok(res) => {
            let token = res.text().await?;
            log::debug!("Token: {}", token);
            Ok(token)
        }
        Err(e) => {
            log::error!("Error registering client: {}", e);
            Err(Error::from(e))
        }
    }
}

/// Refreshes the authentication token for the reMarkable cloud service.
///
/// This function takes an existing authentication token and sends a request to
/// the reMarkable authentication API to obtain a new, refreshed token.
///
/// # Arguments
///
/// * `auth_token` - A string that holds the current authentication token.
///
/// # Returns
///
/// * `Result<String, Error>` - Returns Ok with the new token as a string on success,
///   or an Error if the refresh process fails.
///
/// # Errors
///
/// This function will return an error if:
/// * The HTTP request fails
/// * The server responds with an error status
/// * The response cannot be parsed
pub async fn refresh_token(auth_token: &str) -> Result<String, Error> {
    log::info!("Refreshing token");
    let client = reqwest::Client::new();
    let response = client
        .post(NEW_TOKEN_URL)
        .bearer_auth(auth_token)
        .header("Accept", "application/json")
        .header("Content-Length", "0")
        .send()
        .await?;

    log::debug!("{:?}", response);

    match response.error_for_status() {
        Ok(res) => {
            let token = res.text().await?;
            log::debug!("New Token: {}", token);
            Ok(token)
        }
        Err(e) => {
            log::error!("Error refreshing token: {}", e);
            Err(Error::from(e))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct StorageInfo {
    Status: String,
    Host: String,
}

pub async fn discover_storage(auth_token: &str) -> Result<String, Error> {
    log::info!("Discovering storage host");
    let discovery_request = vec![
        ("environment", "production"),
        ("group", GROUP_AUTH),
        ("apiVer", STORAGE_DISCOVERY_API_VERSION),
    ];
    let client = reqwest::Client::new();
    let response = client
        .get(STORAGE_DISCOVERY_API_URL)
        .bearer_auth(auth_token)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .query(&discovery_request)
        .send()
        .await?;

    log::debug!("{:?}", response);

    match response.error_for_status() {
        Ok(res) => {
            let storage_info = res.json::<StorageInfo>().await?;
            log::debug!("Storage Info: {:?}", storage_info);
            Ok(format!("https://{0}", storage_info.Host))
        }
        Err(e) => {
            log::error!("Error discovering storage: {}", e);
            Err(Error::from(e))
        }
    }
}

pub async fn sync_root(storage_url: &str, auth_token: &str) -> Result<String, Error> {
    log::info!("Listing items in the rmCloud");
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/{}", storage_url, ROOT_SYNC_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Accept", "application/json")
        .header("rm-filename", "roothash")
        .send()
        .await?;

    log::debug!("{:?}", response);

    match response.error_for_status() {
        Ok(res) => {
            let root_hash = res.text().await?;
            log::debug!("Root Hash: {}", root_hash);
            Ok(root_hash)
        }
        Err(e) => {
            log::error!("Error listing items: {}", e);
            Err(Error::from(e))
        }
    }
}

// pub async fn put_content(storage_url: &str, auth_token: &str, content) {
//     log::info!("Listing items in the rmCloud");
//     let client = reqwest::Client::new();
//     let response = client
//         .get(format!("{}/{}", storage_url, ROOT_SYNC_ENDPOINT))
//         .bearer_auth(auth_token)
//         .header("Accept", "application/json")
//         .header("rm-filename", "roothash")
//         .send()
//         .await?;

//     log::debug!("{:?}", response);

//     match response.error_for_status() {
//         Ok(res) => {
//             let root_hash = res.text().await?;
//             log::debug!("Root Hash: {}", root_hash);
//             Ok(root_hash)
//         }
//         Err(e) => {
//             log::error!("Error listing items: {}", e);
//             Err(Error::from(e))
//         }
//     }
// }

pub async fn upload_request(_: &str, auth_token: &str) -> Result<String, Error> {
    log::info!("Requesting to upload a document to the rmCloud");
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/{}", WEBAPP_API_URL_ROOT, DOC_UPLOAD_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Accept", "application/json")
        .header("rm-Source", "WebLibrary")
        .header("Content-Type", "application/pdf")
        .send()
        .await?;

    log::debug!("{:?}", response);

    match response.error_for_status() {
        Ok(res) => {
            let upload_request_resp = res.text().await?;
            log::debug!("Upload request response: {}", upload_request_resp);
            Ok(upload_request_resp)
        }
        Err(e) => {
            log::error!("Error listing items: {}", e);
            Err(Error::from(e))
        }
    }
}

pub async fn upload_file(_: &str, auth_token: &str, file: File) -> Result<String, Error> {
    log::info!("Requesting to upload a document to the rmCloud");
    let stream = FramedRead::new(file, BytesCodec::new());
    let body = Body::wrap_stream(stream);

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/{}", WEBAPP_API_URL_ROOT, DOC_UPLOAD_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Accept-Encoding", "gzip, deflate, br")
        .header("rm-Source", "WebLibrary")
        .header("rm-Meta", "")
        .header("Content-Type", "application/pdf")
        .body(body)
        .send()
        .await?;

    log::debug!("{:?}", response);

    match response.error_for_status() {
        Ok(res) => {
            let upload_request_resp = res.text().await?;
            log::debug!("Upload file response: {}", upload_request_resp);
            Ok(upload_request_resp)
        }
        Err(e) => {
            log::error!("Error listing items: {}", e);
            Err(Error::from(e))
        }
    }
}

pub async fn get_files(
    _storage_url: &str, // Ignored because Sync V4 needs internal host
    auth_token: &str,
) -> Result<(Vec<crate::objects::Document>, String), Error> {
    log::info!("Requesting files version Sync V4");

    let client = reqwest::Client::new();

    // 1. Get the root hash
    let root_hash_response = client
        .get(format!("{}/{}", STORAGE_API_URL_ROOT, ROOT_SYNC_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Accept", "application/json")
        .header("rm-filename", "roothash")
        .send()
        .await?
        .error_for_status()?;

    let root_resp_text = root_hash_response.text().await?;
    log::debug!("Root response: {}", root_resp_text);

    // Parse the root response which is JSON in V4
    let root_info: serde_json::Value = serde_json::from_str(&root_resp_text).map_err(|e| {
        log::error!("Failed to parse root JSON: {}", e);
        Error::from(e)
    })?;

    let root_hash = root_info["hash"]
        .as_str()
        .ok_or_else(|| {
            log::error!("Missing hash in root info");
            Error::Message("Missing hash in root info".to_string())
        })?
        .to_string();

    // 2. Fetch the root index blob
    let root_blob_response = client
        .get(format!(
            "{}/sync/v3/files/{}",
            STORAGE_API_URL_ROOT, root_hash
        ))
        .bearer_auth(auth_token)
        .header("rm-filename", "roothash")
        .send()
        .await?
        .error_for_status()?;

    let root_blob_text = root_blob_response.text().await?;

    // 3. Parse root index
    let mut entries = Vec::new();
    for line in root_blob_text.lines().skip(1) {
        // Skip schema version line
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 5 {
            continue;
        }

        entries.push(V4Entry {
            hash: parts[0].to_string(),
            doc_type: parts[1].to_string(),
            doc_id: parts[2].to_string(),
            subfiles: parts[3].parse().unwrap_or(0),
            size: parts[4].parse().unwrap_or(0),
        });
    }

    // 4. Concurrently fetch metadata for all entries
    let mut tasks = Vec::new();
    let auth_token = auth_token.to_string();
    let client = client.clone();

    for entry in entries {
        let auth_token = auth_token.clone();
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            // Fetch .docSchema to find .metadata hash
            let doc_schema_response = client
                .get(format!(
                    "{}/sync/v3/files/{}",
                    STORAGE_API_URL_ROOT, entry.hash
                ))
                .bearer_auth(&auth_token)
                .header("rm-filename", format!("{}.docSchema", entry.doc_id))
                .send()
                .await;

            let doc_schema_response = match doc_schema_response {
                Ok(r) if r.status().is_success() => r,
                _ => return None,
            };

            let doc_schema_text = doc_schema_response.text().await.ok()?;
            let mut metadata_hash = None;
            for subline in doc_schema_text.lines().skip(1) {
                if subline.contains(".metadata") {
                    let subparts: Vec<&str> = subline.split(':').collect();
                    if subparts.len() >= 1 {
                        metadata_hash = Some(subparts[0].to_string());
                        break;
                    }
                }
            }

            let m_hash = metadata_hash?;
            let metadata_response = client
                .get(format!("{}/sync/v3/files/{}", STORAGE_API_URL_ROOT, m_hash))
                .bearer_auth(&auth_token)
                .header("rm-filename", format!("{}.metadata", entry.doc_id))
                .send()
                .await
                .ok()?;

            if !metadata_response.status().is_success() {
                return None;
            }

            let m_body = metadata_response.text().await.ok()?;
            let metadata_json: V4Metadata = serde_json::from_str(&m_body).ok()?;
            if metadata_json.deleted {
                return None;
            }

            let last_modified = metadata_json
                .last_modified
                .parse::<i64>()
                .ok()
                .and_then(chrono::DateTime::from_timestamp_millis)
                .unwrap_or_default();

            Some(crate::objects::Document {
                id: Uuid::parse_str(&entry.doc_id).unwrap_or(Uuid::nil()),
                version: metadata_json.version,
                message: String::new(),
                success: true,
                blob_url_get: String::new(),
                blob_url_put: String::new(),
                blob_url_put_expires: chrono::Utc::now(),
                last_modified: last_modified,
                doc_type: if metadata_json.doc_type == "CollectionType" {
                    crate::objects::DocumentType::Collection
                } else {
                    crate::objects::DocumentType::Document
                },
                display_name: if metadata_json.visible_name.is_empty() {
                    "Unknown".to_string()
                } else {
                    metadata_json.visible_name
                },
                current_page: 0,
                bookmarked: metadata_json.pinned,
                parent: metadata_json.parent,
                hash: entry.hash.clone(),
            })
        }));
    }

    let results = futures::future::join_all(tasks).await;
    let mut documents = Vec::new();
    for res in results {
        if let Ok(Some(doc)) = res {
            documents.push(doc);
        }
    }

    Ok((documents, root_hash))
}
pub async fn fetch_blob(base_url: &str, auth_token: &str, hash: &str) -> Result<Vec<u8>, Error> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/sync/v3/files/{}", base_url, hash))
        .bearer_auth(auth_token)
        .send()
        .await?
        .error_for_status()?;

    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}

pub async fn upload_blob(
    base_url: &str,
    auth_token: &str,
    hash: &str,
    filename: &str,
    data: Vec<u8>,
    content_type: &str,
) -> Result<(), Error> {
    let checksum = crc32c::crc32c(&data);
    let checksum_bytes = checksum.to_be_bytes();
    let content_md5 = base64::prelude::BASE64_STANDARD.encode(checksum_bytes);
    let hash_header_value = format!("crc32c={}", content_md5);

    let client = reqwest::Client::new();
    let response = client
        .put(format!("{}/sync/v3/files/{}", base_url, hash))
        .bearer_auth(auth_token)
        .header("rm-filename", filename)
        .header("rm-source", "rmapi-rs")
        .header("User-Agent", "rmapi-rs")
        .header("x-goog-hash", hash_header_value)
        .header("Content-Type", content_type)
        .header("Content-Length", data.len().to_string())
        .body(data)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        log::error!(
            "Upload failed for {} ({}): {} - {}",
            filename,
            hash,
            status,
            text
        );
        return Err(Error::Message(format!(
            "Upload failed: {} - {}",
            status, text
        )));
    }
    Ok(())
}

pub async fn update_root(
    base_url: &str,
    auth_token: &str,
    hash: &str,
    generation: u64,
) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "hash": hash,
        "generation": generation,
        "broadcast": true
    });

    client
        .put(format!("{}/sync/v3/root", base_url))
        .bearer_auth(auth_token)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
