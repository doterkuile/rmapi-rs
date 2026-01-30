use crate::error::Error;
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub hash: String,
    pub type_id: String,
    pub id: String,
    pub unknown_count: String, // Usually "0" or "4" or "." for schema
    pub size: u64,
}

impl IndexEntry {
    pub fn new(hash: String, type_id: String, id: String, size: u64) -> Self {
        IndexEntry {
            hash,
            type_id,
            id,
            unknown_count: "0".to_string(), // Default usually 0
            size,
        }
    }

    /// Calculates the root hash for a list of entries using the API's sorted-hash-of-hashes method.
    pub fn calculate_root_hash(entries: &[IndexEntry]) -> Result<String, Error> {
        // Clone to sort without mutating original
        let mut sorted_entries = entries.to_vec();

        // Sort by ID is CRITICAL for the Merkle tree to be deterministic
        sorted_entries.sort_by(|a, b| a.id.cmp(&b.id));

        let mut hasher = Sha256::new();
        for entry in sorted_entries {
            let bytes = hex::decode(&entry.hash).map_err(|e| {
                Error::Message(format!("Invalid hex hash in entry {}: {}", entry.id, e))
            })?;
            hasher.update(bytes);
        }
        Ok(hex::encode(hasher.finalize()))
    }
}

impl fmt::Display for IndexEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.hash, self.type_id, self.id, self.unknown_count, self.size
        )
    }
}

impl FromStr for IndexEntry {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() < 5 {
            return Err(Error::Message(format!("Invalid index line format: {}", s)));
        }

        let hash = parts[0].to_string();
        let type_id = parts[1].to_string();
        let id = parts[2].to_string();
        let unknown_count = parts[3].to_string();
        let size = parts[4]
            .parse::<u64>()
            .map_err(|_| Error::Message(format!("Invalid size in index line: {}", s)))?;

        Ok(IndexEntry {
            hash,
            type_id,
            id,
            unknown_count,
            size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_display() {
        let line = "aabbcc:1:uuid-1234:0:1024";
        let entry = IndexEntry::from_str(line).unwrap();

        assert_eq!(entry.hash, "aabbcc");
        assert_eq!(entry.type_id, "1");
        assert_eq!(entry.id, "uuid-1234");
        assert_eq!(entry.unknown_count, "0");
        assert_eq!(entry.size, 1024);

        assert_eq!(entry.to_string(), line);
    }

    #[test]
    fn test_calculate_root_hash_sorting() {
        // Create two entries out of order
        let entry1 = IndexEntry::new(
            "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            "1".to_string(),
            "b-uuid".to_string(),
            100,
        );
        let entry2 = IndexEntry::new(
            "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            "1".to_string(),
            "a-uuid".to_string(),
            100,
        );

        let list_unordered = vec![entry1.clone(), entry2.clone()];
        let list_ordered = vec![entry2.clone(), entry1.clone()];

        // The hash should be the same regardless of input order because it sorts by ID internally
        let hash1 = IndexEntry::calculate_root_hash(&list_unordered).unwrap();
        let hash2 = IndexEntry::calculate_root_hash(&list_ordered).unwrap();

        assert_eq!(hash1, hash2);
    }
}
