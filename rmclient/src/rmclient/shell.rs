use crate::rmclient::error::Error;
use crate::rmclient::token::write_token_file;
use clap::Parser;
use rmapi::Client;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::PathBuf;

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
}

pub struct Shell {
    client: Client,
    current_path: String,
    token_file_path: PathBuf,
}

impl Shell {
    pub fn new(client: Client, token_file_path: PathBuf) -> Self {
        Shell {
            client,
            current_path: "/".to_string(),
            token_file_path,
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        println!("Welcome to rmapi-rs shell!");
        println!("Loading file tree...");
        if let Err(e) = self.client.list_files().await {
            if e.is_unauthorized() {
                println!("Token expired, refreshing...");
                self.client.refresh_token().await?;
                write_token_file(&self.client, &self.token_file_path).await?;
                self.client.list_files().await?;
            } else {
                return Err(Error::from(e));
            }
        }

        let mut rl: DefaultEditor =
            DefaultEditor::new().map_err(|e| Error::Message(e.to_string()))?;

        loop {
            let prompt = format!("[{}]> ", self.current_path);
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
            ShellCommand::Pwd => println!("{}", self.current_path),
            ShellCommand::Exit | ShellCommand::Quit => return Ok(true),
        }
        Ok(false)
    }

    fn normalize_path(&self, path: &str) -> String {
        let mut components = Vec::new();

        if !path.starts_with('/') {
            // Relative path, start with current components
            for part in self.current_path.split('/').filter(|s| !s.is_empty()) {
                components.push(part.to_string());
            }
        }

        for part in path.split('/').filter(|s| !s.is_empty()) {
            match part {
                "." => {}
                ".." => {
                    components.pop();
                }
                _ => components.push(part.to_string()),
            }
        }

        if components.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", components.join("/"))
        }
    }

    async fn exec_ls(&mut self, path: Option<String>) -> Result<(), Error> {
        let target = match &path {
            Some(p) => self.normalize_path(p),
            None => self.current_path.clone(),
        };
        let entries = self.client.filesystem.list_dir(Some(&target))?;

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
        let target = match path {
            Some(p) => self.normalize_path(&p),
            None => {
                self.current_path = "/".to_string();
                return Ok(());
            }
        };

        match self.client.filesystem.find_node_by_path(&target) {
            Ok(node) => {
                if node.is_directory() {
                    self.current_path = target;
                } else {
                    println!("Not a directory: {}", target);
                }
            }
            Err(_) => {
                println!("No such directory: {}", target);
            }
        }
        Ok(())
    }
}
