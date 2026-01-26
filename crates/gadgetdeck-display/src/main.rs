//! Display Binary for GadgetDeck
//!
//! This binary provides a graphical display for the GadgetDeck using Raylib.
//! It renders button icons to the framebuffer, handles touch input, and
//! emulates various Stream Deck USB devices.
//!
//! ## Features
//! - Supports multiple Stream Deck models: Mini, Pedal, MK.2, XL, Plus
//! - Dynamically adjusts button grid layout based on device type
//! - Displays images received from host software (BMP or JPEG)
//! - Sends button press/release events via USB HID
//! - Stream Deck Plus support with 4 rotary knobs and touchscreen strip
//! - Designed to run without a desktop environment on Raspberry Pi
//!
//! ## Usage
//! ```sh
//! # Emulate Stream Deck Mini (default)
//! gadgetdeck-display
//!
//! # Emulate Stream Deck XL with 8x4 button grid
//! gadgetdeck-display --device xl
//!
//! # Emulate Stream Deck Plus with 4x2 buttons, 4 knobs, and touchscreen
//! gadgetdeck-display --device plus
//!
//! # Emulate Stream Deck MK.2 with custom screen size
//! gadgetdeck-display --device mk2 -W 1920 -H 1080
//! ```

use clap::{Parser, ValueEnum};
use gadgetdeck::{
    ButtonState, GadgetDeck, GadgetDeckConfig, ImageEvent, StreamDeckModel,
    PlusInputState, KnobIndex,
};
use raylib::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// CLI arguments for gadgetdeck-display
#[derive(Parser, Debug)]
#[command(name = "gadgetdeck-display")]
#[command(about = "GadgetDeck Display - Stream Deck emulator with touchscreen UI")]
struct Args {
    /// Device type to emulate
    #[arg(short, long, value_enum, default_value_t = DeviceType::Mini)]
    device: DeviceType,

    /// Serial number (overrides GADGETDECK_SERIAL env var)
    #[arg(short, long)]
    serial: Option<String>,

    /// Screen width in pixels
    #[arg(short = 'W', long, default_value_t = 1600)]
    width: i32,

    /// Screen height in pixels
    #[arg(short = 'H', long, default_value_t = 600)]
    height: i32,
}

/// Device type enum for CLI
#[derive(Debug, Clone, Copy, ValueEnum)]
enum DeviceType {
    /// Stream Deck Mini (6 keys, 3x2 layout)
    Mini,
    /// Stream Deck Pedal (3 foot pedals)
    Pedal,
    /// Stream Deck MK.2 (15 keys, 5x3 layout)
    Mk2,
    /// Stream Deck XL (32 keys, 8x4 layout)
    Xl,
    /// Stream Deck Plus (8 keys, 4 knobs, touchscreen)
    Plus,
}

impl From<DeviceType> for StreamDeckModel {
    fn from(device: DeviceType) -> Self {
        match device {
            DeviceType::Mini => StreamDeckModel::Mini,
            DeviceType::Pedal => StreamDeckModel::Pedal,
            DeviceType::Mk2 => StreamDeckModel::Mk2,
            DeviceType::Xl => StreamDeckModel::Xl,
            DeviceType::Plus => StreamDeckModel::Plus,
        }
    }
}

/// Device layout configuration derived from StreamDeckModel
struct DeviceLayout {
    /// Number of button columns
    cols: usize,
    /// Number of button rows
    rows: usize,
    /// Total button count
    button_count: usize,
    /// Button size in pixels (for display)
    button_size: i32,
    /// Spacing between buttons
    button_spacing: i32,
    /// Corner radius for button rendering
    corner_radius: f32,
    /// Original image size from the device
    image_size: i32,
    /// Whether this device has knobs (Plus only)
    has_knobs: bool,
    /// Number of knobs (4 for Plus, 0 for others)
    knob_count: usize,
    /// Whether this device has a touchscreen strip (Plus only)
    has_touchscreen: bool,
    /// Touchscreen width in pixels (800 for Plus)
    touchscreen_width: i32,
    /// Touchscreen height in pixels (100 for Plus)
    touchscreen_height: i32,
}

