use std::path::Path;

use rmapi::RmClient;

use crate::rmclient::error::Error;

pub async fn ls(client: &RmClient, path: &Path) -> Result<(), Error> {
    let entries = client.filesystem.list_dir(Some(path))?;

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

pub async fn rm(client: &RmClient, path: &Path) -> Result<(), Error> {
    let node = client.filesystem.find_node_by_path(path)?;

    client
        .delete_entry(&node.document)
        .await
        .map_err(Error::Rmapi)?;

    println!("Removed {}", path.display());
    Ok(())
}

pub async fn put(
    client: &mut RmClient,
    path: &Path,
    destination: Option<&Path>,
) -> Result<(), Error> {
    if path.extension() != Some("pdf".as_ref()) {
        return Err(Error::Message("Only PDF files are supported".to_string()));
    }

    let parent_id = match destination {
        Some(dest) if !dest.as_os_str().is_empty() => {
            let node = client.filesystem.find_node_by_path(dest)?;
            if !node.is_directory() {
                return Err(Error::Message(format!(
                    "Destination is not a directory: {}",
                    dest.display()
                )));
            }
            Some(node.id().to_string())
        }
        _ => None,
    };

    client
        .put_document(path, parent_id.as_deref())
        .await
        .map_err(Error::Rmapi)?;

    let dest_display = destination.unwrap_or(Path::new("/")).display();
    println!("Upload successful to {}", dest_display);
    Ok(())
}

pub async fn get(client: &RmClient, path: &Path, recursive: bool) -> Result<(), Error> {
    let node = client.filesystem.find_node_by_path(path)?;
    client
        .download_entry(node, std::path::PathBuf::from("."), recursive)
        .map_err(Error::Rmapi)?
        .await
        .map_err(Error::Rmapi)?;
    println!("Download complete");
    Ok(())
}

pub fn cd(client: &RmClient, path: &Path) -> Result<(), Error> {
    let node = client.filesystem.find_node_by_path(path)?;
    if !node.is_directory() {
        return Err(Error::Message(format!(
            "Not a directory: {}",
            path.display()
        )));
    }
    Ok(())
}

pub async fn mv(client: &RmClient, path: &Path, destination: &Path) -> Result<(), Error> {
    let src_node = client.filesystem.find_node_by_path(path)?;
    let src_id = src_node.id().to_string();

    // Check if destination exists
    match client.filesystem.find_node_by_path(destination) {
        Ok(dest_node) => {
            if dest_node.is_directory() {
                // Move into directory
                let dest_id = dest_node.id();
                client
                    .move_entry(&src_id, &dest_id, None)
                    .await
                    .map_err(Error::Rmapi)?;
            } else {
                return Err(Error::Message("Destination already exists".to_string()));
            }
        }
        Err(_) => {
            // Destination does not exist, treat as rename/move-to-new-name
            // Ensure parent exists
            let parent = destination.parent().unwrap_or(Path::new("/"));

            let parent_node = client.filesystem.find_node_by_path(parent)?;
            if !parent_node.is_directory() {
                return Err(Error::Message(
                    "Destination parent is not a directory".to_string(),
                ));
            }

            let new_name = destination
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| Error::Message("Invalid filename".to_string()))?;

            let parent_id = parent_node.id();
            client
                .move_entry(&src_id, &parent_id, Some(new_name))
                .await
                .map_err(Error::Rmapi)?;
        }
    }

    Ok(())
}
