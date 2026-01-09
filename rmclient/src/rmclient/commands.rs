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
    Ls,
    /// Upload a file to the reMarkable Cloud
    Upload {
        /// Path to the file to upload
        file_path: PathBuf,
    },
}
