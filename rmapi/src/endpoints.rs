use crate::constants::{
    DOC_UPLOAD_ENDPOINT, GROUP_AUTH, HEADER_RM_FILENAME, HEADER_RM_META, HEADER_RM_SOURCE,
    HEADER_X_GOOG_HASH, NEW_CLIENT_URL, NEW_TOKEN_URL, ROOT_SYNC_ENDPOINT, STORAGE_API_URL_ROOT,
    STORAGE_DISCOVERY_API_URL, STORAGE_DISCOVERY_API_VERSION, WEBAPP_API_URL_ROOT,
};
use crate::error::Error;
use crate::objects::{V4Entry, V4Metadata};
use base64::Engine;
use log;
use reqwest::{self, Body};
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct ClientRegistation {
    code: String,
    deviceDesc: String,
    deviceID: String,
}

pub async fn register_client(client: &reqwest::Client, code: &str) -> Result<String, Error> {
    log::info!("Registering client with code: {}", code);
    let registration_info = ClientRegistation {
        code: code.to_string(),
        deviceDesc: "desktop-windows".to_string(),
        deviceID: Uuid::new_v4().to_string(),
    };

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

pub async fn refresh_token(client: &reqwest::Client, auth_token: &str) -> Result<String, Error> {
    log::info!("Refreshing token");
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

pub async fn discover_storage(client: &reqwest::Client, auth_token: &str) -> Result<String, Error> {
    log::info!("Discovering storage host");
    let discovery_request = vec![
        ("environment", "production"),
        ("group", GROUP_AUTH),
        ("apiVer", STORAGE_DISCOVERY_API_VERSION),
    ];
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

pub async fn sync_root(
    client: &reqwest::Client,
    storage_url: &str,
    auth_token: &str,
) -> Result<String, Error> {
    log::info!("Listing items in the rmCloud");
    let response = client
        .get(format!("{}/{}", storage_url, ROOT_SYNC_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Accept", "application/json")
        .header(HEADER_RM_FILENAME, "roothash")
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

pub async fn upload_request(
    client: &reqwest::Client,
    _: &str,
    auth_token: &str,
) -> Result<String, Error> {
    log::info!("Requesting to upload a document to the rmCloud");
    let response = client
        .get(format!("{}/{}", WEBAPP_API_URL_ROOT, DOC_UPLOAD_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Accept", "application/json")
        .header(HEADER_RM_SOURCE, "WebLibrary")
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

pub async fn upload_file(
    client: &reqwest::Client,
    _: &str,
    auth_token: &str,
    file: File,
) -> Result<String, Error> {
    log::info!("Requesting to upload a document to the rmCloud");
    let stream = FramedRead::new(file, BytesCodec::new());
    let body = Body::wrap_stream(stream);

    let response = client
        .post(format!("{}/{}", WEBAPP_API_URL_ROOT, DOC_UPLOAD_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Accept-Encoding", "gzip, deflate, br")
        .header(HEADER_RM_SOURCE, "WebLibrary")
        .header(HEADER_RM_META, "")
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
    client: &reqwest::Client,
    _storage_url: &str, // Ignored because Sync V4 needs internal host
    auth_token: &str,
) -> Result<(Vec<crate::objects::Document>, String), Error> {
    log::info!("Requesting files version Sync V4");

    // 1. Get the root hash
    let root_hash_response = client
        .get(format!("{}/{}", STORAGE_API_URL_ROOT, ROOT_SYNC_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Accept", "application/json")
        .header(HEADER_RM_FILENAME, "roothash")
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
        .header(HEADER_RM_FILENAME, "roothash")
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
                .header(HEADER_RM_FILENAME, format!("{}.docSchema", entry.doc_id))
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
                .header(HEADER_RM_FILENAME, format!("{}.metadata", entry.doc_id))
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

pub async fn fetch_blob(
    client: &reqwest::Client,
    base_url: &str,
    auth_token: &str,
    hash: &str,
) -> Result<Vec<u8>, Error> {
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
    client: &reqwest::Client,
    base_url: &str,
    auth_token: &str,
    hash: &str,
    filename: &str,
    data: &[u8],
    content_type: &str,
) -> Result<(), Error> {
    let checksum = crc32c::crc32c(data);
    let checksum_bytes = checksum.to_be_bytes();
    let content_md5 = base64::engine::general_purpose::STANDARD.encode(checksum_bytes);
    let hash_header_value = format!("crc32c={}", content_md5);

    let response = client
        .put(format!("{}/sync/v3/files/{}", base_url, hash))
        .bearer_auth(auth_token)
        .header(HEADER_RM_FILENAME, filename)
        .header(HEADER_X_GOOG_HASH, hash_header_value)
        .header("Content-Type", content_type)
        .header("Content-Length", data.len().to_string())
        .body(data.to_vec())
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
    client: &reqwest::Client,
    base_url: &str,
    auth_token: &str,
    hash: &str,
    generation: u64,
) -> Result<(), Error> {
    let body = serde_json::json!({
        "hash": hash,
        "generation": generation,
        "broadcast": true
    });

    client
        .put(format!("{}/{}", base_url, ROOT_SYNC_ENDPOINT))
        .bearer_auth(auth_token)
        .header("Content-Type", "application/json")
        .header(HEADER_RM_FILENAME, "roothash")
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
