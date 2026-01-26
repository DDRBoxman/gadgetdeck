//! USB Descriptors for Stream Deck devices
//!
//! This module contains the USB descriptor definitions for various
//! Elgato Stream Deck models to emulate them as USB gadgets.

use usb_gadget::{Class, Id, Strings};

/// USB Vendor ID for Elgato Systems GmbH
pub const ELGATO_VENDOR_ID: u16 = 0x0fd9;

/// Stream Deck device model definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDeckModel {
    /// Stream Deck Mini (Module 6) - 6 keys, 3x2 layout
    Mini,
    /// Stream Deck Pedal - 3 foot pedals
    Pedal,
    /// Stream Deck MK2 (Module 15) - 15 keys, 5x3 layout
    Mk2,
    /// Stream Deck XL (Module 32) - 32 keys, 8x4 layout
    Xl,
    // Future models can be added here:
    // Original,
    // Plus,
}

impl StreamDeckModel {
    /// Returns the USB Product ID for this model
    pub fn product_id(&self) -> u16 {
        match self {
            StreamDeckModel::Mini => 0x0063,
            StreamDeckModel::Pedal => 0x0086,
            StreamDeckModel::Mk2 => 0x00B9,   // Module 15
            StreamDeckModel::Xl => 0x00BA,    // Module 32
        }
    }

    /// Returns the product name string
    pub fn product_name(&self) -> &'static str {
        match self {
            StreamDeckModel::Mini => "Stream Deck Mini",
            StreamDeckModel::Pedal => "Stream Deck Pedal",
            StreamDeckModel::Mk2 => "Stream Deck MK.2",
            StreamDeckModel::Xl => "Stream Deck XL",
        }
    }

    /// Returns the USB ID (vendor + product)
    pub fn usb_id(&self) -> Id {
        Id::new(ELGATO_VENDOR_ID, self.product_id())
    }

    /// Returns the USB device class (0 = defined at interface level)
    pub fn device_class(&self) -> Class {
        match self {
            StreamDeckModel::Mini => Class::new(0, 0, 0),
            StreamDeckModel::Pedal => Class::new(0, 0, 0),
            StreamDeckModel::Mk2 => Class::new(0, 0, 0),
            StreamDeckModel::Xl => Class::new(0, 0, 0),
        }
    }

    /// Returns the USB strings (manufacturer, product, serial)
    pub fn usb_strings(&self, serial: &str) -> Strings {
        Strings::new("Elgato", self.product_name(), serial)
    }

    /// Returns the bcdDevice (device version) - 1.10 = 0x0110
    pub fn bcd_device(&self) -> u16 {
        match self {
            StreamDeckModel::Mini => 0x0110,
            StreamDeckModel::Pedal => 0x0100,
            StreamDeckModel::Mk2 => 0x0100,
            StreamDeckModel::Xl => 0x0100,
        }
    }

    /// Returns the max packet size for EP0
    pub fn max_packet_size0(&self) -> u8 {
        match self {
            StreamDeckModel::Mini => 64,
            StreamDeckModel::Pedal => 64,
            StreamDeckModel::Mk2 => 64,
            StreamDeckModel::Xl => 64,
        }
    }

    /// Returns the max power in mA
    pub fn max_power_ma(&self) -> u16 {
        match self {
            StreamDeckModel::Mini => 200,
            StreamDeckModel::Pedal => 100,
            StreamDeckModel::Mk2 => 500,
            StreamDeckModel::Xl => 500,
        }
    }
}

/// HID-specific configuration for Stream Deck devices
pub struct StreamDeckHidConfig {
    /// HID protocol (0 = none)
    pub protocol: u8,
    /// HID subclass (0 = no subclass)
    pub sub_class: u8,
    /// Maximum packet size for IN endpoint (EP1 IN)
    pub in_max_packet_size: u16,
    /// Maximum packet size for OUT endpoint (EP2 OUT)
    pub out_max_packet_size: u16,
    /// Endpoint polling interval in ms
    pub interval: u8,
    /// HID report descriptor
    pub report_descriptor: Vec<u8>,
    /// Report length
    pub report_len: u8,
}

