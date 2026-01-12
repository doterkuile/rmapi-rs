use crate::endpoints::{
    fetch_blob, get_files, refresh_token, register_client, STORAGE_API_URL_ROOT,
};
use crate::error::Error;
use crate::filesystem::FileSystem;
use crate::objects::Document;

pub struct Client {
    pub auth_token: String,
    pub device_token: Option<String>,
    pub storage_url: String,
    pub filesystem: FileSystem,
}

impl Client {
    pub async fn from_token(auth_token: &str, device_token: Option<String>) -> Result<Self, Error> {
        log::debug!("New client with auth token");
        let filesystem = FileSystem::load_cache().unwrap_or_else(|_| FileSystem::new());
        Ok(Client {
            auth_token: auth_token.to_string(),
            device_token,
            storage_url: STORAGE_API_URL_ROOT.to_string(),
            filesystem,
        })
    }

    pub async fn new(code: &str) -> Result<Self, Error> {
        log::debug!("Registering client with reMarkable Cloud");
        let device_token = register_client(code).await?;
        let user_token = refresh_token(&device_token).await?;
        Client::from_token(&user_token, Some(device_token)).await
    }

    pub async fn refresh_token(&mut self) -> Result<(), Error> {
        log::debug!("Refreshing auth token");
        let token_to_use = self.device_token.as_ref().unwrap_or(&self.auth_token);
        let new_token = refresh_token(token_to_use).await?;
        self.auth_token = new_token;
        Ok(())
    }

    pub async fn list_files(&mut self) -> Result<Vec<Document>, Error> {
        // 1. Get the remote root hash
        let client = reqwest::Client::new();
        let root_hash_response = client
            .get(format!(
                "{}/{}",
                STORAGE_API_URL_ROOT,
                crate::endpoints::ROOT_SYNC_ENDPOINT
            ))
            .bearer_auth(&self.auth_token)
            .header("Accept", "application/json")
            .header("rm-filename", "roothash")
            .send()
            .await?
            .error_for_status()?;

        let root_resp_text = root_hash_response.text().await?;
        let root_info: serde_json::Value = serde_json::from_str(&root_resp_text)?;
        let remote_hash = root_info["hash"].as_str().unwrap_or_default().to_string();

        if remote_hash == self.filesystem.current_hash {
            log::debug!("Cache hit, using local tree");
            return Ok(self.filesystem.get_all_documents());
        }

        let (docs, hash) = get_files(&self.storage_url, &self.auth_token).await?;
        self.filesystem.save_cache(&hash, &docs)?;
        Ok(docs)
    }

    pub async fn download_document(
        &self,
        doc: &Document,
        dest: &std::path::Path,
    ) -> Result<(), Error> {
        log::info!("Downloading document: {}", doc.display_name);

        // 1. Fetch .docSchema to get hashes for all subfiles
        let doc_schema_bytes = fetch_blob(&self.storage_url, &self.auth_token, &doc.hash).await?;
        let doc_schema_str = String::from_utf8(doc_schema_bytes)
            .map_err(|e| Error::Message(format!("Invalid doc schema: {}", e)))?;

        // 2. Parse schema to find subfiles
        // Schema format: <hash>:<file_id>:<filename>:<size>
        let mut subfiles = Vec::new();
        for line in doc_schema_str.lines().skip(1) {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                let hash = parts[0].to_string();
                let file_id = parts[1].to_string();
                let filename = parts[2].to_string();
                subfiles.push((hash, file_id, filename));
            }
        }

        // 3. Check if there is a PDF file in subfiles
        let pdf_file = subfiles
            .iter()
            .find(|(_, _, filename)| filename.ends_with(".pdf"));

        if let Some((hash, _, _)) = pdf_file {
            // It's a PDF, download directly
            log::info!("Found PDF file, downloading directly");
            let content = fetch_blob(&self.storage_url, &self.auth_token, hash).await?;

            // Update destination to end with .pdf instead of .rmdoc if it does
            let dest_path_buf = if dest.to_string_lossy().ends_with(".rmdoc") {
                let stem = dest.file_stem().unwrap().to_string_lossy();
                if stem.ends_with(".pdf") {
                    dest.with_file_name(stem.to_string())
                } else {
                    let new_name = stem.to_string() + ".pdf";
                    dest.with_file_name(new_name)
                }
            } else {
                dest.to_path_buf()
            };

            std::fs::write(&dest_path_buf, content)?;
            log::info!("Saved PDF to: {}", dest_path_buf.display());
            return Ok(());
        }

