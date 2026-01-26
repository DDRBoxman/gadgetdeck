//! Main application state for gadgetdeck-display

use gadgetdeck::{ButtonState, ImageEvent, KnobIndex, PlusInputState, StreamDeckModel};
use raylib::prelude::*;
use std::sync::Arc;

use crate::button::Button;
use crate::knob::Knob;
use crate::layout::DeviceLayout;
use crate::touchscreen::{TouchGesture, TouchscreenStrip};

/// Application state
pub struct App {
    buttons: Vec<Button>,
    /// Knobs for Stream Deck Plus
    knobs: Vec<Knob>,
    /// Touchscreen strip for Stream Deck Plus
    touchscreen: Option<TouchscreenStrip>,
    touch_active: bool,
    touch_position: Vector2,
    last_pressed: Option<usize>,
    /// Last pressed knob index
    last_knob_pressed: Option<usize>,
    /// Index of knob currently being dragged for rotation
    dragging_knob: Option<usize>,
    /// USB button state (shared with USB thread)
    button_state: Arc<ButtonState>,
    /// Plus-specific input state (touchscreen/knobs)
    plus_state: Option<Arc<PlusInputState>>,
    /// Connection status message
    status_msg: String,
    /// Number of images received
    images_received: u64,
    /// Screen width
    screen_width: i32,
    /// Screen height
    screen_height: i32,
    /// Device layout info
    layout: DeviceLayout,
    /// Device model
    model: StreamDeckModel,
}

impl App {
    pub fn new(
        button_state: Arc<ButtonState>,
        plus_state: Option<Arc<PlusInputState>>,
        model: StreamDeckModel,
        screen_width: i32,
        screen_height: i32,
    ) -> Self {
        // Calculate layout based on device model
        let layout = DeviceLayout::from_model(model, screen_width, screen_height);

        // Calculate grid position to center buttons
        // For Plus, we offset buttons upward to make room for knobs and touchscreen
        let grid_width = (layout.cols as i32 * layout.button_size)
            + ((layout.cols as i32 - 1) * layout.button_spacing);
        let grid_height = (layout.rows as i32 * layout.button_size)
            + ((layout.rows as i32 - 1) * layout.button_spacing);

        let _extra_elements_height = if layout.has_knobs { 120 } else { 0 }
            + if layout.has_touchscreen { 120 } else { 0 };

        let start_x = (screen_width - grid_width) / 2;
        let start_y = if layout.has_knobs || layout.has_touchscreen {
            // For Plus: position buttons with more top margin
            80 // Top margin
        } else {
            (screen_height - grid_height) / 2
        };

        let mut buttons = Vec::with_capacity(layout.button_count);

        for row in 0..layout.rows {
            for col in 0..layout.cols {
                let x = start_x + (col as i32 * (layout.button_size + layout.button_spacing));
                let y = start_y + (row as i32 * (layout.button_size + layout.button_spacing));
                let index = row * layout.cols + col;
                buttons.push(Button::new(
                    x as f32,
                    y as f32,
                    index,
                    layout.button_size,
                    layout.corner_radius,
                    layout.image_size,
                    model,
                ));
            }
        }

        // Create touchscreen strip for Plus (above knobs)
        // The strip is 800px wide with 4 segments of 200px each
        let touchscreen = if layout.has_touchscreen {
            let strip_y = start_y + grid_height + 30; // Below buttons with more gap
            let strip_width = 800.0_f32.min((screen_width - 100) as f32);
            let strip_height = 100.0; // Match actual LCD height
            let strip_x = (screen_width as f32 - strip_width) / 2.0;

            Some(TouchscreenStrip::new(
                strip_x,
                strip_y as f32,
                strip_width,
                strip_height,
            ))
        } else {
            None
        };

        // Create knobs for Plus (below touchscreen, aligned with touchscreen segments)
        // Each segment is 200px wide (800/4), so knobs go at 100, 300, 500, 700px from strip start
        let knobs = if layout.has_knobs {
            let knob_radius = 40.0;
            let knob_y = if layout.has_touchscreen {
                // Touchscreen is at start_y + grid_height + 30, height 100
                // So it ends at start_y + grid_height + 130
                // Knob center needs to be at: strip_end + gap + radius
                start_y + grid_height + 130 + 20 + knob_radius as i32 // 20px gap below strip
            } else {
                start_y + grid_height + 60 + knob_radius as i32 // Below buttons
            };

            // Get the touchscreen strip x position and width for alignment
            let strip_width = 800.0_f32.min((screen_width - 100) as f32);
            let strip_x = (screen_width as f32 - strip_width) / 2.0;
            let segment_width = strip_width / 4.0; // 200px per segment (scaled)

            // Align each knob with the center of each touchscreen segment
            (0..layout.knob_count)
                .map(|i| {
                    // Center of segment i = strip_x + segment_width * i + segment_width/2
                    let x = strip_x + segment_width * i as f32 + segment_width / 2.0;
                    Knob::new(x, knob_y as f32, knob_radius, i)
                })
                .collect()
        } else {
            Vec::new()
        };

        Self {
            buttons,
            knobs,
            touchscreen,
            touch_active: false,
            touch_position: Vector2::new(0.0, 0.0),
            last_pressed: None,
            last_knob_pressed: None,
            dragging_knob: None,
            button_state,
            plus_state,
            status_msg: "Waiting for host connection...".to_string(),
            images_received: 0,
            screen_width,
            screen_height,
            layout,
            model,
        }
    }

