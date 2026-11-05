use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use dbx_plugin_sdk::{PluginEmitter, PluginError, PluginHandler, RequestContext};
use serde_json::{json, Value};

use crate::clipboard::{read_clipboard_capture, write_item_to_clipboard};
use crate::media::{build_lightbox_data_url, build_thumb_data_url};
use crate::model::ClipboardItem;
use crate::monitor::start_monitor;
use crate::reveal::open_local_path;
use crate::store::{CaptureDecision, HistoryStore};
use crate::util::{data_dir, fingerprint_capture};

pub struct Plugin {
    store: Arc<Mutex<HistoryStore>>,
    emitter: Arc<Mutex<Option<PluginEmitter>>>,
    monitor_started: AtomicBool,
}

impl Plugin {
    pub fn new(store: Arc<Mutex<HistoryStore>>) -> Self {
        Self {
            store,
            emitter: Arc::new(Mutex::new(None)),
            monitor_started: AtomicBool::new(false),
        }
    }

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
                    store.bump_to_top(&item.id)?;
                    Ok(json!({ "success": true, "kind": item.kind }))
                })
            }
            "clipboard/delete" => {
                let id = require_id(&params)?;
                let deleted = self.with_store(|store| store.delete(id))?;
                Ok(json!({ "success": deleted }))
            }
            "clipboard/favorite" => {
                let id = require_id(&params)?;
                let favorite = self.with_store(|store| store.toggle_favorite(id))?;
                let Some(favorite) = favorite else {
                    return Err(PluginError::new(-32602, "Clipboard item not found"));
                };
                Ok(json!({ "success": true, "favorite": favorite }))
            }
            "clipboard/clear" => {
                self.with_store(|store| store.clear())?;
                Ok(json!({ "success": true }))
            }
            "clipboard/capture" => {
                let Some(capture) = read_clipboard_capture() else {
                    return self.with_store(|store| {
                        Ok(json!({
                            "success": true,
                            "item": Value::Null,
                            "total": store.items.len()
                        }))
                    });
                };
                let fingerprint = fingerprint_capture(&capture)?;
                let decision = self.with_store(|store| Ok(store.decide_capture(&fingerprint)))?;
                let CaptureDecision::Accept { preserved_favorite } = decision else {
                    return self.with_store(|store| {
                        Ok(json!({
                            "success": true,
                            "item": Value::Null,
                            "total": store.items.len()
                        }))
                    });
                };
                let item = match HistoryStore::materialize_capture(capture, &fingerprint) {
                    Ok(item) => item,
                    Err(err) => {
                        self.with_store(|store| {
                            store.release_capture_reservation(&fingerprint);
                            Ok(())
                        })?;
                        return Err(PluginError::new(
                            -32000,
                            format!("Failed to capture clipboard: {err:?}"),
                        ));
                    }
                };
                let item = self.with_store(|store| store.commit_item(item, preserved_favorite))?;
                let total = self.with_store(|store| Ok(store.items.len()))?;
                Ok(json!({ "success": true, "item": item, "total": total }))
            }
            "clipboard/thumb" => {
                let id = require_id(&params)?;
                let item = self.require_item(id)?;
                if item.kind != "image" {
                    return Err(PluginError::new(-32602, "Item is not an image"));
                }
                if let Some(data_url) = item.preview_data_url.filter(|url| !url.is_empty()) {
                    return Ok(json!({ "id": item.id, "dataUrl": data_url }));
                }
                let rel = item
                    .media_file
                    .as_ref()
                    .ok_or_else(|| PluginError::new(-32000, "Image blob missing"))?;
                let data_url = build_thumb_data_url(&data_dir().join(rel))?;
                Ok(json!({ "id": item.id, "dataUrl": data_url }))
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
