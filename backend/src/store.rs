use std::fs;
use std::path::PathBuf;

use dbx_plugin_sdk::PluginError;
use uuid::Uuid;

use crate::media::{build_image_item, decode_png_file};
use crate::model::{default_kind, Capture, ClipboardItem, MAX_ITEMS, MAX_TEXT_CHARS};
use crate::util::{
    data_dir, fingerprint_bytes, fingerprint_capture, fingerprint_paths, fingerprint_text,
    item_matches, log_stderr, now_ms, remove_media_file, truncate_chars,
};

pub struct HistoryStore {
    pub items: Vec<ClipboardItem>,
    path: PathBuf,
    pub last_seen: String,
    pub suppress_once: Option<String>,
    /// Fingerprint currently being materialized outside the lock.
    capture_inflight: Option<String>,
}

pub enum CaptureDecision {
    Skip,
    Accept { preserved_favorite: bool },
}

impl HistoryStore {
    pub fn load(path: PathBuf) -> Self {
        let mut items = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Vec<ClipboardItem>>(&raw).ok())
            .unwrap_or_default();
        let mut dirty = false;
        for item in &mut items {
            if item.kind.is_empty() {
                item.kind = default_kind();
                dirty = true;
            }
            if item.fingerprint.is_empty() {
                item.fingerprint = backfill_fingerprint(item);
                dirty = true;
            }
            // Drop inline thumbs from legacy history.json — UI loads via clipboard/thumb.
            if item.preview_data_url.take().is_some() {
                dirty = true;
            }
        }
        let last_seen = items
            .first()
            .map(|item| item.fingerprint.clone())
            .unwrap_or_default();
        let store = Self {
            items,
            path,
            last_seen,
            suppress_once: None,
            capture_inflight: None,
        };
        if dirty {
            if let Err(err) = store.persist() {
                log_stderr(format!("failed to rewrite migrated history: {err:?}"));
            }
        }
        store
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

    /// Decide whether a fingerprint should become a new history entry.
    /// Consumes `suppress_once` when it matches. Reserves `capture_inflight` on Accept.
    pub fn decide_capture(&mut self, fingerprint: &str) -> CaptureDecision {
        if fingerprint.is_empty() || fingerprint == self.last_seen {
            return CaptureDecision::Skip;
        }
        if self.suppress_once.as_deref() == Some(fingerprint) {
            self.suppress_once = None;
            self.last_seen = fingerprint.to_string();
            return CaptureDecision::Skip;
        }
        if self.capture_inflight.as_deref() == Some(fingerprint) {
            return CaptureDecision::Skip;
        }
        let preserved_favorite = self
            .items
            .iter()
            .find(|existing| existing.fingerprint == fingerprint)
            .map(|existing| existing.favorite)
            .unwrap_or(false);
        self.capture_inflight = Some(fingerprint.to_string());
        CaptureDecision::Accept { preserved_favorite }
    }

    pub fn release_capture_reservation(&mut self, fingerprint: &str) {
        if self.capture_inflight.as_deref() == Some(fingerprint) {
            self.capture_inflight = None;
        }
    }

    /// Build a history item (may write image blobs). Call outside the store lock when possible.
    pub fn materialize_capture(
        capture: Capture,
        fingerprint: &str,
    ) -> Result<ClipboardItem, PluginError> {
        let id = Uuid::new_v4().to_string();
        match capture {
            Capture::Text(text) => {
                let truncated = truncate_chars(&text, MAX_TEXT_CHARS);
                Ok(ClipboardItem {
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
                    fingerprint: fingerprint.to_string(),
                })
            }
            Capture::Image {
                width,
                height,
                rgba,
            } => build_image_item(&id, width, height, &rgba, fingerprint),
            Capture::Files(paths) => {
                let path_strs: Vec<String> = paths
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                if path_strs.is_empty() {
                    return Err(PluginError::new(-32000, "File list is empty"));
                }
                let label = path_strs.join("\n");
                Ok(ClipboardItem {
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
                    fingerprint: fingerprint.to_string(),
                })
            }
        }
    }