impl StreamDeckModel {
    /// Returns the HID configuration for this model
    pub fn hid_config(&self) -> StreamDeckHidConfig {
        match self {
            StreamDeckModel::Mini => StreamDeckHidConfig {
                protocol: 0,
                sub_class: 0,
                in_max_packet_size: 512,   // EP1 IN: 0x0200 = 512 bytes
                out_max_packet_size: 1024, // EP2 OUT: 0x0400 = 1024 bytes
                interval: 1,
                report_descriptor: Self::stream_deck_mini_report_descriptor(),
                // Report length is the max input report size (Report ID 1 = 16 bytes + 1 for report ID)
                report_len: 17,
            },
            StreamDeckModel::Pedal => StreamDeckHidConfig {
                protocol: 0,  // HID_PROTOCOL_REPORT
                sub_class: 0,
                in_max_packet_size: 64,
                out_max_packet_size: 64,
                interval: 1,
                report_descriptor: Self::stream_deck_pedal_report_descriptor(),
                // Report length is the max input report size (Report ID 1 = 7 bytes + 1 for report ID)
                report_len: 8,
            },
            StreamDeckModel::Mk2 | StreamDeckModel::Xl => StreamDeckHidConfig {
                protocol: 0,
                sub_class: 0,
                in_max_packet_size: 512,   // Input report max 512 bytes per docs
                out_max_packet_size: 1024, // Output report max 1024 bytes per docs
                interval: 1,
                report_descriptor: Self::stream_deck_module_15_32_report_descriptor(),
                // Report length: Input report has 4-byte header + payload
                report_len: 32,  // Typical key state report size
            },
        }
    }

    /// Stream Deck Mini HID Report Descriptor (221 bytes)
    /// Captured from real device using: sudo usbhid-dump -d 0fd9:0063
    fn stream_deck_mini_report_descriptor() -> Vec<u8> {
        vec![
            // Usage Page (Consumer Devices)
            0x05, 0x0C,
            // Usage (Consumer Control)
            0x09, 0x01,
            // Collection (Application)
            0xA1, 0x01,
            //   Usage (Consumer Control)
            0x09, 0x01,
            //   Usage Page (Button)
            0x05, 0x09,
            //   Usage Minimum (1)
            0x19, 0x01,
            //   Usage Maximum (16)
            0x29, 0x10,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (1) - Button input report
            0x85, 0x01,
            //   Input (Data, Variable, Absolute)
            0x81, 0x02,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (1023)
            0x96, 0xFF, 0x03,
            //   Report ID (2) - Image output report
            0x85, 0x02,
            //   Output (Data, Variable, Absolute)
            0x91, 0x02,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (31)
            0x95, 0x1F,
            //   Report ID (3) - Feature report
            0x85, 0x03,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (4) - Feature report
            0x85, 0x04,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (5) - Feature report
            0x85, 0x05,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (7) - Feature report
            0x85, 0x07,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (11/0x0B) - Feature report
            0x85, 0x0B,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (160/0xA0) - Feature report
            0x85, 0xA0,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (161/0xA1) - Feature report
            0x85, 0xA1,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (162/0xA2) - Feature report
            0x85, 0xA2,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (16)
            0x95, 0x10,
            //   Report ID (163/0xA3) - Feature report
            0x85, 0xA3,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (31)
            0x95, 0x1F,
            //   Report ID (164/0xA4) - Feature report
            0x85, 0xA4,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF,
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,
            //   Report Count (31)
            0x95, 0x1F,
            //   Report ID (165/0xA5) - Feature report
            0x85, 0xA5,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04,

            // End Collection
            0xC0,
        ]
    }

