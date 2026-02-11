use crate::constants::{
    DOC_UPLOAD_ENDPOINT, GROUP_AUTH, HEADER_RM_FILENAME, HEADER_RM_META, HEADER_RM_SOURCE,
    HEADER_X_GOOG_HASH, ROOT_SYNC_ENDPOINT, STORAGE_DISCOVERY_API_VERSION,
};
use crate::error::Error;
use crate::objects::{ClientRegistration, RootInfo, StorageInfo, V4Entry, V4Metadata};
use base64::Engine;
use log;
use reqwest::{self, Body};

use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use uuid::Uuid;

pub async fn register_client(
    http_client: &reqwest::Client,
    base_url: &str,
    code: &str,
) -> Result<String, Error> {
    log::info!("Registering client with code: {}", code);
    let registration_info = ClientRegistration {
        code: code.to_string(),
        device_desc: "desktop-windows".to_string(),
        device_id: Uuid::new_v4().to_string(),
    };

    let url = format!("{}/token/json/2/device/new", base_url);
    let response = http_client
        .post(&url)
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

pub async fn refresh_user_token(
    http_client: &reqwest::Client,
    base_url: &str,
    device_token: &str,
) -> Result<String, Error> {
    log::info!("Refreshing user token");
    let url = format!("{}/token/json/2/user/new", base_url);
    let response = http_client
        .post(&url)
        .bearer_auth(device_token)
        .header("Accept", "application/json")
        .header("Content-Length", "0")
        .send()
        .await?;

    log::debug!("{:?}", response);

    match response.error_for_status() {
        Ok(res) => {
            let user_token = res.text().await?;
            log::debug!("User Token: {}", user_token);
            Ok(user_token)
        }
        Err(e) => {
            log::error!("Error refreshing user token: {}", e);
            Err(Error::from(e))
        }
    }
}

pub async fn discover_storage(
    http_client: &reqwest::Client,
    base_url: &str,
    user_token: &str,
) -> Result<String, Error> {
    log::info!("Discovering storage host");
    let discovery_request = vec![
        ("environment", "production"),
        ("group", GROUP_AUTH),
        ("apiVer", STORAGE_DISCOVERY_API_VERSION),
    ];
    let url = format!("{}/service/json/1/document-storage", base_url);
    let response = http_client
        .get(&url)
        .bearer_auth(user_token)
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
            Ok(format!("https://{0}", storage_info.host))
        }
        Err(e) => {
            log::error!("Error discovering storage: {}", e);
            Err(Error::from(e))
        }
    }
}

pub async fn get_root_info(
    http_client: &reqwest::Client,
    storage_url: &str,
    user_token: &str,
) -> Result<RootInfo, Error> {
    log::debug!("Fetching root info");
    let response = http_client
        .get(format!("{}/{}", storage_url, ROOT_SYNC_ENDPOINT))
        .bearer_auth(user_token)
        .header("Accept", "application/json")
        .header(HEADER_RM_FILENAME, "roothash")
        .send()
        .await?;

    log::debug!("{:?}", response);
    let res = response.error_for_status().map_err(Error::from)?;
    let root_info = res.json::<RootInfo>().await.map_err(|e| {
        log::error!("Failed to parse root JSON: {}", e);
        Error::from(e)
    })?;
    Ok(root_info)
}

pub async fn upload_request(
    http_client: &reqwest::Client,
    base_url: &str,
    user_token: &str,
) -> Result<String, Error> {
    log::info!("Requesting to upload a document to the rmCloud");
    let response = http_client
        .get(format!("{}/{}", base_url, DOC_UPLOAD_ENDPOINT))
        .bearer_auth(user_token)
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
    http_client: &reqwest::Client,
    base_url: &str,
    user_token: &str,
    file: File,
) -> Result<String, Error> {
    log::info!("Requesting to upload a document to the rmCloud");
    let stream = FramedRead::new(file, BytesCodec::new());
    let body = Body::wrap_stream(stream);

    let response = http_client
        .post(format!("{}/{}", base_url, DOC_UPLOAD_ENDPOINT))
        .bearer_auth(user_token)
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
    http_client: &reqwest::Client,
    storage_url: &str,
    user_token: &str,
) -> Result<(Vec<crate::objects::Document>, String), Error> {
    log::info!("Requesting files version Sync V4");

    // 1. Get the root hash
    let root_info = get_root_info(http_client, storage_url, user_token).await?;
    let root_hash = root_info.hash;

    // 2. Fetch the root index blob
    let root_blob_response = http_client
        .get(format!("{}/sync/v3/files/{}", storage_url, root_hash))
        .bearer_auth(user_token)
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
    let user_token = user_token.to_string();
    let client = http_client.clone();
    let storage_url = storage_url.to_string();

    for entry in entries {
        let user_token = user_token.clone();
        let client = client.clone();
        let storage_url = storage_url.clone();
        tasks.push(tokio::spawn(async move {
            // Fetch .docSchema to find .metadata hash
            let doc_schema_response = client
                .get(format!("{}/sync/v3/files/{}", storage_url, entry.hash))
                .bearer_auth(&user_token)
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
                    if !subparts.is_empty() {
                        metadata_hash = Some(subparts[0].to_string());
                        break;
                    }
                }
            }

            let m_hash = metadata_hash?;
            let metadata_response = client
                .get(format!("{}/sync/v3/files/{}", storage_url, m_hash))
                .bearer_auth(&user_token)
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
                last_modified,
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
    http_client: &reqwest::Client,
    base_url: &str,
    user_token: &str,
    hash: &str,
) -> Result<Vec<u8>, Error> {
    let response = http_client
        .get(format!("{}/sync/v3/files/{}", base_url, hash))
        .bearer_auth(user_token)
        .send()
        .await?
        .error_for_status()?;

    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}

pub async fn upload_blob(
    http_client: &reqwest::Client,
    base_url: &str,
    user_token: &str,
    hash: &str,
    filename: &str,
    data: &[u8],
    content_type: &str,
) -> Result<(), Error> {
    let checksum = crc32c::crc32c(data);
    let checksum_bytes = checksum.to_be_bytes();
    let content_md5 = base64::engine::general_purpose::STANDARD.encode(checksum_bytes);
    let hash_header_value = format!("crc32c={}", content_md5);

    let response = http_client
        .put(format!("{}/sync/v3/files/{}", base_url, hash))
        .bearer_auth(user_token)
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
    http_client: &reqwest::Client,
    base_url: &str,
    user_token: &str,
    hash: &str,
    generation: u64,
) -> Result<(), Error> {
    let body = serde_json::json!({
        "hash": hash,
        "generation": generation,
        "broadcast": true
    });

    http_client
        .put(format!("{}/{}", base_url, ROOT_SYNC_ENDPOINT))
        .bearer_auth(user_token)
        .header("Content-Type", "application/json")
        .header(HEADER_RM_FILENAME, "roothash")
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
