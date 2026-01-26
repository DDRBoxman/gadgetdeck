//! Button UI for gadgetdeck-display

use gadgetdeck::StreamDeckModel;
use raylib::prelude::*;

use crate::image::parse_image_to_raylib;

/// Button display state
pub struct Button {
    pub rect: Rectangle,
    pub pressed: bool,
    pub index: usize,
    /// Raylib texture for the button image (None = no image received)
    texture: Option<Texture2D>,
    /// Raw image data for reloading texture if needed
    image_data: Option<Vec<u8>>,
    /// Button size for this button
    size: i32,
    /// Corner radius for rendering
    corner_radius: f32,
    /// Image size from the device
    image_size: i32,
    /// Device model (for rotation handling)
    model: StreamDeckModel,
}

impl Button {
    pub fn new(x: f32, y: f32, index: usize, size: i32, corner_radius: f32, image_size: i32, model: StreamDeckModel) -> Self {
        Self {
            rect: Rectangle::new(x, y, size as f32, size as f32),
            pressed: false,
            index,
            texture: None,
            image_data: None,
            size,
            corner_radius,
            image_size,
            model,
        }
    }
    
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x 
            && x <= self.rect.x + self.rect.width
            && y >= self.rect.y 
            && y <= self.rect.y + self.rect.height
    }
    
    /// Update the button's image from image data (BMP or JPEG depending on device)
    pub fn update_image(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, image_data: &[u8]) {
        // Stream Deck sends images in device-specific formats:
        // - Mini: 80x80 BMP images in BGR format, rotated 90° counter-clockwise
        // - MK.2/XL/Plus: JPEG images, rotated 180°
        // We need to parse and transform based on the format
        
        // Log diagnostic info for the image data
        let header_hex: String = image_data.iter().take(16)
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        log::info!(
            "Button {} update_image: {} bytes, header: {}",
            self.index, image_data.len(), header_hex
        );
        
        if let Some(image) = parse_image_to_raylib(image_data, self.image_size, self.model) {
            // Drop old texture if present (it will be cleaned up when dropped)
            self.texture = None;
            
            // Load new texture from image
            let texture = rl.load_texture_from_image(thread, &image).ok();
            self.texture = texture;
            self.image_data = Some(image_data.to_vec());
            
            log::info!("Button {} image updated, texture created", self.index);
        } else {
            log::warn!("Failed to parse image data for button {}", self.index);
        }
    }
    
    /// Draw the button
    pub fn draw(&self, d: &mut RaylibDrawHandle) {
        // Only draw background when pressed (no grey background when idle)
        if self.pressed {
            d.draw_rectangle_rounded(
                self.rect,
                self.corner_radius / self.size as f32,
                8,
                Color::DARKBLUE,
            );
        }
        
        // Draw texture if we have one, otherwise draw placeholder
        if let Some(ref texture) = self.texture {
            // Calculate position to center the image in the button
            // Scale based on actual texture dimensions to fit in button
            let tex_width = texture.width as f32;
            let tex_height = texture.height as f32;
            let tex_size = tex_width.max(tex_height);
            let scale = (self.size - 20) as f32 / tex_size.max(1.0);
            let img_width = tex_width * scale;
            let img_height = tex_height * scale;
            let img_x = self.rect.x + (self.size as f32 - img_width) / 2.0;
            let img_y = self.rect.y + (self.size as f32 - img_height) / 2.0;
            
            // Use draw_texture_pro for precise source/dest rectangle control
            let source_rect = Rectangle::new(0.0, 0.0, tex_width, tex_height);
            let dest_rect = Rectangle::new(img_x, img_y, img_width, img_height);
            
            d.draw_texture_pro(
                texture,
                source_rect,
                dest_rect,
                Vector2::new(0.0, 0.0),
                0.0,
                Color::WHITE,
            );
        } else {
            // Draw button number as placeholder
            let label = format!("{}", self.index + 1);
            let text_size = (self.size / 4).max(20).min(48);
            let text_width = d.measure_text(&label, text_size);
            let text_x = self.rect.x as i32 + (self.size - text_width) / 2;
            let text_y = self.rect.y as i32 + (self.size - text_size) / 2;
            d.draw_text(
                &label,
                text_x,
                text_y,
                text_size,
                Color::LIGHTGRAY,
            );
        }
    }
}
