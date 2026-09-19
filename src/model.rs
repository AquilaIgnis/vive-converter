use serde::Serialize;

#[derive(Debug)]
pub struct ConvertedNotebook {
    pub id: String,
    pub name: String,
    pub color_argb: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub sections: Vec<ConvertedSection>,
    pub attachments: Vec<Attachment>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct ConvertedSection {
    pub id: String,
    pub name: String,
    pub color_argb: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub pages: Vec<ConvertedPage>,
}

#[derive(Debug)]
pub struct ConvertedPage {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub doc_json: String,
    pub strokes: Vec<ConvertedStroke>,
}

#[derive(Debug)]
pub struct ConvertedStroke {
    pub id: String,
    pub seq: i32,
    pub brush_family: &'static str,
    pub size_dp: f32,
    pub color_argb: i32,
    pub color_follows_theme: bool,
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub points: Vec<u8>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub id: String,
    pub mime_type: &'static str,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub bytes: Vec<u8>,
    pub ref_count: i32,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: &'static str,
    pub format_version: u32,
    pub app_schema_version: u32,
    pub bundle_id: String,
    pub created_at: i64,
    pub source_notebook_id: String,
    pub notebook_name: String,
    pub database: BundleFile,
    pub attachments: Vec<BundleAttachment>,
    pub counts: BundleCounts,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleFile {
    pub path: &'static str,
    pub byte_count: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleAttachment {
    pub id: String,
    pub path: String,
    pub mime_type: &'static str,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub byte_count: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleCounts {
    pub sections: usize,
    pub pages: usize,
    pub strokes: usize,
    pub revisions: usize,
    pub attachments: usize,
}
