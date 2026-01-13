use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Register this client with reMarkable
    Register {
        /// Registration code from https://my.remarkable.com/device/desktop/connect
        code: String,
    },
    /// Debug command to fetch content of first document
    Debug,
    /// List files in the reMarkable Cloud
    Ls {
        /// Optional path to list
        path: Option<String>,
    },
    /// Start interactive shell
    Shell,
    Upload {
        /// Path to the file to upload
        file_path: PathBuf,
        /// Remote directory to upload to
        remote_path: Option<String>,
    },
    /// Download a file or directory
    Download {
        /// Path to the file or directory to download
        path: String,
        /// Recursive download (for directories)
        #[arg(short, long)]
        recursive: bool,
    },
    /// Rename a file or directory
    Rename {
        /// Name of the file to rename
        name: String,
        /// New name
        new_name: String,
    },
}
