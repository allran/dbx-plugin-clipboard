use std::sync::{Arc, Mutex};
use std::thread;

use dbx_plugin_sdk::PluginEmitter;
use serde_json::json;

use crate::clipboard::{clipboard_generation, read_clipboard_capture};
use crate::model::POLL_INTERVAL;
use crate::store::{CaptureDecision, HistoryStore};
use crate::util::{fingerprint_capture, log_stderr};

pub fn start_monitor(store: Arc<Mutex<HistoryStore>>, emitter: Arc<Mutex<Option<PluginEmitter>>>) {
    thread::spawn(move || {
        let mut last_generation: Option<u64> = None;
        loop {
            thread::sleep(POLL_INTERVAL);

            let generation = clipboard_generation();
            if let Some(generation) = generation {
                if last_generation == Some(generation) {
                    continue;
                }
            }

            let Some(capture) = read_clipboard_capture() else {
                // Keep previous generation so a transient read failure retries next tick.
                continue;
            };
            if let Some(generation) = generation {
                last_generation = Some(generation);
            }

            let fingerprint = match fingerprint_capture(&capture) {
                Ok(fingerprint) => fingerprint,
                Err(err) => {
                    log_stderr(format!("fingerprint failed: {err:?}"));
                    continue;
                }
            };

            let decision = match store.lock() {
                Ok(mut guard) => guard.decide_capture(&fingerprint),
                Err(_) => continue,
            };
            let CaptureDecision::Accept { preserved_favorite } = decision else {
                continue;
            };

            let item = match HistoryStore::materialize_capture(capture, &fingerprint) {
                Ok(item) => item,
                Err(err) => {
                    if let Ok(mut guard) = store.lock() {
                        guard.release_capture_reservation(&fingerprint);
                    }
                    log_stderr(format!("skip capture: {err:?}"));
                    continue;
                }
            };

            let committed = match store.lock() {
                Ok(mut guard) => match guard.commit_item(item, preserved_favorite) {
                    Ok(item) => item,
                    Err(err) => {
                        log_stderr(format!("persist capture failed: {err:?}"));
                        continue;
                    }
                },
                Err(_) => continue,
            };

            if let Ok(slot) = emitter.lock() {
                if let Some(emitter) = slot.as_ref() {
                    if let Err(err) =
                        emitter.event("clipboard/item", json!({ "item": committed }))
                    {
                        log_stderr(format!("event emit failed: {err:?}"));
                    }
                }
            }
        }
    });
}
