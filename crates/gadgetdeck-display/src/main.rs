//! Display Binary for GadgetDeck
//!
//! This binary provides a graphical display for the GadgetDeck using Raylib.
//! It renders button icons to the framebuffer, handles touch input, and
//! emulates various Stream Deck USB devices.
//!
//! ## Features
//! - Supports multiple Stream Deck models: Mini, Pedal, MK.2, XL
//! - Dynamically adjusts button grid layout based on device type
//! - Displays images received from host software (BMP or JPEG)
//! - Sends button press/release events via USB HID
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
//! # Emulate Stream Deck MK.2 with custom screen size
//! gadgetdeck-display --device mk2 -W 1920 -H 1080
//! ```

use clap::{Parser, ValueEnum};
use gadgetdeck::{
    ButtonState, GadgetDeck, GadgetDeckConfig, ImageEvent, StreamDeckModel,
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
}

impl From<DeviceType> for StreamDeckModel {
    fn from(device: DeviceType) -> Self {
        match device {
            DeviceType::Mini => StreamDeckModel::Mini,
            DeviceType::Pedal => StreamDeckModel::Pedal,
            DeviceType::Mk2 => StreamDeckModel::Mk2,
            DeviceType::Xl => StreamDeckModel::Xl,
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
}

impl DeviceLayout {
    fn from_model(model: StreamDeckModel, screen_width: i32, screen_height: i32) -> Self {
        let (cols, rows) = model.key_matrix();
        let cols = cols as usize;
        let rows = rows as usize;
        let button_count = model.key_count() as usize;
        let (img_w, _img_h) = model.key_image_size();
        let image_size = img_w as i32;
        
        // Calculate optimal button size based on screen dimensions and grid
        // Leave some margin for spacing and status bar
        let available_width = screen_width - 100;  // 50px margin on each side
        let available_height = screen_height - 140; // 50px top, 40px status bar, 50px bottom
        
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
}

impl Button {
    fn new(x: f32, y: f32, index: usize, size: i32, corner_radius: f32, image_size: i32) -> Self {
        Self {
            rect: Rectangle::new(x, y, size as f32, size as f32),
            pressed: false,
            index,
            texture: None,
            image_data: None,
            size,
            corner_radius,
            image_size,
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
        // - MK.2/XL: JPEG images, rotated 180°
        // We need to parse and transform based on the format
        
        if let Some(image) = parse_image_to_raylib(image_data, self.image_size) {
            // Drop old texture if present (it will be cleaned up when dropped)
            self.texture = None;
            
            // Load new texture from image
            let texture = rl.load_texture_from_image(thread, &image).ok();
            self.texture = texture;
            self.image_data = Some(image_data.to_vec());
            
            log::debug!("Button {} image updated", self.index);
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
            // Scale the image to fit nicely in the button
            let scale = (self.size - 20) as f32 / self.image_size.max(1) as f32;
            let img_size = (self.image_size as f32 * scale) as i32;
            let img_x = self.rect.x as i32 + (self.size - img_size) / 2;
            let img_y = self.rect.y as i32 + (self.size - img_size) / 2;
            
            // Draw the texture scaled
            d.draw_texture_ex(
                texture,
                Vector2::new(img_x as f32, img_y as f32),
                0.0,
                scale,
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
fn parse_image_to_raylib(image_data: &[u8], expected_size: i32) -> Option<Image> {
    if image_data.len() < 2 {
        log::warn!("Image data too short: {} bytes", image_data.len());
        return None;
    }
    
    // Detect image format by magic bytes
    if image_data[0] == b'B' && image_data[1] == b'M' {
        // BMP format (Stream Deck Mini)
        parse_bmp_to_image(image_data, expected_size)
    } else if image_data[0] == 0xFF && image_data[1] == 0xD8 {
        // JPEG format (Stream Deck MK.2, XL)
        parse_jpeg_to_image(image_data, expected_size)
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
/// Stream Deck MK.2/XL send JPEG images that need 180° rotation
fn parse_jpeg_to_image(jpeg_data: &[u8], _expected_size: i32) -> Option<Image> {
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
    
    // Stream Deck MK.2/XL images are rotated 180°
    // Rotate by reversing all pixels
    let pixel_count = width * height;
    let bytes_per_pixel = 4; // RGBA after conversion
    let data_size = pixel_count * bytes_per_pixel;
    
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
}

/// Application state
struct App {
    buttons: Vec<Button>,
    touch_active: bool,
    touch_position: Vector2,
    last_pressed: Option<usize>,
    /// USB button state (shared with USB thread)
    button_state: Arc<ButtonState>,
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
}

impl App {
    fn new(button_state: Arc<ButtonState>, model: StreamDeckModel, screen_width: i32, screen_height: i32) -> Self {
        // Calculate layout based on device model
        let layout = DeviceLayout::from_model(model, screen_width, screen_height);
        
        // Calculate grid position to center buttons
        let grid_width = (layout.cols as i32 * layout.button_size) + ((layout.cols as i32 - 1) * layout.button_spacing);
        let grid_height = (layout.rows as i32 * layout.button_size) + ((layout.rows as i32 - 1) * layout.button_spacing);
        let start_x = (screen_width - grid_width) / 2;
        let start_y = (screen_height - grid_height) / 2;
        
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
                ));
            }
        }
        
        Self {
            buttons,
            touch_active: false,
            touch_position: Vector2::new(0.0, 0.0),
            last_pressed: None,
            button_state,
            status_msg: "Waiting for host connection...".to_string(),
            images_received: 0,
            screen_width,
            screen_height,
            layout,
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
        }
    }
    
    fn draw(&self, d: &mut RaylibDrawHandle) {
        d.clear_background(Color::BLACK);
        
        // Draw buttons
        for button in &self.buttons {
            button.draw(d);
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
        
        // Button status on right
        let button_status = if let Some(btn) = self.last_pressed {
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
    
    let mut app = App::new(button_state, model, args.width, args.height);
    
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
