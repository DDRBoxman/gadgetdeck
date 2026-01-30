//! Stream Deck Plus input state management
//!
//! This module provides thread-safe state management for Stream Deck Plus
//! specific input events: touchscreen touches/swipes and rotary encoder (knob) events.
//!
//! ## Touch Event Types
//! The touchscreen strip on the Plus supports three event types:
//! - SHORT: Quick tap
//! - LONG: Long press
//! - DRAG: Swipe gesture with start and end coordinates
//!
//! ## Knob Events
//! Each of the 4 knobs (A-D) supports:
//! - Press/Release
//! - Rotation (clockwise/counter-clockwise)

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI8, AtomicU8, AtomicU16, Ordering};

/// Touch event types for the LCD touchscreen strip
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TouchEventType {
    /// Short tap
    Short = 0x01,
    /// Long press
    Long = 0x02,
    /// Drag/swipe gesture
    Drag = 0x03,
}

/// A touchscreen event to be sent to the host
#[derive(Debug, Clone, Copy)]
pub struct TouchEvent {
    /// Event type (SHORT, LONG, or DRAG)
    pub event_type: TouchEventType,
    /// X coordinate of touch start (0-799)
    pub x: u16,
    /// Y coordinate of touch start (0-99)
    pub y: u16,
    /// X coordinate of touch end (0-799), only valid for DRAG events
    pub x_end: u16,
    /// Y coordinate of touch end (0-99), only valid for DRAG events
    pub y_end: u16,
}

impl TouchEvent {
    /// Create a short tap event
    pub fn short_tap(x: u16, y: u16) -> Self {
        Self {
            event_type: TouchEventType::Short,
            x: x.min(799),
            y: y.min(99),
            x_end: 0,
            y_end: 0,
        }
    }

    /// Create a long press event
    pub fn long_press(x: u16, y: u16) -> Self {
        Self {
            event_type: TouchEventType::Long,
            x: x.min(799),
            y: y.min(99),
            x_end: 0,
            y_end: 0,
        }
    }

    /// Create a drag/swipe event
    pub fn drag(x_start: u16, y_start: u16, x_end: u16, y_end: u16) -> Self {
        Self {
            event_type: TouchEventType::Drag,
            x: x_start.min(799),
            y: y_start.min(99),
            x_end: x_end.min(799),
            y_end: y_end.min(99),
        }
    }

    /// Create a horizontal swipe event (convenience for common use case)
    /// y is centered at 50 (middle of strip)
    pub fn swipe_horizontal(x_start: u16, x_end: u16) -> Self {
        Self::drag(x_start, 50, x_end, 50)
    }

    /// Get which segment (A=0, B=1, C=2, D=3) the touch start is in
    pub fn segment(&self) -> u8 {
        (self.x / 200).min(3) as u8
    }

    /// Get which segment the touch end is in (for DRAG events)
    pub fn end_segment(&self) -> u8 {
        (self.x_end / 200).min(3) as u8
    }

    /// Build the 512-byte input report for this touch event
    ///
    /// The Plus HID descriptor specifies 512-byte input reports.
    /// Format (first 14 bytes, rest are zero-padded):
    /// [0] Report ID (0x01)
    /// [1] Event Type indicator (0x02 = touchscreen)
    /// [2-3] Payload length (0x0E 0x00 = 14)
    /// [4] Touch Event Type: 1=SHORT, 2=LONG, 3=DRAG
    /// [5] Always 0x01
    /// [6-7] X coordinate (u16 LE)
    /// [8-9] Y coordinate (u16 LE)
    /// [10-11] X_out coordinate (u16 LE) - for DRAG events
    /// [12-13] Y_out coordinate (u16 LE) - for DRAG events
    pub fn build_input_report(&self) -> Vec<u8> {
        // Must be 512 bytes to match HID descriptor
        let mut report = vec![0u8; 512];
        report[0] = 0x01; // Report ID
        report[1] = 0x02; // Event type indicator = touchscreen
        report[2] = 0x0E; // Payload length low byte (14)
        report[3] = 0x00; // Payload length high byte
        report[4] = self.event_type as u8;
        report[5] = 0x01; // Always 0x01
        report[6] = (self.x & 0xFF) as u8;
        report[7] = ((self.x >> 8) & 0xFF) as u8;
        report[8] = (self.y & 0xFF) as u8;
        report[9] = ((self.y >> 8) & 0xFF) as u8;
        report[10] = (self.x_end & 0xFF) as u8;
        report[11] = ((self.x_end >> 8) & 0xFF) as u8;
        report[12] = (self.y_end & 0xFF) as u8;
        report[13] = ((self.y_end >> 8) & 0xFF) as u8;
        report
    }
}

