use crate::endpoints::{get_files, refresh_token, register_client, STORAGE_API_URL_ROOT};
use crate::error::Error;
use crate::filesystem::FileSystem;
use crate::objects::Document;

pub struct Client {
    pub auth_token: String,
    pub device_token: Option<String>,
    pub storage_url: String,
    pub filesystem: FileSystem,
}

impl Client {
    pub async fn from_token(auth_token: &str, device_token: Option<String>) -> Result<Self, Error> {
        log::debug!("New client with auth token");
        let filesystem = FileSystem::load_cache().unwrap_or_else(|_| FileSystem::new());
        Ok(Client {
            auth_token: auth_token.to_string(),
            device_token,
            storage_url: STORAGE_API_URL_ROOT.to_string(),
            filesystem,
        })
    }

    pub async fn new(code: &str) -> Result<Self, Error> {
        log::debug!("Registering client with reMarkable Cloud");
        let device_token = register_client(code).await?;
        let user_token = refresh_token(&device_token).await?;
        Client::from_token(&user_token, Some(device_token)).await
    }

    pub async fn refresh_token(&mut self) -> Result<(), Error> {
        log::debug!("Refreshing auth token");
        let token_to_use = self.device_token.as_ref().unwrap_or(&self.auth_token);
        let new_token = refresh_token(token_to_use).await?;
        self.auth_token = new_token;
        Ok(())
    }

    pub async fn list_files(&mut self) -> Result<Vec<Document>, Error> {
        // 1. Get the remote root hash
        let client = reqwest::Client::new();
        let root_hash_response = client
            .get(format!(
                "{}/{}",
                STORAGE_API_URL_ROOT,
                crate::endpoints::ROOT_SYNC_ENDPOINT
            ))
            .bearer_auth(&self.auth_token)
            .header("Accept", "application/json")
            .header("rm-filename", "roothash")
            .send()
            .await?
            .error_for_status()?;

        let root_resp_text = root_hash_response.text().await?;
        let root_info: serde_json::Value = serde_json::from_str(&root_resp_text)?;
        let remote_hash = root_info["hash"].as_str().unwrap_or_default().to_string();

        if remote_hash == self.filesystem.current_hash {
            log::debug!("Cache hit, using local tree");
            return Ok(self.filesystem.get_all_documents());
        }

        let (docs, hash) = get_files(&self.storage_url, &self.auth_token).await?;
        self.filesystem.save_cache(&hash, &docs)?;
        Ok(docs)
    }
}
