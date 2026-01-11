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
}
