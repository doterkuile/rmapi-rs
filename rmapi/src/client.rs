use crate::endpoints::{
    fetch_blob, get_files, refresh_token, register_client, update_root, upload_blob,
    STORAGE_API_URL_ROOT,
};
use sha2::{Digest, Sha256};

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
        let filesystem = FileSystem::load_cache().unwrap_or_else(|e| {
            log::error!("Failed to load cache, creating new one. Error: {}", e);
            FileSystem::new()
        });
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
        let remote_hash = root_info["hash"]
            .as_str()
            .ok_or(Error::Message(
                "Missing hash field in root hash response".to_string(),
            ))?
            .to_string();
        if remote_hash == self.filesystem.current_hash {
            log::debug!("Cache unchanged, using local tree");
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

        let doc_schema_bytes = fetch_blob(&self.storage_url, &self.auth_token, &doc.hash).await?;
        let doc_schema_str = String::from_utf8(doc_schema_bytes)
            .map_err(|e| Error::Message(format!("Invalid doc schema: {}", e)))?;

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

        let file = std::fs::File::create(dest)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

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
        use crate::endpoints::upload_blob;

        log::info!("Renaming document {} to {}", doc.display_name, new_name);

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
        let new_metadata_hash = Self::compute_hash(&new_metadata_bytes);

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
            "application/json",
        )
        .await?;

        let old_meta_line = &doc_schema_lines[metadata_line_idx];
        let parts: Vec<&str> = old_meta_line.split(':').collect();
        let new_meta_line = format!(
            "{}:{}:{}:0:{}",
            new_metadata_hash,
            "0", // FileType
            parts[2],
            new_metadata_bytes.len()
        );
        doc_schema_lines[metadata_line_idx] = new_meta_line;

        let new_doc_schema_str = doc_schema_lines.join("\n");
        let new_doc_schema_bytes = new_doc_schema_str.as_bytes();
        let new_doc_schema_hash = Self::compute_hash(new_doc_schema_bytes);

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
            "text/plain",
        )
        .await?;

        let doc_id_str = doc.id.to_string();
        let new_doc_schema_len = new_doc_schema_bytes.len();

        self.modify_root_index(move |root_lines| {
            let mut target_index = 0;
            let mut found = false;

            for (i, line) in root_lines.iter().enumerate() {
                if i == 0 {
                    continue;
                }
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

            let old_root_line = &root_lines[target_index];
            let r_parts: Vec<&str> = old_root_line.split(':').collect();
            // hash:type:id:subfiles:size. We update hash (0) and size (4)
            let new_root_line = format!(
                "{}:{}:{}:{}:{}",
                new_doc_schema_hash, r_parts[1], r_parts[2], r_parts[3], new_doc_schema_len
            );
            root_lines[target_index] = new_root_line;
            Ok(())
        })
        .await?;

        log::info!("Rename successful");
        Ok(())
    }

    pub async fn delete_entry(&self, doc: &Document) -> Result<(), Error> {
        log::info!("Deleting document: {}", doc.display_name);

        self.modify_root_index(move |root_lines| {
            let doc_id_str = doc.id.to_string();
            let mut target_index = Option::<usize>::None;

            for (i, line) in root_lines.iter().enumerate() {
                if i == 0 {
                    continue;
                }
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 && parts[2] == doc_id_str {
                    target_index = Some(i);
                    break;
                }
            }

            if let Some(idx) = target_index {
                root_lines.remove(idx);
                Ok(())
            } else {
                Err(Error::Message(
                    "Document not found in root index".to_string(),
                ))
            }
        })
        .await?;

        log::info!("Deletion successful");
        Ok(())
    }

    pub async fn upload_document(
        &self,
        local_path: &std::path::Path,
        target_dir_path: Option<&str>,
    ) -> Result<(), Error> {
        if !local_path.exists() {
            return Err(Error::Message(format!(
                "File not found: {}",
                local_path.display()
            )));
        }

        let extension = local_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .ok_or_else(|| Error::Message("File has no extension".to_string()))?;

        if extension != "pdf" && extension != "epub" {
            return Err(Error::Message(
                "Only PDF and EPUB files are supported".to_string(),
            ));
        }

        let file_name = local_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let parent_id = if let Some(path) = target_dir_path {
            let node = self
                .filesystem
                .get_node_by_path(path)
                .ok_or_else(|| Error::Message(format!("Target directory not found: {}", path)))?;
            if !node.is_directory() {
                return Err(Error::Message(format!(
                    "Target path is not a directory: {}",
                    path
                )));
            }
            node.document.id.to_string()
        } else {
            String::new() // Root
        };

        let doc_id = uuid::Uuid::new_v4().to_string();
        log::info!("Uploading {} (ID: {})", file_name, doc_id);

        let mut blobs_to_upload: Vec<(String, String, Vec<u8>, &str)> = Vec::new();

        let file_size = tokio::fs::metadata(local_path).await?.len();
        let page_count = 1;
        let mut pages = Vec::new();
        for _ in 0..page_count {
            pages.push(uuid::Uuid::new_v4().to_string());
        }

        let content = crate::objects::internal::Content {
            file_type: extension.clone(),
            size_in_bytes: Some(file_size.to_string()),
            page_count,
            original_page_count: page_count,
            pages,
            document_metadata: Some(serde_json::json!({})),
            ..Default::default()
        };

        let content_bytes = serde_json::to_vec(&content).unwrap();
        log::debug!("Content JSON: {}", String::from_utf8_lossy(&content_bytes));
        let content_hash = Self::compute_hash(&content_bytes);
        blobs_to_upload.push((
            content_hash.clone(),
            format!("{}.content", doc_id),
            content_bytes,
            "application/json",
        ));

        let pagedata_bytes = Vec::new(); // Empty for new files
        let pagedata_hash = Self::compute_hash(&pagedata_bytes);
        blobs_to_upload.push((
            pagedata_hash.clone(),
            format!("{}.pagedata", doc_id),
            pagedata_bytes,
            "text/plain",
        ));

        let file_bytes = tokio::fs::read(local_path).await?;
        let file_hash = Self::compute_hash(&file_bytes);
        let mime_type = if extension == "pdf" {
            "application/pdf"
        } else {
            "application/epub+zip"
        };
        blobs_to_upload.push((
            file_hash.clone(),
            format!("{}.{}", doc_id, extension),
            file_bytes.clone(),
            mime_type,
        ));

        let metadata = crate::endpoints::V4Metadata {
            visible_name: file_name.clone(),
            doc_type: "DocumentType".to_string(),
            parent: parent_id.clone(),
            last_modified: chrono::Utc::now().timestamp_millis().to_string(),
            version: 1,
            pinned: false,
            deleted: false,
        };
        let metadata_bytes = serde_json::to_vec(&metadata).unwrap();
        let metadata_hash = Self::compute_hash(&metadata_bytes);
        blobs_to_upload.push((
            metadata_hash.clone(),
            format!("{}.metadata", doc_id),
            metadata_bytes.clone(), // Clone used for upload
            "application/json",
        ));

        // Format: hash:file_id:filename:size
        let mut doc_schema_lines = Vec::new();
        // Header
        doc_schema_lines.push("3".to_string());

        // .content
        doc_schema_lines.push(format!(
            "{}:{}:{}.content:0:{}",
            content_hash,
            "0", // FileType
            doc_id,
            blobs_to_upload[0].2.len()
        ));
        // .pagedata
        doc_schema_lines.push(format!(
            "{}:{}:{}.pagedata:0:{}",
            pagedata_hash,
            "0", // FileType
            doc_id,
            blobs_to_upload[1].2.len()
        ));
        // .metadata
        doc_schema_lines.push(format!(
            "{}:{}:{}.metadata:0:{}",
            metadata_hash,
            "0", // FileType
            doc_id,
            metadata_bytes.len()
        ));
        // The file itself
        doc_schema_lines.push(format!(
            "{}:{}:{}.{}:0:{}",
            file_hash,
            "0", // FileType
            doc_id,
            extension,
            file_bytes.len()
        ));

        let doc_schema_str = doc_schema_lines.join("\n");
        let doc_schema_bytes = doc_schema_str.as_bytes().to_vec();
        let doc_schema_hash = Self::compute_hash(&doc_schema_bytes);

        blobs_to_upload.push((
            doc_schema_hash.clone(),
            format!("{}.docSchema", doc_id),
            doc_schema_bytes.clone(), // Clone for upload
            "text/plain",
        ));

        for (hash, filename, data, content_type) in blobs_to_upload {
            log::debug!("Uploading blob: {} ({})", filename, hash);
            upload_blob(
                &self.storage_url,
                &self.auth_token,
                &hash,
                &filename,
                data,
                content_type,
            )
            .await?;
        }

        let doc_schema_len = doc_schema_bytes.len();
        self.modify_root_index(move |root_lines| {
            // Add new entry
            // Format: hash:type:id:subfiles:size
            let new_entry = format!(
                "{}:DocumentType:{}:4:{}",
                doc_schema_hash, doc_id, doc_schema_len
            );
            root_lines.push(new_entry);
            Ok(())
        })
        .await?;

        log::info!("Upload successful");
        Ok(())
    }
    fn compute_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    async fn modify_root_index<F>(&self, modifier: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Vec<String>) -> Result<(), Error>,
    {
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

        let root_blob = fetch_blob(&self.storage_url, &self.auth_token, &current_root_hash).await?;
        let root_blob_str = String::from_utf8(root_blob)
            .map_err(|e| Error::Message(format!("Invalid root blob: {}", e)))?;

        let mut root_lines: Vec<String> = root_blob_str.lines().map(|s| s.to_string()).collect();

        modifier(&mut root_lines)?;

        let new_root_blob_str = root_lines.join("\n");
        let new_root_blob_bytes = new_root_blob_str.as_bytes();
        let new_root_hash = Self::compute_hash(new_root_blob_bytes);

        log::info!("Uploading root index: roothash (hash: {})", new_root_hash);
        upload_blob(
            &self.storage_url,
            &self.auth_token,
            &new_root_hash,
            "root.docSchema",
            new_root_blob_bytes.to_vec(),
            "text/plain; charset=UTF-8",
        )
        .await?;

        log::debug!("Updating root with generation: {}", current_generation);
        update_root(
            &self.storage_url,
            &self.auth_token,
            &new_root_hash,
            current_generation,
        )
        .await?;

        Ok(())
    }
}