        // 4. Create zip file (if not PDF)
        let file = std::fs::File::create(dest)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 5. Download and add each subfile to zip
        let mut seen_files = std::collections::HashSet::new();
        for (hash, file_id, _) in subfiles {
            if seen_files.contains(&file_id) {
                continue;
            }
            seen_files.insert(file_id.clone());

            log::debug!("Fetching subfile: {} ({})", file_id, hash);
            let content = fetch_blob(&self.storage_url, &self.auth_token, &hash).await?;
            zip.start_file(file_id.clone(), options)
                .map_err(|e| Error::Message(e.to_string()))?;
            use std::io::Write;
            zip.write_all(&content)
                .map_err(|e| Error::Message(e.to_string()))?;
        }

        zip.finish().map_err(|e| Error::Message(e.to_string()))?;
        Ok(())
    }

    #[async_recursion::async_recursion]
    pub async fn download_tree(
        &self,
        node: &crate::objects::Node,
        local_dest: &std::path::Path,
        recursive: bool,
    ) -> Result<(), Error> {
        let safe_name = node.name().replace("/", "_");

        if !node.is_directory() {
            let file_name = format!("{}.rmdoc", safe_name);
            let dest_path = local_dest.join(&file_name);
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            self.download_document(&node.document, &dest_path).await?;
            // Note: println! might not be desired in library code, but okay for now or use log
            log::info!("Downloaded {}", dest_path.display());
            return Ok(());
        }

        if !recursive {
            return Err(Error::Message(format!(
                "{} is a directory. Use -r to download recursively.",
                node.name()
            )));
        }

        let new_dest = local_dest.join(&safe_name);
        tokio::fs::create_dir_all(&new_dest).await?;
        log::info!("Created directory {}", new_dest.display());

        for child in node.children.values() {
            self.download_tree(child, &new_dest, true).await?;
        }
        Ok(())
    }

    pub async fn rename_entry(&self, doc: &Document, new_name: &str) -> Result<(), Error> {
        use crate::endpoints::{update_root, upload_blob, STORAGE_API_URL_ROOT};
        use sha2::{Digest, Sha256};

        log::info!("Renaming document {} to {}", doc.display_name, new_name);

        // 1. Get current root hash/generation
        let client = reqwest::Client::new();
        let root_hash_response = client
            .get(format!(
                "{}/{}",
                STORAGE_API_URL_ROOT,
                crate::endpoints::ROOT_SYNC_ENDPOINT
            ))
            .bearer_auth(&self.auth_token)
            .header("Accept", "application/json")
            .header("rm-filename", "roothash")
            .send()
            .await?
            .error_for_status()?;

        let root_resp_text = root_hash_response.text().await?;
        let root_info: serde_json::Value = serde_json::from_str(&root_resp_text)?;
        let current_root_hash = root_info["hash"].as_str().unwrap_or_default().to_string();
        let current_generation = root_info["generation"].as_u64().unwrap_or(0);

        // 2. Fetch Root Index Blob
        let root_blob = fetch_blob(&self.storage_url, &self.auth_token, &current_root_hash).await?;
        let root_blob_str = String::from_utf8(root_blob)
            .map_err(|e| Error::Message(format!("Invalid root blob: {}", e)))?;

        // 3. Find current entry and update it
        let mut root_lines: Vec<String> = root_blob_str.lines().map(|s| s.to_string()).collect();
        let doc_id_str = doc.id.to_string();

        let mut target_index = 0;
        let mut found = false;

        for (i, line) in root_lines.iter().enumerate() {
            if i == 0 {
                continue;
            } // Skip schema version
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 && parts[2] == doc_id_str {
                target_index = i;
                found = true;
                break;
            }
        }

        if !found {
            return Err(Error::Message(
                "Document not found in root index".to_string(),
            ));
        }

        // 4. Fetch .docSchema to find metadata hash
        let doc_schema_bytes = fetch_blob(&self.storage_url, &self.auth_token, &doc.hash).await?;
        let doc_schema_str = String::from_utf8(doc_schema_bytes)
            .map_err(|e| Error::Message(format!("Invalid doc schema: {}", e)))?;

        let mut metadata_hash = String::new();
        let mut metadata_line_idx = 0;
        let mut doc_schema_lines: Vec<String> =
            doc_schema_str.lines().map(|s| s.to_string()).collect();

        for (i, line) in doc_schema_lines.iter().enumerate() {
            if line.contains(".metadata") {
                let parts: Vec<&str> = line.split(':').collect();
                metadata_hash = parts[0].to_string();
                metadata_line_idx = i;
                break;
            }
        }

        if metadata_hash.is_empty() {
            return Err(Error::Message(
                "Metadata not found in doc schema".to_string(),
            ));
        }

        // 5. Fetch and Update Metadata
        let metadata_bytes =
            fetch_blob(&self.storage_url, &self.auth_token, &metadata_hash).await?;
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&metadata_bytes).map_err(|e| Error::Message(e.to_string()))?;

        log::info!("Original metadata: {}", metadata);

        metadata["visibleName"] = serde_json::json!(new_name);

        if let Some(v) = metadata["version"].as_u64() {
            metadata["version"] = serde_json::json!(v + 1);
        }

        metadata["lastModified"] =
            serde_json::json!(chrono::Utc::now().timestamp_millis().to_string());
        metadata["metadatamodified"] = serde_json::json!(true);

        log::info!("New metadata: {}", metadata);

        let new_metadata_bytes =
            serde_json::to_vec(&metadata).map_err(|e| Error::Message(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(&new_metadata_bytes);
        let new_metadata_hash = hex::encode(hasher.finalize());

        // Upload Metadata
        log::info!(
            "Uploading metadata: {} (hash: {})",
            format!("{}.metadata", doc.id),
            new_metadata_hash
        );
        upload_blob(
            &self.storage_url,
            &self.auth_token,
            &new_metadata_hash,
            &format!("{}.metadata", doc.id),
            new_metadata_bytes.clone(),
        )
        .await?;

        // 6. Update .docSchema
        let old_meta_line = &doc_schema_lines[metadata_line_idx];
        let parts: Vec<&str> = old_meta_line.split(':').collect();
        let new_meta_line = format!(
            "{}:{}:{}:{}",
            new_metadata_hash,
            parts[1],
            parts[2],
            new_metadata_bytes.len()
        );
        doc_schema_lines[metadata_line_idx] = new_meta_line;

        let new_doc_schema_str = doc_schema_lines.join("\n");
        let new_doc_schema_bytes = new_doc_schema_str.as_bytes();

        let mut hasher = Sha256::new();
        hasher.update(new_doc_schema_bytes);
        let new_doc_schema_hash = hex::encode(hasher.finalize());

        log::info!(
            "Uploading doc schema: {} (hash: {})",
            format!("{}.docSchema", doc.id),
            new_doc_schema_hash
        );
        upload_blob(
            &self.storage_url,
            &self.auth_token,
            &new_doc_schema_hash,
            &format!("{}.docSchema", doc.id),
            new_doc_schema_bytes.to_vec(),
        )
        .await?;

        // 7. Update Root Index
        let old_root_line = &root_lines[target_index];
        let r_parts: Vec<&str> = old_root_line.split(':').collect();
        // hash:type:id:subfiles:size. We update hash (0) and size (4)
        let new_root_line = format!(
            "{}:{}:{}:{}:{}",
            new_doc_schema_hash,
            r_parts[1],
            r_parts[2],
            r_parts[3],
            new_doc_schema_bytes.len()
        );
        root_lines[target_index] = new_root_line;

        let new_root_blob_str = root_lines.join("\n");
        let new_root_blob_bytes = new_root_blob_str.as_bytes();

        let mut hasher = Sha256::new();
        hasher.update(new_root_blob_bytes);
        let new_root_hash = hex::encode(hasher.finalize());

        log::info!("Uploading root index: roothash (hash: {})", new_root_hash);
        upload_blob(
            &self.storage_url,
            &self.auth_token,
            &new_root_hash,
            "roothash",
            new_root_blob_bytes.to_vec(),
        )
        .await?;

        // 8. Update Root
        update_root(
            &self.storage_url,
            &self.auth_token,
            &new_root_hash,
            current_generation + 1,
        )
        .await?;

        log::info!("Rename successful");
        Ok(())
    }
}
