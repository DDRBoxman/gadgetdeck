//! Stream Deck Neo input state management
//!
//! This module provides thread-safe state management for Stream Deck Neo
//! specific features: touch point LED colors and the info bar LCD.
//!
//! ## Touch Points
//! The Neo has 2 touch points (left and right) below the info bar.
//! Unlike the Plus's knobs, these are simple buttons - their press state
//! is handled through ButtonState (indices 8-9).
//! Each touch point has an RGB LED strip that can be controlled.
//!
//! ## Info Bar LCD
//! The Neo has a 248x58 pixel info bar LCD between the buttons and touch points.
//! Images are JPEG format, rotated 180° (upside down).
//! Unlike the Plus, the Neo only supports full-screen LCD updates (no regions).

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Touch point index for the Neo
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NeoTouchPoint {
    /// Left touch point (index 8 in button state)
    Left = 0,
    /// Right touch point (index 9 in button state)
    Right = 1,
}

impl NeoTouchPoint {
    /// Get the button state index for this touch point
    /// Touch points are at indices 8-9 (after the 8 keys)
    pub fn button_index(&self) -> u8 {
        8 + *self as u8
    }

    /// Get the key count offset (for SET_REPORT command)
    /// Per research: key_count + point_index = 8 for left, 9 for right
    pub fn key_offset(&self) -> u8 {
        8 + *self as u8
    }
}

impl From<u8> for NeoTouchPoint {
    fn from(value: u8) -> Self {
        match value {
            0 => NeoTouchPoint::Left,
            _ => NeoTouchPoint::Right,
        }
    }
}

/// RGB color for touch point LED
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

/// Thread-safe input state for Stream Deck Neo specific features
/// 
/// This handles touch point LED colors.
/// Button/touch point press states are handled through ButtonState (8 keys + 2 touch points).
#[derive(Debug)]
pub struct NeoInputState {
    /// Touch point LED colors (left, right)
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

    /// Get the LED color for a touch point
    pub fn get_led_color(&self, point: NeoTouchPoint) -> RgbColor {
        self.led_colors[point as usize].load()
    }

    /// Set the LED color for a touch point
    /// 
    /// Note: This only stores the color locally. The actual LED update
    /// is sent to the device via SET_REPORT command 0x06.
    pub fn set_led_color(&self, point: NeoTouchPoint, color: RgbColor) {
        self.led_colors[point as usize].store(color);
        log::info!(
            "Touch point {:?} LED color set to RGB({}, {}, {})",
            point, color.r, color.g, color.b
        );
    }

    /// Build a SET_REPORT payload for touch point LED color (Report ID 0x03, Command 0x06)
    /// 
    /// Format per research:
    /// [0] Report ID (0x03)
    /// [1] Command (0x06)
    /// [2] Touch Point Index (key_count + point_index: 8 for left, 9 for right)
    /// [3] Red (0x00-0xFF)
    /// [4] Green (0x00-0xFF)
    /// [5] Blue (0x00-0xFF)
    pub fn build_led_color_report(&self, point: NeoTouchPoint) -> Vec<u8> {
        let color = self.get_led_color(point);
        let mut report = vec![0u8; 32];
        report[0] = 0x03;  // Report ID
        report[1] = 0x06;  // Command: set touch point color
        report[2] = point.key_offset();  // Touch point index (8 or 9)
        report[3] = color.r;
        report[4] = color.g;
        report[5] = color.b;
        report
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
    fn test_touch_point_indices() {
        assert_eq!(NeoTouchPoint::Left.button_index(), 8);
        assert_eq!(NeoTouchPoint::Right.button_index(), 9);
        assert_eq!(NeoTouchPoint::Left.key_offset(), 8);
        assert_eq!(NeoTouchPoint::Right.key_offset(), 9);
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
        let left_color = state.get_led_color(NeoTouchPoint::Left);
        assert_eq!(left_color, RgbColor::black());

        // Set a color
        state.set_led_color(NeoTouchPoint::Left, RgbColor::red());
        let left_color = state.get_led_color(NeoTouchPoint::Left);
        assert_eq!(left_color, RgbColor::red());

        // Right should still be black
        let right_color = state.get_led_color(NeoTouchPoint::Right);
        assert_eq!(right_color, RgbColor::black());
    }

    #[test]
    fn test_led_color_report() {
        let state = NeoInputState::new();
        state.set_led_color(NeoTouchPoint::Right, RgbColor::new(100, 150, 200));

        let report = state.build_led_color_report(NeoTouchPoint::Right);
        assert_eq!(report[0], 0x03);  // Report ID
        assert_eq!(report[1], 0x06);  // Command
        assert_eq!(report[2], 9);     // Touch point index (right = 9)
        assert_eq!(report[3], 100);   // Red
        assert_eq!(report[4], 150);   // Green
        assert_eq!(report[5], 200);   // Blue
    }
}
