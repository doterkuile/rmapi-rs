use crate::error::Error;
use crate::objects::{Document, FileTree, Node};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
struct CacheData {
    hash: String,
    documents: Vec<Document>,
}

pub struct FileSystem {
    pub tree: FileTree,
    pub current_hash: String,
    pub docs: Vec<Document>,
}

impl FileSystem {
    pub fn new() -> Self {
        FileSystem {
            tree: FileTree::new(),
            current_hash: String::new(),
            docs: Vec::new(),
        }
    }

    pub fn load_cache() -> Result<Self, Error> {
        let cache_path = Self::cache_path()?;
        if cache_path.exists() {
            let data = fs::read_to_string(cache_path)?;
            let cache: CacheData = serde_json::from_str(&data)?;
            Ok(FileSystem {
                tree: FileTree::build(cache.documents.clone()),
                current_hash: cache.hash,
                docs: cache.documents,
            })
        } else {
            Ok(FileSystem::new())
        }
    }

    pub fn save_cache(&mut self, hash: &str, documents: &[Document]) -> Result<(), Error> {
        let cache_path = Self::cache_path()?;
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        self.docs = documents.to_vec();
        self.current_hash = hash.to_string();
        self.tree = FileTree::build(self.docs.clone());

        let cache = CacheData {
            hash: self.current_hash.clone(),
            documents: self.docs.clone(),
        };
        let data = serde_json::to_string(&cache)?;
        fs::write(cache_path, data)?;
        Ok(())
    }

    pub fn get_all_documents(&self) -> Vec<Document> {
        self.docs.clone()
    }

    fn cache_path() -> Result<PathBuf, Error> {
        Ok(dirs::cache_dir()
            .ok_or_else(|| Error::Message("Could not find cache directory".to_string()))?
            .join("rmapi/tree.cache"))
    }

    pub fn list_dir(&self, path: &str) -> Result<Vec<&Node>, Error> {
        let node = self.find_node_by_path(path)?;
        Ok(node.children.values().collect())
    }

    pub fn find_node_by_path(&self, path: &str) -> Result<&Node, Error> {
        if path == "/" || path.is_empty() {
            return Ok(&self.tree.root);
        }

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = &self.tree.root;

        for part in parts {
            let mut found = None;
            for child in current.children.values() {
                if child.name() == part {
                    found = Some(child);
                    break;
                }
            }

            if let Some(next) = found {
                current = next;
            } else {
                return Err(Error::Message(format!("Path not found: {}", path)));
            }
        }

        Ok(current)
    }
}
