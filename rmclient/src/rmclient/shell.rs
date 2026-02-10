use crate::rmclient::actions;
use crate::rmclient::commands::{CommandContext, Executable};
use crate::rmclient::error::Error;
use clap::Parser;
use rmapi::RmClient;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "", no_binary_name = true)]
enum ShellCommand {
    /// List files in the current or specified path
    Ls {
        /// Optional path to list
        path: Option<PathBuf>,
    },
    /// Change the current directory
    Cd {
        /// Path to navigate to
        path: Option<PathBuf>,
    },
    /// Print the current working directory
    Pwd,
    /// Exit the shell
    Exit,
    /// Alias for Exit
    Quit,
    /// Remove a file
    Rm {
        /// Name of the file to remove
        path: PathBuf,
    },
    /// Upload a file
    Put {
        /// Local path to the file to upload
        path: PathBuf,
        /// Optional target directory (defaults to current directory)
        destination: Option<PathBuf>,
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
        /// Name of the file/directory to move
        path: PathBuf,
        /// Destination path
        destination: PathBuf,
    },
}

impl Executable for ShellCommand {
    async fn execute(&self, ctx: &mut CommandContext<'_>) -> Result<(), Error> {
        let client = ctx
            .client
            .as_mut()
            .ok_or_else(|| Error::Message("Client required".into()))?;

        match self {
            ShellCommand::Ls { path } => {
                let target_path = path.as_deref().unwrap_or(Path::new("."));
                let normalized = rmapi::filesystem::normalize_path(target_path, ctx.current_path);
                actions::ls(client, &normalized).await
            }
            ShellCommand::Rm { path } => {
                let normalized = rmapi::filesystem::normalize_path(path, ctx.current_path);
                if normalized == Path::new("/") {
                    return Err(Error::Message("Cannot remove root directory".into()));
                }
                actions::rm(client, &normalized).await
            }
            ShellCommand::Put { path, destination } => {
                let dest_path = destination
                    .as_deref()
                    .map(|d| rmapi::filesystem::normalize_path(d, ctx.current_path));
                actions::put(client, path, dest_path.as_deref()).await
            }
            ShellCommand::Get { path, recursive } => {
                let normalized = rmapi::filesystem::normalize_path(path, ctx.current_path);
                actions::get(client, &normalized, *recursive).await
            }
            ShellCommand::Mv { path, destination } => {
                let src_normalized = rmapi::filesystem::normalize_path(path, ctx.current_path);
                let dest_normalized =
                    rmapi::filesystem::normalize_path(destination, ctx.current_path);
                actions::mv(client, &src_normalized, &dest_normalized).await
            }
            _ => Ok(()), // Cd, Pwd, Exit handled special
        }
    }
}

pub struct Shell {
    client: RmClient,
    current_path: PathBuf,
    token_file_path: PathBuf,
}

impl Shell {
    pub fn new(client: RmClient, token_file_path: PathBuf) -> Self {
        Shell {
            client,
            current_path: PathBuf::from("/"),
            token_file_path,
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        println!("Welcome to rmapi-rs shell!");
        println!("Loading file tree...");
        crate::rmclient::token::refetch_if_unauthorized(&mut self.client, &self.token_file_path)
            .await?;

        let mut rl: DefaultEditor =
            DefaultEditor::new().map_err(|e| Error::Message(e.to_string()))?;

        loop {
            let prompt = format!("[{}]> ", self.current_path.display());
            match rl.readline(&prompt) {
                Ok(line) => {
                    if self.handle_input(line, &mut rl).await? {
                        break;
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
                Err(err) => return Err(Error::Message(format!("Readline error: {:?}", err))),
            }
        }
        Ok(())
    }

    async fn handle_input(&mut self, line: String, rl: &mut DefaultEditor) -> Result<bool, Error> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(false);
        }
        let _ = rl.add_history_entry(line);

        let parts = shlex::split(line).unwrap_or_default();
        if parts.is_empty() {
            return Ok(false);
        }

        match ShellCommand::try_parse_from(&parts) {
            Ok(cmd) => self.handle_command(cmd).await,
            Err(e) => {
                println!("{}", e);
                Ok(false)
            }
        }
    }

    async fn handle_command(&mut self, cmd: ShellCommand) -> Result<bool, Error> {
        match cmd {
            ShellCommand::Exit | ShellCommand::Quit => return Ok(true),
            ShellCommand::Cd { path } => {
                self.exec_cd(path.as_deref()).await?;
            }
            ShellCommand::Pwd => println!("{}", self.current_path.display()),
            _ => {
                let mut ctx = CommandContext {
                    client: Some(&mut self.client),
                    current_path: &self.current_path,
                    auth_token_file: &self.token_file_path,
                };
                cmd.execute(&mut ctx).await?;

                // Refresh state for commands that modify it
                match cmd {
                    ShellCommand::Rm { .. }
                    | ShellCommand::Put { .. }
                    | ShellCommand::Mv { .. } => {
                        self.client.list_files().await?;
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    async fn exec_cd(&mut self, path: Option<&Path>) -> Result<(), Error> {
        let target = match path {
            Some(p) => rmapi::filesystem::normalize_path(p, &self.current_path),
            None => {
                self.current_path = PathBuf::from("/");
                return Ok(());
            }
        };

        match actions::cd(&self.client, &target) {
            Ok(_) => self.current_path = target,
            Err(e) => println!("{}", e),
        }
        Ok(())
    }
}