    /// Stream Deck Pedal HID Report Descriptor
    /// Translated from TinyUSB TUD_HID_REPORT_DESC_PEDAL macro
    fn stream_deck_pedal_report_descriptor() -> Vec<u8> {
        vec![
            // Usage Page (Consumer Devices)
            0x05, 0x0C,
            // Usage (Consumer Control)
            0x09, 0x01,
            // Collection (Application)
            0xA1, 0x01,

            //   Usage Page (Vendor Defined 0xFF00) - 2 byte form
            0x06, 0x00, 0xFF,

            //   Common field definition
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,

            //   Report ID 2 - Output (1023 bytes)
            0x96, 0xFF, 0x03,  // Report Count (1023)
            0x85, 0x02,        // Report ID (2)
            0x09, 0x01,        // Usage
            0x91, 0x02,        // Output (Data, Variable, Absolute)

            //   Report ID 1 - Input (7 bytes) - Pedal button states
            0x95, 0x07,        // Report Count (7)
            0x85, 0x01,        // Report ID (1)
            0x09, 0x01,        // Usage
            0x81, 0x02,        // Input (Data, Variable, Absolute)

            //   Report ID 3 - Feature (31 bytes)
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x03,        // Report ID (3)
            0x09, 0x01,        // Usage
            0xB1, 0x04,        // Feature (Data, Array, Relative)

            //   Report ID 6 - Feature (31 bytes) - Serial number
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x06,        // Report ID (6)
            0x09, 0x01,        // Usage
            0xB1, 0x04,        // Feature

            //   Report ID 7 - Feature (31 bytes)
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x07,        // Report ID (7)
            0x09, 0x01,        // Usage
            0xB1, 0x04,        // Feature

            //   Report ID 5 - Feature (31 bytes) - Version info
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x05,        // Report ID (5)
            0x09, 0x01,        // Usage
            0xB1, 0x04,        // Feature

            //   Report ID 4 - Feature (31 bytes)
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x04,        // Report ID (4)
            0x09, 0x01,        // Usage
            0xB1, 0x04,        // Feature

            //   Report ID 8 - Feature (31 bytes)
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x08,        // Report ID (8)
            0x09, 0x01,        // Usage
            0xB1, 0x04,        // Feature

            // End Collection
            0xC0,
        ]
    }

    /// Stream Deck Module 15/32 (MK2/XL) HID Report Descriptor
    /// Based on Elgato official documentation for Module 15 and 32 Keys
    /// 
    /// Report structure:
    /// - Input Report (0x01): 512 bytes max, [Report ID, Command, Payload Length (2 bytes), Payload]
    /// - Output Report (0x02): 1024 bytes max, [Report ID, Command, Payload]
    /// - Feature Report (0x03): 32 bytes max, setters
    /// - Feature Report Getters: 0x04 (LD version), 0x05 (AP2 version), 0x06 (serial), 0x07 (AP1 version), 0x08 (unit info), 0x0A (idle time)
    fn stream_deck_module_15_32_report_descriptor() -> Vec<u8> {
        vec![
            // Usage Page (Consumer Devices)
            0x05, 0x0C,
            // Usage (Consumer Control)
            0x09, 0x01,
            // Collection (Application)
            0xA1, 0x01,

            //   Usage Page (Vendor Defined 0xFF00)
            0x06, 0x00, 0xFF,

            //   Common field definition
            //   Logical Minimum (0)
            0x15, 0x00,
            //   Logical Maximum (255)
            0x26, 0xFF, 0x00,
            //   Report Size (8)
            0x75, 0x08,

            //   Report ID 1 - Input (511 bytes) - Key press state change
            //   Format: [Report ID, Command, Payload Length (2 bytes), Key states...]
            0x96, 0xFF, 0x01,  // Report Count (511)
            0x85, 0x01,        // Report ID (1)
            0x09, 0x01,        // Usage
            0x81, 0x02,        // Input (Data, Variable, Absolute)

            //   Report ID 2 - Output (1023 bytes) - Image upload
            //   Format: [Report ID, Command, Payload...]
            0x96, 0xFF, 0x03,  // Report Count (1023)
            0x85, 0x02,        // Report ID (2)
            0x09, 0x01,        // Usage
            0x91, 0x02,        // Output (Data, Variable, Absolute)

            //   Report ID 3 - Feature (31 bytes) - Setters
            //   Commands: 0x02 (show logo), 0x05 (fill LCD), 0x06 (fill key), 0x08 (brightness), 0x0D (sleep), 0x13 (background)
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x03,        // Report ID (3)
            0x09, 0x01,        // Usage
            0xB1, 0x02,        // Feature (Data, Variable, Absolute)

            //   Report ID 4 - Feature (31 bytes) - LD firmware version
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x04,        // Report ID (4)
            0x09, 0x01,        // Usage
            0xB1, 0x02,        // Feature

            //   Report ID 5 - Feature (31 bytes) - AP2 (primary) firmware version
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x05,        // Report ID (5)
            0x09, 0x01,        // Usage
            0xB1, 0x02,        // Feature

            //   Report ID 6 - Feature (31 bytes) - Serial number
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x06,        // Report ID (6)
            0x09, 0x01,        // Usage
            0xB1, 0x02,        // Feature

            //   Report ID 7 - Feature (31 bytes) - AP1 (backup) firmware version
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x07,        // Report ID (7)
            0x09, 0x01,        // Usage
            0xB1, 0x02,        // Feature

            //   Report ID 8 - Feature (31 bytes) - Unit information
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x08,        // Report ID (8)
            0x09, 0x01,        // Usage
            0xB1, 0x02,        // Feature

            //   Report ID 10 (0x0A) - Feature (31 bytes) - Idle time before sleep
            0x95, 0x1F,        // Report Count (31)
            0x85, 0x0A,        // Report ID (10)
            0x09, 0x01,        // Usage
            0xB1, 0x02,        // Feature

            // End Collection
            0xC0,
        ]
    }

