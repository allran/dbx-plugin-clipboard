use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use dbx_plugin_sdk::{
    PluginEmitter, PluginError, PluginHandler, PluginMetadata, PluginServer, RequestContext,
};
use image::imageops::FilterType;
use image::{ImageBuffer, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const MAX_ITEMS: usize = 500;
const MAX_TEXT_CHARS: usize = 100_000;
const MAX_IMAGE_EDGE: u32 = 4096;
const THUMB_EDGE: u32 = 160;
const LIGHTBOX_EDGE: u32 = 1920;
const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipboardItem {
    id: String,
    #[serde(default = "default_kind")]
    kind: String,
    created_at: u64,
    /// Searchable summary. For text items this is the full (truncated) content.
    #[serde(default)]
    text: String,
    #[serde(default)]
    char_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
    /// Relative path under data dir, e.g. `blobs/<id>.png`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    media_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    paths: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview_data_url: Option<String>,
    #[serde(default)]
    fingerprint: String,
}

fn default_kind() -> String {
    "text".to_string()
}

enum Capture {
    Text(String),
    Image {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Files(Vec<PathBuf>),
}

struct HistoryStore {
    items: Vec<ClipboardItem>,
    path: PathBuf,
    last_seen: String,
    suppress_once: Option<String>,
}

impl HistoryStore {
    fn load(path: PathBuf) -> Self {
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

    fn list(&self, query: Option<&str>) -> Vec<ClipboardItem> {
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

    fn push_capture(&mut self, capture: Capture) -> Option<ClipboardItem> {
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

        let id = Uuid::new_v4().to_string();
        let item = match capture {
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
                    fingerprint,
                }
            }
        };

        self.items
            .retain(|existing| existing.fingerprint != item.fingerprint);
        self.items.insert(0, item.clone());
        while self.items.len() > MAX_ITEMS {
            if let Some(removed) = self.items.pop() {
                remove_media_file(&removed);
            }
        }
        self.last_seen = item.fingerprint.clone();
        let _ = self.persist();
        Some(item)
    }

    fn delete(&mut self, id: &str) -> Result<bool, PluginError> {
        let Some(pos) = self.items.iter().position(|item| item.id == id) else {
            return Ok(false);
        };
        let removed = self.items.remove(pos);
        remove_media_file(&removed);
        self.persist()?;
        Ok(true)
    }

    fn clear(&mut self) -> Result<(), PluginError> {
        for item in &self.items {
            remove_media_file(item);
        }
        self.items.clear();
        let _ = fs::remove_dir_all(data_dir().join("blobs"));
        self.persist()
    }

    fn get(&self, id: &str) -> Option<&ClipboardItem> {
        self.items.iter().find(|item| item.id == id)
    }

    fn bump_to_top(&mut self, id: &str) {
        if let Some(pos) = self.items.iter().position(|item| item.id == id) {
            let mut item = self.items.remove(pos);
            item.created_at = now_ms();
            self.items.insert(0, item);
            let _ = self.persist();
        }
    }
}

struct Plugin {
    store: Arc<Mutex<HistoryStore>>,
    emitter: Arc<Mutex<Option<PluginEmitter>>>,
    monitor_started: AtomicBool,
}

impl Plugin {
    fn remember_emitter(&self, emitter: &PluginEmitter) {
        if let Ok(mut slot) = self.emitter.lock() {
            *slot = Some(emitter.clone());
        }
        if self
            .monitor_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            start_monitor(Arc::clone(&self.store), Arc::clone(&self.emitter));
        }
    }

    fn with_store<R>(
        &self,
        f: impl FnOnce(&mut HistoryStore) -> Result<R, PluginError>,
    ) -> Result<R, PluginError> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| PluginError::new(-32000, "History store is poisoned"))?;
        f(&mut store)
    }

    fn require_item(&self, id: &str) -> Result<ClipboardItem, PluginError> {
        self.with_store(|store| {
            store
                .get(id)
                .cloned()
                .ok_or_else(|| PluginError::new(-32602, "Clipboard item not found"))
        })
    }
}

fn require_id(params: &Value) -> Result<&str, PluginError> {
    params
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| PluginError::new(-32602, "Missing id"))
}

