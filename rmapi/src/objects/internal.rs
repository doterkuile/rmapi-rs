use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    #[serde(default = "default_int_minus_one")]
    pub cover_page_number: i32,
    #[serde(default)]
    pub custom_zoom_center_x: i32,
    #[serde(default = "default_custom_zoom_center_y")]
    pub custom_zoom_center_y: i32,
    #[serde(default = "default_orientation")]
    pub custom_zoom_orientation: String,
    #[serde(default = "default_custom_zoom_page_height")]
    pub custom_zoom_page_height: i32,
    #[serde(default = "default_custom_zoom_page_width")]
    pub custom_zoom_page_width: i32,
    #[serde(default = "default_int_1")]
    pub custom_zoom_scale: i32,
    #[serde(default)]
    pub document_metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub dummy_document: bool,
    #[serde(default = "default_extra_metadata")]
    pub extra_metadata: serde_json::Value,
    #[serde(default)]
    pub file_type: String,
    #[serde(default)]
    pub font_name: String,
    #[serde(default = "default_format_version")]
    pub format_version: i32,
    #[serde(default)]
    pub last_opened_page: i32,
    #[serde(default = "default_int_minus_one")]
    pub line_height: i32,
    #[serde(default = "default_int_100")]
    pub margins: i32,
    #[serde(default = "default_orientation")]
    pub orientation: String,
    #[serde(default)]
    pub original_page_count: i32,
    #[serde(default)]
    pub page_count: i32,
    #[serde(default)]
    pub pages: Vec<String>,
    #[serde(default)]
    pub page_tags: Vec<serde_json::Value>,
    #[serde(default, rename = "tags")]
    pub document_tags: Vec<serde_json::Value>,
    #[serde(default, rename = "redirectionPageMap")]
    pub redirection_map: Vec<i32>,
    #[serde(default)]
    pub size_in_bytes: Option<String>,
    #[serde(default = "default_text_alignment")]
    pub text_alignment: String,
    #[serde(default = "default_text_scale")]
    pub text_scale: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<serde_json::Value>,
    #[serde(default = "default_zoom_mode")]
    pub zoom_mode: String,
}

fn default_extra_metadata() -> serde_json::Value {
    serde_json::json!({
        "LastBrushColor": "Black",
        "LastBrushThicknessScale": "2",
        "LastColor": "Black",
        "LastEraserThicknessScale": "2",
        "LastEraserTool": "Eraser",
        "LastPen": "Ballpoint",
        "LastPenColor": "Black",
        "LastPenThicknessScale": "2",
        "LastPencil": "SharpPencil",
        "LastPencilColor": "Black",
        "LastPencilThicknessScale": "2",
        "LastTool": "SharpPencil",
        "ThicknessScale": "2",
        "LastFinelinerv2Size": "1"
    })
}

fn default_orientation() -> String {
    "portrait".to_string()
}

fn default_text_alignment() -> String {
    "justify".to_string()
}

fn default_zoom_mode() -> String {
    "bestFit".to_string()
}

fn default_int_minus_one() -> i32 {
    -1
}

fn default_int_100() -> i32 {
    100
}

fn default_text_scale() -> f64 {
    1.0
}

fn default_format_version() -> i32 {
    1
}

fn default_custom_zoom_center_y() -> i32 {
    936
}

fn default_custom_zoom_page_height() -> i32 {
    1872
}

fn default_custom_zoom_page_width() -> i32 {
    1404
}

fn default_int_1() -> i32 {
    1
}

impl Default for Content {
    fn default() -> Self {
        Content {
            cover_page_number: -1,
            custom_zoom_center_x: 0,
            custom_zoom_center_y: 936,
            custom_zoom_orientation: default_orientation(),
            custom_zoom_page_height: 1872,
            custom_zoom_page_width: 1404,
            custom_zoom_scale: 1,
            document_metadata: None,
            dummy_document: false,
            extra_metadata: default_extra_metadata(),
            file_type: String::new(),
            font_name: String::new(),
            format_version: 1,
            last_opened_page: 0,
            line_height: -1,
            margins: 100,
            orientation: default_orientation(),
            original_page_count: 0,
            page_count: 0,
            pages: Vec::new(),
            page_tags: Vec::new(),
            document_tags: Vec::new(),
            redirection_map: Vec::new(),
            size_in_bytes: None,
            text_alignment: default_text_alignment(),
            text_scale: 1.0,
            transform: None,
            zoom_mode: default_zoom_mode(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PageData {
    // Page data is usually just a list of lines, but for upload we might just create empty defaults
    // or specific content if needed. For now, empty is fine for new PDFs.
}
