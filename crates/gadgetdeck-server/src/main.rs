//! GadgetDeck Web Server Binary
//!
//! This binary sets up a USB gadget emulating a Stream Deck and provides
//! a web interface for controlling it. It exposes REST APIs for button
//! control and image management, as well as a simple web UI with WebSocket
//! support for real-time image updates.

mod cli;
mod handlers;
mod html;
mod state;

use axum::{
    Router,
    routing::{get, post},
};
use clap::Parser;
use gadgetdeck::{GadgetDeck, ImageEvent, StreamDeckModel};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use tokio::signal;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use cli::Args;
use state::{AppState, LcdStore, WsMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Parse CLI arguments
    let args = Args::parse();

    // Convert device type to model
    let model: StreamDeckModel = args.device.into();
    let serial = args
        .serial
        .or_else(|| std::env::var("GADGETDECK_SERIAL").ok())
        .unwrap_or_else(|| "ZZZZZZZZZZZZZZ".to_string());

    // ========================================================================
    // Set up USB Gadget using GadgetDeck
    // ========================================================================

    println!("🎛️  GadgetDeck Web Server");
    println!("   Setting up USB gadget...");

    let mut deck = GadgetDeck::new(model, serial.clone())?;

    println!("   USB {} gadget registered!", model.product_name());
    println!("   Serial: {}", serial);

    // Start the USB processing threads
    deck.start()?;
    println!("   USB threads started");

    // Get shared state from the deck
    let running = deck.running_flag();
    let button_state = deck.button_state();
    let image_store = deck.image_store();
    let image_rx = deck.subscribe_images();
    let plus_state = deck.plus_state();

    // ========================================================================
    // Set up Web Server
    // ========================================================================

    // Create broadcast channel for WebSocket updates
    let (ws_tx, _) = broadcast::channel::<WsMessage>(32);

    // Create LCD store for Plus/Neo models
    let lcd_store = LcdStore::new();

    let (key_cols, _key_rows) = model.key_matrix();
    let state = AppState {
        button_state: button_state.clone(),
        image_store: image_store.clone(),
        model,
        key_cols: key_cols as usize,
        running: running.clone(),
        ws_tx: ws_tx.clone(),
        plus_state: plus_state.clone(),
        lcd_store: lcd_store.clone(),
    };

    // Spawn a task to watch for image changes and broadcast them
    let watch_running = running.clone();
    let watch_tx = ws_tx.clone();
    let watch_lcd_store = lcd_store.clone();
    tokio::task::spawn_blocking(move || {
        use base64::Engine;
        use std::time::Duration;

        while watch_running.load(Ordering::Relaxed) {
            // Wait for image events with timeout so we can check running flag
            match image_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(ImageEvent::Updated { key_index, image }) => {
                    let image_data =
                        base64::engine::general_purpose::STANDARD.encode(image.as_bytes());
                    let update = WsMessage::ImageUpdate {
                        button_id: key_index,
                        image_data,
                    };
                    // Ignore send errors (no subscribers)
                    let _ = watch_tx.send(update);
                }
                Ok(ImageEvent::LcdUpdated {
                    x_offset,
                    y_offset,
                    width,
                    height,
                    image,
                }) => {
                    // LCD updates for Stream Deck Plus touchscreen
                    log::debug!(
                        "LCD image update: x_off={}, y_off={}, {}x{}, {} bytes",
                        x_offset,
                        y_offset,
                        width,
                        height,
                        image.len()
                    );

                    // Store the segment for replay on new WebSocket connections
                    watch_lcd_store.update(
                        x_offset,
                        y_offset,
                        width,
                        height,
                        image.as_bytes().to_vec(),
                    );

                    let image_data =
                        base64::engine::general_purpose::STANDARD.encode(image.as_bytes());
                    let update = WsMessage::LcdUpdate {
                        x_offset,
                        y_offset,
                        width,
                        height,
                        image_data,
                    };
                    let _ = watch_tx.send(update);
                }
                Ok(ImageEvent::LedColorUpdated { button, color }) => {
                    // LED color updates for Neo buttons 8-9
                    log::debug!(
                        "LED color update: button {} RGB({}, {}, {})",
                        button,
                        color.r,
                        color.g,
                        color.b
                    );
                    let update = WsMessage::LedColorUpdate {
                        button_id: button,
                        r: color.r,
                        g: color.g,
                        b: color.b,
                    };
                    let _ = watch_tx.send(update);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Check running flag and continue
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Channel closed, exit
                    break;
                }
            }
        }
    });

    // Build the router
    let app = Router::new()
        // Web UI
        .route("/", get(handlers::index_handler))
        // WebSocket for real-time updates
        .route("/ws", get(handlers::ws_handler))
        // API endpoints
        .route("/api/status", get(handlers::status_handler))
        .route("/api/buttons", get(handlers::buttons_handler))
        .route(
            "/api/buttons/{id}/press",
            post(handlers::press_button_handler),
        )
        .route(
            "/api/buttons/{id}/release",
            post(handlers::release_button_handler),
        )
        .route(
            "/api/buttons/{id}/click",
            post(handlers::click_button_handler),
        )
        .route("/api/images", get(handlers::images_handler))
        .route("/api/images/{id}", get(handlers::get_image_handler))
        // Plus-specific endpoints (knobs and touchscreen)
        .route("/api/knobs", get(handlers::knobs_handler))
        .route("/api/knobs/{id}/press", post(handlers::press_knob_handler))
        .route(
            "/api/knobs/{id}/release",
            post(handlers::release_knob_handler),
        )
        .route("/api/knobs/{id}/click", post(handlers::click_knob_handler))
        .route("/api/knobs/{id}/turn", post(handlers::turn_knob_handler))
        .route("/api/lcd/tap", post(handlers::lcd_tap_handler))
        .route("/api/lcd/swipe", post(handlers::lcd_swipe_handler))
        // Neo-specific endpoints (button LEDs for buttons 8-9)
        .route("/api/buttons/leds", get(handlers::button_leds_handler))
        .route(
            "/api/buttons/{id}/led",
            get(handlers::get_button_led_handler),
        )
        .route(
            "/api/buttons/{id}/led",
            post(handlers::set_button_led_handler),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Parse bind address from CLI args or environment
    let addr: SocketAddr = std::env::var("GADGETDECK_BIND")
        .unwrap_or(args.bind)
        .parse()
        .expect("Invalid bind address");

    println!("   Model: {}", model.product_name());
    println!("   Listening on: http://{}", addr);
    println!("   WebSocket: ws://{}/ws", addr);
    println!("\n   Press Ctrl+C to stop\n");

    // Create the listener
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Run with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(running.clone()))
        .await?;

    println!("\nStopping GadgetDeck...");
    deck.stop();

    println!("Web server stopped.");
    Ok(())
}

async fn shutdown_signal(running: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    running.store(false, Ordering::SeqCst);
    println!("\nShutdown signal received...");
}
