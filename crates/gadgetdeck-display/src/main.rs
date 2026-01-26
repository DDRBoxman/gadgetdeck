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

mod app;
mod button;
mod cli;
mod image;
mod knob;
mod layout;
mod touchscreen;

use clap::Parser;
use gadgetdeck::{GadgetDeck, GadgetDeckConfig, StreamDeckModel};
use std::sync::atomic::Ordering;

use app::App;
use cli::Args;

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