impl PluginHandler for Plugin {
    fn handle(
        &self,
        _context: RequestContext,
        method: &str,
        params: Value,
        emitter: &PluginEmitter,
    ) -> Result<Value, PluginError> {
        self.remember_emitter(emitter);

        match method {
            "clipboard/list" => {
                let query = params.get("query").and_then(Value::as_str);
                self.with_store(|store| {
                    let items = store.list(query);
                    Ok(json!({ "items": items, "total": items.len() }))
                })
            }
            "clipboard/copy" => {
                let id = require_id(&params)?;
                let item = self.require_item(id)?;
                write_item_to_clipboard(&item)?;
                self.with_store(|store| {
                    store.suppress_once = Some(item.fingerprint.clone());
                    store.last_seen = item.fingerprint.clone();
                    store.bump_to_top(&item.id);
                    Ok(())
                })?;
                Ok(json!({ "success": true, "kind": item.kind }))
            }
            "clipboard/delete" => {
                let id = require_id(&params)?;
                let deleted = self.with_store(|store| store.delete(id))?;
                Ok(json!({ "success": deleted }))
            }
            "clipboard/clear" => {
                self.with_store(|store| store.clear())?;
                Ok(json!({ "success": true }))
            }
            "clipboard/capture" => {
                let capture = read_clipboard_capture();
                self.with_store(|store| {
                    let item = capture.and_then(|c| store.push_capture(c));
                    Ok(json!({
                        "success": true,
                        "item": item,
                        "total": store.items.len()
                    }))
                })
            }
            "clipboard/media" => {
                let id = require_id(&params)?;
                let item = self.require_item(id)?;
                if item.kind != "image" {
                    return Err(PluginError::new(-32602, "Item is not an image"));
                }
                let rel = item
                    .media_file
                    .as_ref()
                    .ok_or_else(|| PluginError::new(-32000, "Image blob missing"))?;
                let (width, height, data_url) = build_lightbox_data_url(&data_dir().join(rel))?;
                Ok(json!({
                    "id": item.id,
                    "width": width,
                    "height": height,
                    "dataUrl": data_url
                }))
            }
            "clipboard/open-path" => {
                let path = params
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| PluginError::new(-32602, "Missing path"))?;
                open_local_path(path)?;
                Ok(json!({ "success": true, "path": path }))
            }
            _ => Err(PluginError::method_not_found(method)),
        }
    }
}