    pub fn update(&mut self, rl: &RaylibHandle) {
        // Handle touch input
        // Raylib treats touch as mouse on Pi
        let touch_count = rl.get_touch_point_count();

        if touch_count > 0 {
            self.touch_active = true;
            self.touch_position = rl.get_touch_position(0);
        } else if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            // Fallback to mouse for testing on desktop
            self.touch_active = true;
            self.touch_position = rl.get_mouse_position();
        } else {
            self.touch_active = false;
        }

        // Update button states
        let mut current_pressed = None;

        for (i, button) in self.buttons.iter_mut().enumerate() {
            let was_pressed = button.pressed;
            button.pressed =
                self.touch_active && button.contains(self.touch_position.x, self.touch_position.y);

            if button.pressed {
                current_pressed = Some(i);
            }

            // Update USB button state on changes
            if button.pressed && !was_pressed {
                self.button_state.press(i as u8);
                log::info!("Button {} pressed (touch)", i);
            } else if !button.pressed && was_pressed {
                self.button_state.release(i as u8);
                log::info!("Button {} released (touch)", i);
            }
        }

        self.last_pressed = current_pressed;

        // Update knob states (for Plus) with drag-to-rotate support
        let mut current_knob_pressed = None;

        // Handle knob drag rotation
        if self.touch_active {
            // Check if we're starting a new drag on a knob
            if self.dragging_knob.is_none() {
                for (i, knob) in self.knobs.iter_mut().enumerate() {
                    if knob.contains(self.touch_position.x, self.touch_position.y) {
                        knob.start_drag(self.touch_position.x);
                        self.dragging_knob = Some(i);
                        current_knob_pressed = Some(i);
                        log::debug!("Started drag on knob {}", knob.label);
                        break;
                    }
                }
            } else if let Some(drag_idx) = self.dragging_knob {
                // Continue existing drag
                if drag_idx < self.knobs.len() {
                    let knob = &mut self.knobs[drag_idx];
                    let steps = knob.update_drag(self.touch_position.x);
                    current_knob_pressed = Some(drag_idx);

                    // Send rotation events if steps crossed threshold
                    if steps != 0 {
                        let direction = if steps > 0 {
                            "clockwise"
                        } else {
                            "counter-clockwise"
                        };
                        log::info!(
                            "Knob {} turned {} ({} steps, drag)",
                            knob.label,
                            direction,
                            steps.abs()
                        );
                        if let Some(ref plus) = self.plus_state {
                            plus.turn_knob(KnobIndex::from(drag_idx as u8), steps);
                        }
                    }
                }
            }
        } else {
            // Touch released - end any active drag
            if let Some(drag_idx) = self.dragging_knob.take() {
                if drag_idx < self.knobs.len() {
                    let knob = &mut self.knobs[drag_idx];
                    let was_tap = knob.end_drag();

                    // If it was a tap (no significant movement), send press+release
                    if was_tap {
                        log::info!("Knob {} tapped", knob.label);
                        if let Some(ref plus) = self.plus_state {
                            plus.press_knob(KnobIndex::from(drag_idx as u8));
                            plus.release_knob(KnobIndex::from(drag_idx as u8));
                        }
                    }
                }
            }

            // Clear pressed state on all knobs
            for knob in self.knobs.iter_mut() {
                knob.pressed = false;
            }
        }

        // Update visual pressed state for the knob being interacted with
        if let Some(idx) = current_knob_pressed {
            if idx < self.knobs.len() {
                self.knobs[idx].pressed = true;
            }
        }

        self.last_knob_pressed = current_knob_pressed;

        // Update touchscreen strip (for Plus) with swipe detection
        if let Some(ref mut strip) = self.touchscreen {
            let is_over_strip = strip.contains(self.touch_position.x, self.touch_position.y);

            if self.touch_active && is_over_strip {
                let (rel_x, rel_y) =
                    strip.get_relative_touch(self.touch_position.x, self.touch_position.y);
                strip.touch_position = Some(self.touch_position);

                // Start tracking touch if not already
                if !strip.is_touching {
                    strip.on_touch_start(rel_x, rel_y);
                    log::debug!("Touchscreen touch started at ({}, {})", rel_x, rel_y);
                }
            } else if strip.is_touching {
                // Touch ended or moved off strip
                let (rel_x, rel_y) = if is_over_strip {
                    strip.get_relative_touch(self.touch_position.x, self.touch_position.y)
                } else {
                    // Use last known position if moved off strip
                    strip.touch_start.unwrap_or((400, 50))
                };

                if let Some(gesture) = strip.on_touch_end(rel_x, rel_y) {
                    self.handle_touch_gesture(gesture);
                }
            } else {
                strip.touch_position = None;
            }
        }

