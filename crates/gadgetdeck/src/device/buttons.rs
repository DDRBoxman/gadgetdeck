//! Button state management for Stream Deck devices
//!
//! This module provides a thread-safe way to track and update button states.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

use crate::usb::StreamDeckModel;

/// Maximum number of buttons supported (Stream Deck XL has 32)
const MAX_BUTTONS: usize = 32;

/// Thread-safe button state manager
#[derive(Debug)]
pub struct ButtonState {
    /// Button pressed states (true = pressed, false = released)
    buttons: [AtomicBool; MAX_BUTTONS],
    /// Number of buttons for this model
    num_buttons: AtomicU8,
    /// Flag indicating state has changed since last read
    changed: AtomicBool,
}

impl ButtonState {
    /// Create a new button state for the given model
    pub fn new(model: StreamDeckModel) -> Arc<Self> {
        let num_buttons = match model {
            StreamDeckModel::Mini => 6,
            StreamDeckModel::Pedal => 3,
            StreamDeckModel::Mk2 => 15,
            StreamDeckModel::Xl => 32,
        };
        
        Arc::new(Self {
            buttons: std::array::from_fn(|_| AtomicBool::new(false)),
            num_buttons: AtomicU8::new(num_buttons),
            changed: AtomicBool::new(false),
        })
    }
    
    /// Get the number of buttons
    pub fn num_buttons(&self) -> u8 {
        self.num_buttons.load(Ordering::Relaxed)
    }
    
    /// Check if a button is pressed
    pub fn is_pressed(&self, button: u8) -> bool {
        if button as usize >= MAX_BUTTONS {
            return false;
        }
        self.buttons[button as usize].load(Ordering::Relaxed)
    }
    
    /// Press a button (set state to pressed)
    pub fn press(&self, button: u8) {
        if button as usize >= MAX_BUTTONS {
            return;
        }
        let was_pressed = self.buttons[button as usize].swap(true, Ordering::Relaxed);
        if !was_pressed {
            self.changed.store(true, Ordering::Relaxed);
            log::info!("Button {} pressed", button);
        }
    }
    
    /// Release a button (set state to released)
    pub fn release(&self, button: u8) {
        if button as usize >= MAX_BUTTONS {
            return;
        }
        let was_pressed = self.buttons[button as usize].swap(false, Ordering::Relaxed);
        if was_pressed {
            self.changed.store(true, Ordering::Relaxed);
            log::info!("Button {} released", button);
        }
    }
    
    /// Press and hold a button for a duration, then release
    pub fn click(&self, button: u8) {
        self.press(button);
        std::thread::sleep(std::time::Duration::from_millis(50));
        self.release(button);
    }
    
    /// Check if state has changed and clear the flag
    pub fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
    }
    
    /// Build the input report for the current button state
    /// Returns a Vec<u8> with the appropriate format for the device model:
    /// - Mini (Module 6): Report ID 0x01 + button states (1 byte each)
    /// - Pedal: Report ID 0x01 + [0x00, 0x03, 0x00] header + button states at indices 3,4,5
    /// - MK2/XL (Module 15/32): Report ID 0x01 + Command 0x00 + Length (u16 LE) + button states
    pub fn build_input_report(&self, model: StreamDeckModel) -> Vec<u8> {
        let num_buttons = self.num_buttons.load(Ordering::Relaxed) as usize;
        
        match model {
            StreamDeckModel::Mini => {
                // Module 6 format: [Report ID, button states...]
                let hid_config = model.hid_config();
                let mut report = vec![0u8; hid_config.report_len as usize];
                report[0] = 0x01;  // Report ID
                
                // Each button is 1 byte (0x00 = released, 0x01 = pressed)
                for i in 0..num_buttons {
                    if i + 1 < report.len() {
                        report[i + 1] = if self.buttons[i].load(Ordering::Relaxed) { 0x01 } else { 0x00 };
                    }
                }
                
                report
            }
            StreamDeckModel::Pedal => {
                // Pedal format (7 bytes + report ID = 8 bytes total):
                // [0] Report ID (0x01)
                // [1] 0x00
                // [2] 0x03 - command/identifier
                // [3] 0x00
                // [4] Button 1 state (left pedal)
                // [5] Button 2 state (center pedal)
                // [6] Button 3 state (right pedal)
                // [7] 0x00
                let hid_config = model.hid_config();
                let mut report = vec![0u8; hid_config.report_len as usize];
                report[0] = 0x01;  // Report ID
                report[1] = 0x00;
                report[2] = 0x03;  // Command/identifier for button state
                report[3] = 0x00;
                
                // Button states at indices 4, 5, 6 (buttons 0, 1, 2)
                for i in 0..num_buttons.min(3) {
                    report[4 + i] = if self.buttons[i].load(Ordering::Relaxed) { 0x01 } else { 0x00 };
                }
                
                report
            }
            StreamDeckModel::Mk2 | StreamDeckModel::Xl => {
                // Module 15/32 format:
                // [0] Report ID (0x01)
                // [1] Command (0x00 for key press state change)
                // [2-3] Payload length (u16 LE) = number of keys
                // [4+] Key states
                // 
                // IMPORTANT: The HID report descriptor specifies 511 bytes of data (+ 1 report ID = 512 total).
                // We MUST send exactly 512 bytes for the host software to properly receive the report.
                let header_size = 4;
                let report_len = 512; // Fixed size per HID descriptor (Report ID + 511 bytes)
                let mut report = vec![0u8; report_len];
                
                report[0] = 0x01;  // Report ID
                report[1] = 0x00;  // Command: key press state change
                
                // Payload length (u16 LE) = number of keys
                let payload_len = num_buttons as u16;
                report[2] = (payload_len & 0xFF) as u8;
                report[3] = ((payload_len >> 8) & 0xFF) as u8;
                
                // Key states starting at offset 4
                for i in 0..num_buttons {
                    report[header_size + i] = if self.buttons[i].load(Ordering::Relaxed) { 0x01 } else { 0x00 };
                }
                
                // Remaining bytes (after key states) are already zero-initialized
                report
            }
        }
    }
}

