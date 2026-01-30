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
    /// Stream Deck Plus - 8 keys (4x2), 4 knobs, touchscreen
    Plus,
    /// Stream Deck Neo - 8 keys (4x2), 2 touch points, info bar LCD
    Neo,
}

impl StreamDeckModel {
    /// Returns the USB Product ID for this model
    pub fn product_id(&self) -> u16 {
        match self {
            StreamDeckModel::Mini => 0x0063,
            StreamDeckModel::Pedal => 0x0086,
            StreamDeckModel::Mk2 => 0x00B9, // Module 15
            StreamDeckModel::Xl => 0x00BA,  // Module 32
            StreamDeckModel::Plus => 0x0084,
            StreamDeckModel::Neo => 0x009A,
        }
    }

    /// Returns the product name string
    pub fn product_name(&self) -> &'static str {
        match self {
            StreamDeckModel::Mini => "Stream Deck Mini",
            StreamDeckModel::Pedal => "Stream Deck Pedal",
            StreamDeckModel::Mk2 => "Stream Deck MK.2",
            StreamDeckModel::Xl => "Stream Deck XL",
            StreamDeckModel::Plus => "Stream Deck +",
            StreamDeckModel::Neo => "Stream Deck Neo",
        }
    }

    /// Returns the USB ID (vendor + product)
    pub fn usb_id(&self) -> Id {
        Id::new(ELGATO_VENDOR_ID, self.product_id())
    }

    /// Returns the USB device class (0 = defined at interface level)
    pub fn device_class(&self) -> Class {
        Class::new(0, 0, 0)
    }

    /// Returns the USB strings (manufacturer, product, serial)
    pub fn usb_strings(&self, serial: &str) -> Strings {
        Strings::new("Elgato", self.product_name(), serial)
    }

    /// Returns the bcdDevice (device version) - 1.10 = 0x0110
    pub fn bcd_device(&self) -> u16 {
        0x0110
    }

    /// Returns the number of buttons for this model
    pub fn num_buttons(&self) -> u8 {
        match self {
            StreamDeckModel::Mini => 6,
            StreamDeckModel::Pedal => 3,
            StreamDeckModel::Mk2 => 15,
            StreamDeckModel::Xl => 32,
            StreamDeckModel::Plus => 8,
            StreamDeckModel::Neo => 10, // 8 keys + 2 touch points
        }
    }

    /// Returns the max packet size for EP0
    pub fn max_packet_size0(&self) -> u8 {
        64
    }

    /// Returns the max power in mA
    pub fn max_power_ma(&self) -> u16 {
        match self {
            StreamDeckModel::Mini => 200,
            StreamDeckModel::Pedal => 100,
            StreamDeckModel::Mk2 => 500,
            StreamDeckModel::Xl => 500,
            StreamDeckModel::Plus => 500,
            StreamDeckModel::Neo => 500,
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
                protocol: 0, // HID_PROTOCOL_REPORT
                sub_class: 0,
                in_max_packet_size: 64,
                out_max_packet_size: 64,
                interval: 1,
                report_descriptor: Self::stream_deck_pedal_report_descriptor(),
                // Report length is the max input report size (Report ID 1 = 7 bytes + 1 for report ID)
                report_len: 8,
            },
            StreamDeckModel::Mk2 | StreamDeckModel::Xl | StreamDeckModel::Neo => StreamDeckHidConfig {
                protocol: 0,
                sub_class: 0,
                in_max_packet_size: 512,
                out_max_packet_size: 1024,
                interval: 1,
                report_descriptor: Self::stream_deck_modern_report_descriptor(),
                report_len: 32,
            },
            StreamDeckModel::Plus => StreamDeckHidConfig {
                protocol: 0,
                sub_class: 0,
                in_max_packet_size: 512,
                out_max_packet_size: 1024,
                interval: 1,
                report_descriptor: Self::stream_deck_modern_report_descriptor(),
                report_len: 14,
            },
        }
    }

    /// Stream Deck Mini HID Report Descriptor (221 bytes)
    /// Captured from real device using: sudo usbhid-dump -d 0fd9:0063
    fn stream_deck_mini_report_descriptor() -> Vec<u8> {
        vec![
            // Usage Page (Consumer Devices)
            0x05, 0x0C, // Usage (Consumer Control)
            0x09, 0x01, // Collection (Application)
            0xA1, 0x01, //   Usage (Consumer Control)
            0x09, 0x01, //   Usage Page (Button)
            0x05, 0x09, //   Usage Minimum (1)
            0x19, 0x01, //   Usage Maximum (16)
            0x29, 0x10, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (1) - Button input report
            0x85, 0x01, //   Input (Data, Variable, Absolute)
            0x81, 0x02, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (1023)
            0x96, 0xFF, 0x03, //   Report ID (2) - Image output report
            0x85, 0x02, //   Output (Data, Variable, Absolute)
            0x91, 0x02, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (31)
            0x95, 0x1F, //   Report ID (3) - Feature report
            0x85, 0x03,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (4) - Feature report
            0x85, 0x04,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (5) - Feature report
            0x85, 0x05,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (7) - Feature report
            0x85, 0x07,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (11/0x0B) - Feature report
            0x85, 0x0B,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (160/0xA0) - Feature report
            0x85, 0xA0,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (161/0xA1) - Feature report
            0x85, 0xA1,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (162/0xA2) - Feature report
            0x85, 0xA2,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (16)
            0x95, 0x10, //   Report ID (163/0xA3) - Feature report
            0x85, 0xA3,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (31)
            0x95, 0x1F, //   Report ID (164/0xA4) - Feature report
            0x85, 0xA4,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, //   Usage (Vendor Defined 0xFF00)
            0x0A, 0x00, 0xFF, //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report Count (31)
            0x95, 0x1F, //   Report ID (165/0xA5) - Feature report
            0x85, 0xA5,
            //   Feature (Data, Variable, Absolute, No Wrap, Linear, No Preferred, No Null)
            0xB1, 0x04, // End Collection
            0xC0,
        ]
    }

    /// Stream Deck Pedal HID Report Descriptor
    /// Translated from TinyUSB TUD_HID_REPORT_DESC_PEDAL macro
    fn stream_deck_pedal_report_descriptor() -> Vec<u8> {
        vec![
            // Usage Page (Consumer Devices)
            0x05, 0x0C, // Usage (Consumer Control)
            0x09, 0x01, // Collection (Application)
            0xA1, 0x01, //   Usage Page (Vendor Defined 0xFF00) - 2 byte form
            0x06, 0x00, 0xFF,
            //   Common field definition
            //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08, //   Report ID 2 - Output (1023 bytes)
            0x96, 0xFF, 0x03, // Report Count (1023)
            0x85, 0x02, // Report ID (2)
            0x09, 0x01, // Usage
            0x91, 0x02, // Output (Data, Variable, Absolute)
            //   Report ID 1 - Input (7 bytes) - Pedal button states
            0x95, 0x07, // Report Count (7)
            0x85, 0x01, // Report ID (1)
            0x09, 0x01, // Usage
            0x81, 0x02, // Input (Data, Variable, Absolute)
            //   Report ID 3 - Feature (31 bytes)
            0x95, 0x1F, // Report Count (31)
            0x85, 0x03, // Report ID (3)
            0x09, 0x01, // Usage
            0xB1, 0x04, // Feature (Data, Array, Relative)
            //   Report ID 6 - Feature (31 bytes) - Serial number
            0x95, 0x1F, // Report Count (31)
            0x85, 0x06, // Report ID (6)
            0x09, 0x01, // Usage
            0xB1, 0x04, // Feature
            //   Report ID 7 - Feature (31 bytes)
            0x95, 0x1F, // Report Count (31)
            0x85, 0x07, // Report ID (7)
            0x09, 0x01, // Usage
            0xB1, 0x04, // Feature
            //   Report ID 5 - Feature (31 bytes) - Version info
            0x95, 0x1F, // Report Count (31)
            0x85, 0x05, // Report ID (5)
            0x09, 0x01, // Usage
            0xB1, 0x04, // Feature
            //   Report ID 4 - Feature (31 bytes)
            0x95, 0x1F, // Report Count (31)
            0x85, 0x04, // Report ID (4)
            0x09, 0x01, // Usage
            0xB1, 0x04, // Feature
            //   Report ID 8 - Feature (31 bytes)
            0x95, 0x1F, // Report Count (31)
            0x85, 0x08, // Report ID (8)
            0x09, 0x01, // Usage
            0xB1, 0x04, // Feature
            // End Collection
            0xC0,
        ]
    }

    /// Stream Deck Module 15/32/Plus/Neo HID Report Descriptor
    /// Based on Elgato official documentation and reverse engineering research.
    ///
    /// This descriptor is shared by MK2, XL, Plus, and Neo models as they all use
    /// the same report structure:
    /// - Input Report (0x01): 512 bytes max, [Report ID, Command, Payload Length (2 bytes), Payload]
    /// - Output Report (0x02): 1024 bytes max, [Report ID, Command, Payload]
    /// - Feature Reports: 0x03 (setters), 0x04-0x08 (getters), 0x0A (idle time)
    ///
    /// Model-specific behavior is handled at the protocol level, not the descriptor level.
    fn stream_deck_modern_report_descriptor() -> Vec<u8> {
        vec![
            // Usage Page (Consumer Devices)
            0x05, 0x0C, // Usage (Consumer Control)
            0x09, 0x01, // Collection (Application)
            0xA1, 0x01, //   Usage Page (Vendor Defined 0xFF00)
            0x06, 0x00, 0xFF,
            //   Common field definition
            //   Logical Minimum (0)
            0x15, 0x00, //   Logical Maximum (255)
            0x26, 0xFF, 0x00, //   Report Size (8)
            0x75, 0x08,
            //   Report ID 1 - Input (511 bytes) - Key/Knob/Touch state
            0x96, 0xFF, 0x01, // Report Count (511)
            0x85, 0x01, // Report ID (1)
            0x09, 0x01, // Usage
            0x81, 0x02, // Input (Data, Variable, Absolute)
            //   Report ID 2 - Output (1023 bytes) - Image upload
            0x96, 0xFF, 0x03, // Report Count (1023)
            0x85, 0x02, // Report ID (2)
            0x09, 0x01, // Usage
            0x91, 0x02, // Output (Data, Variable, Absolute)
            //   Report ID 3 - Feature (31 bytes) - Setters
            0x95, 0x1F, // Report Count (31)
            0x85, 0x03, // Report ID (3)
            0x09, 0x01, // Usage
            0xB1, 0x02, // Feature (Data, Variable, Absolute)
            //   Report ID 4 - Feature (31 bytes) - LD firmware version
            0x95, 0x1F, // Report Count (31)
            0x85, 0x04, // Report ID (4)
            0x09, 0x01, // Usage
            0xB1, 0x02, // Feature
            //   Report ID 5 - Feature (31 bytes) - AP2 (primary) firmware version
            0x95, 0x1F, // Report Count (31)
            0x85, 0x05, // Report ID (5)
            0x09, 0x01, // Usage
            0xB1, 0x02, // Feature
            //   Report ID 6 - Feature (31 bytes) - Serial number
            0x95, 0x1F, // Report Count (31)
            0x85, 0x06, // Report ID (6)
            0x09, 0x01, // Usage
            0xB1, 0x02, // Feature
            //   Report ID 7 - Feature (31 bytes) - AP1 (backup) firmware version
            0x95, 0x1F, // Report Count (31)
            0x85, 0x07, // Report ID (7)
            0x09, 0x01, // Usage
            0xB1, 0x02, // Feature
            //   Report ID 8 - Feature (31 bytes) - Unit information
            0x95, 0x1F, // Report Count (31)
            0x85, 0x08, // Report ID (8)
            0x09, 0x01, // Usage
            0xB1, 0x02, // Feature
            //   Report ID 10 (0x0A) - Feature (31 bytes) - Idle time before sleep
            0x95, 0x1F, // Report Count (31)
            0x85, 0x0A, // Report ID (10)
            0x09, 0x01, // Usage
            0xB1, 0x02, // Feature
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
            StreamDeckModel::Mini => 0xA1, // Mini uses 0xA1 for AP2 (primary firmware) per Elgato docs
            StreamDeckModel::Pedal => 0x05,
            StreamDeckModel::Mk2
            | StreamDeckModel::Xl
            | StreamDeckModel::Plus
            | StreamDeckModel::Neo => 0x05, // Module 15/32/Plus/Neo use 0x05 for AP2 (primary)
        }
    }

    /// Returns the report ID used for serial number
    ///
    /// Based on Elgato official documentation:
    /// - Module 6: Report ID 0x03
    /// - Module 15/32: Report ID 0x06
    pub fn serial_report_id(&self) -> u8 {
        match self {
            StreamDeckModel::Mini => 0x03, // Mini uses report 0x03 for serial per Elgato docs
            StreamDeckModel::Pedal => 0x06,
            StreamDeckModel::Mk2
            | StreamDeckModel::Xl
            | StreamDeckModel::Plus
            | StreamDeckModel::Neo => 0x06, // Module 15/32/Plus/Neo use 0x06 for serial
        }
    }

    /// Returns the version feature report data
    ///
    /// Based on Elgato official documentation:
    /// - Module 6 (Mini): Report ID 0xA1, [Report ID, N/A[4], Version String ASCII[12]]
    /// - Pedal: Report ID 0x05, version string at offset 6
    /// - Module 15/32/Neo: Report ID 0x05, [Report ID, Data Length (0x0C), Checksum[4], Version String ASCII[8]]
    /// - Plus: Report ID 0x05, version string at offset 5
    pub fn version_report(&self) -> Vec<u8> {
        let mut report = vec![0u8; 32];
        match self {
            StreamDeckModel::Mini => {
                report[0] = 0xA1;
                let version = b"1.0.170602";
                report[5..5 + version.len()].copy_from_slice(version);
            }
            StreamDeckModel::Pedal => {
                report[0] = 0x05;
                let version = b"1.0.0";
                report[6..6 + version.len()].copy_from_slice(version);
            }
            StreamDeckModel::Mk2 | StreamDeckModel::Xl | StreamDeckModel::Neo => {
                // Module 15/32/Neo: [Report ID, Length, Checksum[4], Version String[8]]
                report[0] = 0x05;
                report[1] = 0x0C;
                let version = b"1.0.0.0\0";
                report[6..6 + version.len()].copy_from_slice(version);
            }
            StreamDeckModel::Plus => {
                report[0] = 0x05;
                let version = b"1.0.0.0\0";
                report[5..5 + version.len()].copy_from_slice(version);
            }
        }
        report
    }

    /// Returns the serial feature report data
    ///
    /// Based on Elgato official documentation:
    /// - Module 6 (Mini): Report ID 0x03, [Report ID, N/A[4], Serial Number String ASCII]
    /// - Pedal: Report ID 0x06, serial at offset 2
    /// - Module 15/32/Neo: Report ID 0x06, [Report ID, Data Length, Serial Number String ASCII]
    /// - Plus: Report ID 0x06, serial at offset 5
    pub fn serial_report(&self, serial: &str) -> Vec<u8> {
        let mut report = vec![0u8; 32];
        let serial_bytes = serial.as_bytes();
        match self {
            StreamDeckModel::Mini => {
                report[0] = 0x03;
                let copy_len = serial_bytes.len().min(12);
                report[5..5 + copy_len].copy_from_slice(&serial_bytes[..copy_len]);
            }
            StreamDeckModel::Pedal => {
                report[0] = 0x06;
                let copy_len = serial_bytes.len().min(30);
                report[2..2 + copy_len].copy_from_slice(&serial_bytes[..copy_len]);
            }
            StreamDeckModel::Mk2 | StreamDeckModel::Xl | StreamDeckModel::Neo => {
                // Module 15/32/Neo: [Report ID, Length, Serial String ASCII]
                report[0] = 0x06;
                let copy_len = serial_bytes.len().min(14);
                report[1] = copy_len as u8;
                report[2..2 + copy_len].copy_from_slice(&serial_bytes[..copy_len]);
            }
            StreamDeckModel::Plus => {
                report[0] = 0x06;
                let copy_len = serial_bytes.len().min(27);
                report[5..5 + copy_len].copy_from_slice(&serial_bytes[..copy_len]);
            }
        }
        report
    }

    /// Creates a firmware version report with the standard format
    /// Format: [Report ID, Data Length (0x0C), Checksum[4], Version String ASCII[8]]
    fn make_firmware_report(report_id: u8) -> Vec<u8> {
        let mut report = vec![0u8; 32];
        report[0] = report_id;
        report[1] = 0x0C;
        let version = b"1.0.0.0\0";
        report[6..6 + version.len()].copy_from_slice(version);
        report
    }

    /// Handle a GET_REPORT request for a feature report
    /// Returns the report data if the report ID is recognized, None otherwise
    pub fn get_feature_report(&self, report_id: u8, serial: &str) -> Option<Vec<u8>> {
        if report_id == self.version_report_id() {
            Some(self.version_report())
        } else if report_id == self.serial_report_id() {
            Some(self.serial_report(serial))
        } else if report_id == 0x08 {
            self.unit_info_report()
        } else {
            // Handle additional feature report IDs for modern models
            match self {
                StreamDeckModel::Mk2
                | StreamDeckModel::Xl
                | StreamDeckModel::Plus
                | StreamDeckModel::Neo => match report_id {
                    0x04 => Some(Self::make_firmware_report(0x04)), // LD firmware
                    0x07 => Some(Self::make_firmware_report(0x07)), // AP1 (backup) firmware
                    0x0A => {
                        let mut report = vec![0u8; 32];
                        report[0] = 0x0A;
                        report[1] = 0x04;
                        Some(report)
                    }
                    _ => None,
                },
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
        ConfigAttributes {
            bus_powered: true,
            remote_wakeup: true,
            config_value: 1,
            num_interfaces: 1,
        }
    }

    /// Returns the number of keys for this model
    pub fn key_count(&self) -> u8 {
        match self {
            StreamDeckModel::Mini => 6,
            StreamDeckModel::Pedal => 3,
            StreamDeckModel::Mk2 => 15,
            StreamDeckModel::Xl => 32,
            StreamDeckModel::Plus => 8,
            StreamDeckModel::Neo => 8,
        }
    }

    /// Returns the key matrix dimensions (columns, rows)
    pub fn key_matrix(&self) -> (u8, u8) {
        match self {
            StreamDeckModel::Mini => (3, 2),
            StreamDeckModel::Pedal => (3, 1),
            StreamDeckModel::Mk2 => (5, 3),
            StreamDeckModel::Xl => (8, 4),
            StreamDeckModel::Plus => (4, 2),
            StreamDeckModel::Neo => (4, 2),
        }
    }

    /// Returns the key image dimensions (width, height) in pixels
    pub fn key_image_size(&self) -> (u16, u16) {
        match self {
            StreamDeckModel::Mini => (80, 80),
            StreamDeckModel::Pedal => (0, 0), // Pedal has no display
            StreamDeckModel::Mk2 => (72, 72),
            StreamDeckModel::Xl => (96, 96),
            StreamDeckModel::Plus => (120, 120),
            StreamDeckModel::Neo => (96, 96),
        }
    }

    /// Returns the LCD dimensions (width, height) in pixels
    pub fn lcd_size(&self) -> (u16, u16) {
        match self {
            StreamDeckModel::Mini => (320, 240),
            StreamDeckModel::Pedal => (0, 0), // Pedal has no display
            StreamDeckModel::Mk2 => (480, 272),
            StreamDeckModel::Xl => (1024, 600),
            StreamDeckModel::Plus => (800, 100), // 800x240 for buttons (2 rows × 120px) + 800x100 touchscreen
            StreamDeckModel::Neo => (248, 58),   // Info bar LCD
        }
    }

    /// Returns the image format for this model
    pub fn image_format(&self) -> &'static str {
        match self {
            StreamDeckModel::Mini => "BMP", // Module 6 uses BMP, rotated 90° clockwise
            StreamDeckModel::Pedal => "NONE",
            StreamDeckModel::Mk2
            | StreamDeckModel::Xl
            | StreamDeckModel::Plus
            | StreamDeckModel::Neo => "JPEG", // All newer models use JPEG
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
                report[0] = 0x08; // Report ID
                report[1] = 3; // Rows
                report[2] = 5; // Columns
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
                report[0] = 0x08; // Report ID
                report[1] = 4; // Rows
                report[2] = 8; // Columns
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
            StreamDeckModel::Plus => {
                let mut report = vec![0u8; 32];
                report[0] = 0x08; // Report ID
                report[1] = 2; // Rows
                report[2] = 4; // Columns
                // Key width (120) as u16 LE
                report[3] = 120;
                report[4] = 0;
                // Key height (120) as u16 LE
                report[5] = 120;
                report[6] = 0;
                // LCD width (800) as u16 LE - touchscreen strip
                report[7] = 0x20;
                report[8] = 0x03;
                // LCD height (100) as u16 LE - touchscreen strip only
                report[9] = 0x64;
                report[10] = 0x00;
                // BPP
                report[11] = 24;
                // Color scheme (RGB)
                report[12] = 0;
                Some(report)
            }
            StreamDeckModel::Neo => {
                let mut report = vec![0u8; 32];
                report[0] = 0x08; // Report ID
                report[1] = 2; // Rows
                report[2] = 4; // Columns
                // Key width (96) as u16 LE
                report[3] = 96;
                report[4] = 0;
                // Key height (96) as u16 LE
                report[5] = 96;
                report[6] = 0;
                // LCD width (248) as u16 LE - info bar
                report[7] = 0xF8;
                report[8] = 0x00;
                // LCD height (58) as u16 LE - info bar
                report[9] = 0x3A;
                report[10] = 0x00;
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
