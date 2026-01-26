use crate::endpoints::{
    fetch_blob, get_files, refresh_token, register_client, update_root, upload_blob,
    STORAGE_API_URL_ROOT,
};
use crate::error::Error;
use crate::filesystem::FileSystem;
use crate::objects::Document;
use sha2::{Digest, Sha256};

pub struct Client {
    pub auth_token: String,
    pub device_token: Option<String>,
    pub storage_url: String,
    pub filesystem: FileSystem,
}

impl Client {
    pub async fn from_token(auth_token: &str, device_token: Option<String>) -> Result<Self, Error> {
        log::debug!("New client with auth token");
        let filesystem = FileSystem::load_cache().unwrap_or_else(|e| {
            log::error!("Failed to load cache, creating new one. Error: {}", e);
            FileSystem::new()
        });
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
        let remote_hash = root_info["hash"]
            .as_str()
            .ok_or(Error::Message(
                "Missing hash field in root hash response".to_string(),
            ))?
            .to_string();
        if remote_hash == self.filesystem.current_hash {
            log::debug!("Cache unchanged, using local tree");
            return Ok(self.filesystem.get_all_documents());
        }

        let (docs, hash) = get_files(&self.storage_url, &self.auth_token).await?;
        self.filesystem.save_cache(&hash, &docs)?;
        Ok(docs)
    }

    pub async fn delete_entry(&self, doc: &Document) -> Result<(), Error> {
        log::info!("Deleting document: {} ({})", doc.display_name, doc.id);

        self.modify_root_index(move |root_lines| {
            let doc_id_str = doc.id.to_string();
            let mut target_index = Option::<usize>::None;

            for (i, line) in root_lines.iter().enumerate() {
                if i == 0 {
                    continue;
                }
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 && parts[2] == doc_id_str {
                    target_index = Some(i);
                    break;
                }
            }

            if let Some(idx) = target_index {
                root_lines.remove(idx);
                Ok(())
            } else {
                Err(Error::Message(
                    "Document not found in root index".to_string(),
                ))
            }
        })
        .await?;

        log::info!("Deletion successful");
        Ok(())
    }

    async fn modify_root_index<F>(&self, modifier: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Vec<String>) -> Result<(), Error>,
    {
        // 1. Get root hash
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
        let root_hash = root_info["hash"]
            .as_str()
            .ok_or(Error::Message("Missing hash".to_string()))?;
        let generation = root_info["generation"].as_u64().unwrap_or(0);

        // 2. Fetch root index blob
        let root_blob = fetch_blob(&self.storage_url, &self.auth_token, root_hash).await?;
        let root_content = String::from_utf8(root_blob)?;
        let mut root_lines: Vec<String> = root_content.lines().map(|s| s.to_string()).collect();

        // 3. Apply modifier
        modifier(&mut root_lines)?;

        // 4. Reconstruct root index
        let new_root_content = root_lines.join("\n");
        let new_root_bytes = new_root_content.into_bytes();
        let new_root_hash = self.compute_hash(&new_root_bytes);

        // 5. Upload new root index blob
        upload_blob(
            &self.storage_url,
            &self.auth_token,
            &new_root_hash,
            "root",
            new_root_bytes,
            "application/octet-stream",
        )
        .await?;

        // 6. Update root pointer
        update_root(
            &self.storage_url,
            &self.auth_token,
            &new_root_hash,
            generation + 1,
        )
        .await?;

        Ok(())
    }

    fn compute_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }
}
