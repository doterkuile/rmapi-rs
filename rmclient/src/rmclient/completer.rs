use crate::rmclient::shell::ShellCommand;
use clap::CommandFactory;
use rmapi::RmClient;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::path::{Path, PathBuf};

pub struct RmCompleter {
    pub client: RmClient,
    pub current_path: PathBuf,
}

impl RmCompleter {
    pub fn new(client: RmClient, current_path: PathBuf) -> Self {
        Self {
            client,
            current_path,
        }
    }
}

impl Completer for RmCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let (prefix, _) = line.split_at(pos);
        let parts: Vec<String> = shlex::split(prefix).unwrap_or_default();

        // 1. Command name completion
        if parts.len() <= 1 && !prefix.ends_with(' ') {
            let cmd = ShellCommand::command();
            let mut candidates = Vec::new();
            for subcommand in cmd.get_subcommands() {
                let name = subcommand.get_name();
                if name.starts_with(prefix) {
                    candidates.push(Pair {
                        display: name.to_string(),
                        replacement: name.to_string(),
                    });
                }
            }
            return Ok((0, candidates));
        }

        // 2. Path completion for commands that take paths
        if !parts.is_empty() {
            let needs_path = matches!(parts[0].as_str(), "ls" | "cd" | "rm" | "get" | "mv" | "put");

            if needs_path {
                let current_input = if prefix.ends_with(' ') {
                    String::new()
                } else {
                    parts.last().cloned().unwrap_or_default()
                };

                let target_path = rmapi::filesystem::normalize_path(
                    Path::new(&current_input),
                    &self.current_path,
                );

                let (dir_to_list, filter_prefix) =
                    if current_input.ends_with('/') || current_input.is_empty() {
                        (target_path, String::new())
                    } else {
                        let parent = target_path.parent().unwrap_or(Path::new("/")).to_path_buf();
                        let file_name = target_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        (parent, file_name)
                    };

                if let Ok(entries) = self.client.filesystem.list_dir(Some(&dir_to_list)) {
                    let mut candidates = Vec::new();
                    let start = pos - current_input.len();

                    for node in entries {
                        let name = node.name();
                        if name.starts_with(&filter_prefix) {
                            let mut replacement = name.to_string();
                            if node.is_directory() {
                                replacement.push('/');
                            }
                            candidates.push(Pair {
                                display: name.to_string(),
                                replacement,
                            });
                        }
                    }
                    return Ok((start, candidates));
                }
            }
        }

        Ok((0, Vec::new()))
    }
}

impl Helper for RmCompleter {}
impl Hinter for RmCompleter {
    type Hint = String;
}
impl Highlighter for RmCompleter {}
impl Validator for RmCompleter {}
