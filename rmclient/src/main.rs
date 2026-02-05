use clap::Parser;
use std::path::{Path, PathBuf};

use rmapi::RmClient;

mod rmclient;
use crate::rmclient::actions;
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

async fn prepare_client(auth_token_file: &Path) -> Result<RmClient, Error> {
    let mut client = client_from_token_file(auth_token_file).await?;
    crate::rmclient::token::refetch_if_unauthorized(&mut client, auth_token_file).await?;
    Ok(client)
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
            let client = prepare_client(&args.auth_token_file).await?;
            let target_path = path.as_deref().unwrap_or(Path::new("/"));
            actions::ls(&client, target_path).await?;
        }
        Commands::Shell => {
            let client = client_from_token_file(&args.auth_token_file).await?;
            let mut shell = crate::rmclient::shell::Shell::new(client, args.auth_token_file);
            shell.run().await?;
        }
        Commands::Put { path, destination } => {
            let mut client = prepare_client(&args.auth_token_file).await?;
            let destination_path = if let Some(dest) = destination {
                Some(rmapi::filesystem::normalize_path(&dest, Path::new("/")))
            } else {
                None
            };
            actions::put(&mut client, &path, destination_path.as_deref()).await?;
        }
        Commands::Rm { path } => {
            let client = prepare_client(&args.auth_token_file).await?;
            let normalized_path = rmapi::filesystem::normalize_path(&path, Path::new("/"));
            actions::rm(&client, &normalized_path).await?;
        }
        Commands::Get { path, recursive } => {
            let client = prepare_client(&args.auth_token_file).await?;
            let normalized_path = rmapi::filesystem::normalize_path(&path, Path::new("/"));
            actions::get(&client, &normalized_path, recursive).await?;
        }
    }
    Ok(())
}
