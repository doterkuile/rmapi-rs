use clap::Parser;
use std::path::{Path, PathBuf};

mod rmclient;
use crate::rmclient::actions;
use crate::rmclient::commands::Commands;
use crate::rmclient::error::Error;
use crate::rmclient::token::{
    client_from_registration_code, client_from_token_file, default_token_file_path,
};

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
            let _client = client_from_registration_code(&code, &args.auth_token_file).await?;
            println!(
                "Registration successful! Token saved to {:?}",
                args.auth_token_file
            );
        }
        Commands::Ls { path } => {
            let client = client_from_token_file(&args.auth_token_file).await?;
            let target_path = path.as_deref().unwrap_or(Path::new("/"));
            actions::ls(&client, target_path).await?;
        }
        Commands::Shell => {
            let client = client_from_token_file(&args.auth_token_file).await?;
            let mut shell = crate::rmclient::shell::Shell::new(client, args.auth_token_file);
            shell.run().await?;
        }
        Commands::Put { path, destination } => {
            let mut client = client_from_token_file(&args.auth_token_file).await?;
            let destination_path = if let Some(dest) = destination {
                Some(rmapi::filesystem::normalize_path(&dest, Path::new("/")))
            } else {
                None
            };
            actions::put(&mut client, &path, destination_path.as_deref()).await?;
        }
        Commands::Rm { path } => {
            let client = client_from_token_file(&args.auth_token_file).await?;
            let normalized_path = rmapi::filesystem::normalize_path(&path, Path::new("/"));
            actions::rm(&client, &normalized_path).await?;
        }
        Commands::Get { path, recursive } => {
            let client = client_from_token_file(&args.auth_token_file).await?;
            let normalized_path = rmapi::filesystem::normalize_path(&path, Path::new("/"));
            actions::get(&client, &normalized_path, recursive).await?;
        }
        Commands::Mv { path, destination } => {
            let client = client_from_token_file(&args.auth_token_file).await?;
            let normalized_path = rmapi::filesystem::normalize_path(&path, Path::new("/"));
            let normalized_destination =
                rmapi::filesystem::normalize_path(&destination, Path::new("/"));
            actions::mv(&client, &normalized_path, &normalized_destination).await?;
        }
    }
    Ok(())
}
