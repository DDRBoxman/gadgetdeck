//! Application state and WebSocket message types

use gadgetdeck::{ButtonState, PlusInputState, StreamDeckModel};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
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
    #[serde(rename = "lcd")]
    LcdUpdate {
        x_offset: u16,
        y_offset: u16,
        width: u16,
        height: u16,
        /// Base64-encoded JPEG image data
        image_data: String,
    },
}

#[derive(Clone, Serialize)]
pub struct ButtonStateInfo {
    pub id: u8,
    pub pressed: bool,
}

/// Knob info for API responses
#[derive(Clone, Serialize)]
pub struct KnobInfo {
    pub id: u8,
    pub name: String,
}

/// Swipe request body
#[derive(Clone, Deserialize)]
pub struct SwipeRequest {
    pub start_x: u16,
    pub start_y: u16,
    pub end_x: u16,
    pub end_y: u16,
}

/// Tap request body
#[derive(Clone, Deserialize)]
pub struct TapRequest {
    pub x: u16,
    pub y: u16,
}

/// Turn knob request body
#[derive(Clone, Deserialize)]
pub struct TurnKnobRequest {
    /// Positive = clockwise, negative = counter-clockwise
    pub steps: i8,
}

/// Stored LCD segment for replay on WebSocket connect
#[derive(Clone)]
pub struct LcdSegment {
    pub x_offset: u16,
    pub y_offset: u16,
    pub width: u16,
    pub height: u16,
    pub image_data: Vec<u8>,
}

/// LCD state store - keeps the most recent segment for each position
#[derive(Clone, Default)]
pub struct LcdStore {
    /// Store segments by their x_offset (typically 0, 200, 400, 600 for 4 segments)
    segments: Arc<RwLock<Vec<LcdSegment>>>,
}

impl LcdStore {
    pub fn new() -> Self {
        Self {
            segments: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Store or update an LCD segment
    pub fn update(&self, x_offset: u16, y_offset: u16, width: u16, height: u16, image_data: Vec<u8>) {
        let mut segments = self.segments.write();
        // Find and update existing segment at this position, or add new one
        if let Some(seg) = segments.iter_mut().find(|s| s.x_offset == x_offset && s.y_offset == y_offset) {
            seg.width = width;
            seg.height = height;
            seg.image_data = image_data;
        } else {
            segments.push(LcdSegment {
                x_offset,
                y_offset,
                width,
                height,
                image_data,
            });
        }
    }

    /// Get all stored segments
    pub fn get_all(&self) -> Vec<LcdSegment> {
        self.segments.read().clone()
    }
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
    /// Plus-specific input state (touchscreen/knobs) - only present for Plus model
    pub plus_state: Option<Arc<PlusInputState>>,
    /// LCD segment store for Plus model
    pub lcd_store: LcdStore,
}
