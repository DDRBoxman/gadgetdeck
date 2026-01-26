//! Image handling for Stream Deck devices
//!
//! This module handles receiving and storing images sent from the host to the
//! Stream Deck device. Images are sent as multi-packet output reports and need
//! to be accumulated before they can be used.
//!
//! ## Stream Deck Mini (Module 6) Image Protocol
//!
//! Images are 80x80 pixels in BMP format, sent via Report ID 0x02.
//! Each packet is up to 1024 bytes with a 16-byte header:
//!
//! ```text
//! Header (16 bytes):
//!   [0]     0x02        - Report ID
//!   [1]     command     - 0x01 = write image
//!   [2]     page_number - Packet sequence (0, 1, 2, ...)
//!   [3]     0x00        - Reserved
//!   [4]     is_last     - 1 if final packet, 0 otherwise
//!   [5]     key_index+1 - Button number (1-6 for Mini)
//!   [6-15]  padding     - Zeros
//!
//! Payload: Up to 1008 bytes of image data per packet
//! ```
//!
//! ## Stream Deck MK2/XL (Module 15/32) Image Protocol
//!
//! Images are JPEG format, rotated 180°, sent via Report ID 0x02 with command 0x07:
//!
//! ```text
//! Header (8 bytes):
//!   [0]     0x02        - Report ID
//!   [1]     0x07        - Command (Update Key Image)
//!   [2]     key_index   - Button number (0-based)
//!   [3]     is_last     - 1 if final packet, 0 otherwise
//!   [4-5]   chunk_size  - Size of image data in this packet (u16 LE)
//!   [6-7]   chunk_index - Packet sequence (u16 LE, 0-based)
//!
//! Payload: Image data bytes (variable length, fills rest of 1024-byte report)
//! ```
//!
//! ## Subscribing to Image Events
//!
//! Use [`ImageStore::subscribe`] to receive notifications when images are updated:
//!
//! ```ignore
//! let store = ImageStore::new();
//! let mut rx = store.subscribe();
//! 
//! // In another task/thread:
//! while let Ok(event) = rx.recv() {
//!     match event {
//!         ImageEvent::Updated { key_index, image } => {
//!             println!("Button {} image updated: {} bytes", key_index, image.len());
//!         }
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;

/// Event emitted when an image is updated
#[derive(Debug, Clone)]
pub enum ImageEvent {
    /// An image was updated for a button
    Updated {
        /// Button index (0-based, for API consistency)
        key_index: u8,
        /// The completed image
        image: ButtonImage,
    },
}

/// Receiver for image events
/// 
/// This is returned by [`ImageStore::subscribe`] and can be used to receive
/// notifications when images are updated.
pub struct ImageEventReceiver {
    rx: mpsc::Receiver<ImageEvent>,
}

impl ImageEventReceiver {
    /// Block until the next event is received
    pub fn recv(&self) -> Result<ImageEvent, mpsc::RecvError> {
        self.rx.recv()
    }

    /// Try to receive an event without blocking
    pub fn try_recv(&self) -> Result<ImageEvent, mpsc::TryRecvError> {
        self.rx.try_recv()
    }

    /// Receive with a timeout
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<ImageEvent, mpsc::RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }
}

/// Image command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageCommand {
    /// Write image data to a button - Mini uses 0x01
    WriteImage,
    /// Update key image - MK2/XL uses 0x07
    UpdateKeyImage,
    /// Update full screen - MK2/XL uses 0x08
    UpdateFullScreen,
    /// Unknown command
    Unknown(u8),
}

impl ImageCommand {
    /// The command byte value
    pub fn as_u8(&self) -> u8 {
        match self {
            ImageCommand::WriteImage => 0x01,
            ImageCommand::UpdateKeyImage => 0x07,
            ImageCommand::UpdateFullScreen => 0x08,
            ImageCommand::Unknown(v) => *v,
        }
    }
}

