# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Repository overview
This is a Rust workspace with two crates:
- `rmapi/`: library crate implementing a client for the reMarkable Cloud APIs.
- `rmclient/`: CLI crate that depends on `rmapi` and exposes common operations (register, list, interactive shell, remove).

## Common development commands
All commands below are run from the repository root.

### Build / check
- Build all crates:
  - `cargo build`
- Fast compile check (no codegen):
  - `cargo check`
- Build only the CLI:
  - `cargo build -p rmclient`
- Build only the library:
  - `cargo build -p rmapi`

### Run the CLI
- Show CLI help:
  - `cargo run -p rmclient -- --help`

- Register a device (creates a token file):
  - `cargo run -p rmclient -- register <code>`
  - The `<code>` is the registration code from `https://my.remarkable.com/device/desktop/connect` (see `rmclient/src/rmclient/commands.rs`).

- Use a specific token file path:
  - `cargo run -p rmclient -- --auth-token-file /path/to/token register <code>`
  - Or set `RMAPI_AUTH_TOKEN_FILE=/path/to/token`.

- List files (root):
  - `cargo run -p rmclient -- ls`

- List files in a specific path:
  - `cargo run -p rmclient -- ls /Some/Folder`

- Start interactive shell:
  - `cargo run -p rmclient -- shell`

### Tests
- Run all tests in the workspace:
  - `cargo test`

- Run tests for a single crate:
  - `cargo test -p rmapi`
  - `cargo test -p rmclient`

- Run a single test by name substring (Rust’s built-in test filter):
  - `cargo test -p rmapi <test_name_substring>`

### Lint / format
- Format (rustfmt):
  - `cargo fmt`

- Lint (clippy):
  - `cargo clippy --all-targets --all-features`

## High-level architecture
### Workspace structure
- Root `Cargo.toml` defines a workspace with members `rmapi` and `rmclient`.

### `rmapi` (library)
Primary entrypoint is `rmapi::Client` (re-exported from `rmapi/src/lib.rs`). The client wraps:
- authentication/session data (`auth_token`, `device_token`)
- a `FileSystem` cache that provides a tree-like view of documents/collections

Key modules:
- `rmapi/src/client.rs`
  - High-level operations used by the CLI.
  - `Client::new(code)` registers a device and obtains a user token.
  - `Client::from_token(...)` builds a client from existing tokens and loads the local cache.
  - `Client::list_files()` implements a sync strategy:
    - fetches the remote “root hash”
    - compares it to the cached `FileSystem.current_hash`
    - if unchanged, returns cached documents; otherwise fetches + rebuilds cache.
  - `Client::delete_entry(doc)` removes an entry by editing the “root index” blob and updating the root pointer.

- `rmapi/src/endpoints.rs`
  - Low-level HTTP calls and API constants/paths.
  - Implements the “Sync V4” listing flow by reading `sync/v4/root`, then fetching/decoding the root index blob and entry `.metadata` files.
  - Blob mutation helpers:
    - `fetch_blob` / `upload_blob` for `sync/v3/files/<hash>`
    - `update_root` for updating the root pointer.

- `rmapi/src/filesystem.rs` + `rmapi/src/objects/*`
  - Defines `Document` and `DocumentType` (`Document` vs `Collection`).
  - Builds an in-memory tree (`FileTree` / `Node`) based on parent IDs.
  - Maintains a JSON cache on disk and a `current_path` for shell-like navigation.

Notes that matter when editing:
- The cache file is stored under the OS cache directory as `rmapi/tree.cache` (see `FileSystem::cache_path`).
- The tree includes a special virtual `trash` node to model reMarkable’s deleted-items parent ID.

### `rmclient` (CLI)
- Entry point: `rmclient/src/main.rs` (Tokio async main + Clap parsing).
- Subcommands are defined in `rmclient/src/rmclient/commands.rs`.

Token handling:
- Default token file path is under the OS config directory: `rmapi/auth_token` (see `rmclient/src/rmclient/token.rs`).
- The token file may be:
  - JSON containing `{ device_token, user_token }` (current format)
  - plain text (legacy format); in this case it is treated as a user token only.

Interactive shell:
- Implemented in `rmclient/src/rmclient/shell.rs` using `rustyline`.
- Shell commands are parsed with Clap from a tokenized command line (via `shlex`).
- The shell maintains its own `current_path` string and relies on `rmapi::filesystem::FileSystem` for `ls`, `cd`, and `rm`.

Feature status:
- Upload exists as a CLI subcommand but is currently not implemented for Sync V4 (see `rmclient/src/main.rs`).