/// Knob (rotary encoder) index
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum KnobIndex {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
}

impl From<u8> for KnobIndex {
    fn from(value: u8) -> Self {
        match value {
            0 => KnobIndex::A,
            1 => KnobIndex::B,
            2 => KnobIndex::C,
            _ => KnobIndex::D,
        }
    }
}

/// Knob rotation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnobDirection {
    /// Clockwise rotation
    Clockwise,
    /// Counter-clockwise rotation
    CounterClockwise,
}

/// A knob event to be sent to the host
#[derive(Debug, Clone, Copy)]
pub enum KnobEvent {
    /// Knob was pressed
    Press(KnobIndex),
    /// Knob was released
    Release(KnobIndex),
    /// Knob was rotated (includes direction and steps)
    Turn {
        knob: KnobIndex,
        direction: KnobDirection,
        steps: u8,
    },
}

impl KnobEvent {
    /// Build the 512-byte input report for this knob event
    ///
    /// The Plus HID descriptor specifies 512-byte input reports.
    /// Format (first 9 bytes, rest are zero-padded):
    /// [0] Report ID (0x01)
    /// [1] Event type indicator (0x03 = knob)
    /// [2] Payload length (0x05)
    /// [3] 0x00
    /// [4] IsTurn: 0x01 = turning, 0x00 = pressing
    /// [5] Knob A value
    /// [6] Knob B value
    /// [7] Knob C value
    /// [8] Knob D value
    ///
    /// For turn events: 0x01 = clockwise, 0xFF = counter-clockwise
    /// For press events: 0x01 = pressed, 0x00 = released
    pub fn build_input_report(&self) -> Vec<u8> {
        // Must be 512 bytes to match HID descriptor
        let mut report = vec![0u8; 512];
        report[0] = 0x01; // Report ID
        report[1] = 0x03; // Event type indicator = knob
        report[2] = 0x05; // Payload length (5)
        report[3] = 0x00;

        match self {
            KnobEvent::Press(knob) => {
                report[4] = 0x00; // IsTurn = false (pressing)
                report[5 + *knob as usize] = 0x01; // Pressed
            }
            KnobEvent::Release(_knob) => {
                report[4] = 0x00; // IsTurn = false (pressing)
                // All zeros = all released
                // We need to only release this knob - but the protocol sends full state
                // For simplicity, we leave all at 0x00 (all released)
            }
            KnobEvent::Turn {
                knob,
                direction,
                steps,
            } => {
                report[4] = 0x01; // IsTurn = true
                let value = match direction {
                    KnobDirection::Clockwise => (*steps).min(127),
                    KnobDirection::CounterClockwise => (256u16 - *steps as u16) as u8,
                };
                report[5 + *knob as usize] = value;
            }
        }

        report
    }
}

/// Maximum number of knobs (Stream Deck Plus has 4)
const MAX_KNOBS: usize = 4;

/// Thread-safe input state for Stream Deck Plus specific features
///
/// This handles touchscreen events and knob (rotary encoder) events.
/// Use in conjunction with ButtonState for full Plus functionality.
#[derive(Debug)]
pub struct PlusInputState {
    /// Pending touch event (None if no event pending)
    touch_event_type: AtomicU8,
    touch_x: AtomicU16,
    touch_y: AtomicU16,
    touch_x_end: AtomicU16,
    touch_y_end: AtomicU16,
    touch_pending: AtomicBool,

    /// Knob pressed states
    knob_pressed: [AtomicBool; MAX_KNOBS],

    /// Pending knob events
    /// Positive = clockwise steps, Negative = counter-clockwise steps
    /// 0 = no pending rotation
    knob_rotation: [AtomicI8; MAX_KNOBS],

    /// Flag indicating knob state has changed since last read
    knob_changed: AtomicBool,
}