fn data_dir() -> PathBuf {
    env::var_os("DBX_PLUGIN_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::temp_dir().join("dbx-plugin-data").join("alan.clipboard"))
}

fn history_path() -> PathBuf {
    data_dir().join("history.json")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

fn fingerprint_text(text: &str) -> String {
    format!("text:{}", hash_bytes(text.as_bytes()))
}

fn fingerprint_bytes(bytes: &[u8]) -> String {
    format!("image:{}", hash_bytes(bytes))
}

fn fingerprint_paths(paths: &[PathBuf]) -> String {
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

fn item_matches(item: &ClipboardItem, needle: &str) -> bool {
    if item.text.to_lowercase().contains(needle) || item.kind.to_lowercase().contains(needle) {
        return true;
    }
    if let Some(paths) = &item.paths {
        return paths.iter().any(|p| p.to_lowercase().contains(needle));
    }
    false
}

fn remove_media_file(item: &ClipboardItem) {
    if let Some(rel) = &item.media_file {
        let path = data_dir().join(rel);
        let _ = fs::remove_file(path);
    }
}

fn build_image_item(
    id: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
    fingerprint: &str,
) -> Result<ClipboardItem, PluginError> {
    let (store_w, store_h, store_rgba) = maybe_downscale(width, height, rgba, MAX_IMAGE_EDGE)?;
    let png = encode_png(store_w, store_h, &store_rgba)?;
    let blobs = data_dir().join("blobs");
    fs::create_dir_all(&blobs)
        .map_err(|err| PluginError::new(-32000, format!("Failed to create blobs dir: {err}")))?;
    let rel = format!("blobs/{id}.png");
    fs::write(data_dir().join(&rel), &png)
        .map_err(|err| PluginError::new(-32000, format!("Failed to write image blob: {err}")))?;

    let (thumb_w, thumb_h, thumb_rgba) =
        maybe_downscale(store_w, store_h, &store_rgba, THUMB_EDGE)?;
    let thumb_png = encode_png(thumb_w, thumb_h, &thumb_rgba)?;
    let preview_data_url = format!("data:image/png;base64,{}", BASE64.encode(thumb_png));

    Ok(ClipboardItem {
        id: id.to_string(),
        kind: "image".into(),
        created_at: now_ms(),
        char_count: 0,
        text: format!("[Image {store_w}×{store_h}]"),
        width: Some(store_w),
        height: Some(store_h),
        media_file: Some(rel),
        paths: None,
        preview_data_url: Some(preview_data_url),
        fingerprint: fingerprint.to_string(),
    })
}

fn maybe_downscale(
    width: u32,
    height: u32,
    rgba: &[u8],
    max_edge: u32,
) -> Result<(u32, u32, Vec<u8>), PluginError> {
    let expected = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    if rgba.len() < expected {
        return Err(PluginError::new(-32000, "Image buffer is truncated"));
    }
    let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba[..expected].to_vec())
        .ok_or_else(|| PluginError::new(-32000, "Invalid image buffer"))?;
    if width <= max_edge && height <= max_edge {
        return Ok((width, height, img.into_raw()));
    }
    let scale = (max_edge as f32 / width.max(height) as f32).min(1.0);
    let new_w = ((width as f32) * scale).round().max(1.0) as u32;
    let new_h = ((height as f32) * scale).round().max(1.0) as u32;
    let resized = image::imageops::resize(&img, new_w, new_h, FilterType::Triangle);
    Ok((new_w, new_h, resized.into_raw()))
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, PluginError> {
    let img: RgbaImage = ImageBuffer::from_raw(width, height, rgba.to_vec())
        .ok_or_else(|| PluginError::new(-32000, "Invalid image buffer"))?;
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)
        .map_err(|err| PluginError::new(-32000, format!("PNG encode failed: {err}")))?;
    Ok(out.into_inner())
}

fn decode_png_file(path: &Path) -> Result<(u32, u32, Vec<u8>), PluginError> {
    let bytes = fs::read(path)
        .map_err(|err| PluginError::new(-32000, format!("Failed to read image blob: {err}")))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|err| PluginError::new(-32000, format!("Failed to decode image blob: {err}")))?
        .to_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

/// Build a lightbox-sized JPEG data URL kept under the Host API 2 MiB JSON bridge limit.
fn build_lightbox_data_url(path: &Path) -> Result<(u32, u32, String), PluginError> {
    let (width, height, rgba) = decode_png_file(path)?;
    let (view_w, view_h, view_rgba) = maybe_downscale(width, height, &rgba, LIGHTBOX_EDGE)?;
    let img: RgbaImage = ImageBuffer::from_raw(view_w, view_h, view_rgba)
        .ok_or_else(|| PluginError::new(-32000, "Invalid image buffer"))?;
    let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();

    let mut quality = 85u8;
    loop {
        let mut out = Cursor::new(Vec::new());
        {
            use image::ImageEncoder;
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            encoder
                .write_image(rgb.as_raw(), view_w, view_h, image::ExtendedColorType::Rgb8)
                .map_err(|err| PluginError::new(-32000, format!("JPEG encode failed: {err}")))?;
        }
        let bytes = out.into_inner();
        let data_url = format!("data:image/jpeg;base64,{}", BASE64.encode(&bytes));
        // Keep payload comfortably under the 2 MiB Host JSON bridge limit.
        if data_url.len() <= 1_500_000 || quality <= 45 {
            return Ok((view_w, view_h, data_url));
        }
        quality = quality.saturating_sub(15);
    }
}

fn read_clipboard_capture() -> Option<Capture> {
    let mut clipboard = match Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(err) => {
            log_stderr(format!("clipboard unavailable: {err}"));
            return None;
        }
    };

    // Prefer file lists, then images, then text.
    match clipboard.get().file_list() {
        Ok(paths) if !paths.is_empty() => return Some(Capture::Files(paths)),
        Ok(_) | Err(arboard::Error::ContentNotAvailable) => {}
        Err(err) => log_stderr(format!("file_list error: {err}")),
    }

    match clipboard.get_image() {
        Ok(image) if image.width > 0 && image.height > 0 => {
            return Some(Capture::Image {
                width: image.width as u32,
                height: image.height as u32,
                rgba: image.bytes.into_owned(),
            });
        }
        Ok(_) | Err(arboard::Error::ContentNotAvailable) => {}
        Err(err) => log_stderr(format!("image error: {err}")),
    }

    match clipboard.get_text() {
        Ok(text) if !text.is_empty() => Some(Capture::Text(text)),
        Ok(_) | Err(arboard::Error::ContentNotAvailable) => None,
        Err(err) => {
            log_stderr(format!("text error: {err}"));
            None
        }
    }
}

fn write_item_to_clipboard(item: &ClipboardItem) -> Result<(), PluginError> {
    let mut clipboard = Clipboard::new()
        .map_err(|err| PluginError::new(-32000, format!("Clipboard unavailable: {err}")))?;

    match item.kind.as_str() {
        "image" => {
            let rel = item
                .media_file
                .as_ref()
                .ok_or_else(|| PluginError::new(-32000, "Image blob missing"))?;
            let (width, height, rgba) = decode_png_file(&data_dir().join(rel))?;
            clipboard
                .set_image(ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: rgba.into(),
                })
                .map_err(|err| PluginError::new(-32000, format!("Failed to write image: {err}")))
        }
        "files" => {
            let paths = item
                .paths
                .as_ref()
                .ok_or_else(|| PluginError::new(-32000, "File paths missing"))?;
            let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
            clipboard
                .set()
                .file_list(&path_bufs)
                .map_err(|err| PluginError::new(-32000, format!("Failed to write files: {err}")))
        }
        _ => clipboard
            .set_text(item.text.clone())
            .map_err(|err| PluginError::new(-32000, format!("Failed to write text: {err}"))),
    }
}

