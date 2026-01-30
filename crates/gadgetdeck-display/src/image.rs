//! Image parsing utilities for gadgetdeck-display

use gadgetdeck::StreamDeckModel;
use raylib::prelude::*;

/// Parse an image (BMP or JPEG) and convert to Raylib Image
/// - Stream Deck Mini: 80x80 24-bit BMP, rotated 90° CCW
/// - Stream Deck MK.2/XL: JPEG, rotated 180°
/// - Stream Deck Plus: JPEG, no rotation needed
pub fn parse_image_to_raylib(
    image_data: &[u8],
    expected_size: i32,
    model: StreamDeckModel,
) -> Option<Image> {
    if image_data.len() < 2 {
        log::warn!("Image data too short: {} bytes", image_data.len());
        return None;
    }

    // Determine if this model needs 180° rotation
    // Note: Plus does NOT need rotation unlike MK.2/XL/Neo
    let needs_rotation = matches!(
        model,
        StreamDeckModel::Mk2 | StreamDeckModel::Xl | StreamDeckModel::Neo
    );

    // Detect image format by magic bytes
    if image_data[0] == b'B' && image_data[1] == b'M' {
        // BMP format (Stream Deck Mini)
        parse_bmp_to_image(image_data, expected_size)
    } else if image_data[0] == 0xFF && image_data[1] == 0xD8 {
        // JPEG format (Stream Deck MK.2, XL, Plus)
        parse_jpeg_to_image(image_data, expected_size, needs_rotation)
    } else {
        log::warn!(
            "Unknown image format: {:02X} {:02X}",
            image_data[0],
            image_data[1]
        );
        None
    }
}

/// Parse a BMP image and convert to Raylib Image
/// Stream Deck Mini sends 80x80 24-bit BMP images that need transformation.
fn parse_bmp_to_image(bmp_data: &[u8], expected_size: i32) -> Option<Image> {
    // BMP Header structure:
    // 0-1: "BM" signature
    // 2-5: file size
    // 10-13: pixel data offset
    // 14-17: header size (40 for BITMAPINFOHEADER)
    // 18-21: width
    // 22-25: height
    // 26-27: planes (1)
    // 28-29: bits per pixel (24 for BGR)

    if bmp_data.len() < 54 {
        log::warn!("BMP data too short: {} bytes", bmp_data.len());
        return None;
    }

    let pixel_offset =
        u32::from_le_bytes([bmp_data[10], bmp_data[11], bmp_data[12], bmp_data[13]]) as usize;
    let width = i32::from_le_bytes([bmp_data[18], bmp_data[19], bmp_data[20], bmp_data[21]]);
    let height = i32::from_le_bytes([bmp_data[22], bmp_data[23], bmp_data[24], bmp_data[25]]);
    let bits_per_pixel = u16::from_le_bytes([bmp_data[28], bmp_data[29]]);

    log::debug!(
        "BMP: {}x{}, {} bpp, offset {}",
        width,
        height,
        bits_per_pixel,
        pixel_offset
    );

    if width != expected_size || height.abs() != expected_size {
        log::warn!(
            "Unexpected BMP size: {}x{} (expected {}x{})",
            width,
            height,
            expected_size,
            expected_size
        );
    }

    if bits_per_pixel != 24 {
        log::warn!(
            "Unexpected bits per pixel: {} (expected 24)",
            bits_per_pixel
        );
        return None;
    }

    // BMP rows are padded to 4-byte boundaries
    let row_size = ((width * 3 + 3) / 4 * 4) as usize;
    let abs_height = height.abs() as usize;
    let width_usize = width as usize;

    // Create RGBA pixel buffer (for Raylib)
    let mut rgba_pixels = vec![0u8; width_usize * abs_height * 4];

    // BMP stores rows bottom-to-top (unless height is negative)
    let bottom_up = height > 0;

    for row in 0..abs_height {
        let src_row = if bottom_up { abs_height - 1 - row } else { row };
        let src_offset = pixel_offset + src_row * row_size;

        for col in 0..width_usize {
            let src_idx = src_offset + col * 3;
            if src_idx + 2 >= bmp_data.len() {
                continue;
            }

            // BMP is BGR, convert to RGBA
            let b = bmp_data[src_idx];
            let g = bmp_data[src_idx + 1];
            let r = bmp_data[src_idx + 2];

            // Apply transformation to correct Stream Deck image orientation
            // Rotate 90° CW: (col, row) -> (height - 1 - row, col)
            // Then flip horizontally: x -> (width - 1 - x)
            let new_col = row; // flipped from (abs_height - 1 - row)
            let new_row = col;
            let dst_idx = (new_row * width_usize + new_col) * 4;

            rgba_pixels[dst_idx] = r;
            rgba_pixels[dst_idx + 1] = g;
            rgba_pixels[dst_idx + 2] = b;
            rgba_pixels[dst_idx + 3] = 255; // Alpha
        }
    }

    // Create Raylib image from RGBA pixels
    let image = unsafe {
        let img = Image::gen_image_color(width, height.abs(), Color::BLACK);
        // Copy our pixel data into the image
        std::ptr::copy_nonoverlapping(rgba_pixels.as_ptr(), img.data as *mut u8, rgba_pixels.len());
        img
    };

    Some(image)
}