impl DeviceLayout {
    fn from_model(model: StreamDeckModel, screen_width: i32, screen_height: i32) -> Self {
        let (cols, rows) = model.key_matrix();
        let cols = cols as usize;
        let rows = rows as usize;
        let button_count = model.key_count() as usize;
        let (img_w, _img_h) = model.key_image_size();
        let image_size = img_w as i32;
        
        // Plus-specific features
        let is_plus = matches!(model, StreamDeckModel::Plus);
        let has_knobs = is_plus;
        let knob_count = if is_plus { 4 } else { 0 };
        let has_touchscreen = is_plus;
        let touchscreen_width = if is_plus { 800 } else { 0 };
        let touchscreen_height = if is_plus { 100 } else { 0 };
        
        // Calculate optimal button size based on screen dimensions and grid
        // Leave some margin for spacing and status bar
        // For Plus, also reserve space for knobs below buttons and touchscreen at bottom
        let knob_area_height = if has_knobs { 120 } else { 0 };  // Space for knob UI
        let touchscreen_area_height = if has_touchscreen { 120 } else { 0 };  // Space for touchscreen strip
        
        let available_width = screen_width - 100;  // 50px margin on each side
        let available_height = screen_height - 140 - knob_area_height - touchscreen_area_height; // 50px top, 40px status bar, 50px bottom
        
        // Calculate button size that fits the grid
        let max_button_width = (available_width - (cols as i32 - 1) * 30) / cols as i32;
        let max_button_height = (available_height - (rows as i32 - 1) * 30) / rows as i32;
        
        // Use the smaller dimension to keep buttons square
        let button_size = max_button_width.min(max_button_height).min(200); // Cap at 200px
        let button_spacing = (button_size / 4).max(20).min(50); // Proportional spacing
        
        Self {
            cols,
            rows,
            button_count,
            button_size,
            button_spacing,
            corner_radius: (button_size as f32 * 0.1).min(20.0),
            image_size,
            has_knobs,
            knob_count,
            has_touchscreen,
            touchscreen_width,
            touchscreen_height,
        }
    }
}

/// Knob (rotary encoder) display state for Stream Deck Plus
struct Knob {
    /// Center position
    center: Vector2,
    /// Radius for the knob
    radius: f32,
    /// Knob index (0-3 for A-D)
    index: usize,
    /// Whether the knob is currently pressed
    pressed: bool,
    /// Current rotation value (for visual feedback)
    rotation: f32,
    /// Label for the knob
    label: String,
    /// Whether the knob is being dragged for rotation
    is_dragging: bool,
    /// X position where drag started
    drag_start_x: f32,
    /// Last X position during drag (for incremental updates)
    drag_last_x: f32,
    /// Accumulated drag distance for visual feedback (resets periodically)
    drag_indicator: f32,
}

impl Knob {
    fn new(center_x: f32, center_y: f32, radius: f32, index: usize) -> Self {
        let labels = ["A", "B", "C", "D"];
        Self {
            center: Vector2::new(center_x, center_y),
            radius,
            index,
            pressed: false,
            rotation: 0.0,
            label: labels.get(index).unwrap_or(&"?").to_string(),
            is_dragging: false,
            drag_start_x: 0.0,
            drag_last_x: 0.0,
            drag_indicator: 0.0,
        }
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        let dx = x - self.center.x;
        let dy = y - self.center.y;
        (dx * dx + dy * dy) <= (self.radius * self.radius)
    }

    /// Draw the knob
    fn draw(&self, d: &mut RaylibDrawHandle) {
        let base_color = if self.is_dragging {
            Color::new(80, 120, 200, 255)  // Highlight when dragging
        } else if self.pressed { 
            Color::new(60, 100, 180, 255) 
        } else { 
            Color::new(50, 50, 60, 255) 
        };

        // Outer ring
        d.draw_circle_v(self.center, self.radius, Color::new(80, 80, 90, 255));
        
        // Inner knob body
        d.draw_circle_v(self.center, self.radius * 0.85, base_color);
        
        // Rotation indicator line
        let indicator_length = self.radius * 0.6;
        let angle_rad = self.rotation.to_radians();
        let indicator_end = Vector2::new(
            self.center.x + angle_rad.sin() * indicator_length,
            self.center.y - angle_rad.cos() * indicator_length,
        );
        d.draw_line_ex(
            self.center,
            indicator_end,
            3.0,
            Color::WHITE,
        );
        
        // Draw drag direction indicator when dragging
        if self.is_dragging && self.drag_indicator.abs() > 5.0 {
            let arrow_y = self.center.y;
            let arrow_length = (self.drag_indicator.abs() * 0.5).min(self.radius * 0.8);
            let arrow_color = if self.drag_indicator > 0.0 {
                Color::new(100, 255, 100, 200)  // Green for clockwise (right)
            } else {
                Color::new(255, 100, 100, 200)  // Red for counter-clockwise (left)
            };
            
            if self.drag_indicator > 0.0 {
                // Right arrow (clockwise)
                let arrow_start = Vector2::new(self.center.x, arrow_y);
                let arrow_end = Vector2::new(self.center.x + arrow_length, arrow_y);
                d.draw_line_ex(arrow_start, arrow_end, 4.0, arrow_color);
                // Arrowhead
                d.draw_triangle(
                    Vector2::new(arrow_end.x + 8.0, arrow_y),
                    Vector2::new(arrow_end.x - 4.0, arrow_y - 6.0),
                    Vector2::new(arrow_end.x - 4.0, arrow_y + 6.0),
                    arrow_color,
                );
            } else {
                // Left arrow (counter-clockwise)
                let arrow_start = Vector2::new(self.center.x, arrow_y);
                let arrow_end = Vector2::new(self.center.x - arrow_length, arrow_y);
                d.draw_line_ex(arrow_start, arrow_end, 4.0, arrow_color);
                // Arrowhead
                d.draw_triangle(
                    Vector2::new(arrow_end.x - 8.0, arrow_y),
                    Vector2::new(arrow_end.x + 4.0, arrow_y - 6.0),
                    Vector2::new(arrow_end.x + 4.0, arrow_y + 6.0),
                    arrow_color,
                );
            }
        }
        
        // Draw label below
        let label_y = self.center.y as i32 + self.radius as i32 + 5;
        let text_size = 16;
        let text_width = d.measure_text(&self.label, text_size);
        d.draw_text(
            &self.label,
            self.center.x as i32 - text_width / 2,
            label_y,
            text_size,
            Color::LIGHTGRAY,
        );
        
        // Border - highlight when dragging
        let border_color = if self.is_dragging {
            Color::new(100, 200, 255, 255)  // Cyan when dragging
        } else if self.pressed { 
            Color::WHITE 
        } else { 
            Color::GRAY 
        };
        d.draw_circle_lines(self.center.x as i32, self.center.y as i32, self.radius, border_color);
    }
    