fn log_stderr(message: impl AsRef<str>) {
    let _ = writeln!(io::stderr(), "[alan.clipboard] {}", message.as_ref());
}

fn open_local_path(raw: &str) -> Result<(), PluginError> {
    let path = PathBuf::from(raw);
    if raw.trim().is_empty() {
        return Err(PluginError::new(-32602, "Path is empty"));
    }
    if !path.is_absolute() {
        return Err(PluginError::new(-32602, "Path must be absolute"));
    }
    if !path.exists() {
        return Err(PluginError::new(
            -32000,
            format!("Path does not exist: {}", path.display()),
        ));
    }

    // Reveal the path in the system file manager (do not open with default app).
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", raw])
            .spawn()
            .map_err(|err| PluginError::new(-32000, format!("Failed to reveal path: {err}")))?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(format!("/select,{raw}"))
            .spawn()
            .map_err(|err| PluginError::new(-32000, format!("Failed to reveal path: {err}")))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let folder = if path.is_dir() {
            path.clone()
        } else {
            path.parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| PluginError::new(-32000, "Path has no parent directory"))?
        };
        Command::new("xdg-open")
            .arg(&folder)
            .spawn()
            .map_err(|err| PluginError::new(-32000, format!("Failed to open folder: {err}")))?;
    }

    Ok(())
}

fn start_monitor(store: Arc<Mutex<HistoryStore>>, emitter: Arc<Mutex<Option<PluginEmitter>>>) {
    thread::spawn(move || loop {
        thread::sleep(POLL_INTERVAL);
        let Some(capture) = read_clipboard_capture() else {
            continue;
        };
        let item = match store.lock() {
            Ok(mut guard) => guard.push_capture(capture),
            Err(_) => continue,
        };
        if let Some(item) = item {
            if let Ok(slot) = emitter.lock() {
                if let Some(emitter) = slot.as_ref() {
                    if let Err(err) = emitter.event("clipboard/item", json!({ "item": item })) {
                        log_stderr(format!("event emit failed: {err:?}"));
                    }
                }
            }
        }
    });
}

fn main() -> std::io::Result<()> {
    let store = Arc::new(Mutex::new(HistoryStore::load(history_path())));
    if let Some(capture) = read_clipboard_capture() {
        let _ = store.lock().map(|mut guard| guard.push_capture(capture));
    }

    log_stderr(format!("history path: {}", history_path().display()));

    let metadata =
        PluginMetadata::new("alan.clipboard", env!("CARGO_PKG_VERSION")).with_capability("events");
    let plugin = Plugin {
        store,
        emitter: Arc::new(Mutex::new(None)),
        monitor_started: AtomicBool::new(false),
    };
    PluginServer::new(metadata, plugin).serve()
}
