//! Info bar LCD display for Stream Deck Neo
//!
//! The Neo has a 248x58 pixel info bar LCD between the main buttons and
//! two additional LED buttons (indices 8-9) on either side of the info bar.

use raylib::prelude::*;

/// Stream Deck Neo info bar LCD dimensions
pub const INFO_BAR_WIDTH: usize = 248;
pub const INFO_BAR_HEIGHT: usize = 58;

/// LED button for Stream Deck Neo (buttons 8 and 9)
pub struct LedButton {
    /// Rectangle for the button area
    rect: Rectangle,
    /// Button index (8 for left, 9 for right)
    pub index: u8,
    /// Current LED color (RGB)
    pub led_color: (u8, u8, u8),
    /// Whether the button is currently pressed
    pub pressed: bool,
}

impl LedButton {
    pub fn new(x: f32, y: f32, width: f32, height: f32, index: u8) -> Self {
        Self {
            rect: Rectangle::new(x, y, width, height),
            index,
            led_color: (0, 0, 0), // Default to off (black)
            pressed: false,
        }
    }

    /// Check if a point is within this button
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.width
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.height
    }

    /// Set the LED color
    pub fn set_led_color(&mut self, r: u8, g: u8, b: u8) {
        self.led_color = (r, g, b);
    }

    /// Draw the LED button
    pub fn draw(&self, d: &mut RaylibDrawHandle) {
        // Background with LED color glow effect
        let (r, g, b) = self.led_color;
        let led_color = Color::new(r, g, b, 255);

        // Draw button background
        let bg_color = if self.pressed {
            Color::new(80, 80, 80, 255)
        } else {
            Color::new(40, 40, 40, 255)
        };

        d.draw_rectangle_rounded(self.rect, 0.2, 8, bg_color);

        // Draw LED strip at the top of the button
        let led_strip_height = 6.0;
        let led_rect = Rectangle::new(
            self.rect.x + 4.0,
            self.rect.y + 4.0,
            self.rect.width - 8.0,
            led_strip_height,
        );
        d.draw_rectangle_rounded(led_rect, 0.5, 4, led_color);

        // Draw glow effect if LED is on
        if r > 0 || g > 0 || b > 0 {
            let glow_color = Color::new(r, g, b, 60);
            d.draw_rectangle_rounded(self.rect, 0.2, 8, glow_color);
        }

        // Draw border
        let border_color = if self.pressed {
            Color::WHITE
        } else {
            Color::new(100, 100, 100, 255)
        };
        d.draw_rectangle_rounded_lines(self.rect, 0.2, 8, border_color);

        // Draw button label
        let label = if self.index == 8 { "L" } else { "R" };
        let label_size = 16;
        let label_width = d.measure_text(label, label_size);
        let label_x = self.rect.x as i32 + (self.rect.width as i32 - label_width) / 2;
        let label_y = self.rect.y as i32 + (self.rect.height as i32 - label_size) / 2 + 8;
        d.draw_text(label, label_x, label_y, label_size, Color::LIGHTGRAY);
    }
}

/// Info bar LCD display for Stream Deck Neo
pub struct InfoBar {
    /// Rectangle for the info bar area
    rect: Rectangle,
    /// Raylib texture for the info bar image
    texture: Option<Texture2D>,
    /// Image buffer (248x58 RGBA) for the LCD content
    image_buffer: Vec<u8>,
    /// Whether the buffer has been modified and needs texture update
    buffer_dirty: bool,
}

