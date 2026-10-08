use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::ClipboardItem;

pub fn data_dir() -> PathBuf {
    env::var_os("DBX_PLUGIN_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("dbx-plugin-data").join("io.dbx.clipboard"))
}

pub fn history_path() -> PathBuf {
    data_dir().join("history.json")
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

pub fn fingerprint_text(text: &str) -> String {
    format!("text:{}", hash_bytes(text.as_bytes()))
}

pub fn fingerprint_bytes(bytes: &[u8]) -> String {
    format!("image:{}", hash_bytes(bytes))
}

pub fn fingerprint_paths(paths: &[PathBuf]) -> String {
    let mut normalized: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    normalized.sort();
    format!("files:{}", hash_bytes(normalized.join("\n").as_bytes()))
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

pub fn item_matches(item: &ClipboardItem, needle: &str) -> bool {
    if item.text.to_lowercase().contains(needle) || item.kind.to_lowercase().contains(needle) {
        return true;
    }
    if let Some(paths) = &item.paths {
        return paths.iter().any(|p| p.to_lowercase().contains(needle));
    }
    false
}

pub fn remove_media_file(item: &ClipboardItem) {
    if let Some(rel) = &item.media_file {
        let path = data_dir().join(rel);
        let _ = std::fs::remove_file(path);
    }
}

pub fn log_stderr(message: impl AsRef<str>) {
    let _ = writeln!(io::stderr(), "[io.dbx.clipboard] {}", message.as_ref());
}