impl From<u8> for ImageCommand {
    fn from(value: u8) -> Self {
        match value {
            0x01 => ImageCommand::WriteImage,
            0x07 => ImageCommand::UpdateKeyImage,
            0x08 => ImageCommand::UpdateFullScreen,
            other => ImageCommand::Unknown(other),
        }
    }
}

/// Protocol type for image packets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    /// Module 6 (Mini) protocol - 16-byte header
    Module6,
    /// Module 15/32 (MK2/XL) protocol - 8-byte header
    Module15_32,
}

/// Parsed image packet header (unified for all protocols)
#[derive(Debug, Clone)]
pub struct ImagePacketHeader {
    /// Report ID (should be 0x02)
    pub report_id: u8,
    /// Command type
    pub command: ImageCommand,
    /// Packet sequence number (0-based)
    pub page_number: u16,
    /// Whether this is the last packet
    pub is_last: bool,
    /// Button index (0-based for API, internally converted)
    pub key_index: u8,
    /// Size of payload data in this packet (MK2/XL only)
    pub chunk_size: u16,
    /// Which protocol was detected
    pub protocol: ImageProtocol,
    /// Header size for this protocol
    pub header_size: usize,
}

impl ImagePacketHeader {
    /// Header size for Mini (Module 6)
    pub const SIZE_MINI: usize = 16;
    /// Header size for MK2/XL (Module 15/32)
    pub const SIZE_MK2_XL: usize = 8;

    /// Parse a header from raw bytes, auto-detecting protocol from command
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE_MK2_XL {
            return None;
        }
        
        let report_id = data[0];
        let command = ImageCommand::from(data[1]);
        
        // Detect protocol based on command byte
        match command {
            ImageCommand::WriteImage => {
                // Mini protocol - 16 byte header
                if data.len() < Self::SIZE_MINI {
                    return None;
                }
                Some(Self {
                    report_id,
                    command,
                    page_number: data[2] as u16,
                    is_last: data[4] != 0,
                    key_index: data[5].saturating_sub(1), // Convert 1-based to 0-based
                    chunk_size: (data.len() - Self::SIZE_MINI) as u16,
                    protocol: ImageProtocol::Module6,
                    header_size: Self::SIZE_MINI,
                })
            }
            ImageCommand::UpdateKeyImage | ImageCommand::UpdateFullScreen => {
                // MK2/XL protocol - 8 byte header
                // [0] Report ID
                // [1] Command (0x07 or 0x08)
                // [2] Key Index (0-based)
                // [3] Is Last (transfer done flag)
                // [4-5] Chunk Size (u16 LE)
                // [6-7] Chunk Index (u16 LE)
                let key_index = data[2];
                let is_last = data[3] != 0;
                let chunk_size = u16::from_le_bytes([data[4], data[5]]);
                let page_number = u16::from_le_bytes([data[6], data[7]]);
                
                Some(Self {
                    report_id,
                    command,
                    page_number,
                    is_last,
                    key_index,
                    chunk_size,
                    protocol: ImageProtocol::Module15_32,
                    header_size: Self::SIZE_MK2_XL,
                })
            }
            ImageCommand::Unknown(_) => {
                log::warn!("Unknown image command: 0x{:02X}", data[1]);
                None
            }
        }
    }
}

/// An image packet containing header and payload data
#[derive(Debug, Clone)]
pub struct ImagePacket {
    /// Parsed header
    pub header: ImagePacketHeader,
    /// Image data payload (after header)
    pub payload: Vec<u8>,
}

impl ImagePacket {
    /// Parse a complete packet from raw output report data
    pub fn parse(data: &[u8]) -> Option<Self> {
        let header = ImagePacketHeader::parse(data)?;
        
        // Extract payload based on detected header size
        let payload = if data.len() > header.header_size {
            // For MK2/XL, only take chunk_size bytes of payload
            let payload_start = header.header_size;
            let payload_end = match header.protocol {
                ImageProtocol::Module6 => data.len(),
                ImageProtocol::Module15_32 => {
                    // Use the chunk_size field, but don't exceed available data
                    (payload_start + header.chunk_size as usize).min(data.len())
                }
            };
            data[payload_start..payload_end].to_vec()
        } else {
            Vec::new()
        };

        Some(Self { header, payload })
    }
}

