//! GadgetDeck - USB Gadget Stream Deck Emulator Library
//!
//! This library provides functionality to emulate Stream Deck devices
//! as USB gadgets on Linux systems.
//!
//! ## Quick Start
//!
//! The easiest way to use this library is through the [`GadgetDeck`] struct:
//!
//! ```no_run
//! use gadgetdeck::{GadgetDeck, GadgetDeckConfig, StreamDeckModel};
//!
//! let config = GadgetDeckConfig::new(StreamDeckModel::Mini, "ZZZZZZZZZZZZZZ");
//! let mut deck = GadgetDeck::new(config).expect("Failed to create GadgetDeck");
//! deck.start().expect("Failed to start");
//!
//! // Access button state and image store through the deck
//! let buttons = deck.button_state();
//! let images = deck.image_store();
//! ```
//!
//! ## Module Organization
//!
//! - [`gadgetdeck`] - Main entry point struct for managing the USB gadget
//! - [`usb`] - USB gadget functionality (HID, descriptors, custom HID)
//! - [`device`] - Device state management (buttons, images)

pub mod device;
pub mod gadgetdeck;
pub mod usb;

// Re-export the main GadgetDeck struct and config at crate root
pub use gadgetdeck::{GadgetDeck, GadgetDeckConfig, GadgetDeckError};

// Re-export commonly used types at crate root for convenience
pub use device::{ButtonImage, ButtonState, ImageError, ImageEvent, ImageEventReceiver, ImagePacket, ImagePacketHeader, ImageStats, ImageStore};
pub use device::{PlusInputState, TouchEvent, TouchEventType, KnobEvent, KnobIndex, KnobDirection};
pub use usb::{CustomHid, StreamDeckModel, run_input_report_sender, run_output_report_receiver};
