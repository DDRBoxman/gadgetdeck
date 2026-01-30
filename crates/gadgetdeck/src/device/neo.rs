//! Stream Deck Neo LED state management
//!
//! This module provides thread-safe state management for Stream Deck Neo
//! LED colors and the info bar LCD.
//!
//! ## Button LEDs
//! The Neo has 10 buttons: 8 standard keys (indices 0-7) and 2 additional
//! buttons below the info bar (indices 8-9). Buttons 8-9 have RGB LED strips
//! that can be controlled independently of the button press state.
//!
//! ## Info Bar LCD
//! The Neo has a 248x58 pixel info bar LCD between the buttons.
//! Images are JPEG format, rotated 180° (upside down).
//! Unlike the Plus, the Neo only supports full-screen LCD updates (no regions).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// Button indices that have LED capability on the Neo
pub const NEO_LED_BUTTON_LEFT: u8 = 8;
pub const NEO_LED_BUTTON_RIGHT: u8 = 9;

/// RGB color for button LEDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    /// Create a new RGB color
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Black (off)
    pub fn black() -> Self {
        Self::new(0, 0, 0)
    }

    /// White
    pub fn white() -> Self {
        Self::new(255, 255, 255)
    }

    /// Red
    pub fn red() -> Self {
        Self::new(255, 0, 0)
    }

    /// Green
    pub fn green() -> Self {
        Self::new(0, 255, 0)
    }

    /// Blue
    pub fn blue() -> Self {
        Self::new(0, 0, 255)
    }
}

/// Thread-safe LED state for Stream Deck Neo
///
/// This manages LED colors for buttons 8-9 (the buttons with RGB LED strips).
/// Button press states are handled through ButtonState.
#[derive(Debug)]
pub struct NeoInputState {
    /// LED colors for buttons 8-9 (stored at indices 0-1)
    led_colors: [AtomicRgb; 2],
}

/// Atomic RGB color storage
#[derive(Debug)]
struct AtomicRgb {
    r: AtomicU8,
    g: AtomicU8,
    b: AtomicU8,
}

impl AtomicRgb {
    fn new(color: RgbColor) -> Self {
        Self {
            r: AtomicU8::new(color.r),
            g: AtomicU8::new(color.g),
            b: AtomicU8::new(color.b),
        }
    }

    fn load(&self) -> RgbColor {
        RgbColor {
            r: self.r.load(Ordering::Relaxed),
            g: self.g.load(Ordering::Relaxed),
            b: self.b.load(Ordering::Relaxed),
        }
    }

    fn store(&self, color: RgbColor) {
        self.r.store(color.r, Ordering::Relaxed);
        self.g.store(color.g, Ordering::Relaxed);
        self.b.store(color.b, Ordering::Relaxed);
    }
}