/// Builder for accumulating multi-packet images
#[derive(Debug)]
struct ImageBuilder {
    /// Button index this image is for (0-based)
    key_index: u8,
    /// Accumulated image data
    data: Vec<u8>,
    /// Expected next page number (u16 for MK2/XL support)
    next_page: u16,
    /// Whether the image is complete
    complete: bool,
}

impl ImageBuilder {
    fn new(key_index: u8) -> Self {
        Self {
            key_index,
            data: Vec::with_capacity(96 * 96 * 3), // Max JPEG estimate for XL (96x96)
            next_page: 0,
            complete: false,
        }
    }

    /// Add a packet to this image builder
    /// Returns true if the image is now complete
    fn add_packet(&mut self, packet: &ImagePacket) -> Result<bool, ImageError> {
        // Verify key index matches
        if packet.header.key_index != self.key_index {
            return Err(ImageError::KeyMismatch {
                expected: self.key_index,
                got: packet.header.key_index,
            });
        }

        // Verify page sequence
        if packet.header.page_number != self.next_page {
            return Err(ImageError::PageSequenceError {
                expected: self.next_page,
                got: packet.header.page_number,
            });
        }

        // Append payload
        self.data.extend_from_slice(&packet.payload);
        self.next_page += 1;

        // Check if complete
        if packet.header.is_last {
            self.complete = true;
        }

        Ok(self.complete)
    }

    /// Reset the builder for a new image
    fn reset(&mut self) {
        self.data.clear();
        self.next_page = 0;
        self.complete = false;
    }
}

/// Errors that can occur during image handling
#[derive(Debug, Clone)]
pub enum ImageError {
    /// Packet too short to parse
    PacketTooShort { len: usize },
    /// Invalid report ID
    InvalidReportId { expected: u8, got: u8 },
    /// Key index mismatch in multi-packet image
    KeyMismatch { expected: u8, got: u8 },
    /// Page sequence error
    PageSequenceError { expected: u16, got: u16 },
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageError::PacketTooShort { len } => {
                write!(f, "Packet too short: {} bytes", len)
            }
            ImageError::InvalidReportId { expected, got } => {
                write!(f, "Invalid report ID: expected 0x{:02X}, got 0x{:02X}", expected, got)
            }
            ImageError::KeyMismatch { expected, got } => {
                write!(f, "Key index mismatch: expected {}, got {}", expected, got)
            }
            ImageError::PageSequenceError { expected, got } => {
                write!(f, "Page sequence error: expected {}, got {}", expected, got)
            }
        }
    }
}

impl std::error::Error for ImageError {}

/// A completed image for a button
#[derive(Debug, Clone)]
pub struct ButtonImage {
    /// Button index (0-based)
    pub key_index: u8,
    /// Raw image data (BMP for Mini, JPEG for MK2/XL)
    pub data: Vec<u8>,
    /// Timestamp when the image was received
    pub received_at: std::time::Instant,
}

impl ButtonImage {
    /// Get the image data as bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get the size of the image data
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the image is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Save the image to a file for debugging
    /// 
    /// The image is saved as a BMP file (since Stream Deck Mini uses BMP format).
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;
        file.write_all(&self.data)?;
        log::debug!("Saved debug image to {:?} ({} bytes)", path, self.data.len());
        Ok(())
    }
}

/// Thread-safe store for button images
/// 
/// This store receives image packets, accumulates multi-packet images,
/// and stores completed images for each button.
/// 
/// ## Subscribing to Events
/// 
/// Use [`ImageStore::subscribe`] to receive notifications when images are updated.
/// Multiple subscribers are supported.
#[derive(Clone)]
pub struct ImageStore {
    inner: Arc<Mutex<ImageStoreInner>>,
}

