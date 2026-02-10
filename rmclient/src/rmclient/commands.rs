use crate::rmclient::actions;
use crate::rmclient::error::Error;
use clap::Subcommand;
use rmapi::RmClient;
use std::path::{Path, PathBuf};

pub struct CommandContext<'a> {
    pub client: Option<&'a mut RmClient>,
    pub current_path: &'a Path,
    pub auth_token_file: &'a Path,
}

pub trait Executable {
    async fn execute(&self, ctx: &mut CommandContext<'_>) -> Result<(), Error>;
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Register this client with reMarkable
    Register {
        /// Registration code from https://my.remarkable.com/device/desktop/connect
        code: String,
    },
    /// List files in the reMarkable Cloud
    Ls {
        /// Optional path to list
        path: Option<PathBuf>,
    },
    /// Start interactive shell
    Shell,
    /// Upload a file to the reMarkable Cloud
    Put {
        /// Path to the file to upload
        path: PathBuf,
        /// Optional target directory (defaults to root)
        destination: Option<PathBuf>,
    },
    /// Remove a file or directory
    Rm {
        /// Path of the file to remove
        path: PathBuf,
    },
    /// Download a file or directory
    Get {
        /// Path of the file/directory to download
        path: PathBuf,
        /// Recursive download
        #[arg(short, long)]
        recursive: bool,
    },
    /// Move a file or directory
    Mv {
        /// Path of the file/directory to move
        path: PathBuf,
        /// Destination path
        destination: PathBuf,
    },
}

impl Executable for Commands {
    async fn execute(&self, ctx: &mut CommandContext<'_>) -> Result<(), Error> {
        match self {
            Commands::Register { code } => {
                let client = RmClient::new(code).await?;
                crate::rmclient::token::write_token_file(&client, ctx.auth_token_file).await?;
                println!(
                    "Registration successful! Token saved to {:?}",
                    ctx.auth_token_file
                );
                Ok(())
            }
            Commands::Ls { path } => {
                let client = ctx
                    .client
                    .as_mut()
                    .ok_or_else(|| Error::Message("Client required".into()))?;
                let target_path = path.as_deref().unwrap_or(Path::new("."));
                let normalized = rmapi::filesystem::normalize_path(target_path, ctx.current_path);
                actions::ls(client, &normalized).await
            }
            Commands::Shell => {
                let client =
                    crate::rmclient::token::client_from_token_file(ctx.auth_token_file).await?;
                let mut shell =
                    crate::rmclient::shell::Shell::new(client, ctx.auth_token_file.to_path_buf());
                shell.run().await
            }
            Commands::Put { path, destination } => {
                let client = ctx
                    .client
                    .as_mut()
                    .ok_or_else(|| Error::Message("Client required".into()))?;
                let dest_path = destination
                    .as_deref()
                    .map(|d| rmapi::filesystem::normalize_path(d, ctx.current_path));
                actions::put(client, path, dest_path.as_deref()).await
            }
            Commands::Rm { path } => {
                let client = ctx
                    .client
                    .as_mut()
                    .ok_or_else(|| Error::Message("Client required".into()))?;
                let normalized = rmapi::filesystem::normalize_path(path, ctx.current_path);
                actions::rm(client, &normalized).await
            }
            Commands::Get { path, recursive } => {
                let client = ctx
                    .client
                    .as_mut()
                    .ok_or_else(|| Error::Message("Client required".into()))?;
                let normalized = rmapi::filesystem::normalize_path(path, ctx.current_path);
                actions::get(client, &normalized, *recursive).await
            }
            Commands::Mv { path, destination } => {
                let client = ctx
                    .client
                    .as_mut()
                    .ok_or_else(|| Error::Message("Client required".into()))?;
                let src_normalized = rmapi::filesystem::normalize_path(path, ctx.current_path);
                let dest_normalized =
                    rmapi::filesystem::normalize_path(destination, ctx.current_path);
                actions::mv(client, &src_normalized, &dest_normalized).await
            }
        }
    }
}
