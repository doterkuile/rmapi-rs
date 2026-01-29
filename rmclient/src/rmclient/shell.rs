use crate::rmclient::error::Error;
use crate::rmclient::token::write_token_file;
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
        /// Name of the file/directory to download
        path: String,
        /// Recursive download
        #[arg(short, long)]
        recursive: bool,
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

        let entries = self.client.filesystem.list_dir(Some(target))?;
        for node in entries {
            let suffix = if node.is_directory() { "/" } else { "" };
            let last_modified = node.document.last_modified.format("%Y-%m-%d %H:%M:%S");
            println!(
                "{:<40}  {}",
                format!("{}{}", node.name(), suffix),
                last_modified
            );
        }
        Ok(())
    }

    async fn exec_rm(&mut self, path: &Path) -> Result<(), Error> {
        let target = rmapi::filesystem::normalize_path(path, &self.current_path);

        if target == Path::new("/") {
            println!("Error: Cannot remove the root directory.");
            return Ok(());
        }

        let node = self.client.filesystem.find_node_by_path(&target)?;

        self.client
            .delete_entry(&node.document)
            .await
            .map_err(Error::Rmapi)?;

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

        match self.client.filesystem.find_node_by_path(&target) {
            Ok(node) => {
                if node.is_directory() {
                    self.current_path = target;
                } else {
                    println!("Not a directory: {}", target.display());
                }
            }
            Err(_) => {
                println!("No such directory: {}", target.display());
            }
        }
        Ok(())
    }

    async fn exec_put(&mut self, path: &Path, destination: Option<&Path>) -> Result<(), Error> {
        if path.extension() != Some("pdf".as_ref()) {
            return Err(Error::Message("Only PDF files are supported".to_string()));
        }

        let target = if let Some(dest) = destination {
            rmapi::filesystem::normalize_path(dest, &self.current_path)
        } else {
            self.current_path.clone()
        };

        let parent_id = {
            let node = self.client.filesystem.find_node_by_path(&target)?;
            if !node.is_directory() {
                return Err(Error::Message(format!(
                    "Destination is not a directory: {}",
                    target.display()
                )));
            }
            node.id().to_string()
        };

        self.client
            .put_document(path, Some(&parent_id))
            .await
            .map_err(Error::Rmapi)?;
        // Refresh file list
        self.client.list_files().await?;
        println!(
            "Uploaded {} as new document to {}",
            path.display(),
            target.display()
        );
        Ok(())
    }

    async fn exec_get(&mut self, path: String, recursive: bool) -> Result<(), Error> {
        let target = rmapi::filesystem::normalize_path(&path, &self.current_path);
        let node = self
            .client
            .filesystem
            .find_node_by_path(&target)
            .map_err(Error::from)?; // Error converts from rmapi::Error via From impl

        self.client
            .download_entry(node, PathBuf::from("."), recursive)
            .map_err(Error::Rmapi)?
            .await
            .map_err(Error::Rmapi)?;

        println!("Download complete");
        Ok(())
    }
}
