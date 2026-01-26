//! CLI argument parsing for gadgetdeck-server

use clap::{Parser, ValueEnum};
use gadgetdeck::StreamDeckModel;

/// CLI arguments for gadgetdeck-server
#[derive(Parser, Debug)]
#[command(name = "gadgetdeck-server")]
#[command(about = "GadgetDeck Web Server - Stream Deck emulator with web UI")]
pub struct Args {
    /// Device type to emulate
    #[arg(short, long, value_enum, default_value_t = DeviceType::Mini)]
    pub device: DeviceType,

    /// Serial number (overrides GADGETDECK_SERIAL env var)
    #[arg(short, long)]
    pub serial: Option<String>,

    /// Bind address for the web server
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    pub bind: String,
}

/// Device type enum for CLI
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DeviceType {
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