    pub fn commit_item(
        &mut self,
        mut item: ClipboardItem,
        preserved_favorite: bool,
    ) -> Result<ClipboardItem, PluginError> {
        item.favorite = preserved_favorite;
        let fingerprint = item.fingerprint.clone();
        self.release_capture_reservation(&fingerprint);

        // Another path already committed this content while we were materializing.
        if fingerprint == self.last_seen {
            remove_media_file(&item);
            if let Some(existing) = self
                .items
                .iter()
                .find(|existing| existing.fingerprint == fingerprint)
                .cloned()
            {
                return Ok(existing);
            }
            return Ok(item);
        }

        let mut next = self.items.clone();
        let mut orphaned = Vec::new();
        next.retain(|existing| {
            if existing.fingerprint == fingerprint {
                orphaned.push(existing.clone());
                false
            } else {
                true
            }
        });
        next.insert(0, item.clone());
        while next.len() > MAX_ITEMS {
            if let Some(pos) = next.iter().rposition(|existing| !existing.favorite) {
                orphaned.push(next.remove(pos));
            } else if let Some(removed) = next.pop() {
                orphaned.push(removed);
            } else {
                break;
            }
        }

        let previous_items = std::mem::replace(&mut self.items, next);
        let previous_last_seen = std::mem::replace(&mut self.last_seen, fingerprint);
        if let Err(err) = self.persist() {
            self.items = previous_items;
            self.last_seen = previous_last_seen;
            // Drop the newly written blob; replaced/evicted files stay intact.
            remove_media_file(&item);
            return Err(err);
        }

        for removed in orphaned {
            remove_media_file(&removed);
        }
        Ok(item)
    }

    pub fn push_capture(&mut self, capture: Capture) -> Result<Option<ClipboardItem>, PluginError> {
        let fingerprint = fingerprint_capture(&capture)?;
        let CaptureDecision::Accept { preserved_favorite } = self.decide_capture(&fingerprint)
        else {
            return Ok(None);
        };
        let item = match Self::materialize_capture(capture, &fingerprint) {
            Ok(item) => item,
            Err(err) => {
                self.release_capture_reservation(&fingerprint);
                log_stderr(format!("skip capture: {err:?}"));
                return Ok(None);
            }
        };
        Ok(Some(self.commit_item(item, preserved_favorite)?))
    }

    pub fn delete(&mut self, id: &str) -> Result<bool, PluginError> {
        let Some(pos) = self.items.iter().position(|item| item.id == id) else {
            return Ok(false);
        };
        let removed = self.items.remove(pos);
        if let Err(err) = self.persist() {
            self.items.insert(pos, removed);
            return Err(err);
        }
        remove_media_file(&removed);
        Ok(true)
    }

    pub fn clear(&mut self) -> Result<(), PluginError> {
        let previous_items = std::mem::take(&mut self.items);
        let previous_last_seen = std::mem::take(&mut self.last_seen);
        let previous_suppress = self.suppress_once.take();
        self.capture_inflight = None;
        if let Err(err) = self.persist() {
            self.items = previous_items;
            self.last_seen = previous_last_seen;
            self.suppress_once = previous_suppress;
            return Err(err);
        }
        for item in &previous_items {
            remove_media_file(item);
        }
        let _ = fs::remove_dir_all(data_dir().join("blobs"));
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ClipboardItem> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn bump_to_top(&mut self, id: &str) -> Result<(), PluginError> {
        let Some(pos) = self.items.iter().position(|item| item.id == id) else {
            return Ok(());
        };
        let mut item = self.items.remove(pos);
        item.created_at = now_ms();
        self.items.insert(0, item);
        if let Err(err) = self.persist() {
            // Best-effort restore order without inventing timestamps.
            if let Some(current) = self.items.first().cloned() {
                self.items.remove(0);
                let insert_at = pos.min(self.items.len());
                self.items.insert(insert_at, current);
            }
            return Err(err);
        }
        Ok(())
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<Option<bool>, PluginError> {
        let Some(pos) = self.items.iter().position(|item| item.id == id) else {
            return Ok(None);
        };
        self.items[pos].favorite = !self.items[pos].favorite;
        let favorite = self.items[pos].favorite;
        if let Err(err) = self.persist() {
            self.items[pos].favorite = !favorite;
            return Err(err);
        }
        Ok(Some(favorite))
    }
}

fn backfill_fingerprint(item: &ClipboardItem) -> String {
    match item.kind.as_str() {
        "image" => {
            if let Some(rel) = &item.media_file {
                match decode_png_file(&data_dir().join(rel)) {
                    Ok((_, _, rgba)) => fingerprint_bytes(&rgba),
                    Err(_) => format!("image-missing:{}", item.id),
                }
            } else {
                format!("image-missing:{}", item.id)
            }
        }
        "files" => {
            let paths: Vec<PathBuf> = item
                .paths
                .as_ref()
                .map(|paths| paths.iter().map(PathBuf::from).collect())
                .unwrap_or_default();
            if paths.is_empty() {
                fingerprint_text(&item.text)
            } else {
                fingerprint_paths(&paths)
            }
        }
        _ => fingerprint_text(&item.text),
    }
}
