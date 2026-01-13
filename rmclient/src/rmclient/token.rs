use crate::rmclient::error::Error;
use dirs::config_dir;
use rmapi::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug)]
pub struct AuthData {
    pub device_token: String,
    pub user_token: String,
}

pub fn default_token_file_path() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("rmapi/auth_token")
}

pub async fn write_token_file(client: &Client, auth_token_file: &Path) -> Result<(), Error> {
    if let Some(parent) = auth_token_file.parent() {
        log::debug!("Making client config dir {:?}", parent);
        tokio::fs::create_dir_all(parent).await?;
    }

    if let Some(device_token) = &client.device_token {
        let auth_data = AuthData {
            device_token: device_token.clone(),
            user_token: client.auth_token.clone(),
        };
        let json = serde_json::to_string_pretty(&auth_data)
            .map_err(|e| Error::Rmapi(rmapi::error::Error::Message(e.to_string())))?;
        tokio::fs::write(auth_token_file, json).await?;
    } else {
        tokio::fs::write(auth_token_file, &client.auth_token).await?;
    }

    log::debug!("Saving auth token to: {:?}", auth_token_file);
    Ok(())
}