impl PlusInputState {
    /// Create a new PlusInputState
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            touch_event_type: AtomicU8::new(0),
            touch_x: AtomicU16::new(0),
            touch_y: AtomicU16::new(0),
            touch_x_end: AtomicU16::new(0),
            touch_y_end: AtomicU16::new(0),
            touch_pending: AtomicBool::new(false),
            knob_pressed: std::array::from_fn(|_| AtomicBool::new(false)),
            knob_rotation: std::array::from_fn(|_| AtomicI8::new(0)),
            knob_changed: AtomicBool::new(false),
        })
    }

    /// Queue a touch event to be sent
    pub fn send_touch(&self, event: TouchEvent) {
        self.touch_event_type
            .store(event.event_type as u8, Ordering::Relaxed);
        self.touch_x.store(event.x, Ordering::Relaxed);
        self.touch_y.store(event.y, Ordering::Relaxed);
        self.touch_x_end.store(event.x_end, Ordering::Relaxed);
        self.touch_y_end.store(event.y_end, Ordering::Relaxed);
        self.touch_pending.store(true, Ordering::Release);
        log::info!(
            "Touch event queued: {:?} at ({}, {})",
            event.event_type,
            event.x,
            event.y
        );
    }

    /// Queue a short tap event
    pub fn tap(&self, x: u16, y: u16) {
        self.send_touch(TouchEvent::short_tap(x, y));
    }

    /// Queue a long press event
    pub fn long_press(&self, x: u16, y: u16) {
        self.send_touch(TouchEvent::long_press(x, y));
    }

    /// Queue a drag/swipe event
    pub fn swipe(&self, x_start: u16, y_start: u16, x_end: u16, y_end: u16) {
        self.send_touch(TouchEvent::drag(x_start, y_start, x_end, y_end));
    }

    /// Queue a horizontal swipe (common use case)
    pub fn swipe_horizontal(&self, x_start: u16, x_end: u16) {
        self.send_touch(TouchEvent::swipe_horizontal(x_start, x_end));
    }

    /// Take the pending touch event (returns None if no event pending)
    pub fn take_touch_event(&self) -> Option<TouchEvent> {
        if self.touch_pending.swap(false, Ordering::Acquire) {
            let event_type = match self.touch_event_type.load(Ordering::Relaxed) {
                0x01 => TouchEventType::Short,
                0x02 => TouchEventType::Long,
                0x03 => TouchEventType::Drag,
                _ => return None,
            };
            Some(TouchEvent {
                event_type,
                x: self.touch_x.load(Ordering::Relaxed),
                y: self.touch_y.load(Ordering::Relaxed),
                x_end: self.touch_x_end.load(Ordering::Relaxed),
                y_end: self.touch_y_end.load(Ordering::Relaxed),
            })
        } else {
            None
        }
    }

    /// Press a knob
    pub fn press_knob(&self, knob: KnobIndex) {
        let idx = knob as usize;
        if idx < MAX_KNOBS {
            let was_pressed = self.knob_pressed[idx].swap(true, Ordering::Relaxed);
            if !was_pressed {
                self.knob_changed.store(true, Ordering::Release);
                log::info!("Knob {:?} pressed", knob);
            }
        }
    }

    /// Release a knob
    pub fn release_knob(&self, knob: KnobIndex) {
        let idx = knob as usize;
        if idx < MAX_KNOBS {
            let was_pressed = self.knob_pressed[idx].swap(false, Ordering::Relaxed);
            if was_pressed {
                self.knob_changed.store(true, Ordering::Release);
                log::info!("Knob {:?} released", knob);
            }
        }
    }

    /// Turn a knob (positive = clockwise, negative = counter-clockwise)
    pub fn turn_knob(&self, knob: KnobIndex, steps: i8) {
        let idx = knob as usize;
        if idx < MAX_KNOBS && steps != 0 {
            // Accumulate rotation steps
            self.knob_rotation[idx].fetch_add(steps, Ordering::Relaxed);
            self.knob_changed.store(true, Ordering::Release);
            let direction = if steps > 0 {
                "clockwise"
            } else {
                "counter-clockwise"
            };
            log::info!(
                "Knob {:?} turned {} ({} steps)",
                knob,
                direction,
                steps.abs()
            );
        }
    }

    /// Check if a knob is pressed
    pub fn is_knob_pressed(&self, knob: KnobIndex) -> bool {
        let idx = knob as usize;
        if idx < MAX_KNOBS {
            self.knob_pressed[idx].load(Ordering::Relaxed)
        } else {
            false
        }
    }

    /// Take the pending knob rotation for a knob (resets to 0)
    pub fn take_knob_rotation(&self, knob: KnobIndex) -> i8 {
        let idx = knob as usize;
        if idx < MAX_KNOBS {
            self.knob_rotation[idx].swap(0, Ordering::Relaxed)
        } else {
            0
        }
    }

    /// Check if any knob state has changed and clear the flag
    pub fn take_knob_changed(&self) -> bool {
        self.knob_changed.swap(false, Ordering::Acquire)
    }

    /// Build a knob press input report (512 bytes to match HID descriptor)
    pub fn build_knob_press_report(&self) -> Vec<u8> {
        let mut report = vec![0u8; 512];
        report[0] = 0x01; // Report ID
        report[1] = 0x03; // Event type = knob
        report[2] = 0x05; // Payload length
        report[3] = 0x00;
        report[4] = 0x00; // IsTurn = false (pressing)

        for i in 0..MAX_KNOBS {
            report[5 + i] = if self.knob_pressed[i].load(Ordering::Relaxed) {
                0x01
            } else {
                0x00
            };
        }

        report
    }

    /// Build a knob turn input report for a specific knob (512 bytes to match HID descriptor)
    pub fn build_knob_turn_report(&self, knob: KnobIndex, steps: i8) -> Vec<u8> {
        let mut report = vec![0u8; 512];
        report[0] = 0x01; // Report ID
        report[1] = 0x03; // Event type = knob
        report[2] = 0x05; // Payload length
        report[3] = 0x00;
        report[4] = 0x01; // IsTurn = true

        let idx = knob as usize;
        if idx < MAX_KNOBS {
            let value = if steps > 0 {
                steps.min(127) as u8
            } else {
                (256 + steps as i16) as u8 // e.g., -1 becomes 255 (0xFF)
            };
            report[5 + idx] = value;
        }

        report
    }
}