/// Parse a JPEG image and convert to Raylib Image
/// Stream Deck MK.2/XL/Plus send JPEG images that need 180° rotation
fn parse_jpeg_to_image(jpeg_data: &[u8], _expected_size: i32, rotate_180: bool) -> Option<Image> {
    // Log the first few bytes to verify it's valid JPEG data
    let display_len = jpeg_data.len().min(16);
    let hex_str: String = jpeg_data[..display_len]
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ");
    log::debug!("JPEG data ({} bytes): {}", jpeg_data.len(), hex_str);

    // Verify JPEG magic bytes
    if jpeg_data.len() < 2 || jpeg_data[0] != 0xFF || jpeg_data[1] != 0xD8 {
        log::warn!(
            "Invalid JPEG magic bytes: expected FF D8, got {:02X} {:02X}",
            jpeg_data.get(0).unwrap_or(&0),
            jpeg_data.get(1).unwrap_or(&0)
        );
        return None;
    }

    // Try to load JPEG using Raylib's built-in image loading
    let extension = std::ffi::CString::new(".jpg").ok()?;

    let mut loaded_image = unsafe {
        let img = raylib::ffi::LoadImageFromMemory(
            extension.as_ptr(),
            jpeg_data.as_ptr(),
            jpeg_data.len() as i32,
        );

        if img.data.is_null() {
            log::warn!(
                "Failed to load JPEG image from memory ({} bytes)",
                jpeg_data.len()
            );
            return None;
        }

        img
    };

    let width = loaded_image.width as usize;
    let height = loaded_image.height as usize;
    let format = loaded_image.format;

    log::debug!("JPEG loaded: {}x{} format={}", width, height, format);

    // Convert to RGBA format if needed (format 7 = PIXELFORMAT_UNCOMPRESSED_R8G8B8A8)
    // Format 4 = PIXELFORMAT_UNCOMPRESSED_R8G8B8 (RGB, no alpha)
    if format != 7 {
        unsafe {
            raylib::ffi::ImageFormat(&mut loaded_image, 7); // Convert to RGBA
        }
        log::debug!("Converted image to RGBA format");
    }

    // Stream Deck MK.2/XL/Plus images are rotated 180°
    // Rotate by reversing all pixels if needed
    let pixel_count = width * height;
    let bytes_per_pixel = 4; // RGBA after conversion
    let data_size = pixel_count * bytes_per_pixel;

    if rotate_180 {
        let mut rotated_pixels = vec![0u8; data_size];

        unsafe {
            let src_data = loaded_image.data as *const u8;

            // 180° rotation = reverse all pixels
            for i in 0..pixel_count {
                let src_idx = i * bytes_per_pixel;
                let dst_idx = (pixel_count - 1 - i) * bytes_per_pixel;

                // Copy RGBA pixel
                std::ptr::copy_nonoverlapping(
                    src_data.add(src_idx),
                    rotated_pixels.as_mut_ptr().add(dst_idx),
                    bytes_per_pixel,
                );
            }

            // Unload the original
            raylib::ffi::UnloadImage(loaded_image);
        }

        // Create Raylib Image with the rotated RGBA data
        let image = unsafe {
            let img = Image::gen_image_color(width as i32, height as i32, Color::BLACK);
            std::ptr::copy_nonoverlapping(
                rotated_pixels.as_ptr(),
                img.data as *mut u8,
                rotated_pixels.len(),
            );
            img
        };

        Some(image)
    } else {
        // No rotation needed, convert directly
        let image = unsafe {
            let img = Image::gen_image_color(width as i32, height as i32, Color::BLACK);
            std::ptr::copy_nonoverlapping(
                loaded_image.data as *const u8,
                img.data as *mut u8,
                data_size,
            );
            raylib::ffi::UnloadImage(loaded_image);
            img
        };

        Some(image)
    }
}
