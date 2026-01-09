use crate::error::Error;
use crate::objects::remarkable_object::RemarkableObject;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

impl RemarkableObject for Document {
    fn register_client(_: String) -> Result<String, Error> {
        unimplemented!()
    }
}