impl Default for PlusInputState {
    fn default() -> Self {
        Self {
            touch_event_type: AtomicU8::new(0),
            touch_x: AtomicU16::new(0),
            touch_y: AtomicU16::new(0),
            touch_x_end: AtomicU16::new(0),
            touch_y_end: AtomicU16::new(0),
            touch_pending: AtomicBool::new(false),
            knob_pressed: std::array::from_fn(|_| AtomicBool::new(false)),
            knob_rotation: std::array::from_fn(|_| AtomicI8::new(0)),
            knob_changed: AtomicBool::new(false),
        }
    }
}

impl Clone for PlusInputState {
    fn clone(&self) -> Self {
        Self {
            touch_event_type: AtomicU8::new(self.touch_event_type.load(Ordering::Relaxed)),
            touch_x: AtomicU16::new(self.touch_x.load(Ordering::Relaxed)),
            touch_y: AtomicU16::new(self.touch_y.load(Ordering::Relaxed)),
            touch_x_end: AtomicU16::new(self.touch_x_end.load(Ordering::Relaxed)),
            touch_y_end: AtomicU16::new(self.touch_y_end.load(Ordering::Relaxed)),
            touch_pending: AtomicBool::new(self.touch_pending.load(Ordering::Relaxed)),
            knob_pressed: std::array::from_fn(|i| {
                AtomicBool::new(self.knob_pressed[i].load(Ordering::Relaxed))
            }),
            knob_rotation: std::array::from_fn(|i| {
                AtomicI8::new(self.knob_rotation[i].load(Ordering::Relaxed))
            }),
            knob_changed: AtomicBool::new(self.knob_changed.load(Ordering::Relaxed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_event_short_tap() {
        let event = TouchEvent::short_tap(400, 50);
        assert_eq!(event.event_type, TouchEventType::Short);
        assert_eq!(event.x, 400);
        assert_eq!(event.y, 50);
        assert_eq!(event.segment(), 2); // 400/200 = 2 (segment C)
    }

    #[test]
    fn test_touch_event_drag() {
        let event = TouchEvent::drag(100, 50, 600, 50);
        assert_eq!(event.event_type, TouchEventType::Drag);
        assert_eq!(event.x, 100);
        assert_eq!(event.x_end, 600);
        assert_eq!(event.segment(), 0); // Start in segment A
        assert_eq!(event.end_segment(), 3); // End in segment D
    }

    #[test]
    fn test_touch_event_report() {
        let event = TouchEvent::short_tap(100, 50);
        let report = event.build_input_report();

        assert_eq!(report.len(), 512);
        assert_eq!(report[0], 0x01); // Report ID
        assert_eq!(report[1], 0x02); // Touchscreen event
        assert_eq!(report[4], 0x01); // SHORT event type
        assert_eq!(report[6], 100); // X low byte
        assert_eq!(report[7], 0); // X high byte
        assert_eq!(report[8], 50); // Y low byte
        assert_eq!(report[9], 0); // Y high byte
    }

    #[test]
    fn test_drag_event_report() {
        let event = TouchEvent::drag(100, 25, 500, 75);
        let report = event.build_input_report();

        assert_eq!(report[0], 0x01); // Report ID
        assert_eq!(report[1], 0x02); // Touchscreen event
        assert_eq!(report[4], 0x03); // DRAG event type
        assert_eq!(report[6], 100); // X start low byte
        assert_eq!(report[8], 25); // Y start low byte
        assert_eq!(report[10], 244); // X end low byte (500 & 0xFF = 244)
        assert_eq!(report[11], 1); // X end high byte (500 >> 8 = 1)
        assert_eq!(report[12], 75); // Y end low byte
    }

    #[test]
    fn test_knob_event_press_report() {
        let event = KnobEvent::Press(KnobIndex::B);
        let report = event.build_input_report();

        assert_eq!(report.len(), 512);
        assert_eq!(report[0], 0x01); // Report ID
        assert_eq!(report[1], 0x03); // Knob event
        assert_eq!(report[4], 0x00); // IsTurn = false
        assert_eq!(report[5], 0x00); // Knob A
        assert_eq!(report[6], 0x01); // Knob B pressed
        assert_eq!(report[7], 0x00); // Knob C
        assert_eq!(report[8], 0x00); // Knob D
    }

    #[test]
    fn test_knob_event_turn_report() {
        let event = KnobEvent::Turn {
            knob: KnobIndex::C,
            direction: KnobDirection::CounterClockwise,
            steps: 1,
        };
        let report = event.build_input_report();

        assert_eq!(report[0], 0x01); // Report ID
        assert_eq!(report[1], 0x03); // Knob event
        assert_eq!(report[4], 0x01); // IsTurn = true
        assert_eq!(report[5], 0x00); // Knob A
        assert_eq!(report[6], 0x00); // Knob B
        assert_eq!(report[7], 0xFF); // Knob C turned CCW
        assert_eq!(report[8], 0x00); // Knob D
    }

    #[test]
    fn test_plus_input_state_touch() {
        let state = PlusInputState::new();

        // No event initially
        assert!(state.take_touch_event().is_none());

        // Queue a tap
        state.tap(200, 50);

        // Take the event
        let event = state.take_touch_event().expect("Should have event");
        assert_eq!(event.event_type, TouchEventType::Short);
        assert_eq!(event.x, 200);
        assert_eq!(event.y, 50);

        // Event should be consumed
        assert!(state.take_touch_event().is_none());
    }

    #[test]
    fn test_plus_input_state_swipe() {
        let state = PlusInputState::new();

        state.swipe(50, 50, 750, 50);

        let event = state.take_touch_event().expect("Should have event");
        assert_eq!(event.event_type, TouchEventType::Drag);
        assert_eq!(event.x, 50);
        assert_eq!(event.x_end, 750);
    }

    #[test]
    fn test_plus_input_state_knob() {
        let state = PlusInputState::new();

        assert!(!state.is_knob_pressed(KnobIndex::A));
        assert!(!state.take_knob_changed());

        state.press_knob(KnobIndex::A);
        assert!(state.is_knob_pressed(KnobIndex::A));
        assert!(state.take_knob_changed());
        assert!(!state.take_knob_changed()); // Should be cleared

        state.release_knob(KnobIndex::A);
        assert!(!state.is_knob_pressed(KnobIndex::A));
        assert!(state.take_knob_changed());
    }

    #[test]
    fn test_plus_input_state_knob_turn() {
        let state = PlusInputState::new();

        state.turn_knob(KnobIndex::B, 3);
        assert!(state.take_knob_changed());

        let rotation = state.take_knob_rotation(KnobIndex::B);
        assert_eq!(rotation, 3);

        // Should be reset after take
        assert_eq!(state.take_knob_rotation(KnobIndex::B), 0);
    }

    #[test]
    fn test_plus_input_state_build_reports() {
        let state = PlusInputState::new();

        state.press_knob(KnobIndex::D);
        let press_report = state.build_knob_press_report();
        assert_eq!(press_report[8], 0x01); // Knob D pressed

        let turn_report = state.build_knob_turn_report(KnobIndex::A, -2);
        assert_eq!(turn_report[4], 0x01); // IsTurn = true
        assert_eq!(turn_report[5], 0xFE); // -2 = 254 (0xFE)
    }
}