struct ImageStoreInner {
    /// Completed images indexed by key (1-based)
    images: HashMap<u8, ButtonImage>,
    /// In-progress image builders indexed by key (1-based)
    builders: HashMap<u8, ImageBuilder>,
    /// Statistics
    stats: ImageStats,
    /// Subscribers for image events
    subscribers: Vec<mpsc::Sender<ImageEvent>>,
}

/// Statistics about image reception
#[derive(Debug, Clone, Default)]
pub struct ImageStats {
    /// Total packets received
    pub packets_received: u64,
    /// Total images completed
    pub images_completed: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Errors encountered
    pub errors: u64,
}

impl ImageStore {
    /// Create a new image store
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ImageStoreInner {
                images: HashMap::new(),
                builders: HashMap::new(),
                stats: ImageStats::default(),
                subscribers: Vec::new(),
            })),
        }
    }

    /// Subscribe to image update events
    /// 
    /// Returns a receiver that will receive [`ImageEvent`] notifications
    /// whenever an image is updated. Multiple subscribers are supported.
    /// 
    /// The channel is unbounded, so subscribers should process events
    /// promptly to avoid memory buildup.
    pub fn subscribe(&self) -> ImageEventReceiver {
        let (tx, rx) = mpsc::channel();
        let mut inner = self.inner.lock().unwrap();
        inner.subscribers.push(tx);
        ImageEventReceiver { rx }
    }

    /// Process an incoming output report packet
    /// 
    /// Returns Ok(Some(key_index)) if an image was completed,
    /// Ok(None) if more packets are needed,
    /// Err if there was a parsing or protocol error.
    pub fn process_packet(&self, data: &[u8]) -> Result<Option<u8>, ImageError> {
        // Minimum size is MK2/XL header (8 bytes)
        if data.len() < ImagePacketHeader::SIZE_MK2_XL {
            return Err(ImageError::PacketTooShort { len: data.len() });
        }

        // Verify this is an image report
        if data[0] != 0x02 {
            return Err(ImageError::InvalidReportId { expected: 0x02, got: data[0] });
        }

        let packet = ImagePacket::parse(data)
            .ok_or(ImageError::PacketTooShort { len: data.len() })?;
        
        // Log first packet of each image for debugging
        if packet.header.page_number == 0 {
            log::info!(
                "Image transfer started: key={}, protocol={:?}, chunk_size={}, is_last={}",
                packet.header.key_index,
                packet.header.protocol,
                packet.header.chunk_size,
                packet.header.is_last
            );
        }

        let mut inner = self.inner.lock().unwrap();
        inner.stats.packets_received += 1;
        inner.stats.bytes_received += data.len() as u64;

        let key_index = packet.header.key_index;

        // Get or create builder for this key
        let builder = inner.builders
            .entry(key_index)
            .or_insert_with(|| ImageBuilder::new(key_index));

        // If starting a new image (page 0), reset the builder
        if packet.header.page_number == 0 {
            builder.reset();
        }

        // Add packet to builder
        let add_result = builder.add_packet(&packet);
        
        // Now we can access other fields of inner since we're done with builder
        match add_result {
            Ok(complete) => {
                if complete {
                    // Get the builder again to take its data
                    let builder = inner.builders.get_mut(&key_index).unwrap();
                    
                    // Image is complete, store it
                    // key_index is already 0-based (converted in header parsing)
                    let image = ButtonImage {
                        key_index,
                        data: std::mem::take(&mut builder.data),
                        received_at: std::time::Instant::now(),
                    };
                    
                    let image_size = image.len();
                    
                    // Notify subscribers (remove any that have been dropped)
                    let event = ImageEvent::Updated {
                        key_index,
                        image: image.clone(),
                    };
                    inner.subscribers.retain(|tx| tx.send(event.clone()).is_ok());
                    
                    inner.images.insert(key_index, image);
                    inner.stats.images_completed += 1;
                    
                    log::info!(
                        "Image complete for button {}: {} bytes",
                        key_index,
                        image_size
                    );
                    
                    Ok(Some(key_index))
                } else {
                    log::debug!(
                        "Image packet {}/? for button {} ({} bytes payload)",
                        packet.header.page_number + 1,
                        key_index,
                        packet.payload.len()
                    );
                    Ok(None)
                }
            }
            Err(e) => {
                inner.stats.errors += 1;
                // Reset builder on error to recover
                if let Some(builder) = inner.builders.get_mut(&key_index) {
                    builder.reset();
                }
                Err(e)
            }
        }
    }

    /// Get a completed image for a button (0-based index)
    /// 
    /// All indices are 0-based. The protocol conversion happens internally
    /// when parsing packets.
    pub fn get_image(&self, key_index: u8) -> Option<ButtonImage> {
        let inner = self.inner.lock().unwrap();
        inner.images.get(&key_index).cloned()
    }

    /// Get all completed images
    pub fn get_all_images(&self) -> HashMap<u8, ButtonImage> {
        let inner = self.inner.lock().unwrap();
        inner.images.clone()
    }

    /// Get statistics
    pub fn stats(&self) -> ImageStats {
        let inner = self.inner.lock().unwrap();
        inner.stats.clone()
    }

    /// Clear all stored images
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.images.clear();
        inner.builders.clear();
    }
}

