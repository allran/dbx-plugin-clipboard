use std::fs;
use std::path::PathBuf;

use dbx_plugin_sdk::PluginError;
use uuid::Uuid;

use crate::media::build_image_item;
use crate::model::{default_kind, Capture, ClipboardItem, MAX_ITEMS, MAX_TEXT_CHARS};
use crate::util::{
    data_dir, fingerprint_bytes, fingerprint_paths, fingerprint_text, item_matches, log_stderr,
    now_ms, remove_media_file, truncate_chars,
};

pub struct HistoryStore {
    pub items: Vec<ClipboardItem>,
    path: PathBuf,
    pub last_seen: String,
    pub suppress_once: Option<String>,
}

impl HistoryStore {
    pub fn load(path: PathBuf) -> Self {
        let mut items = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Vec<ClipboardItem>>(&raw).ok())
            .unwrap_or_default();
        for item in &mut items {
            if item.fingerprint.is_empty() {
                item.fingerprint = fingerprint_text(&item.text);
            }
            if item.kind.is_empty() {
                item.kind = default_kind();
            }
        }
        let last_seen = items
            .first()
            .map(|item| item.fingerprint.clone())
            .unwrap_or_default();
        Self {
            items,
            path,
            last_seen,
            suppress_once: None,
        }
    }

    fn persist(&self) -> Result<(), PluginError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PluginError::new(-32000, format!("Failed to create data dir: {err}"))
            })?;
        }
        let raw = serde_json::to_string_pretty(&self.items).map_err(|err| {
            PluginError::new(-32000, format!("Failed to serialize history: {err}"))
        })?;
        fs::write(&self.path, raw)
            .map_err(|err| PluginError::new(-32000, format!("Failed to write history: {err}")))?;
        Ok(())
    }

    pub fn list(&self, query: Option<&str>) -> Vec<ClipboardItem> {
        let Some(query) = query.map(str::trim).filter(|q| !q.is_empty()) else {
            return self.items.clone();
        };
        let needle = query.to_lowercase();
        self.items
            .iter()
            .filter(|item| item_matches(item, &needle))
            .cloned()
            .collect()
    }

    pub fn push_capture(&mut self, capture: Capture) -> Option<ClipboardItem> {
        let fingerprint = match &capture {
            Capture::Text(text) => fingerprint_text(text),
            Capture::Image { rgba, .. } => fingerprint_bytes(rgba),
            Capture::Files(paths) => fingerprint_paths(paths),
        };

        if fingerprint.is_empty() || fingerprint == self.last_seen {
            return None;
        }
        if self.suppress_once.as_ref() == Some(&fingerprint) {
            self.suppress_once = None;
            self.last_seen = fingerprint;
            return None;
        }

        let preserved_favorite = self
            .items
            .iter()
            .find(|existing| existing.fingerprint == fingerprint)
            .map(|existing| existing.favorite)
            .unwrap_or(false);

        let id = Uuid::new_v4().to_string();
        let mut item = match capture {
            Capture::Text(text) => {
                let truncated = truncate_chars(&text, MAX_TEXT_CHARS);
                ClipboardItem {
                    id,
                    kind: "text".into(),
                    created_at: now_ms(),
                    char_count: truncated.chars().count(),
                    text: truncated,
                    width: None,
                    height: None,
                    media_file: None,
                    paths: None,
                    preview_data_url: None,
                    favorite: false,
                    fingerprint,
                }
            }
            Capture::Image {
                width,
                height,
                rgba,
            } => match build_image_item(&id, width, height, &rgba, &fingerprint) {
                Ok(item) => item,
                Err(err) => {
                    log_stderr(format!("skip image capture: {err:?}"));
                    return None;
                }
            },
            Capture::Files(paths) => {
                let path_strs: Vec<String> = paths
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                if path_strs.is_empty() {
                    return None;
                }
                let label = path_strs.join("\n");
                ClipboardItem {
                    id,
                    kind: "files".into(),
                    created_at: now_ms(),
                    char_count: path_strs.len(),
                    text: label,
                    width: None,
                    height: None,
                    media_file: None,
                    paths: Some(path_strs),
                    preview_data_url: None,
                    favorite: false,
                    fingerprint,
                }
            }
        };
        item.favorite = preserved_favorite;

        self.items
            .retain(|existing| existing.fingerprint != item.fingerprint);
        self.items.insert(0, item.clone());
        while self.items.len() > MAX_ITEMS {
            // Prefer dropping oldest non-favorite items first.
            if let Some(pos) = self.items.iter().rposition(|existing| !existing.favorite) {
                let removed = self.items.remove(pos);
                remove_media_file(&removed);
            } else if let Some(removed) = self.items.pop() {
                remove_media_file(&removed);
            } else {
                break;
            }
        }
        self.last_seen = item.fingerprint.clone();
        let _ = self.persist();
        Some(item)
    }

    pub fn delete(&mut self, id: &str) -> Result<bool, PluginError> {
        let Some(pos) = self.items.iter().position(|item| item.id == id) else {
            return Ok(false);
        };
        let removed = self.items.remove(pos);
        remove_media_file(&removed);
        self.persist()?;
        Ok(true)
    }

    pub fn clear(&mut self) -> Result<(), PluginError> {
        for item in &self.items {
            remove_media_file(item);
        }
        self.items.clear();
        let _ = fs::remove_dir_all(data_dir().join("blobs"));
        self.persist()
    }

    pub fn get(&self, id: &str) -> Option<&ClipboardItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn bump_to_top(&mut self, id: &str) {
        if let Some(pos) = self.items.iter().position(|item| item.id == id) {
            let mut item = self.items.remove(pos);
            item.created_at = now_ms();
            self.items.insert(0, item);
            let _ = self.persist();
        }
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<Option<bool>, PluginError> {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return Ok(None);
        };
        item.favorite = !item.favorite;
        let favorite = item.favorite;
        self.persist()?;
        Ok(Some(favorite))
    }
}