    /// Returns the report ID used for version info
    /// 
    /// Based on Elgato official documentation:
    /// - Module 6: 0xA0 (LD), 0xA1 (AP2/Primary), 0xA2 (AP1/Backup)
    /// - Module 15/32: 0x04 (LD), 0x05 (AP2/Primary), 0x07 (AP1/Backup)
    pub fn version_report_id(&self) -> u8 {
        match self {
            StreamDeckModel::Mini => 0xA1,  // Mini uses 0xA1 for AP2 (primary firmware) per Elgato docs
            StreamDeckModel::Pedal => 0x05,
            StreamDeckModel::Mk2 | StreamDeckModel::Xl => 0x05,  // Module 15/32 use 0x05 for AP2 (primary)
        }
    }

    /// Returns the report ID used for serial number
    /// 
    /// Based on Elgato official documentation:
    /// - Module 6: Report ID 0x03
    /// - Module 15/32: Report ID 0x06
    pub fn serial_report_id(&self) -> u8 {
        match self {
            StreamDeckModel::Mini => 0x03,  // Mini uses report 0x03 for serial per Elgato docs
            StreamDeckModel::Pedal => 0x06,
            StreamDeckModel::Mk2 | StreamDeckModel::Xl => 0x06,  // Module 15/32 use 0x06 for serial
        }
    }

    /// Returns the version feature report data
    /// For Mini (Module 6): Report ID 0xA1 (AP2), 32 bytes, version string at offset 5
    /// For Pedal: Report ID 0x05, 32 bytes, version string at offset 6
    /// For MK2/XL (Module 15/32): Report ID 0x05, 32 bytes, format [Report ID, Data Length, Checksum[4], Version String[8]]
    /// 
    /// Based on Elgato official documentation:
    /// - Module 6: Response format is [Report ID, N/A[4], Version String ASCII[12]]
    /// - Module 15/32: Response format is [Report ID, Data Length (0x0C), Checksum[4], Version String ASCII[8]]
    pub fn version_report(&self) -> Vec<u8> {
        match self {
            StreamDeckModel::Mini => {
                // Mini (Module 6) version report is 32 bytes (feature report max size)
                // Per Elgato docs: version string starts at offset 0x05
                let mut report = vec![0u8; 32];
                report[0] = 0xA1;  // Report ID for Mini
                // Version string at offset 5, format like "1.0.170602"
                let version = b"1.0.170602";  // Firmware version string
                let copy_len = version.len().min(12);
                report[5..5 + copy_len].copy_from_slice(&version[..copy_len]);
                report
            }
            StreamDeckModel::Pedal => {
                // Pedal version report is 32 bytes
                // Based on python-elgato-streamdeck: version string starts at [6:]
                let mut report = vec![0u8; 32];
                report[0] = 0x05;  // Report ID for Pedal
                let version = b"1.0.0";  // Firmware version string
                let copy_len = version.len().min(26);
                report[6..6 + copy_len].copy_from_slice(&version[..copy_len]);
                report
            }
            StreamDeckModel::Mk2 | StreamDeckModel::Xl => {
                // Module 15/32 version report is 32 bytes
                // Per Elgato docs: [Report ID, Data Length (0x0C), Checksum[4], Version String ASCII[8]]
                let mut report = vec![0u8; 32];
                report[0] = 0x05;  // Report ID for AP2 firmware version
                report[1] = 0x0C;  // Data length
                // Checksum at bytes 2-5 (can be zeros for emulation)
                // Version string at offset 6, 8 bytes max
                let version = b"1.0.0.0\0";  // Firmware version string
                let copy_len = version.len().min(8);
                report[6..6 + copy_len].copy_from_slice(&version[..copy_len]);
                report
            }
        }
    }

