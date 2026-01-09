use crate::rmclient::error::Error;
use rmapi::Client;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

pub struct Shell {
    client: Client,
    current_path: String,
}

impl Shell {
    pub fn new(client: Client) -> Self {
        Shell {
            client,
            current_path: "/".to_string(),
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        println!("Welcome to rmapi-rs shell!");
        println!("Loading file tree...");
        let _ = self.client.list_files().await?;

        let mut rl: DefaultEditor =
            DefaultEditor::new().map_err(|e| Error::Message(e.to_string()))?;

        loop {
            let readline = rl.readline(&format!("[{}]> ", self.current_path));
            match readline {
                Ok(line) => {
                    let _ = rl.add_history_entry(line.as_str());
                    let parts: Vec<&str> = line.trim().split_whitespace().collect();
                    if parts.is_empty() {
                        continue;
                    }

                    match parts[0] {
                        "ls" => {
                            let target = if parts.len() > 1 {
                                parts[1]
                            } else {
                                &self.current_path
                            };
                            match self.client.filesystem.list_dir(target) {
                                Ok(entries) => {
                                    for node in entries {
                                        let suffix = if node.is_directory() { "/" } else { "" };
                                        println!("{}{}", node.name(), suffix);
                                    }
                                }
                                Err(e) => println!("Error: {}", e),
                            }
                        }
                        "cd" => {
                            if parts.len() > 1 {
                                let target = parts[1];
                                let new_path = if target.starts_with('/') {
                                    target.to_string()
                                } else {
                                    let base = if self.current_path.ends_with('/') {
                                        &self.current_path
                                    } else {
                                        &format!("{}/", self.current_path)
                                    };
                                    format!("{}{}", base, target)
                                };

                                match self.client.filesystem.find_node_by_path(&new_path) {
                                    Ok(node) => {
                                        if node.is_directory() {
                                            self.current_path = new_path;
                                        } else {
                                            println!("Not a directory: {}", target);
                                        }
                                    }
                                    Err(e) => println!("Error: {}", e),
                                }
                            } else {
                                self.current_path = "/".to_string();
                            }
                        }
                        "pwd" => println!("{}", self.current_path),
                        "exit" | "quit" => break,
                        _ => println!("Unknown command: {}", parts[0]),
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }
        Ok(())
    }
}
