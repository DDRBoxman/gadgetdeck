//! Touchscreen strip UI for Stream Deck Plus

use raylib::prelude::*;

/// Stream Deck Plus touchscreen dimensions
pub const LCD_WIDTH: usize = 800;
pub const LCD_HEIGHT: usize = 100;

/// Minimum distance (in pixels) to consider a movement as a swipe
const SWIPE_MIN_DISTANCE: u16 = 50;
/// Maximum time (in ms) for a short tap
const SHORT_TAP_MAX_MS: u128 = 200;
/// Minimum time (in ms) for a long press
const LONG_PRESS_MIN_MS: u128 = 500;

/// Detected touch gesture on the touchscreen strip
#[derive(Debug, Clone, Copy)]
pub enum TouchGesture {
    /// Short tap at coordinates
    ShortTap { x: u16, y: u16 },
    /// Long press at coordinates
    LongPress { x: u16, y: u16 },
    /// Drag/swipe from start to end coordinates
    Drag { start_x: u16, start_y: u16, end_x: u16, end_y: u16 },
}

/// Touchscreen strip display state for Stream Deck Plus
pub struct TouchscreenStrip {
    /// Rectangle for the touchscreen area
    rect: Rectangle,
    /// Raylib texture for the touchscreen image
    texture: Option<Texture2D>,
    /// Composite image buffer (800x100 RGBA) for assembling segments
    composite_buffer: Vec<u8>,
    /// Whether the composite buffer has been modified and needs texture update
    buffer_dirty: bool,
    /// Last touch position (if touched)
    pub touch_position: Option<Vector2>,
    /// Touch start position for swipe detection (relative coords)
    pub touch_start: Option<(u16, u16)>,
    /// Touch start time for distinguishing tap vs long press
    pub touch_start_time: Option<std::time::Instant>,
    /// Whether a touch is currently active
    pub is_touching: bool,
}

impl TouchscreenStrip {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        // Initialize composite buffer as black (RGBA)
        let composite_buffer = vec![0u8; LCD_WIDTH * LCD_HEIGHT * 4];
        Self {
            rect: Rectangle::new(x, y, width, height),
            texture: None,
            composite_buffer,
            buffer_dirty: false,
            touch_position: None,
            touch_start: None,
            touch_start_time: None,
            is_touching: false,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.width
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.height
    }

    /// Get touch coordinates relative to the strip (0-799, 0-99)
    pub fn get_relative_touch(&self, x: f32, y: f32) -> (u16, u16) {
        let rel_x = ((x - self.rect.x) / self.rect.width * 800.0).clamp(0.0, 799.0) as u16;
        let rel_y = ((y - self.rect.y) / self.rect.height * 100.0).clamp(0.0, 99.0) as u16;
        (rel_x, rel_y)
    }
    
    /// Handle touch start event
    pub fn on_touch_start(&mut self, rel_x: u16, rel_y: u16) {
        self.touch_start = Some((rel_x, rel_y));
        self.touch_start_time = Some(std::time::Instant::now());
        self.is_touching = true;
    }
    
    /// Handle touch end event, returns the detected gesture if any
    pub fn on_touch_end(&mut self, rel_x: u16, rel_y: u16) -> Option<TouchGesture> {
        let result = if let (Some((start_x, start_y)), Some(start_time)) = (self.touch_start, self.touch_start_time) {
            let duration_ms = start_time.elapsed().as_millis();
            let dx = (rel_x as i32 - start_x as i32).abs() as u16;
            let dy = (rel_y as i32 - start_y as i32).abs() as u16;
            let distance = dx.max(dy);
            
            if distance >= SWIPE_MIN_DISTANCE {
                // It's a swipe/drag
                Some(TouchGesture::Drag {
                    start_x,
                    start_y,
                    end_x: rel_x,
                    end_y: rel_y,
                })
            } else if duration_ms >= LONG_PRESS_MIN_MS {
                // Long press
                Some(TouchGesture::LongPress { x: start_x, y: start_y })
            } else {
                // Short tap
                Some(TouchGesture::ShortTap { x: start_x, y: start_y })
            }
        } else {
            None
        };
        
        self.touch_start = None;
        self.touch_start_time = None;
        self.is_touching = false;
        self.touch_position = None;
        
        result
    }
    
    /// Handle touch cancel (touch moved off strip)
    pub fn on_touch_cancel(&mut self) {
        self.touch_start = None;
        self.touch_start_time = None;
        self.is_touching = false;
        self.touch_position = None;
    }

