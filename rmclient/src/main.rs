use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::Path;

use dirs::config_dir;
use rmapi::Client;
use std::path::PathBuf;

mod rmclient;
use crate::rmclient::commands::Commands;
use crate::rmclient::error::Error;

pub fn default_token_file_path() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("rmapi/auth_token")
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(
        short = 't',
        long = "auth-token-file",
        env = "RMAPI_AUTH_TOKEN_FILE",
        help = "Path to the file that holds a previously generated session token",
        default_value = default_token_file_path().into_os_string()
    )]
    auth_token_file: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Serialize, Deserialize, Debug)]
struct AuthData {
    device_token: String,
    user_token: String,
}

async fn write_token_file(client: &Client, auth_token_file: &Path) -> Result<(), Error> {
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

#[allow(dead_code)]
async fn refresh_client_token(client: &mut Client, auth_token_file: &Path) -> Result<(), Error> {
    client.refresh_token().await?;
    log::debug!("Saving new auth token to: {:?}", auth_token_file);
    write_token_file(client, auth_token_file).await?;
    Ok(())
}

async fn client_from_token_file(auth_token_file: &Path) -> Result<Client, Error> {
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
        if let Ok(auth_data) = serde_json::from_str::<AuthData>(&file_content) {
            Ok(Client::from_token(&auth_data.user_token, Some(auth_data.device_token)).await?)
        } else {
            // Fallback to legacy plain text token (treat as user token only)
            Ok(Client::from_token(&file_content.trim(), None).await?)
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse();

    if let Err(err) = run(args).await {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), Error> {
    match args.command {
        Commands::Register { code } => {
            let client = Client::new(&code).await?;
            write_token_file(&client, &args.auth_token_file).await?;
            println!(
                "Registration successful! Token saved to {:?}",
                args.auth_token_file
            );
        }
        Commands::Ls { path } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            let _ = client.list_files().await?; // Populate tree/cache
            let target_path = path.as_deref();
            let entries = client
                .filesystem
                .list_dir(target_path)
                .map_err(|e| Error::Rmapi(e))?;

            for node in entries {
                let suffix = if node.is_directory() { "/" } else { "" };
                let last_modified = node.document.last_modified.format("%Y-%m-%d %H:%M:%S");
                println!(
                    "{:<40}  {}",
                    format!("{}{}", node.name(), suffix),
                    last_modified
                );
            }
        }
        Commands::Shell => {
            let client = client_from_token_file(&args.auth_token_file).await?;
            let mut shell = crate::rmclient::shell::Shell::new(client);
            shell.run().await?;
        }
        Commands::Upload { file_path: _ } => {
            println!("Upload is currently not implemented for Sync V4");
        }
        Commands::Download { path, recursive } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            let _ = client.list_files().await?; // Populate tree/cache

            let root_node = client.filesystem.get_node_by_path(&path).ok_or_else(|| {
                Error::Rmapi(rmapi::error::Error::Message(format!(
                    "Path not found: {}",
                    path
                )))
            })?;

            client
                .download_tree(root_node, Path::new("."), recursive)
                .await
                .map_err(Error::Rmapi)?;
        }
    }
    Ok(())
}