    /// Returns the serial feature report data
    /// 
    /// Based on Elgato official documentation:
    /// - Module 6: Report ID 0x03, Response: [Report ID, N/A[4], Serial Number String ASCII]
    /// - Module 15/32: Report ID 0x06, Response: [Report ID, Data Length (0x0C or 0x0E), Serial Number String ASCII]
    pub fn serial_report(&self, serial: &str) -> Vec<u8> {
        match self {
            StreamDeckModel::Mini => {
                // Mini (Module 6) serial report is 32 bytes (feature report max size)
                // Per Elgato docs: serial string starts at offset 0x05
                let mut report = vec![0u8; 32];
                report[0] = 0x03;  // Report ID for Mini serial
                let serial_bytes = serial.as_bytes();
                let copy_len = serial_bytes.len().min(12);
                report[5..5 + copy_len].copy_from_slice(&serial_bytes[..copy_len]);
                report
            }
            StreamDeckModel::Pedal => {
                // Pedal serial report is 32 bytes
                // Based on python-elgato-streamdeck: serial string starts at [2:]
                let mut report = vec![0u8; 32];
                report[0] = 0x06;  // Report ID for Pedal serial
                let serial_bytes = serial.as_bytes();
                let copy_len = serial_bytes.len().min(30);
                report[2..2 + copy_len].copy_from_slice(&serial_bytes[..copy_len]);
                report
            }
            StreamDeckModel::Mk2 | StreamDeckModel::Xl => {
                // Module 15/32 serial report is 32 bytes
                // Per Elgato docs: [Report ID, Data Length (0x0C or 0x0E), Serial Number String ASCII]
                // Serial number is 14 characters
                let mut report = vec![0u8; 32];
                report[0] = 0x06;  // Report ID for MK2/XL serial
                let serial_bytes = serial.as_bytes();
                let copy_len = serial_bytes.len().min(14);
                report[1] = copy_len as u8;  // Data length
                report[2..2 + copy_len].copy_from_slice(&serial_bytes[..copy_len]);
                report
            }
        }
    }

