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
pub use image::{
    ButtonImage, ImageError, ImageEvent, ImageEventReceiver, ImagePacket, ImagePacketHeader,
    ImageStats, ImageStore,
};
pub use neo::{NEO_LED_BUTTON_LEFT, NEO_LED_BUTTON_RIGHT, NeoInputState, RgbColor};
pub use plus::{KnobDirection, KnobEvent, KnobIndex, PlusInputState, TouchEvent, TouchEventType};
