use crate::constants::{
    DOC_TYPE_DOCUMENT, HEADER_RM_FILENAME, MIME_TYPE_DOC_SCHEMA, MIME_TYPE_JSON,
    MIME_TYPE_OCTET_STREAM, MIME_TYPE_PDF, MSG_UNKNOWN_COUNT_0, MSG_UNKNOWN_COUNT_4, ROOT_ID,
    ROOT_SYNC_ENDPOINT, STORAGE_API_URL_ROOT,
};
use crate::endpoints::{
    fetch_blob, get_files, get_root_info, refresh_token, register_client, update_root, upload_blob,
};
use crate::error::Error;
use crate::filesystem::FileSystem;
use crate::objects::{Document, ExtraMetadata, IndexEntry, V4Content, V4Metadata};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::str::FromStr;
use uuid::Uuid;
use zip;

pub struct RmClient {
    pub auth_token: String,
    pub device_token: Option<String>,
    pub storage_url: String,
    pub filesystem: FileSystem,
    pub http_client: reqwest::Client,
}

impl RmClient {
    pub async fn from_token(auth_token: &str, device_token: Option<String>) -> Result<Self, Error> {
        log::debug!("New client with auth token");
        let filesystem = FileSystem::load_cache().unwrap_or_else(|e| {
            log::error!("Failed to load cache, creating new one. Error: {}", e);
            FileSystem::new()
        });
        Ok(RmClient {
            auth_token: auth_token.to_string(),
            device_token,
            storage_url: STORAGE_API_URL_ROOT.to_string(),
            filesystem,
            http_client: reqwest::Client::new(),
        })
    }

    pub async fn new(code: &str) -> Result<Self, Error> {
        log::debug!("Registering client with reMarkable Cloud");
        let http_client = reqwest::Client::new();
        let device_token = register_client(&http_client, code).await?;
        let user_token = refresh_token(&http_client, &device_token).await?;
        RmClient::from_token(&user_token, Some(device_token)).await
    }

    pub async fn refresh_token(&mut self) -> Result<(), Error> {
        log::debug!("Refreshing auth token");
        let token_to_use = self.device_token.as_ref().unwrap_or(&self.auth_token);
        let new_token = refresh_token(&self.http_client, token_to_use).await?;
        self.auth_token = new_token;
        Ok(())
    }

    pub async fn check_authentication(&self) -> Result<(), Error> {
        get_root_info(&self.http_client, &self.storage_url, &self.auth_token).await?;
        Ok(())
    }

    pub async fn list_files(&mut self) -> Result<Vec<Document>, Error> {
        // 1. Get the remote root hash
        let root_info =
            get_root_info(&self.http_client, &self.storage_url, &self.auth_token).await?;
        let remote_hash = root_info.hash;
        if remote_hash == self.filesystem.current_hash {
            log::debug!("Cache unchanged, using local tree");
            return Ok(self.filesystem.get_all_documents());
        }

        let (docs, hash) =
            get_files(&self.http_client, &self.storage_url, &self.auth_token).await?;
        self.filesystem.save_cache(&hash, &docs)?;
        Ok(docs)
    }