impl InfoBar {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        // Initialize image buffer as black (RGBA)
        let image_buffer = vec![0u8; INFO_BAR_WIDTH * INFO_BAR_HEIGHT * 4];
        Self {
            rect: Rectangle::new(x, y, width, height),
            texture: None,
            image_buffer,
            buffer_dirty: false,
        }
    }

    /// Check if a point is within the info bar
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.width
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.height
    }

    /// Update the info bar image from JPEG data
    /// Neo info bar receives full-screen updates (no segments like Plus)
    pub fn update_image(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        image_data: &[u8],
    ) {
        if image_data.len() < 2 {
            return;
        }

        // Check JPEG magic
        if image_data[0] != 0xFF || image_data[1] != 0xD8 {
            log::warn!("Info bar image is not JPEG format");
            return;
        }

        // Decode the JPEG
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
                log::warn!("Failed to load info bar JPEG ({} bytes)", image_data.len());
                return;
            }
            img
        };

        let img_width = loaded_image.width as usize;
        let img_height = loaded_image.height as usize;

        log::debug!("Info bar image decoded: {}x{}", img_width, img_height);

        // Convert to RGBA format if needed (format 7 = PIXELFORMAT_UNCOMPRESSED_R8G8B8A8)
        if loaded_image.format != 7 {
            unsafe {
                raylib::ffi::ImageFormat(&mut loaded_image, 7);
            }
        }

        // Neo info bar images are rotated 180° (upside down)
        // Rotate by reversing all pixels (same as image.rs approach for MK2/XL/Neo)
        let pixel_count = img_width.min(INFO_BAR_WIDTH) * img_height.min(INFO_BAR_HEIGHT);
        let bytes_per_pixel = 4; // RGBA

        unsafe {
            let src_data = loaded_image.data as *const u8;
            let copy_width = img_width.min(INFO_BAR_WIDTH);
            let copy_height = img_height.min(INFO_BAR_HEIGHT);

            // 180° rotation = reverse all pixels
            for row in 0..copy_height {
                for col in 0..copy_width {
                    // Source: read forward
                    let src_idx = (row * img_width + col) * bytes_per_pixel;
                    // Dest: write reversed (180° rotation)
                    let dst_row = copy_height - 1 - row;
                    let dst_col = copy_width - 1 - col;
                    let dst_idx = (dst_row * INFO_BAR_WIDTH + dst_col) * bytes_per_pixel;

                    self.image_buffer[dst_idx] = *src_data.add(src_idx);
                    self.image_buffer[dst_idx + 1] = *src_data.add(src_idx + 1);
                    self.image_buffer[dst_idx + 2] = *src_data.add(src_idx + 2);
                    self.image_buffer[dst_idx + 3] = *src_data.add(src_idx + 3);
                }
            }

            raylib::ffi::UnloadImage(loaded_image);
        }

        self.buffer_dirty = true;
        self.rebuild_texture(rl, thread);
    }

    /// Rebuild the texture from the buffer
    fn rebuild_texture(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        if !self.buffer_dirty {
            return;
        }

        // Create a Raylib image from our buffer
        let image = unsafe {
            let img =
                Image::gen_image_color(INFO_BAR_WIDTH as i32, INFO_BAR_HEIGHT as i32, Color::BLACK);
            std::ptr::copy_nonoverlapping(
                self.image_buffer.as_ptr(),
                img.data as *mut u8,
                self.image_buffer.len(),
            );
            img
        };

        // Drop old texture and create new one
        self.texture = None;
        self.texture = rl.load_texture_from_image(thread, &image).ok();
        self.buffer_dirty = false;

        log::debug!("Info bar texture rebuilt");
    }

    /// Draw the info bar
    pub fn draw(&self, d: &mut RaylibDrawHandle) {
        // Draw background
        d.draw_rectangle_rounded(self.rect, 0.1, 4, Color::new(20, 20, 20, 255));

        // Draw the LCD image if we have one
        if let Some(ref texture) = self.texture {
            // Scale to fit the display rect
            let source = Rectangle::new(0.0, 0.0, INFO_BAR_WIDTH as f32, INFO_BAR_HEIGHT as f32);
            let dest = self.rect;
            d.draw_texture_pro(
                texture,
                source,
                dest,
                Vector2::new(0.0, 0.0),
                0.0,
                Color::WHITE,
            );
        } else {
            // Draw placeholder text
            let text = "Info Bar";
            let text_size = 14;
            let text_width = d.measure_text(text, text_size);
            let text_x = self.rect.x as i32 + (self.rect.width as i32 - text_width) / 2;
            let text_y = self.rect.y as i32 + (self.rect.height as i32 - text_size) / 2;
            d.draw_text(text, text_x, text_y, text_size, Color::GRAY);
        }

        // Draw border
        d.draw_rectangle_rounded_lines(self.rect, 0.1, 4, Color::new(60, 60, 60, 255));
    }
}
