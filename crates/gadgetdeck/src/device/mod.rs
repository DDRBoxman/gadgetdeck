//! Device module - Stream Deck device state management
//!
//! This module contains button state management and image handling
//! for Stream Deck devices.

pub mod buttons;
pub mod image;
pub mod neo;
pub mod plus;

// Re-exports for convenience
pub use buttons::ButtonState;
pub use image::{ButtonImage, ImageStore, ImageStats, ImagePacket, ImagePacketHeader, ImageError, ImageEvent, ImageEventReceiver};
pub use neo::{NeoInputState, RgbColor, NEO_LED_BUTTON_LEFT, NEO_LED_BUTTON_RIGHT};
pub use plus::{PlusInputState, TouchEvent, TouchEventType, KnobEvent, KnobIndex, KnobDirection};
