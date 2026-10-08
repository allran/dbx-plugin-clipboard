use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const MAX_ITEMS: usize = 500;
pub const MAX_TEXT_CHARS: usize = 100_000;
pub const MAX_IMAGE_EDGE: u32 = 4096;
pub const THUMB_EDGE: u32 = 160;
pub const LIGHTBOX_EDGE: u32 = 1920;
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub created_at: u64,
    /// Searchable summary. For text items this is the full (truncated) content.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub char_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Relative path under data dir, e.g. `blobs/<id>.png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_data_url: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub fingerprint: String,
}

pub fn default_kind() -> String {
    "text".to_string()
}

pub enum Capture {
    Text(String),
    Image {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Files(Vec<PathBuf>),
}
