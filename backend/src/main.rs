mod clipboard;
mod media;
mod model;
mod monitor;
mod plugin;
mod reveal;
mod store;
mod util;

use std::sync::{Arc, Mutex};

use dbx_plugin_sdk::{PluginMetadata, PluginServer};

use clipboard::read_clipboard_capture;
use plugin::Plugin;
use store::HistoryStore;
use util::{history_path, log_stderr};

fn main() -> std::io::Result<()> {
    let store = Arc::new(Mutex::new(HistoryStore::load(history_path())));
    if let Some(capture) = read_clipboard_capture() {
        let _ = store.lock().map(|mut guard| guard.push_capture(capture));
    }

    log_stderr(format!("history path: {}", history_path().display()));

    let metadata =
        PluginMetadata::new("io.dbx.clipboard", env!("CARGO_PKG_VERSION")).with_capability("events");
    let plugin = Plugin::new(store);
    PluginServer::new(metadata, plugin).serve()
}
