use clap::Subcommand;
use std::path::PathBuf;

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
    },
    /// Remove a file or directory
    Rm {
        /// Path of the file to remove
        path: PathBuf,
    },
}
