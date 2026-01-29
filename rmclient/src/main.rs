use clap::Parser;
use std::path::{Path, PathBuf};

use rmapi::RmClient;

mod rmclient;
use crate::rmclient::commands::Commands;
use crate::rmclient::error::Error;
use crate::rmclient::token::{default_token_file_path, write_token_file};

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

async fn client_from_token_file(auth_token_file: &Path) -> Result<RmClient, Error> {
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
            Ok(RmClient::from_token(&auth_data.user_token, Some(auth_data.device_token)).await?)
        } else {
            // Fallback to legacy plain text token (treat as user token only)
            Ok(RmClient::from_token(&file_content.trim(), None).await?)
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
            let client = RmClient::new(&code).await?;
            write_token_file(&client, &args.auth_token_file).await?;
            println!(
                "Registration successful! Token saved to {:?}",
                args.auth_token_file
            );
        }
        Commands::Ls { path } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            crate::rmclient::token::refetch_if_unauthorized(&mut client, &args.auth_token_file)
                .await?;

            let target_path = path.as_deref().unwrap_or(Path::new("/"));
            let entries = client.filesystem.list_dir(Some(target_path))?;

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
            let mut shell = crate::rmclient::shell::Shell::new(client, args.auth_token_file);
            shell.run().await?;
        }
        Commands::Put { path, destination } => {
            if path.extension() != Some("pdf".as_ref()) {
                return Err(Error::Message("Only PDF files are supported".to_string()));
            }
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            crate::rmclient::token::refetch_if_unauthorized(&mut client, &args.auth_token_file)
                .await?;

            let parent_id = match destination {
                Some(dest) if !dest.as_os_str().is_empty() => {
                    let normalized = rmapi::filesystem::normalize_path(&dest, Path::new("/"));
                    let node = client
                        .filesystem
                        .find_node_by_path(&normalized)
                        .map_err(Error::Rmapi)?;
                    if !node.is_directory() {
                        return Err(Error::Message(format!(
                            "Destination is not a directory: {}",
                            dest.display()
                        )));
                    }
                    Some(node.id().to_string())
                }
                _ => None,
            };

            client
                .put_document(&path, parent_id.as_deref())
                .await
                .map_err(Error::Rmapi)?;
            println!("Upload successful");
        }
        Commands::Rm { path } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            crate::rmclient::token::refetch_if_unauthorized(&mut client, &args.auth_token_file)
                .await?;

            let normalized_path = rmapi::filesystem::normalize_path(&path, Path::new("/"));
            let node = client
                .filesystem
                .find_node_by_path(&normalized_path)
                .map_err(Error::Rmapi)?;

            client
                .delete_entry(&node.document)
                .await
                .map_err(Error::Rmapi)?;
            println!("Removal successful");
        }
        Commands::Get { name, recursive } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            crate::rmclient::token::refetch_if_unauthorized(&mut client, &args.auth_token_file)
                .await?;

            let path = if name.starts_with('/') {
                name.clone()
            } else {
                format!("/{}", name)
            };

            let node = client
                .filesystem
                .find_node_by_path(&path)
                .map_err(Error::Rmapi)?;

            client
                .download_entry(node, PathBuf::from("."), recursive)
                .map_err(Error::Rmapi)?
                .await
                .map_err(Error::Rmapi)?;
            println!("Download complete");
        }
    }
    Ok(())
}
