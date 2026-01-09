use crate::error::Error;
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

const DOC_UPLOAD_ENDPOINT: &str = "doc/v2/files";
const ROOT_SYNC_ENDPOINT: &str = "sync/v4/root";
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
        ("enviorment", "production"),
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

pub async fn get_files(_: &str, auth_token: &str) -> Result<Vec<crate::objects::Document>, Error> {
    log::info!("Requesting files on the rmCloud");

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
            let files = res.json::<Vec<crate::objects::Document>>().await?;
            log::debug!("Get files request response: {:?}", files);
            Ok(files)
        }
        Err(e) => {
            log::error!("Error listing items: {}", e);
            Err(Error::from(e))
        }
    }
}
