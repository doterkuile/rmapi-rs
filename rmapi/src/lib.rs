pub mod client;
pub mod constants;
pub mod endpoints;
pub mod error;
pub mod filesystem;
pub mod objects;

/// Re-exports the `RmClient` struct from the `client` module.
pub use client::RmClient;
/// Re-exports the `Error` type from the `error` module.
pub use error::Error;
