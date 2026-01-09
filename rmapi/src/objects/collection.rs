use crate::error::Error;
use crate::objects::remarkable_object::RemarkableObject;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct Collection {
    #[serde(rename = "ID")]
    pub id: Uuid,
    #[serde(rename = "Version")]
    pub version: u64,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "VisibleName")]
    pub display_name: String,
    #[serde(rename = "ModifiedClient")]
    pub last_modified: DateTime<Utc>,
    #[serde(rename = "Parent")]
    pub parent: String,
}

impl RemarkableObject for Collection {
    fn register_client(_: String) -> Result<String, Error> {
        unimplemented!()
    }
}