impl Clone for ButtonState {
    fn clone(&self) -> Self {
        Self {
            buttons: std::array::from_fn(|i| AtomicBool::new(self.buttons[i].load(Ordering::Relaxed))),
            num_buttons: AtomicU8::new(self.num_buttons.load(Ordering::Relaxed)),
            changed: AtomicBool::new(self.changed.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_state_mini() {
        let state = ButtonState::new(StreamDeckModel::Mini);
        assert_eq!(state.num_buttons(), 6);
        
        // All buttons start released
        for i in 0..6 {
            assert!(!state.is_pressed(i));
        }
        
        // Press button 0
        state.press(0);
        assert!(state.is_pressed(0));
        assert!(state.take_changed());
        assert!(!state.take_changed());  // Changed flag should be cleared
        
        // Release button 0
        state.release(0);
        assert!(!state.is_pressed(0));
        assert!(state.take_changed());
    }

    #[test]
    fn test_input_report_format_mini() {
        let state = ButtonState::new(StreamDeckModel::Mini);
        
        // Press buttons 0 and 2
        state.press(0);
        state.press(2);
        
        let report = state.build_input_report(StreamDeckModel::Mini);
        assert_eq!(report[0], 0x01);  // Report ID
        assert_eq!(report[1], 0x01);  // Button 0 pressed
        assert_eq!(report[2], 0x00);  // Button 1 released
        assert_eq!(report[3], 0x01);  // Button 2 pressed
    }
    
    #[test]
    fn test_input_report_format_mk2() {
        let state = ButtonState::new(StreamDeckModel::Mk2);
        
        // Press buttons 0 and 5
        state.press(0);
        state.press(5);
        
        let report = state.build_input_report(StreamDeckModel::Mk2);
        assert_eq!(report[0], 0x01);  // Report ID
        assert_eq!(report[1], 0x00);  // Command: key press state change
        assert_eq!(report[2], 15);    // Payload length low byte (15 keys)
        assert_eq!(report[3], 0);     // Payload length high byte
        assert_eq!(report[4], 0x01);  // Button 0 pressed
        assert_eq!(report[5], 0x00);  // Button 1 released
        assert_eq!(report[9], 0x01);  // Button 5 pressed
    }
    
    #[test]
    fn test_input_report_format_xl() {
        let state = ButtonState::new(StreamDeckModel::Xl);
        
        // Press button 31 (last button)
        state.press(31);
        
        let report = state.build_input_report(StreamDeckModel::Xl);
        assert_eq!(report.len(), 512);  // Must be exactly 512 bytes per HID descriptor
        assert_eq!(report[0], 0x01);  // Report ID
        assert_eq!(report[1], 0x00);  // Command: key press state change
        assert_eq!(report[2], 32);    // Payload length low byte (32 keys)
        assert_eq!(report[3], 0);     // Payload length high byte
        assert_eq!(report[35], 0x01); // Button 31 pressed (4 header + 31 = 35)
    }
    
    #[test]
    fn test_input_report_length_mk2() {
        let state = ButtonState::new(StreamDeckModel::Mk2);
        let report = state.build_input_report(StreamDeckModel::Mk2);
        // MK2/XL must send exactly 512 bytes to match HID report descriptor
        assert_eq!(report.len(), 512);
    }
}
