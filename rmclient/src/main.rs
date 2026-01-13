use clap::Parser;
use std::path::{Path, PathBuf};

use rmapi::Client;

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
        if let Ok(auth_data) =
            serde_json::from_str::<crate::rmclient::token::AuthData>(&file_content)
        {
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
            if let Err(e) = client.list_files().await {
                if e.is_unauthorized() {
                    log::info!("Token expired, refreshing...");
                    client.refresh_token().await?;
                    write_token_file(&client, &args.auth_token_file).await?;
                    client.list_files().await?;
                } else {
                    return Err(Error::Rmapi(e));
                }
            }

            let target_path = path.as_deref().unwrap_or("/");
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
        Commands::Upload {
            file_path,
            remote_path,
        } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            if let Err(e) = client.list_files().await {
                if e.is_unauthorized() {
                    log::info!("Token expired, refreshing...");
                    client.refresh_token().await?;
                    write_token_file(&client, &args.auth_token_file).await?;
                    client.list_files().await?;
                } else {
                    return Err(Error::Rmapi(e));
                }
            }
            client
                .upload_document(&file_path, remote_path.as_deref())
                .await
                .map_err(Error::Rmapi)?;
            println!("Upload successful");
        }
        Commands::Debug => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            println!("Debugging...");
            if let Err(e) = client.list_files().await {
                if e.is_unauthorized() {
                    log::info!("Token expired, refreshing...");
                    client.refresh_token().await?;
                    write_token_file(&client, &args.auth_token_file).await?;
                    client.list_files().await?;
                } else {
                    return Err(Error::Rmapi(e));
                }
            }
            let files = client.filesystem.get_all_documents();
            if let Some(doc) = files
                .iter()
                .find(|d| d.doc_type == rmapi::objects::DocumentType::Document)
            {
                println!("Found document: {}", doc.display_name);
                // Fetch schema
                let schema_bytes = rmapi::endpoints::fetch_blob(
                    &client.storage_url,
                    &client.auth_token,
                    &doc.hash,
                )
                .await
                .map_err(|e| Error::Rmapi(e))?;
                let schema = String::from_utf8_lossy(&schema_bytes);
                println!("Schema:\n{}", schema);

                for line in schema.lines() {
                    if line.contains(".content") {
                        let parts: Vec<&str> = line.split(':').collect();
                        let content_hash = parts[0];
                        println!("Fetching content hash: {}", content_hash);
                        let content_bytes = rmapi::endpoints::fetch_blob(
                            &client.storage_url,
                            &client.auth_token,
                            content_hash,
                        )
                        .await
                        .map_err(|e| Error::Rmapi(e))?;
                        println!("Content JSON:\n{}", String::from_utf8_lossy(&content_bytes));
                        break;
                    }
                }
            } else {
                println!("No documents found.");
            }
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
        Commands::Rename { name, new_name } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            if let Err(e) = client.list_files().await {
                if e.is_unauthorized() {
                    log::info!("Token expired, refreshing...");
                    client.refresh_token().await?;
                    write_token_file(&client, &args.auth_token_file).await?;
                    client.list_files().await?;
                } else {
                    return Err(Error::Rmapi(e));
                }
            }

            let files = client.filesystem.get_all_documents();
            let doc = files
                .iter()
                .find(|d| d.display_name == name)
                .ok_or_else(|| {
                    Error::Rmapi(rmapi::error::Error::Message(format!(
                        "Document not found: {}",
                        name
                    )))
                })?;

            client
                .rename_entry(doc, &new_name)
                .await
                .map_err(Error::Rmapi)?;
            println!("Rename successful");
        }
        Commands::Remove { name } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            if let Err(e) = client.list_files().await {
                if e.is_unauthorized() {
                    log::info!("Token expired, refreshing...");
                    client.refresh_token().await?;
                    write_token_file(&client, &args.auth_token_file).await?;
                    client.list_files().await?;
                } else {
                    return Err(Error::Rmapi(e));
                }
            }

            let files = client.filesystem.get_all_documents();
            let doc = files
                .iter()
                .find(|d| d.display_name == name)
                .ok_or_else(|| {
                    Error::Rmapi(rmapi::error::Error::Message(format!(
                        "Document not found: {}",
                        name
                    )))
                })?;

            client.delete_entry(doc).await.map_err(Error::Rmapi)?;
            println!("Removal successful");
        }
    }
    Ok(())
}