impl Default for ImageStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a Mini-style test packet (16-byte header, command 0x01)
    /// key_index here is 1-based as the Mini protocol uses
    fn make_mini_test_packet(key_index_1based: u8, page: u8, is_last: bool, payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0u8; 16 + payload.len()];
        data[0] = 0x02;     // Report ID
        data[1] = 0x01;     // Command: write image (Mini)
        data[2] = page;     // Page number
        data[3] = 0x00;     // Reserved
        data[4] = if is_last { 1 } else { 0 };
        data[5] = key_index_1based;  // 1-based key index
        // bytes 6-15 are padding zeros
        data[16..].copy_from_slice(payload);
        data
    }
    
    /// Create a MK2/XL-style test packet (8-byte header, command 0x07)
    /// key_index is 0-based
    fn make_mk2_test_packet(key_index: u8, chunk_index: u16, is_last: bool, payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0u8; 8 + payload.len()];
        data[0] = 0x02;     // Report ID
        data[1] = 0x07;     // Command: update key image (MK2/XL)
        data[2] = key_index;  // 0-based key index
        data[3] = if is_last { 1 } else { 0 };
        // Chunk size (u16 LE)
        let chunk_size = payload.len() as u16;
        data[4] = (chunk_size & 0xFF) as u8;
        data[5] = ((chunk_size >> 8) & 0xFF) as u8;
        // Chunk index (u16 LE)
        data[6] = (chunk_index & 0xFF) as u8;
        data[7] = ((chunk_index >> 8) & 0xFF) as u8;
        data[8..].copy_from_slice(payload);
        data
    }

    #[test]
    fn test_parse_mini_header() {
        let packet = make_mini_test_packet(3, 0, false, &[]);
        let header = ImagePacketHeader::parse(&packet).unwrap();
        
        assert_eq!(header.report_id, 0x02);
        assert_eq!(header.command, ImageCommand::WriteImage);
        assert_eq!(header.page_number, 0);
        assert!(!header.is_last);
        assert_eq!(header.key_index, 2);  // 3-1 = 2 (converted to 0-based)
        assert_eq!(header.protocol, ImageProtocol::Module6);
    }
    
    #[test]
    fn test_parse_mk2_header() {
        let packet = make_mk2_test_packet(5, 0, true, &[0xFFu8; 100]);
        let header = ImagePacketHeader::parse(&packet).unwrap();
        
        assert_eq!(header.report_id, 0x02);
        assert_eq!(header.command, ImageCommand::UpdateKeyImage);
        assert_eq!(header.page_number, 0);
        assert!(header.is_last);
        assert_eq!(header.key_index, 5);  // Already 0-based
        assert_eq!(header.chunk_size, 100);
        assert_eq!(header.protocol, ImageProtocol::Module15_32);
    }

    #[test]
    fn test_single_packet_image_mini() {
        let store = ImageStore::new();
        
        let payload = vec![0xAB; 100];
        // Use 1-based key index 1 -> becomes 0-based 0
        let packet = make_mini_test_packet(1, 0, true, &payload);
        
        let result = store.process_packet(&packet).unwrap();
        assert_eq!(result, Some(0));  // Returns 0-based index
        
        let image = store.get_image(0).unwrap();
        assert_eq!(image.key_index, 0);
        assert_eq!(image.data, payload);
    }
    
    #[test]
    fn test_single_packet_image_mk2() {
        let store = ImageStore::new();
        
        let payload = vec![0xCD; 100];
        let packet = make_mk2_test_packet(5, 0, true, &payload);
        
        let result = store.process_packet(&packet).unwrap();
        assert_eq!(result, Some(5));
        
        let image = store.get_image(5).unwrap();
        assert_eq!(image.key_index, 5);
        assert_eq!(image.data, payload);
    }

    #[test]
    fn test_multi_packet_image_mini() {
        let store = ImageStore::new();
        
        // First packet (key index 2 in 1-based = 1 in 0-based)
        let payload1 = vec![0x11; 1008];
        let packet1 = make_mini_test_packet(2, 0, false, &payload1);
        let result = store.process_packet(&packet1).unwrap();
        assert_eq!(result, None);
        
        // Second packet
        let payload2 = vec![0x22; 1008];
        let packet2 = make_mini_test_packet(2, 1, false, &payload2);
        let result = store.process_packet(&packet2).unwrap();
        assert_eq!(result, None);
        
        // Final packet
        let payload3 = vec![0x33; 500];
        let packet3 = make_mini_test_packet(2, 2, true, &payload3);
        let result = store.process_packet(&packet3).unwrap();
        assert_eq!(result, Some(1));  // 0-based index
        
        let image = store.get_image(1).unwrap();
        assert_eq!(image.key_index, 1);
        assert_eq!(image.len(), 1008 + 1008 + 500);
    }
    
    #[test]
    fn test_multi_packet_image_mk2() {
        let store = ImageStore::new();
        
        // First packet
        let payload1 = vec![0x11; 500];
        let packet1 = make_mk2_test_packet(3, 0, false, &payload1);
        let result = store.process_packet(&packet1).unwrap();
        assert_eq!(result, None);
        
        // Final packet
        let payload2 = vec![0x22; 300];
        let packet2 = make_mk2_test_packet(3, 1, true, &payload2);
        let result = store.process_packet(&packet2).unwrap();
        assert_eq!(result, Some(3));
        
        let image = store.get_image(3).unwrap();
        assert_eq!(image.key_index, 3);
        assert_eq!(image.len(), 500 + 300);
    }

    #[test]
    fn test_page_sequence_error() {
        let store = ImageStore::new();
        
        // First packet
        let packet1 = make_mini_test_packet(1, 0, false, &[0x11; 100]);
        store.process_packet(&packet1).unwrap();
        
        // Skip page 1, send page 2
        let packet2 = make_mini_test_packet(1, 2, false, &[0x22; 100]);
        let result = store.process_packet(&packet2);
        
        assert!(matches!(result, Err(ImageError::PageSequenceError { .. })));
    }

    #[test]
    fn test_stats() {
        let store = ImageStore::new();
        
        let packet = make_mini_test_packet(1, 0, true, &[0xAB; 100]);
        store.process_packet(&packet).unwrap();
        
        let stats = store.stats();
        assert_eq!(stats.packets_received, 1);
        assert_eq!(stats.images_completed, 1);
        assert_eq!(stats.bytes_received, 116); // 16 header + 100 payload
        assert_eq!(stats.errors, 0);
    }
}
