use clap::Parser;
use std::path::{Path, PathBuf};

use rmapi::RmClient;

mod rmclient;
use crate::rmclient::commands::{CommandContext, Commands, Executable};
use crate::rmclient::error::Error;
use crate::rmclient::token::{client_from_token_file, default_token_file_path};

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

async fn prepare_client(auth_token_file: &Path) -> Result<RmClient, Error> {
    let mut client = client_from_token_file(auth_token_file).await?;
    crate::rmclient::token::refetch_if_unauthorized(&mut client, auth_token_file).await?;
    Ok(client)
}

async fn run(args: Args) -> Result<(), Error> {
    let mut client = match args.command {
        Commands::Register { .. } => None,
        _ => Some(prepare_client(&args.auth_token_file).await?),
    };

    let mut ctx = CommandContext {
        client: client.as_mut(),
        current_path: Path::new("/"),
        auth_token_file: &args.auth_token_file,
    };

    args.command.execute(&mut ctx).await
}
