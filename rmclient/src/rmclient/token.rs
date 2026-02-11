use crate::rmclient::error::Error;
use dirs::config_dir;
use rmapi::RmClient;
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

pub async fn client_from_token_file(auth_token_file: &Path) -> Result<RmClient, Error> {
    if !auth_token_file.exists() {
        Err(Error::TokenFileNotFound)
    } else if !auth_token_file.is_file() {
        Err(Error::TokenFileInvalid)
    } else {
        let file_content = tokio::fs::read_to_string(&auth_token_file).await?;
        log::debug!(
            "Using token from {:?} to create a new client",
            auth_token_file
        );

        // Try parsing as JSON first
        if let Ok(auth_data) =
            serde_json::from_str::<crate::rmclient::token::AuthData>(&file_content)
        {
            let mut client = RmClient::new(
                &auth_data.device_token,
                Some(&auth_data.user_token),
                None,
                None,
                None,
            )
            .await?;
            refetch_if_unauthorized(&mut client, auth_token_file).await?;
            Ok(client)
        } else {
            Err(Error::TokenFileInvalid)
        }
    }
}

pub async fn client_from_registration_code(
    code: &str,
    auth_token_file: &Path,
) -> Result<RmClient, Error> {
    log::debug!("Registering client with reMarkable Cloud");

    let client = RmClient::register_client(code, None).await?;
    write_token_file(&client, auth_token_file).await?;
    Ok(client)
}

pub async fn write_token_file(client: &RmClient, auth_token_file: &Path) -> Result<(), Error> {
    if let Some(parent) = auth_token_file.parent() {
        log::debug!("Making client config dir {:?}", parent);
        tokio::fs::create_dir_all(parent).await?;
    }

    let auth_data = AuthData {
        device_token: client.device_token.clone(),
        user_token: client.user_token.clone(),
    };
    let json = serde_json::to_string_pretty(&auth_data)
        .map_err(|e| Error::Rmapi(rmapi::error::Error::Message(e.to_string())))?;
    tokio::fs::write(auth_token_file, json).await?;

    log::debug!("Saving auth token to: {:?}", auth_token_file);
    Ok(())
}

pub async fn refetch_if_unauthorized(
    client: &mut RmClient,
    auth_token_file: &Path,
) -> Result<(), Error> {
    if let Err(e) = client.list_files().await {
        if e.is_unauthorized() {
            log::info!("Token expired, refreshing...");
            client.refresh_user_token().await?;
            write_token_file(client, auth_token_file).await?;
            client.list_files().await.map_err(Error::Rmapi)?;
        } else {
            return Err(Error::Rmapi(e));
        }
    }
    Ok(())
}