    /// Turn the knob (positive = clockwise, negative = counter-clockwise)
    fn turn(&mut self, delta: f32) {
        self.rotation = (self.rotation + delta) % 360.0;
        if self.rotation < 0.0 {
            self.rotation += 360.0;
        }
    }
    
    /// Start a drag operation
    fn start_drag(&mut self, x: f32) {
        self.is_dragging = true;
        self.drag_start_x = x;
        self.drag_last_x = x;
        self.drag_indicator = 0.0;
    }
    
    /// Update drag and return the number of steps to send (if threshold crossed)
    /// Each step represents ~20 pixels of horizontal drag
    fn update_drag(&mut self, x: f32) -> i8 {
        if !self.is_dragging {
            return 0;
        }
        
        let delta = x - self.drag_last_x;
        self.drag_indicator += delta;
        
        // Calculate steps based on accumulated movement since last step sent
        // ~20 pixels per step
        const PIXELS_PER_STEP: f32 = 20.0;
        let accumulated = x - self.drag_start_x;
        let total_steps = (accumulated / PIXELS_PER_STEP) as i32;
        let last_steps = ((self.drag_last_x - self.drag_start_x) / PIXELS_PER_STEP) as i32;
        let new_steps = (total_steps - last_steps) as i8;
        
        if new_steps != 0 {
            self.drag_last_x = x;
            // Update visual rotation
            self.turn(new_steps as f32 * 15.0);  // 15 degrees per step
        }
        
        new_steps
    }
    
    /// End a drag operation, returns true if it was a short tap (no significant movement)
    fn end_drag(&mut self) -> bool {
        let was_tap = self.is_dragging && (self.drag_last_x - self.drag_start_x).abs() < 10.0;
        self.is_dragging = false;
        self.drag_indicator = 0.0;
        was_tap
    }
}

/// Detected touch gesture on the touchscreen strip
#[derive(Debug, Clone, Copy)]
enum TouchGesture {
    /// Short tap at coordinates
    ShortTap { x: u16, y: u16 },
    /// Long press at coordinates
    LongPress { x: u16, y: u16 },
    /// Drag/swipe from start to end coordinates
    Drag { start_x: u16, start_y: u16, end_x: u16, end_y: u16 },
}

/// Touchscreen strip display state for Stream Deck Plus
struct TouchscreenStrip {
    /// Rectangle for the touchscreen area
    rect: Rectangle,
    /// Raylib texture for the touchscreen image
    texture: Option<Texture2D>,
    /// Composite image buffer (800x100 RGBA) for assembling segments
    composite_buffer: Vec<u8>,
    /// Whether the composite buffer has been modified and needs texture update
    buffer_dirty: bool,
    /// Last touch position (if touched)
    touch_position: Option<Vector2>,
    /// Touch start position for swipe detection (relative coords)
    touch_start: Option<(u16, u16)>,
    /// Touch start time for distinguishing tap vs long press
    touch_start_time: Option<std::time::Instant>,
    /// Whether a touch is currently active
    is_touching: bool,
}

/// Stream Deck Plus touchscreen dimensions
const LCD_WIDTH: usize = 800;
const LCD_HEIGHT: usize = 100;