        // Handle scroll wheel for knob rotation simulation (desktop testing)
        let wheel = rl.get_mouse_wheel_move();
        if wheel != 0.0 {
            // Rotate the knob that the mouse is over
            for (i, knob) in self.knobs.iter_mut().enumerate() {
                if knob.contains(self.touch_position.x, self.touch_position.y) {
                    let delta = wheel * 15.0; // 15 degrees per scroll step
                    knob.turn(delta);
                    let steps = if wheel > 0.0 { 1i8 } else { -1i8 };
                    log::info!(
                        "Knob {} turned {} (wheel)",
                        knob.label,
                        if delta > 0.0 { "right" } else { "left" }
                    );
                    if let Some(ref plus) = self.plus_state {
                        plus.turn_knob(KnobIndex::from(i as u8), steps);
                    }
                    break;
                }
            }
        }
    }

    /// Handle a detected touch gesture on the touchscreen strip
    fn handle_touch_gesture(&self, gesture: TouchGesture) {
        log::info!("Touch gesture detected: {:?}", gesture);

        if let Some(ref plus) = self.plus_state {
            match gesture {
                TouchGesture::ShortTap { x, y } => {
                    plus.tap(x, y);
                }
                TouchGesture::LongPress { x, y } => {
                    plus.long_press(x, y);
                }
                TouchGesture::Drag {
                    start_x,
                    start_y,
                    end_x,
                    end_y,
                } => {
                    plus.swipe(start_x, start_y, end_x, end_y);
                }
            }
        }
    }

    /// Process image events from USB
    pub fn process_image_event(
        &mut self,
        event: ImageEvent,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
    ) {
        match event {
            ImageEvent::Updated { key_index, image } => {
                self.images_received += 1;
                self.status_msg = format!("Connected - {} images received", self.images_received);

                let idx = key_index as usize;
                if idx < self.buttons.len() {
                    self.buttons[idx].update_image(rl, thread, image.as_bytes());
                    log::info!("Updated button {} image ({} bytes)", idx, image.len());
                }
            }
            ImageEvent::LcdUpdated {
                x_offset,
                y_offset,
                width,
                height,
                image,
            } => {
                self.images_received += 1;
                self.status_msg = format!("Connected - {} images received", self.images_received);

                // Update touchscreen strip image segment
                if let Some(ref mut strip) = self.touchscreen {
                    strip.update_image(rl, thread, image.as_bytes(), x_offset, y_offset, width, height);
                    log::info!(
                        "Updated touchscreen image: x_off={}, y_off={}, {}x{}, {} bytes",
                        x_offset,
                        y_offset,
                        width,
                        height,
                        image.len()
                    );
                }
            }
        }
    }

    pub fn draw(&self, d: &mut RaylibDrawHandle) {
        d.clear_background(Color::BLACK);

        // Draw buttons
        for button in &self.buttons {
            button.draw(d);
        }

        // Draw knobs (for Plus)
        for knob in &self.knobs {
            knob.draw(d);
        }

        // Draw touchscreen strip (for Plus)
        if let Some(ref strip) = self.touchscreen {
            strip.draw(d);
        }

        // Draw touch indicator (for debugging)
        if self.touch_active {
            d.draw_circle_v(self.touch_position, 8.0, Color::RED);
        }

        // Draw status bar at bottom
        let status_y = self.screen_height - 40;
        d.draw_rectangle(0, status_y, self.screen_width, 40, Color::new(30, 30, 30, 255));

        // Status message
        d.draw_text(&self.status_msg, 20, status_y + 10, 20, Color::LIGHTGRAY);

        // Button/knob status on right
        let button_status = if let Some(knob_idx) = self.last_knob_pressed {
            let label = ["A", "B", "C", "D"].get(knob_idx).unwrap_or(&"?");
            format!("Knob {} active", label)
        } else if let Some(btn) = self.last_pressed {
            format!("Button {} active", btn + 1)
        } else {
            "Ready".to_string()
        };
        let status_width = d.measure_text(&button_status, 20);
        d.draw_text(
            &button_status,
            self.screen_width - status_width - 20,
            status_y + 10,
            20,
            Color::LIGHTGRAY,
        );

        // FPS in top-left
        d.draw_fps(10, 10);
    }
}
