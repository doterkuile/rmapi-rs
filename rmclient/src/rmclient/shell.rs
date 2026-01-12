use crate::rmclient::error::Error;
use clap::Parser;
use rmapi::Client;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

#[derive(Parser, Debug)]
#[command(name = "", no_binary_name = true)]
enum ShellCommand {
    /// List files in the current or specified path
    Ls {
        /// Optional path to list
        path: Option<String>,
    },
    /// Change the current directory
    Cd {
        /// Path to navigate to
        path: Option<String>,
    },
    /// Print the current working directory
    Pwd,
    /// Exit the shell
    Exit,
    /// Alias for Exit
    Quit,
    /// Download a file or directory
    Download {
        /// Path to the file or directory to download
        path: String,
        /// Recursive download (for directories)
        #[arg(short, long)]
        recursive: bool,
    },
    /// Rename a file
    Mv {
        /// Current path
        path: String,
        /// New name
        new_name: String,
    },
}

pub struct Shell {
    client: Client,
}

impl Shell {
    pub fn new(client: Client) -> Self {
        Shell { client }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        println!("Welcome to rmapi-rs shell!");
        println!("Loading file tree...");
        self.client.list_files().await?;

        let mut rl: DefaultEditor =
            DefaultEditor::new().map_err(|e| Error::Message(e.to_string()))?;

        loop {
            let prompt = format!("[{}]> ", self.client.filesystem.pwd());
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
            ShellCommand::Ls { path } => self.exec_ls(path).await?,
            ShellCommand::Cd { path } => self.exec_cd(path).await?,
            ShellCommand::Pwd => println!("{}", self.client.filesystem.pwd()),
            ShellCommand::Exit | ShellCommand::Quit => return Ok(true),
            ShellCommand::Download { path, recursive } => {
                self.exec_download(path, recursive).await?
            }
            ShellCommand::Mv { path, new_name } => self.exec_mv(path, new_name).await?,
        }
        Ok(false)
    }

    async fn exec_mv(&mut self, path: String, new_name: String) -> Result<(), Error> {
        let node = self
            .client
            .filesystem
            .get_node_by_path(&path)
            .ok_or_else(|| Error::Message(format!("Path not found: {}", path)))?;

        self.client
            .rename_entry(&node.document, &new_name)
            .await
            .map_err(|e| Error::Rmapi(e))?;
        // Refresh file list to see changes
        self.client.list_files().await?;
        println!("Renamed {} to {}", path, new_name);
        Ok(())
    }

    async fn exec_ls(&mut self, path: Option<String>) -> Result<(), Error> {
        let entries = self.client.filesystem.list_dir(path.as_deref())?;

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

    async fn exec_cd(&mut self, path: Option<String>) -> Result<(), Error> {
        let target = path.as_deref().unwrap_or("/");
        self.client.filesystem.cd(target)?;
        Ok(())
    }

    async fn exec_download(&self, path: String, recursive: bool) -> Result<(), Error> {
        let node = self
            .client
            .filesystem
            .get_node_by_path(&path)
            .ok_or_else(|| Error::Message(format!("Path not found: {}", path)))?;
        self.client
            .download_tree(node, std::path::Path::new("."), recursive)
            .await
            .map_err(Error::Rmapi)?;
        Ok(())
    }
}