    /// Handle a GET_REPORT request for a feature report
    /// Returns the report data if the report ID is recognized, None otherwise
    pub fn get_feature_report(&self, report_id: u8, serial: &str) -> Option<Vec<u8>> {
        if report_id == self.version_report_id() {
            Some(self.version_report())
        } else if report_id == self.serial_report_id() {
            Some(self.serial_report(serial))
        } else if report_id == 0x08 {
            // Unit information report (Module 15/32 only)
            self.unit_info_report()
        } else {
            // Handle additional feature report IDs for MK2/XL
            match self {
                StreamDeckModel::Mk2 | StreamDeckModel::Xl => {
                    match report_id {
                        0x04 => {
                            // LD firmware version
                            let mut report = vec![0u8; 32];
                            report[0] = 0x04;
                            report[1] = 0x0C;
                            let version = b"1.0.0.0\0";
                            report[6..6 + version.len()].copy_from_slice(version);
                            Some(report)
                        }
                        0x07 => {
                            // AP1 (backup) firmware version
                            let mut report = vec![0u8; 32];
                            report[0] = 0x07;
                            report[1] = 0x0C;
                            let version = b"1.0.0.0\0";
                            report[6..6 + version.len()].copy_from_slice(version);
                            Some(report)
                        }
                        0x0A => {
                            // Idle time before sleep
                            let mut report = vec![0u8; 32];
                            report[0] = 0x0A;
                            report[1] = 0x04;  // Data length
                            // 0 seconds = sleep disabled
                            Some(report)
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }
    }
}

/// Configuration attributes matching the Stream Deck Mini
pub struct ConfigAttributes {
    /// Bus powered
    pub bus_powered: bool,
    /// Remote wakeup capability
    pub remote_wakeup: bool,
    /// Configuration value
    pub config_value: u8,
    /// Number of interfaces
    pub num_interfaces: u8,
}

impl StreamDeckModel {
    /// Returns the configuration attributes for this model
    pub fn config_attributes(&self) -> ConfigAttributes {
        match self {
            StreamDeckModel::Mini => ConfigAttributes {
                bus_powered: true,
                remote_wakeup: true,
                config_value: 1,
                num_interfaces: 1,
            },
            StreamDeckModel::Pedal => ConfigAttributes {
                bus_powered: true,
                remote_wakeup: true,
                config_value: 1,
                num_interfaces: 1,
            },
            StreamDeckModel::Mk2 | StreamDeckModel::Xl => ConfigAttributes {
                bus_powered: true,
                remote_wakeup: true,
                config_value: 1,
                num_interfaces: 1,
            },
        }
    }

    /// Returns the number of keys for this model
    pub fn key_count(&self) -> u8 {
        match self {
            StreamDeckModel::Mini => 6,
            StreamDeckModel::Pedal => 3,
            StreamDeckModel::Mk2 => 15,
            StreamDeckModel::Xl => 32,
        }
    }

    /// Returns the key matrix dimensions (columns, rows)
    pub fn key_matrix(&self) -> (u8, u8) {
        match self {
            StreamDeckModel::Mini => (3, 2),
            StreamDeckModel::Pedal => (3, 1),
            StreamDeckModel::Mk2 => (5, 3),
            StreamDeckModel::Xl => (8, 4),
        }
    }

    /// Returns the key image dimensions (width, height) in pixels
    pub fn key_image_size(&self) -> (u16, u16) {
        match self {
            StreamDeckModel::Mini => (80, 80),
            StreamDeckModel::Pedal => (0, 0),  // Pedal has no display
            StreamDeckModel::Mk2 => (72, 72),
            StreamDeckModel::Xl => (96, 96),
        }
    }

    /// Returns the LCD dimensions (width, height) in pixels
    pub fn lcd_size(&self) -> (u16, u16) {
        match self {
            StreamDeckModel::Mini => (320, 240),
            StreamDeckModel::Pedal => (0, 0),  // Pedal has no display
            StreamDeckModel::Mk2 => (480, 272),
            StreamDeckModel::Xl => (1024, 600),
        }
    }

    /// Returns the image format for this model
    pub fn image_format(&self) -> &'static str {
        match self {
            StreamDeckModel::Mini => "BMP",       // Module 6 uses BMP, rotated 90° clockwise
            StreamDeckModel::Pedal => "NONE",
            StreamDeckModel::Mk2 | StreamDeckModel::Xl => "JPEG",  // Module 15/32 use JPEG, rotated 180°
        }
    }

    /// Returns the unit information report (Report ID 0x08) for Module 15/32
    /// Format per Elgato docs:
    /// [0x00] Report ID (0x08)
    /// [0x01] Rows, [0x02] Cols, [0x03-0x04] Key Width, [0x05-0x06] Key Height,
    /// [0x07-0x08] LCD Width, [0x09-0x0A] LCD Height, [0x0B] BPP, [0x0C] Color Scheme, ...
    pub fn unit_info_report(&self) -> Option<Vec<u8>> {
        match self {
            StreamDeckModel::Mk2 => {
                let mut report = vec![0u8; 32];
                report[0] = 0x08;  // Report ID
                report[1] = 3;  // Rows
                report[2] = 5;  // Columns
                // Key width (72) as u16 LE
                report[3] = 72;
                report[4] = 0;
                // Key height (72) as u16 LE
                report[5] = 72;
                report[6] = 0;
                // LCD width (480) as u16 LE
                report[7] = 0xE0;
                report[8] = 0x01;
                // LCD height (272) as u16 LE
                report[9] = 0x10;
                report[10] = 0x01;
                // BPP
                report[11] = 24;
                // Color scheme (RGB)
                report[12] = 0;
                Some(report)
            }
            StreamDeckModel::Xl => {
                let mut report = vec![0u8; 32];
                report[0] = 0x08;  // Report ID
                report[1] = 4;  // Rows
                report[2] = 8;  // Columns
                // Key width (96) as u16 LE
                report[3] = 96;
                report[4] = 0;
                // Key height (96) as u16 LE
                report[5] = 96;
                report[6] = 0;
                // LCD width (1024) as u16 LE
                report[7] = 0x00;
                report[8] = 0x04;
                // LCD height (600) as u16 LE
                report[9] = 0x58;
                report[10] = 0x02;
                // BPP
                report[11] = 24;
                // Color scheme (RGB)
                report[12] = 0;
                Some(report)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_deck_mini_ids() {
        let model = StreamDeckModel::Mini;
        assert_eq!(model.product_id(), 0x0063);
        assert_eq!(model.product_name(), "Stream Deck Mini");
        assert_eq!(model.bcd_device(), 0x0110);
    }

    #[test]
    fn test_stream_deck_mini_hid_config() {
        let config = StreamDeckModel::Mini.hid_config();
        assert_eq!(config.in_max_packet_size, 512);
        assert_eq!(config.out_max_packet_size, 1024);
        assert_eq!(config.protocol, 0);
        assert_eq!(config.sub_class, 0);
    }
}