    pub async fn delete_entry(&self, doc: &Document) -> Result<(), Error> {
        log::info!("Deleting document: {} ({})", doc.display_name, doc.id);

        self.modify_root_index(|root_entries| {
            let doc_id_str = doc.id.to_string();
            let mut target_index = None;

            if let Some(idx) = root_entries.iter().position(|e| e.id == doc_id_str) {
                root_entries.remove(idx);
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

    pub async fn put_document(&mut self, local_path: &std::path::Path) -> Result<(), Error> {
        let uuid = Uuid::new_v4().to_string();
        let display_name = local_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        log::info!("Uploading document: {} as {}", display_name, uuid);

        let pdf_data = tokio::fs::read(local_path).await?;
        let pdf_hash = self.compute_hash(&pdf_data);
        let pdf_size = pdf_data.len() as u64;

        let timestamp = Utc::now().timestamp_millis().to_string();

        let metadata = V4Metadata {
            visible_name: display_name.to_string(),
            doc_type: DOC_TYPE_DOCUMENT.to_string(),
            parent: "".to_string(),
            created_time: timestamp.clone(),
            last_modified: timestamp.clone(),
            version: 0,
            pinned: false,
            deleted: false,
            metadata_modified: false,
            modified: false,
            synced: true,
        };
        let metadata_json = serde_json::to_vec(&metadata)?;
        let metadata_hash = self.compute_hash(&metadata_json);
        let metadata_size = metadata_json.len() as u64;

        let content = V4Content {
            extra_metadata: ExtraMetadata::default(),
            file_type: "pdf".to_string(),
            last_opened_page: 0,
            line_height: -1,
            margins: 180,
            orientation: "portrait".to_string(),
            page_count: 0,
            pages: vec![],
            tags: vec![],
            text_scale: 1.0,
            transform: crate::objects::DocumentTransform::new().into_map(),
        };
        let content_json = serde_json::to_vec(&content)?;
        let content_hash = self.compute_hash(&content_json);
        let content_size = content_json.len() as u64;

        let pagedata_data = b"Blank\n";
        let pagedata_hash = self.compute_hash(pagedata_data);
        let pagedata_size = pagedata_data.len() as u64;

        // Upload blobs
        self.upload_part(&pdf_hash, &uuid, "pdf", &pdf_data, MIME_TYPE_PDF)
            .await?;
        self.upload_part(
            &metadata_hash,
            &uuid,
            "metadata",
            &metadata_json,
            MIME_TYPE_JSON,
        )
        .await?;
        self.upload_part(
            &content_hash,
            &uuid,
            "content",
            &content_json,
            MIME_TYPE_JSON,
        )
        .await?;
        self.upload_part(
            &pagedata_hash,
            &uuid,
            "pagedata",
            pagedata_data,
            MIME_TYPE_OCTET_STREAM,
        )
        .await?;

        // Create docSchema using IndexEntry
        let mut entries = Vec::new();

        entries.push(IndexEntry::new(
            content_hash.clone(),
            MSG_UNKNOWN_COUNT_0.to_string(),
            format!("{}.content", uuid),
            content_size,
        ));
        entries.push(IndexEntry::new(
            metadata_hash.clone(),
            MSG_UNKNOWN_COUNT_0.to_string(),
            format!("{}.metadata", uuid),
            metadata_size,
        ));
        entries.push(IndexEntry::new(
            pagedata_hash.clone(),
            MSG_UNKNOWN_COUNT_0.to_string(),
            format!("{}.pagedata", uuid),
            pagedata_size,
        ));
        entries.push(IndexEntry::new(
            pdf_hash.clone(),
            MSG_UNKNOWN_COUNT_0.to_string(),
            format!("{}.pdf", uuid),
            pdf_size,
        ));

        // calculate_root_hash sorts internally, so we don't need to manually sort entries for hashing
        // BUT we do need them sorted for the file content to match the hash
        let doc_hash = IndexEntry::calculate_root_hash(&entries)?;

        // Sort entries to write them to file in correct order
        entries.sort_by(|a, b| a.id.cmp(&b.id));

        let mut doc_schema_content = String::from("3\n");
        for entry in &entries {
            doc_schema_content.push_str(&entry.to_string());
            doc_schema_content.push('\n');
        }

        self.upload_part(
            &doc_hash,
            &uuid,
            "docSchema",
            doc_schema_content.as_bytes(),
            MIME_TYPE_DOC_SCHEMA,
        )
        .await?;

        // Update Root
        let total_size = pdf_size + metadata_size + content_size + pagedata_size;
        let mut new_entry = IndexEntry::new(doc_hash, ROOT_ID.to_string(), uuid, total_size);
        new_entry.unknown_count = MSG_UNKNOWN_COUNT_4.to_string();

        self.modify_root_index(move |root_entries| {
            root_entries.push(new_entry);
            Ok(())
        })
        .await?;

        Ok(())
    }

    async fn modify_root_index<F>(&self, modifier: F) -> Result<(), Error>
    where
        F: FnOnce(&mut Vec<IndexEntry>) -> Result<(), Error>,
    {
        // 1. Get root hash
        let root_info =
            get_root_info(&self.http_client, &self.storage_url, &self.auth_token).await?;
        let root_hash = root_info.hash;
        let generation = root_info.generation;

        // 2. Fetch root index blob
        let root_blob = fetch_blob(
            &self.http_client,
            &self.storage_url,
            &self.auth_token,
            &root_hash,
        )
        .await?;
        let root_content = String::from_utf8(root_blob)?;

        let mut root_entries: Vec<IndexEntry> = Vec::new();
        for line in root_content.lines().skip(1) {
            // Skip "3"
            if !line.is_empty() {
                root_entries.push(IndexEntry::from_str(line)?);
            }
        }

        // 3. Apply modifier
        modifier(&mut root_entries)?;

        // 4. Reconstruct root index
        // calculate_root_hash sorts internally
        let new_root_hash = IndexEntry::calculate_root_hash(&root_entries)?;

        // Sort for writing content
        root_entries.sort_by(|a, b| a.id.cmp(&b.id));

        let mut new_root_content = String::from("3\n");
        for entry in root_entries {
            new_root_content.push_str(&entry.to_string());
            new_root_content.push('\n');
        }
        let new_root_bytes = new_root_content.into_bytes();

        // 5. Upload new root index blob
        upload_blob(
            &self.http_client,
            &self.storage_url,
            &self.auth_token,
            &new_root_hash,
            "root.docSchema",
            new_root_bytes.as_slice(),
            MIME_TYPE_DOC_SCHEMA,
        )
        .await?;

        // 6. Update root pointer
        update_root(
            &self.http_client,
            &self.storage_url,
            &self.auth_token,
            &new_root_hash,
            generation,
        )
        .await?;

        Ok(())
    }

    pub fn compute_hash(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    async fn upload_part(
        &self,
        hash: &str,
        uuid: &str,
        ext: &str,
        data: &[u8],
        mime: &str,
    ) -> Result<(), Error> {
        upload_blob(
            &self.http_client,
            &self.storage_url,
            &self.auth_token,
            hash,
            &format!("{}.{}", uuid, ext),
            data,
            mime,
        )
        .await
    }

    pub async fn download_document(
        &self,
        doc_id: &Uuid,
        target_basename: &std::path::Path,
    ) -> Result<std::path::PathBuf, Error> {
        let doc_id_str = doc_id.to_string();
        log::info!("Downloading document: {}", doc_id_str);

        // 1. Get root hash
        let root_info =
            get_root_info(&self.http_client, &self.storage_url, &self.auth_token).await?;
        let root_hash = root_info.hash;

        // 2. Fetch root index blob
        let root_blob = fetch_blob(
            &self.http_client,
            &self.storage_url,
            &self.auth_token,
            &root_hash,
        )
        .await?;
        let root_content = String::from_utf8(root_blob)?;

        // 3. Find entry for doc_id
        let mut entry_hash = None;
        for line in root_content.lines().skip(1) {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 && parts[2] == doc_id_str {
                entry_hash = Some(parts[0].to_string());
                break;
            }
        }

        let entry_hash = entry_hash.ok_or(Error::Message(
            "Document not found in root index".to_string(),
        ))?;

        // 4. Fetch docSchema
        let doc_schema_bytes = fetch_blob(
            &self.http_client,
            &self.storage_url,
            &self.auth_token,
            &entry_hash,
        )
        .await?;
        let doc_schema_content = String::from_utf8(doc_schema_bytes)?;

        // 5. Parse docSchema
        let mut subfiles = Vec::new();
        for line in doc_schema_content.lines().skip(1) {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                subfiles.push((parts[0].to_string(), parts[2].to_string()));
            }
        }

        // 6. Determine download type
        let main_file = subfiles
            .iter()
            .find(|(_, name)| name.ends_with(".pdf") || name.ends_with(".epub"));

        if let Some((hash, name)) = main_file {
            let ext = std::path::Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("pdf");
            let output_path = target_basename.with_extension(ext);
            log::info!("Downloading single file to {:?}", output_path);

            let data =
                fetch_blob(&self.http_client, &self.storage_url, &self.auth_token, hash).await?;
            tokio::fs::write(&output_path, data).await?;
            Ok(output_path)
        } else {
            let output_path = target_basename.with_extension("rmdoc");
            log::info!("Creating rmdoc at {:?}", output_path);

            // Fetch all blobs
            let mut blob_data = Vec::new();
            for (hash, name) in &subfiles {
                let data = fetch_blob(&self.http_client, &self.storage_url, &self.auth_token, hash)
                    .await?;
                blob_data.push((name.clone(), data));
            }

            // Write ZIP (blocking)
            let path_clone = output_path.clone();
            tokio::task::spawn_blocking(move || -> Result<(), Error> {
                let file = std::fs::File::create(&path_clone)?;
                let mut zip = zip::ZipWriter::new(file);
                let options = zip::write::FileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored);

                for (name, data) in blob_data {
                    zip.start_file(name, options)
                        .map_err(|e| Error::Message(e.to_string()))?;
                    zip.write_all(&data)?;
                }
                zip.finish().map_err(|e| Error::Message(e.to_string()))?;
                Ok(())
            })
            .await
            .map_err(|e| Error::Message(e.to_string()))??;

            Ok(output_path)
        }
    }

    pub fn download_entry<'a>(
        &'a self,
        node: &'a crate::objects::Node,
        target_path: std::path::PathBuf,
        recursive: bool,
    ) -> Result<
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>,
        Error,
    > {
        if node.is_directory() && !recursive {
            return Err(Error::Message(format!(
                "{} is a directory. Use -r to download recursively.",
                node.name()
            )));
        }

        Ok(Box::pin(async move {
            if node.is_directory() {
                let dir_name = node.name();
                let new_dir = target_path.join(dir_name);
                tokio::fs::create_dir_all(&new_dir).await?;
                log::info!("Created directory {:?}", new_dir);

                for child in node.children.values() {
                    let fut = self.download_entry(child, new_dir.clone(), true)?;
                    fut.await?;
                }
            } else {
                let target_base = target_path.join(node.name());
                self.download_document(&node.document.id, &target_base)
                    .await?;
                log::info!("Downloaded {}", node.name());
            }
            Ok(())
        }))
    }
}
