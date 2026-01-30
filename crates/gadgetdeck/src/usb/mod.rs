//! USB module - USB gadget and device handling
//!
//! This module contains all USB-related functionality including
//! custom HID implementation and device descriptors.

pub mod custom_hid;
pub mod descriptors;

// Re-exports for convenience
pub use custom_hid::{
    CustomHid, run_input_report_sender, run_output_report_receiver, run_plus_input_report_sender,
};
pub use descriptors::StreamDeckModel;
