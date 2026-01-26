//! Application state and WebSocket message types

use gadgetdeck::{ButtonState, StreamDeckModel};
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Message sent over WebSocket when an image updates
#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "image")]
    ImageUpdate {
        button_id: u8,
        /// Base64-encoded image data
        image_data: String,
    },
    #[serde(rename = "buttons")]
    ButtonsUpdate { buttons: Vec<ButtonStateInfo> },
}

#[derive(Clone, Serialize)]
pub struct ButtonStateInfo {
    pub id: u8,
    pub pressed: bool,
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub button_state: Arc<ButtonState>,
    pub image_store: gadgetdeck::ImageStore,
    pub model: StreamDeckModel,
    /// Number of button columns (for UI grid layout)
    pub key_cols: usize,
    pub running: Arc<AtomicBool>,
    /// Broadcast channel for WebSocket updates
    pub ws_tx: broadcast::Sender<WsMessage>,
}