impl NeoInputState {
    /// Create a new NeoInputState with LEDs off (black)
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            led_colors: [
                AtomicRgb::new(RgbColor::black()),
                AtomicRgb::new(RgbColor::black()),
            ],
        })
    }

    /// Check if a button has LED capability
    pub fn has_led(button: u8) -> bool {
        button == NEO_LED_BUTTON_LEFT || button == NEO_LED_BUTTON_RIGHT
    }

    /// Convert button index to internal LED index (returns None if button has no LED)
    fn button_to_led_index(button: u8) -> Option<usize> {
        match button {
            NEO_LED_BUTTON_LEFT => Some(0),
            NEO_LED_BUTTON_RIGHT => Some(1),
            _ => None,
        }
    }

    /// Get the LED color for a button (returns None if button has no LED)
    pub fn get_led_color(&self, button: u8) -> Option<RgbColor> {
        Self::button_to_led_index(button).map(|idx| self.led_colors[idx].load())
    }

    /// Set the LED color for a button (returns false if button has no LED)
    ///
    /// Note: This only stores the color locally. The actual LED update
    /// is sent to the device via SET_REPORT command 0x06.
    pub fn set_led_color(&self, button: u8, color: RgbColor) -> bool {
        if let Some(idx) = Self::button_to_led_index(button) {
            self.led_colors[idx].store(color);
            log::info!(
                "Button {} LED color set to RGB({}, {}, {})",
                button,
                color.r,
                color.g,
                color.b
            );
            true
        } else {
            false
        }
    }

    /// Build a SET_REPORT payload for button LED color (Report ID 0x03, Command 0x06)
    /// Returns None if the button has no LED.
    ///
    /// Format per research:
    /// [0] Report ID (0x03)
    /// [1] Command (0x06)
    /// [2] Button index (8 for left, 9 for right)
    /// [3] Red (0x00-0xFF)
    /// [4] Green (0x00-0xFF)
    /// [5] Blue (0x00-0xFF)
    pub fn build_led_color_report(&self, button: u8) -> Option<Vec<u8>> {
        let color = self.get_led_color(button)?;
        let mut report = vec![0u8; 32];
        report[0] = 0x03; // Report ID
        report[1] = 0x06; // Command: set LED color
        report[2] = button; // Button index (8 or 9)
        report[3] = color.r;
        report[4] = color.g;
        report[5] = color.b;
        Some(report)
    }

    /// Get all buttons that have LEDs
    pub fn led_buttons() -> &'static [u8] {
        &[NEO_LED_BUTTON_LEFT, NEO_LED_BUTTON_RIGHT]
    }
}

impl Default for NeoInputState {
    fn default() -> Self {
        Self {
            led_colors: [
                AtomicRgb::new(RgbColor::black()),
                AtomicRgb::new(RgbColor::black()),
            ],
        }
    }
}

impl Clone for NeoInputState {
    fn clone(&self) -> Self {
        Self {
            led_colors: [
                AtomicRgb::new(self.led_colors[0].load()),
                AtomicRgb::new(self.led_colors[1].load()),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_led_button_indices() {
        assert_eq!(NEO_LED_BUTTON_LEFT, 8);
        assert_eq!(NEO_LED_BUTTON_RIGHT, 9);
        assert!(NeoInputState::has_led(8));
        assert!(NeoInputState::has_led(9));
        assert!(!NeoInputState::has_led(0));
        assert!(!NeoInputState::has_led(7));
    }

    #[test]
    fn test_rgb_color() {
        let red = RgbColor::red();
        assert_eq!(red.r, 255);
        assert_eq!(red.g, 0);
        assert_eq!(red.b, 0);

        let custom = RgbColor::new(128, 64, 32);
        assert_eq!(custom.r, 128);
        assert_eq!(custom.g, 64);
        assert_eq!(custom.b, 32);
    }

    #[test]
    fn test_neo_input_state() {
        let state = NeoInputState::new();

        // Default is black (off)
        let left_color = state.get_led_color(8);
        assert_eq!(left_color, Some(RgbColor::black()));

        // Set a color
        assert!(state.set_led_color(8, RgbColor::red()));
        let left_color = state.get_led_color(8);
        assert_eq!(left_color, Some(RgbColor::red()));

        // Right should still be black
        let right_color = state.get_led_color(9);
        assert_eq!(right_color, Some(RgbColor::black()));

        // Non-LED button returns None
        assert_eq!(state.get_led_color(0), None);
        assert!(!state.set_led_color(0, RgbColor::red()));
    }

    #[test]
    fn test_led_color_report() {
        let state = NeoInputState::new();
        state.set_led_color(9, RgbColor::new(100, 150, 200));

        let report = state.build_led_color_report(9).unwrap();
        assert_eq!(report[0], 0x03); // Report ID
        assert_eq!(report[1], 0x06); // Command
        assert_eq!(report[2], 9); // Button index
        assert_eq!(report[3], 100); // Red
        assert_eq!(report[4], 150); // Green
        assert_eq!(report[5], 200); // Blue

        // Non-LED button returns None
        assert!(state.build_led_color_report(0).is_none());
    }
}
