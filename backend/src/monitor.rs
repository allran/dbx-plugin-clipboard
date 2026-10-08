use std::sync::{Arc, Mutex};
use std::thread;

use dbx_plugin_sdk::PluginEmitter;
use serde_json::json;

use crate::clipboard::read_clipboard_capture;
use crate::model::POLL_INTERVAL;
use crate::store::HistoryStore;
use crate::util::log_stderr;

pub fn start_monitor(store: Arc<Mutex<HistoryStore>>, emitter: Arc<Mutex<Option<PluginEmitter>>>) {
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
