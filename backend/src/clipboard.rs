use std::path::PathBuf;

use arboard::{Clipboard, ImageData};
use dbx_plugin_sdk::PluginError;

use crate::media::decode_png_file;
use crate::model::{Capture, ClipboardItem};
use crate::util::{data_dir, log_stderr};

/// Cheap clipboard generation token when the OS exposes one.
/// `None` means the platform has no cheap probe — callers should read content.
pub fn clipboard_generation() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSPasteboard;
        let pasteboard = NSPasteboard::generalPasteboard();
        return Some(pasteboard.changeCount() as u64);
    }
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn GetClipboardSequenceNumber() -> u32;
        }
        // SAFETY: Win32 clipboard sequence API has no preconditions.
        return Some(unsafe { GetClipboardSequenceNumber() } as u64);
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        None
    }
}

pub fn read_clipboard_capture() -> Option<Capture> {
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

pub fn write_item_to_clipboard(item: &ClipboardItem) -> Result<(), PluginError> {
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
