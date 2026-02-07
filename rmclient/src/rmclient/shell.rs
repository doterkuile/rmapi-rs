use crate::rmclient::actions;
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
            ShellCommand::Ls { path } => self.exec_ls(path.as_deref()).await?,
            ShellCommand::Cd { path } => self.exec_cd(path.as_deref()).await?,
            ShellCommand::Pwd => println!("{}", self.current_path.display()),
            ShellCommand::Exit | ShellCommand::Quit => return Ok(true),
            ShellCommand::Rm { path } => self.exec_rm(&path).await?,
            ShellCommand::Put { path, destination } => {
                self.exec_put(&path, destination.as_deref()).await?
            }
            ShellCommand::Get { path, recursive } => self.exec_get(&path, recursive).await?,
            ShellCommand::Mv { path, destination } => self.exec_mv(&path, &destination).await?,
        }
        Ok(false)
    }

    async fn exec_ls(&mut self, path: Option<&Path>) -> Result<(), Error> {
        let target_buf;
        let target = if let Some(p) = path {
            target_buf = rmapi::filesystem::normalize_path(p, &self.current_path);
            &target_buf
        } else {
            &self.current_path
        };

        actions::ls(&self.client, target).await
    }

    async fn exec_rm(&mut self, path: &Path) -> Result<(), Error> {
        let target = rmapi::filesystem::normalize_path(path, &self.current_path);

        if target == Path::new("/") {
            println!("Error: Cannot remove the root directory.");
            return Ok(());
        }

        actions::rm(&self.client, &target).await?;

        // Refresh file list
        self.client.list_files().await?;
        println!("Removed {}", target.display());
        Ok(())
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

    async fn exec_put(&mut self, path: &Path, destination: Option<&Path>) -> Result<(), Error> {
        let destination_path = if let Some(dest) = destination {
            Some(rmapi::filesystem::normalize_path(dest, &self.current_path))
        } else {
            None
        };

        actions::put(&mut self.client, path, destination_path.as_deref()).await?;

        // Refresh file list
        self.client.list_files().await?;
        Ok(())
    }

    async fn exec_get(&mut self, path: &Path, recursive: bool) -> Result<(), Error> {
        let target = rmapi::filesystem::normalize_path(path, &self.current_path);
        actions::get(&self.client, &target, recursive).await
    }

    async fn exec_mv(&mut self, path: &Path, destination: &Path) -> Result<(), Error> {
        let src_target = rmapi::filesystem::normalize_path(path, &self.current_path);
        let dest_target = rmapi::filesystem::normalize_path(destination, &self.current_path);

        actions::mv(&self.client, &src_target, &dest_target).await?;

        // Refresh file list
        self.client.list_files().await?;
        Ok(())
    }
}