/// Minimum distance (in pixels) to consider a movement as a swipe
const SWIPE_MIN_DISTANCE: u16 = 50;
/// Maximum time (in ms) for a short tap
const SHORT_TAP_MAX_MS: u128 = 200;
/// Minimum time (in ms) for a long press
const LONG_PRESS_MIN_MS: u128 = 500;

impl TouchscreenStrip {
    fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
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

    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.width
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.height
    }

    /// Get touch coordinates relative to the strip (0-799, 0-99)
    fn get_relative_touch(&self, x: f32, y: f32) -> (u16, u16) {
        let rel_x = ((x - self.rect.x) / self.rect.width * 800.0).clamp(0.0, 799.0) as u16;
        let rel_y = ((y - self.rect.y) / self.rect.height * 100.0).clamp(0.0, 99.0) as u16;
        (rel_x, rel_y)
    }
    
    /// Handle touch start event
    fn on_touch_start(&mut self, rel_x: u16, rel_y: u16) {
        self.touch_start = Some((rel_x, rel_y));
        self.touch_start_time = Some(std::time::Instant::now());
        self.is_touching = true;
    }
    
    /// Handle touch end event, returns the detected gesture if any
    fn on_touch_end(&mut self, rel_x: u16, rel_y: u16) -> Option<TouchGesture> {
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
    fn on_touch_cancel(&mut self) {
        self.touch_start = None;
        self.touch_start_time = None;
        self.is_touching = false;
        self.touch_position = None;
    }

    /// Update a segment of the touchscreen image at the given x_offset, y_offset
    fn update_image(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, image_data: &[u8], x_offset: u16, y_offset: u16, width: u16, height: u16) {
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
    fn draw(&self, d: &mut RaylibDrawHandle) {
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

/// Button display state
struct Button {
    rect: Rectangle,
    pressed: bool,
    index: usize,
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
    fn new(x: f32, y: f32, index: usize, size: i32, corner_radius: f32, image_size: i32, model: StreamDeckModel) -> Self {
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
    
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect.x 
            && x <= self.rect.x + self.rect.width
            && y >= self.rect.y 
            && y <= self.rect.y + self.rect.height
    }
    
    /// Update the button's image from image data (BMP or JPEG depending on device)
    fn update_image(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, image_data: &[u8]) {
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
    fn draw(&self, d: &mut RaylibDrawHandle) {
        let base_color = if self.pressed { Color::DARKBLUE } else { Color::DARKGRAY };
        
        // Button background with rounded corners
        d.draw_rectangle_rounded(
            self.rect,
            self.corner_radius / self.size as f32,
            8,
            base_color,
        );
        
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
        
        // Button border - brighter when pressed
        let border_color = if self.pressed { Color::WHITE } else { Color::GRAY };
        d.draw_rectangle_rounded_lines(
            self.rect,
            self.corner_radius / self.size as f32,
            8,
            border_color,
        );
    }
}

/// Parse an image (BMP or JPEG) and convert to Raylib Image
/// - Stream Deck Mini: 80x80 24-bit BMP, rotated 90° CCW
/// - Stream Deck MK.2/XL: JPEG, rotated 180°
/// - Stream Deck Plus: JPEG, no rotation needed
fn parse_image_to_raylib(image_data: &[u8], expected_size: i32, model: StreamDeckModel) -> Option<Image> {
    if image_data.len() < 2 {
        log::warn!("Image data too short: {} bytes", image_data.len());
        return None;
    }
    
    // Determine if this model needs 180° rotation
    // Note: Plus does NOT need rotation unlike MK.2/XL
    let needs_rotation = matches!(model, StreamDeckModel::Mk2 | StreamDeckModel::Xl);
    
    // Detect image format by magic bytes
    if image_data[0] == b'B' && image_data[1] == b'M' {
        // BMP format (Stream Deck Mini)
        parse_bmp_to_image(image_data, expected_size)
    } else if image_data[0] == 0xFF && image_data[1] == 0xD8 {
        // JPEG format (Stream Deck MK.2, XL, Plus)
        parse_jpeg_to_image(image_data, expected_size, needs_rotation)
    } else {
        log::warn!("Unknown image format: {:02X} {:02X}", image_data[0], image_data[1]);
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
    
    let pixel_offset = u32::from_le_bytes([bmp_data[10], bmp_data[11], bmp_data[12], bmp_data[13]]) as usize;
    let width = i32::from_le_bytes([bmp_data[18], bmp_data[19], bmp_data[20], bmp_data[21]]);
    let height = i32::from_le_bytes([bmp_data[22], bmp_data[23], bmp_data[24], bmp_data[25]]);
    let bits_per_pixel = u16::from_le_bytes([bmp_data[28], bmp_data[29]]);
    
    log::debug!("BMP: {}x{}, {} bpp, offset {}", width, height, bits_per_pixel, pixel_offset);
    
    if width != expected_size || height.abs() != expected_size {
        log::warn!("Unexpected BMP size: {}x{} (expected {}x{})", 
            width, height, expected_size, expected_size);
    }
    
    if bits_per_pixel != 24 {
        log::warn!("Unexpected bits per pixel: {} (expected 24)", bits_per_pixel);
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
            let new_col = row;  // flipped from (abs_height - 1 - row)
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
        std::ptr::copy_nonoverlapping(
            rgba_pixels.as_ptr(),
            img.data as *mut u8,
            rgba_pixels.len(),
        );
        img
    };
    
    Some(image)
}

/// Parse a JPEG image and convert to Raylib Image
/// Stream Deck MK.2/XL/Plus send JPEG images that need 180° rotation
fn parse_jpeg_to_image(jpeg_data: &[u8], _expected_size: i32, rotate_180: bool) -> Option<Image> {
    // Log the first few bytes to verify it's valid JPEG data
    let display_len = jpeg_data.len().min(16);
    let hex_str: String = jpeg_data[..display_len].iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ");
    log::debug!("JPEG data ({} bytes): {}", jpeg_data.len(), hex_str);
    
    // Verify JPEG magic bytes
    if jpeg_data.len() < 2 || jpeg_data[0] != 0xFF || jpeg_data[1] != 0xD8 {
        log::warn!("Invalid JPEG magic bytes: expected FF D8, got {:02X} {:02X}", 
            jpeg_data.get(0).unwrap_or(&0), jpeg_data.get(1).unwrap_or(&0));
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
            log::warn!("Failed to load JPEG image from memory ({} bytes)", jpeg_data.len());
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

/// Application state
struct App {
    buttons: Vec<Button>,
    /// Knobs for Stream Deck Plus
    knobs: Vec<Knob>,
    /// Touchscreen strip for Stream Deck Plus
    touchscreen: Option<TouchscreenStrip>,
    touch_active: bool,
    touch_position: Vector2,
    last_pressed: Option<usize>,
    /// Last pressed knob index
    last_knob_pressed: Option<usize>,
    /// Index of knob currently being dragged for rotation
    dragging_knob: Option<usize>,
    /// USB button state (shared with USB thread)
    button_state: Arc<ButtonState>,
    /// Plus-specific input state (touchscreen/knobs)
    plus_state: Option<Arc<PlusInputState>>,
    /// Connection status message
    status_msg: String,
    /// Number of images received
    images_received: u64,
    /// Screen width
    screen_width: i32,
    /// Screen height
    screen_height: i32,
    /// Device layout info
    layout: DeviceLayout,
    /// Device model
    model: StreamDeckModel,
}

impl App {
    fn new(button_state: Arc<ButtonState>, plus_state: Option<Arc<PlusInputState>>, model: StreamDeckModel, screen_width: i32, screen_height: i32) -> Self {
        // Calculate layout based on device model
        let layout = DeviceLayout::from_model(model, screen_width, screen_height);
        
        // Calculate grid position to center buttons
        // For Plus, we offset buttons upward to make room for knobs and touchscreen
        let grid_width = (layout.cols as i32 * layout.button_size) + ((layout.cols as i32 - 1) * layout.button_spacing);
        let grid_height = (layout.rows as i32 * layout.button_size) + ((layout.rows as i32 - 1) * layout.button_spacing);
        
        let _extra_elements_height = if layout.has_knobs { 120 } else { 0 } 
            + if layout.has_touchscreen { 120 } else { 0 };
        
        let start_x = (screen_width - grid_width) / 2;
        let start_y = if layout.has_knobs || layout.has_touchscreen {
            // For Plus: position buttons with more top margin
            80  // Top margin
        } else {
            (screen_height - grid_height) / 2
        };
        
        let mut buttons = Vec::with_capacity(layout.button_count);
        
        for row in 0..layout.rows {
            for col in 0..layout.cols {
                let x = start_x + (col as i32 * (layout.button_size + layout.button_spacing));
                let y = start_y + (row as i32 * (layout.button_size + layout.button_spacing));
                let index = row * layout.cols + col;
                buttons.push(Button::new(
                    x as f32, 
                    y as f32, 
                    index, 
                    layout.button_size, 
                    layout.corner_radius,
                    layout.image_size,
                    model,
                ));
            }
        }
        
        // Create touchscreen strip for Plus (above knobs)
        // The strip is 800px wide with 4 segments of 200px each
        let touchscreen = if layout.has_touchscreen {
            let strip_y = start_y + grid_height + 30;  // Below buttons with more gap
            let strip_width = 800.0_f32.min((screen_width - 100) as f32);
            let strip_height = 100.0;  // Match actual LCD height
            let strip_x = (screen_width as f32 - strip_width) / 2.0;
            
            Some(TouchscreenStrip::new(strip_x, strip_y as f32, strip_width, strip_height))
        } else {
            None
        };
        
        // Create knobs for Plus (below touchscreen, aligned with touchscreen segments)
        // Each segment is 200px wide (800/4), so knobs go at 100, 300, 500, 700px from strip start
        let knobs = if layout.has_knobs {
            let knob_radius = 40.0;
            let knob_y = if layout.has_touchscreen {
                // Touchscreen is at start_y + grid_height + 30, height 100
                // So it ends at start_y + grid_height + 130
                // Knob center needs to be at: strip_end + gap + radius
                start_y + grid_height + 130 + 20 + knob_radius as i32  // 20px gap below strip
            } else {
                start_y + grid_height + 60 + knob_radius as i32  // Below buttons
            };
            
            // Get the touchscreen strip x position and width for alignment
            let strip_width = 800.0_f32.min((screen_width - 100) as f32);
            let strip_x = (screen_width as f32 - strip_width) / 2.0;
            let segment_width = strip_width / 4.0;  // 200px per segment (scaled)
            
            // Align each knob with the center of each touchscreen segment
            (0..layout.knob_count)
                .map(|i| {
                    // Center of segment i = strip_x + segment_width * i + segment_width/2
                    let x = strip_x + segment_width * i as f32 + segment_width / 2.0;
                    Knob::new(x, knob_y as f32, knob_radius, i)
                })
                .collect()
        } else {
            Vec::new()
        };
        
        Self {
            buttons,
            knobs,
            touchscreen,
            touch_active: false,
            touch_position: Vector2::new(0.0, 0.0),
            last_pressed: None,
            last_knob_pressed: None,
            dragging_knob: None,
            button_state,
            plus_state,
            status_msg: "Waiting for host connection...".to_string(),
            images_received: 0,
            screen_width,
            screen_height,
            layout,
            model,
        }
    }
    
    fn update(&mut self, rl: &RaylibHandle) {
        // Handle touch input
        // Raylib treats touch as mouse on Pi
        let touch_count = rl.get_touch_point_count();
        
        if touch_count > 0 {
            self.touch_active = true;
            self.touch_position = rl.get_touch_position(0);
        } else if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            // Fallback to mouse for testing on desktop
            self.touch_active = true;
            self.touch_position = rl.get_mouse_position();
        } else {
            self.touch_active = false;
        }
        
        // Update button states
        let mut current_pressed = None;
        
        for (i, button) in self.buttons.iter_mut().enumerate() {
            let was_pressed = button.pressed;
            button.pressed = self.touch_active && button.contains(self.touch_position.x, self.touch_position.y);
            
            if button.pressed {
                current_pressed = Some(i);
            }
            
            // Update USB button state on changes
            if button.pressed && !was_pressed {
                self.button_state.press(i as u8);
                log::info!("Button {} pressed (touch)", i);
            } else if !button.pressed && was_pressed {
                self.button_state.release(i as u8);
                log::info!("Button {} released (touch)", i);
            }
        }
        
        self.last_pressed = current_pressed;
        
        // Update knob states (for Plus) with drag-to-rotate support
        let mut current_knob_pressed = None;
        
        // Handle knob drag rotation
        if self.touch_active {
            // Check if we're starting a new drag on a knob
            if self.dragging_knob.is_none() {
                for (i, knob) in self.knobs.iter_mut().enumerate() {
                    if knob.contains(self.touch_position.x, self.touch_position.y) {
                        knob.start_drag(self.touch_position.x);
                        self.dragging_knob = Some(i);
                        current_knob_pressed = Some(i);
                        log::debug!("Started drag on knob {}", knob.label);
                        break;
                    }
                }
            } else if let Some(drag_idx) = self.dragging_knob {
                // Continue existing drag
                if drag_idx < self.knobs.len() {
                    let knob = &mut self.knobs[drag_idx];
                    let steps = knob.update_drag(self.touch_position.x);
                    current_knob_pressed = Some(drag_idx);
                    
                    // Send rotation events if steps crossed threshold
                    if steps != 0 {
                        let direction = if steps > 0 { "clockwise" } else { "counter-clockwise" };
                        log::info!("Knob {} turned {} ({} steps, drag)", knob.label, direction, steps.abs());
                        if let Some(ref plus) = self.plus_state {
                            plus.turn_knob(KnobIndex::from(drag_idx as u8), steps);
                        }
                    }
                }
            }
        } else {
            // Touch released - end any active drag
            if let Some(drag_idx) = self.dragging_knob.take() {
                if drag_idx < self.knobs.len() {
                    let knob = &mut self.knobs[drag_idx];
                    let was_tap = knob.end_drag();
                    
                    // If it was a tap (no significant movement), send press+release
                    if was_tap {
                        log::info!("Knob {} tapped", knob.label);
                        if let Some(ref plus) = self.plus_state {
                            plus.press_knob(KnobIndex::from(drag_idx as u8));
                            plus.release_knob(KnobIndex::from(drag_idx as u8));
                        }
                    }
                }
            }
            
            // Clear pressed state on all knobs
            for knob in self.knobs.iter_mut() {
                knob.pressed = false;
            }
        }
        
        // Update visual pressed state for the knob being interacted with
        if let Some(idx) = current_knob_pressed {
            if idx < self.knobs.len() {
                self.knobs[idx].pressed = true;
            }
        }
        
        self.last_knob_pressed = current_knob_pressed;
        
        // Update touchscreen strip (for Plus) with swipe detection
        if let Some(ref mut strip) = self.touchscreen {
            let is_over_strip = strip.contains(self.touch_position.x, self.touch_position.y);
            
            if self.touch_active && is_over_strip {
                let (rel_x, rel_y) = strip.get_relative_touch(self.touch_position.x, self.touch_position.y);
                strip.touch_position = Some(self.touch_position);
                
                // Start tracking touch if not already
                if !strip.is_touching {
                    strip.on_touch_start(rel_x, rel_y);
                    log::debug!("Touchscreen touch started at ({}, {})", rel_x, rel_y);
                }
            } else if strip.is_touching {
                // Touch ended or moved off strip
                let (rel_x, rel_y) = if is_over_strip {
                    strip.get_relative_touch(self.touch_position.x, self.touch_position.y)
                } else {
                    // Use last known position if moved off strip
                    strip.touch_start.unwrap_or((400, 50))
                };
                
                if let Some(gesture) = strip.on_touch_end(rel_x, rel_y) {
                    self.handle_touch_gesture(gesture);
                }
            } else {
                strip.touch_position = None;
            }
        }
        
        // Handle scroll wheel for knob rotation simulation (desktop testing)
        let wheel = rl.get_mouse_wheel_move();
        if wheel != 0.0 {
            // Rotate the knob that the mouse is over
            for (i, knob) in self.knobs.iter_mut().enumerate() {
                if knob.contains(self.touch_position.x, self.touch_position.y) {
                    let delta = wheel * 15.0;  // 15 degrees per scroll step
                    knob.turn(delta);
                    let steps = if wheel > 0.0 { 1i8 } else { -1i8 };
                    log::info!("Knob {} turned {} (wheel)", knob.label, if delta > 0.0 { "right" } else { "left" });
                    if let Some(ref plus) = self.plus_state {
                        plus.turn_knob(KnobIndex::from(i as u8), steps);
                    }
                    break;
                }
            }
        }
    }
    
    /// Handle a detected touch gesture on the touchscreen strip
    fn handle_touch_gesture(&self, gesture: TouchGesture) {
        log::info!("Touch gesture detected: {:?}", gesture);
        
        if let Some(ref plus) = self.plus_state {
            match gesture {
                TouchGesture::ShortTap { x, y } => {
                    plus.tap(x, y);
                }
                TouchGesture::LongPress { x, y } => {
                    plus.long_press(x, y);
                }
                TouchGesture::Drag { start_x, start_y, end_x, end_y } => {
                    plus.swipe(start_x, start_y, end_x, end_y);
                }
            }
        }
    }
    
    /// Process image events from USB
    fn process_image_event(&mut self, event: ImageEvent, rl: &mut RaylibHandle, thread: &RaylibThread) {
        match event {
            ImageEvent::Updated { key_index, image } => {
                self.images_received += 1;
                self.status_msg = format!("Connected - {} images received", self.images_received);
                
                let idx = key_index as usize;
                if idx < self.buttons.len() {
                    self.buttons[idx].update_image(rl, thread, image.as_bytes());
                    log::info!("Updated button {} image ({} bytes)", idx, image.len());
                }
            }
            ImageEvent::LcdUpdated { x_offset, y_offset, width, height, image } => {
                self.images_received += 1;
                self.status_msg = format!("Connected - {} images received", self.images_received);
                
                // Update touchscreen strip image segment
                if let Some(ref mut strip) = self.touchscreen {
                    strip.update_image(rl, thread, image.as_bytes(), x_offset, y_offset, width, height);
                    log::info!(
                        "Updated touchscreen image: x_off={}, y_off={}, {}x{}, {} bytes",
                        x_offset, y_offset, width, height, image.len()
                    );
                }
            }
        }
    }
    
    fn draw(&self, d: &mut RaylibDrawHandle) {
        d.clear_background(Color::BLACK);
        
        // Draw buttons
        for button in &self.buttons {
            button.draw(d);
        }
        
        // Draw knobs (for Plus)
        for knob in &self.knobs {
            knob.draw(d);
        }
        
        // Draw touchscreen strip (for Plus)
        if let Some(ref strip) = self.touchscreen {
            strip.draw(d);
        }
        
        // Draw touch indicator (for debugging)
        if self.touch_active {
            d.draw_circle_v(self.touch_position, 8.0, Color::RED);
        }
        
        // Draw status bar at bottom
        let status_y = self.screen_height - 40;
        d.draw_rectangle(0, status_y, self.screen_width, 40, Color::new(30, 30, 30, 255));
        
        // Status message
        d.draw_text(&self.status_msg, 20, status_y + 10, 20, Color::LIGHTGRAY);
        
        // Button/knob status on right
        let button_status = if let Some(knob_idx) = self.last_knob_pressed {
            let label = ["A", "B", "C", "D"].get(knob_idx).unwrap_or(&"?");
            format!("Knob {} active", label)
        } else if let Some(btn) = self.last_pressed {
            format!("Button {} active", btn + 1)
        } else {
            "Ready".to_string()
        };
        let status_width = d.measure_text(&button_status, 20);
        d.draw_text(&button_status, self.screen_width - status_width - 20, status_y + 10, 20, Color::LIGHTGRAY);
        
        // FPS in top-left
        d.draw_fps(10, 10);
    }
}

fn main() {
    // Parse CLI arguments
    let args = Args::parse();
    
    // Initialize logging
    env_logger::init();
    
    // Use device type from CLI argument
    let model: StreamDeckModel = args.device.into();
    let (cols, rows) = model.key_matrix();
    let button_count = model.key_count();
    
    println!("GadgetDeck Display starting...");
    println!("Screen: {}x{}", args.width, args.height);
    println!("Button grid: {}x{} ({} buttons)", cols, rows, button_count);
    
    // Print Plus-specific info
    if matches!(model, StreamDeckModel::Plus) {
        println!("Plus features: 4 knobs, 800x100 touchscreen strip");
    }
    
    let serial = args.serial
        .or_else(|| std::env::var("GADGETDECK_SERIAL").ok())
        .unwrap_or_else(|| "GDECK0000001".to_string());
    
    println!("Emulating: {} (Serial: {})", model.product_name(), serial);
    
    // Create and initialize the GadgetDeck
    println!("Setting up USB gadget...");
    let config = GadgetDeckConfig::new(model, serial);
    
    let mut deck = match GadgetDeck::new(config) {
        Ok(deck) => {
            println!("USB gadget created!");
            deck
        }
        Err(e) => {
            eprintln!("Failed to create USB gadget: {}", e);
            eprintln!("Make sure you're running on a device with USB gadget support.");
            std::process::exit(1);
        }
    };
    
    // Set up signal handler for clean shutdown
    let running = deck.running_flag();
    ctrlc::set_handler(move || {
        println!("\nReceived shutdown signal...");
        running.store(false, Ordering::SeqCst);
    }).expect("Failed to set signal handler");
    
    // Start the USB processing threads
    if let Err(e) = deck.start() {
        eprintln!("Failed to start USB gadget: {}", e);
        std::process::exit(1);
    }
    println!("USB threads started");
    
    // Get shared state
    let button_state = deck.button_state();
    let plus_state = deck.plus_state();
    let image_rx = deck.subscribe_images();
    
    // Initialize Raylib
    println!("Initializing display...");
    let window_title = format!("GadgetDeck - {}", model.product_name());
    let (mut rl, thread) = raylib::init()
        .size(args.width, args.height)
        .title(&window_title)
        .vsync()
        .build();
    
    // Set target FPS for better power efficiency
    rl.set_target_fps(60);
    
    // Optionally hide cursor for embedded display
    // rl.hide_cursor();
    
    let mut app = App::new(button_state, plus_state, model, args.width, args.height);
    
    println!("Display initialized. Touch buttons to interact.");
    println!("Connect Stream Deck software to send images.");
    println!("Press Ctrl+C to exit.");
    
    // Main loop
    while !rl.window_should_close() && deck.is_running() {
        // Process any pending image events
        while let Ok(event) = image_rx.try_recv() {
            app.process_image_event(event, &mut rl, &thread);
        }
        
        app.update(&rl);
        
        let mut d = rl.begin_drawing(&thread);
        app.draw(&mut d);
    }
    
    println!("GadgetDeck Display shutting down...");
    
    // Stop the deck (this cleans up USB gadget and waits for threads)
    deck.stop();
    
    println!("Goodbye!");
}
