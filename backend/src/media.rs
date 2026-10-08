use std::fs;
use std::io::Cursor;
use std::path::Path;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use dbx_plugin_sdk::PluginError;
use image::imageops::FilterType;
use image::{ImageBuffer, ImageFormat, RgbaImage};

use crate::model::{ClipboardItem, LIGHTBOX_EDGE, MAX_IMAGE_EDGE, THUMB_EDGE};
use crate::util::{data_dir, now_ms};

pub fn build_image_item(
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
        favorite: false,
        fingerprint: fingerprint.to_string(),
    })
}

fn maybe_downscale(
    width: u32,
    height: u32,
    rgba: &[u8],
    max_edge: u32,
) -> Result<(u32, u32, Vec<u8>), PluginError> {
    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
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

pub fn decode_png_file(path: &Path) -> Result<(u32, u32, Vec<u8>), PluginError> {
    let bytes = fs::read(path)
        .map_err(|err| PluginError::new(-32000, format!("Failed to read image blob: {err}")))?;
    let img = image::load_from_memory(&bytes)
        .map_err(|err| PluginError::new(-32000, format!("Failed to decode image blob: {err}")))?
        .to_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

/// Build a lightbox-sized JPEG data URL kept under the Host API 2 MiB JSON bridge limit.
pub fn build_lightbox_data_url(path: &Path) -> Result<(u32, u32, String), PluginError> {
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
