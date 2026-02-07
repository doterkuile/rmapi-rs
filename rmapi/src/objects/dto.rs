use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct V4Metadata {
    #[serde(rename = "visibleName", default)]
    pub visible_name: String,
    #[serde(rename = "type", default)]
    pub doc_type: String,
    #[serde(default)]
    pub parent: String,
    #[serde(rename = "createdTime", default)]
    pub created_time: String,
    #[serde(rename = "lastModified", default)]
    pub last_modified: String,
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(rename = "metadataModified", default)]
    pub metadata_modified: bool,
    #[serde(default)]
    pub modified: bool,
    #[serde(default)]
    pub synced: bool,
    #[serde(flatten)]
    pub other: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct V4Content {
    #[serde(rename = "extraMetadata", default)]
    pub extra_metadata: ExtraMetadata,
    #[serde(rename = "fileType", default)]
    pub file_type: String,
    #[serde(rename = "lastOpenedPage", default)]
    pub last_opened_page: u32,
    #[serde(rename = "lineHeight", default = "default_line_height")]
    pub line_height: i32,
    #[serde(default)]
    pub margins: u32,
    #[serde(default)]
    pub orientation: String,
    #[serde(rename = "pageCount", default)]
    pub page_count: u32,
    #[serde(default)]
    pub pages: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(rename = "textScale", default = "default_text_scale")]
    pub text_scale: f32,
    #[serde(default)]
    pub transform: std::collections::HashMap<String, f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtraMetadata {
    #[serde(rename = "LastBrushColor", default)]
    pub last_brush_color: String,
    #[serde(rename = "LastBrushThicknessScale", default)]
    pub last_brush_thickness_scale: String,
    #[serde(rename = "LastColor", default)]
    pub last_color: String,
    #[serde(rename = "LastEraserThicknessScale", default)]
    pub last_eraser_thickness_scale: String,
    #[serde(rename = "LastEraserTool", default)]
    pub last_eraser_tool: String,
    #[serde(rename = "LastPen", default)]
    pub last_pen: String,
    #[serde(rename = "LastPenColor", default)]
    pub last_pen_color: String,
    #[serde(rename = "LastPenThicknessScale", default)]
    pub last_pen_thickness_scale: String,
    #[serde(rename = "LastPencil", default)]
    pub last_pencil: String,
    #[serde(rename = "LastPencilColor", default)]
    pub last_pencil_color: String,
    #[serde(rename = "LastPencilThicknessScale", default)]
    pub last_pencil_thickness_scale: String,
    #[serde(rename = "LastTool", default)]
    pub last_tool: String,
    #[serde(rename = "ThicknessScale", default)]
    pub thickness_scale: String,
    #[serde(rename = "LastFinelinerv2Size", default)]
    pub last_finelinerv2_size: String,
}

impl Default for ExtraMetadata {
    fn default() -> Self {
        Self {
            last_brush_color: "Black".to_string(),
            last_brush_thickness_scale: "2".to_string(),
            last_color: "Black".to_string(),
            last_eraser_thickness_scale: "2".to_string(),
            last_eraser_tool: "Eraser".to_string(),
            last_pen: "Ballpoint".to_string(),
            last_pen_color: "Black".to_string(),
            last_pen_thickness_scale: "2".to_string(),
            last_pencil: "SharpPencil".to_string(),
            last_pencil_color: "Black".to_string(),
            last_pencil_thickness_scale: "2".to_string(),
            last_tool: "SharpPencil".to_string(),
            thickness_scale: "2".to_string(),
            last_finelinerv2_size: "1".to_string(),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct V4Entry {
    pub hash: String,
    pub doc_type: String,
    pub doc_id: String,
    pub subfiles: u32,
    pub size: u64,
}

fn default_line_height() -> i32 {
    -1
}

fn default_text_scale() -> f32 {
    1.0
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientRegistration {
    pub code: String,
    #[serde(rename = "deviceDesc")]
    pub device_desc: String,
    #[serde(rename = "deviceID")]
    pub device_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageInfo {
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "Host")]
    pub host: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RootInfo {
    pub hash: String,
    pub generation: u64,
}
