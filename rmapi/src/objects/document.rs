use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum DocumentType {
    #[default]
    Document,
    Collection,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Document {
    #[serde(rename = "ID")]
    pub id: Uuid,
    #[serde(rename = "Version")]
    pub version: u64,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "Success")]
    pub success: bool,
    #[serde(rename = "BlobURLGet")]
    pub blob_url_get: String,
    #[serde(rename = "BlobURLPut")]
    pub blob_url_put: String,
    #[serde(rename = "BlobURLPutExpires")]
    pub blob_url_put_expires: DateTime<Utc>,
    #[serde(rename = "ModifiedClient")]
    pub last_modified: DateTime<Utc>,
    #[serde(rename = "Type")]
    pub doc_type: DocumentType,
    #[serde(rename = "VisibleName")]
    pub display_name: String,
    #[serde(rename = "CurrentPage")]
    pub current_page: u64,
    #[serde(rename = "Bookmarked")]
    pub bookmarked: bool,
    #[serde(rename = "Parent")]
    pub parent: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DocumentTransform {
    #[serde(flatten)]
    pub map: HashMap<String, f32>,
}

impl DocumentTransform {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert("m11".to_string(), 1.0);
        map.insert("m12".to_string(), 0.0);
        map.insert("m13".to_string(), 0.0);
        map.insert("m21".to_string(), 0.0);
        map.insert("m22".to_string(), 1.0);
        map.insert("m23".to_string(), 0.0);
        map.insert("m31".to_string(), 0.0);
        map.insert("m32".to_string(), 0.0);
        map.insert("m33".to_string(), 1.0);
        Self { map }
    }

    pub fn into_map(self) -> HashMap<String, f32> {
        self.map
    }
}