    /// Update a segment of the touchscreen image at the given x_offset, y_offset
    pub fn update_image(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, image_data: &[u8], x_offset: u16, y_offset: u16, width: u16, height: u16) {
        // Stream Deck Plus touchscreen receives JPEG images in segments
        if image_data.len() < 2 {
            return;
        }
        
        if image_data[0] != 0xFF || image_data[1] != 0xD8 {
            log::warn!("LCD image is not JPEG format");
            return;
        }
        
        // Decode the JPEG segment
        let extension = match std::ffi::CString::new(".jpg") {
            Ok(ext) => ext,
            Err(_) => return,
        };
        
        let mut loaded_image = unsafe {
            let img = raylib::ffi::LoadImageFromMemory(
                extension.as_ptr(),
                image_data.as_ptr(),
                image_data.len() as i32,
            );
            
            if img.data.is_null() {
                log::warn!("Failed to load LCD JPEG segment ({} bytes)", image_data.len());
                return;
            }
            img
        };
        
        let img_width = loaded_image.width as usize;
        let img_height = loaded_image.height as usize;
        
        log::debug!(
            "LCD segment: x_off={}, y_off={}, {}x{} (decoded {}x{})",
            x_offset, y_offset, width, height, img_width, img_height
        );
        
        // Convert to RGBA format if needed (format 7 = PIXELFORMAT_UNCOMPRESSED_R8G8B8A8)
        if loaded_image.format != 7 {
            unsafe {
                raylib::ffi::ImageFormat(&mut loaded_image, 7);
            }
        }
        
        // Copy the segment into our composite buffer at the correct offset
        let x_off = x_offset as usize;
        let y_off = y_offset as usize;
        unsafe {
            let src_data = loaded_image.data as *const u8;
            
            for row in 0..img_height {
                let dst_y = y_off + row;
                if dst_y >= LCD_HEIGHT {
                    break;
                }
                
                for col in 0..img_width {
                    let dst_x = x_off + col;
                    if dst_x >= LCD_WIDTH {
                        break;
                    }
                    
                    let src_idx = (row * img_width + col) * 4;
                    let dst_idx = (dst_y * LCD_WIDTH + dst_x) * 4;
                    
                    // Copy RGBA pixel
                    self.composite_buffer[dst_idx] = *src_data.add(src_idx);
                    self.composite_buffer[dst_idx + 1] = *src_data.add(src_idx + 1);
                    self.composite_buffer[dst_idx + 2] = *src_data.add(src_idx + 2);
                    self.composite_buffer[dst_idx + 3] = *src_data.add(src_idx + 3);
                }
            }
            
            raylib::ffi::UnloadImage(loaded_image);
        }
        
        self.buffer_dirty = true;
        
        // Rebuild the texture from the composite buffer
        self.rebuild_texture(rl, thread);
    }
    
    /// Rebuild the texture from the composite buffer
    fn rebuild_texture(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        if !self.buffer_dirty {
            return;
        }
        
        // Create a Raylib image from our composite buffer
        let image = unsafe {
            let img = Image::gen_image_color(LCD_WIDTH as i32, LCD_HEIGHT as i32, Color::BLACK);
            std::ptr::copy_nonoverlapping(
                self.composite_buffer.as_ptr(),
                img.data as *mut u8,
                self.composite_buffer.len(),
            );
            img
        };
        
        // Drop old texture and create new one
        self.texture = None;
        self.texture = rl.load_texture_from_image(thread, &image).ok();
        self.buffer_dirty = false;
        
        log::debug!("LCD composite texture rebuilt");
    }

    /// Draw the touchscreen strip
    pub fn draw(&self, d: &mut RaylibDrawHandle) {
        // Background
        d.draw_rectangle_rec(self.rect, Color::new(20, 20, 25, 255));
        
        // Draw texture if we have one
        if let Some(ref texture) = self.texture {
            // Scale to fit the strip area
            let scale_x = self.rect.width / texture.width as f32;
            let scale_y = self.rect.height / texture.height as f32;
            let scale = scale_x.min(scale_y);
            
            let img_width = texture.width as f32 * scale;
            let img_height = texture.height as f32 * scale;
            let img_x = self.rect.x + (self.rect.width - img_width) / 2.0;
            let img_y = self.rect.y + (self.rect.height - img_height) / 2.0;
            
            d.draw_texture_ex(
                texture,
                Vector2::new(img_x, img_y),
                0.0,
                scale,
                Color::WHITE,
            );
        } else {
            // Draw placeholder text
            let text = "Touchscreen Strip";
            let text_size = 14;
            let text_width = d.measure_text(text, text_size);
            let text_x = self.rect.x as i32 + (self.rect.width as i32 - text_width) / 2;
            let text_y = self.rect.y as i32 + (self.rect.height as i32 - text_size) / 2;
            d.draw_text(text, text_x, text_y, text_size, Color::GRAY);
        }
        
        // Draw touch position indicator if touched
        if let Some(pos) = self.touch_position {
            d.draw_circle_v(pos, 8.0, Color::new(255, 100, 100, 180));
        }
        
        // Border
        d.draw_rectangle_lines_ex(self.rect, 2.0, Color::GRAY);
        
        // Segment dividers (800px / 4 = 200px per segment)
        let segment_width = self.rect.width / 4.0;
        for i in 1..4 {
            let x = self.rect.x + segment_width * i as f32;
            d.draw_line_v(
                Vector2::new(x, self.rect.y),
                Vector2::new(x, self.rect.y + self.rect.height),
                Color::new(60, 60, 70, 255),
            );
        }
    }
}
