use crate::error::Error;
use crate::objects::{Document, FileTree, Node};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Serialize, Deserialize)]
struct CacheData {
    hash: String,
    documents: Vec<Document>,
}

pub struct FileSystem {
    pub tree: FileTree,
    pub current_hash: String,
    pub docs: Vec<Document>,
    pub current_path: PathBuf,
}

impl FileSystem {
    pub fn new() -> Self {
        FileSystem {
            tree: FileTree::new(),
            current_hash: String::new(),
            docs: Vec::new(),
            current_path: PathBuf::from("/"),
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
                current_path: PathBuf::from("/"),
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

    pub fn list_dir(&self, path: Option<&Path>) -> Result<Vec<&Node>, Error> {
        let target = path.unwrap_or(&self.current_path);
        let node = self.find_node_by_path(target)?;
        let mut entries: Vec<&Node> = node.children.values().collect();

        // Sort entries: directories first, then files, both alphabetically
        entries.sort_by(|a, b| match (a.is_directory(), b.is_directory()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name().to_lowercase().cmp(&b.name().to_lowercase()),
        });

        Ok(entries)
    }

    pub fn cd(&mut self, path: &Path) -> Result<(), Error> {
        let normalized = normalize_path(path, &self.current_path);

        let node = self.find_node_by_path(&normalized)?;
        if node.is_directory() {
            self.current_path = normalized;
            Ok(())
        } else {
            Err(Error::Message(format!(
                "Not a directory: {}",
                path.display()
            )))
        }
    }

    pub fn pwd(&self) -> &Path {
        &self.current_path
    }

    pub fn find_node_by_path(&self, path: &Path) -> Result<&Node, Error> {
        let normalized_path = normalize_path(path, Path::new("/"));

        // Check if root
        if normalized_path
            .components()
            .all(|c| !matches!(c, Component::Normal(_)))
        {
            return Ok(&self.tree.root);
        }

        let mut current = &self.tree.root;

        for part in normalized_path.components().filter_map(|c| match c {
            Component::Normal(p) => Some(p),
            _ => None,
        }) {
            let part_str = part.to_string_lossy();

            let found = current
                .children
                .values()
                .find(|node| node.name() == part_str);

            if let Some(next) = found {
                current = next;
            } else {
                return Err(Error::Message(format!(
                    "Path not found: {}",
                    path.display()
                )));
            }
        }

        Ok(current)
    }
}

pub fn normalize_path(path: &Path, cwd: &Path) -> PathBuf {
    let mut components: Vec<Component> = Vec::new();

    if path.is_relative() {
        for comp in cwd.components() {
            components.push(comp);
        }
    }

    // Iterate through path components safely
    for comp in path.components() {
        match comp {
            Component::CurDir => {} // This is "." - do nothing
            Component::ParentDir => {
                // This is ".." - pop the last part if possible
                // We check to avoid popping the RootDir or Prefix
                if let Some(last) = components.last() {
                    if matches!(last, Component::Normal(_)) {
                        components.pop();
                    }
                }
            }
            _ => components.push(comp),
        }
    }

    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(
            normalize_path(Path::new("/foo/bar"), Path::new("/")),
            PathBuf::from("/foo/bar")
        );
        assert_eq!(
            normalize_path(Path::new("bar/baz"), Path::new("/foo")),
            PathBuf::from("/foo/bar/baz")
        );
        assert_eq!(
            normalize_path(Path::new("../baz"), Path::new("/foo/bar")),
            PathBuf::from("/foo/baz")
        );
        assert_eq!(
            normalize_path(Path::new("./baz"), Path::new("/foo")),
            PathBuf::from("/foo/baz")
        );
        assert_eq!(
            normalize_path(Path::new("../../.."), Path::new("/foo/bar")),
            PathBuf::from("/")
        );
        assert_eq!(
            normalize_path(Path::new("/"), Path::new("/any")),
            PathBuf::from("/")
        );
        assert_eq!(
            normalize_path(Path::new(""), Path::new("/foo")),
            PathBuf::from("/foo")
        );
        assert_eq!(
            normalize_path(Path::new(".."), Path::new("/")),
            PathBuf::from("/")
        );
        assert_eq!(
            normalize_path(Path::new("/foo/../bar"), Path::new("/")),
            PathBuf::from("/bar")
        );
        assert_eq!(
            normalize_path(Path::new("foo//bar/"), Path::new("/")),
            PathBuf::from("/foo/bar")
        );
    }
}
