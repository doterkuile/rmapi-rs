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
        path: Option<String>,
    },
    /// Start interactive shell
    Shell,
    Upload {
        /// Path to the file to upload
        file_path: PathBuf,
    },
    /// Download a file or directory
    Download {
        /// Path to the file or directory to download
        path: String,
        /// Recursive download (for directories)
        #[arg(short, long)]
        recursive: bool,
    },
}
