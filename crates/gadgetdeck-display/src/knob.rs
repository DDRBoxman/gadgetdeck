//! Knob (rotary encoder) UI for Stream Deck Plus

use raylib::prelude::*;

/// Knob (rotary encoder) display state for Stream Deck Plus
pub struct Knob {
    /// Center position
    pub center: Vector2,
    /// Radius for the knob
    pub radius: f32,
    /// Knob index (0-3 for A-D)
    pub index: usize,
    /// Whether the knob is currently pressed
    pub pressed: bool,
    /// Current rotation value (for visual feedback)
    pub rotation: f32,
    /// Label for the knob
    pub label: String,
    /// Whether the knob is being dragged for rotation
    pub is_dragging: bool,
    /// X position where drag started
    drag_start_x: f32,
    /// Last X position during drag (for incremental updates)
    drag_last_x: f32,
    /// Accumulated drag distance for visual feedback (resets periodically)
    drag_indicator: f32,
}

impl Knob {
    pub fn new(center_x: f32, center_y: f32, radius: f32, index: usize) -> Self {
        let labels = ["A", "B", "C", "D"];
        Self {
            center: Vector2::new(center_x, center_y),
            radius,
            index,
            pressed: false,
            rotation: 0.0,
            label: labels.get(index).unwrap_or(&"?").to_string(),
            is_dragging: false,
            drag_start_x: 0.0,
            drag_last_x: 0.0,
            drag_indicator: 0.0,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        let dx = x - self.center.x;
        let dy = y - self.center.y;
        (dx * dx + dy * dy) <= (self.radius * self.radius)
    }

    /// Draw the knob
    pub fn draw(&self, d: &mut RaylibDrawHandle) {
        let base_color = if self.is_dragging {
            Color::new(80, 120, 200, 255) // Highlight when dragging
        } else if self.pressed {
            Color::new(60, 100, 180, 255)
        } else {
            Color::new(50, 50, 60, 255)
        };

        // Outer ring
        d.draw_circle_v(self.center, self.radius, Color::new(80, 80, 90, 255));

        // Inner knob body
        d.draw_circle_v(self.center, self.radius * 0.85, base_color);

        // Rotation indicator line
        let indicator_length = self.radius * 0.6;
        let angle_rad = self.rotation.to_radians();
        let indicator_end = Vector2::new(
            self.center.x + angle_rad.sin() * indicator_length,
            self.center.y - angle_rad.cos() * indicator_length,
        );
        d.draw_line_ex(self.center, indicator_end, 3.0, Color::WHITE);

        // Draw drag direction indicator when dragging
        if self.is_dragging && self.drag_indicator.abs() > 5.0 {
            let arrow_y = self.center.y;
            let arrow_length = (self.drag_indicator.abs() * 0.5).min(self.radius * 0.8);
            let arrow_color = if self.drag_indicator > 0.0 {
                Color::new(100, 255, 100, 200) // Green for clockwise (right)
            } else {
                Color::new(255, 100, 100, 200) // Red for counter-clockwise (left)
            };

            if self.drag_indicator > 0.0 {
                // Right arrow (clockwise)
                let arrow_start = Vector2::new(self.center.x, arrow_y);
                let arrow_end = Vector2::new(self.center.x + arrow_length, arrow_y);
                d.draw_line_ex(arrow_start, arrow_end, 4.0, arrow_color);
                // Arrowhead
                d.draw_triangle(
                    Vector2::new(arrow_end.x + 8.0, arrow_y),
                    Vector2::new(arrow_end.x - 4.0, arrow_y - 6.0),
                    Vector2::new(arrow_end.x - 4.0, arrow_y + 6.0),
                    arrow_color,
                );
            } else {
                // Left arrow (counter-clockwise)
                let arrow_start = Vector2::new(self.center.x, arrow_y);
                let arrow_end = Vector2::new(self.center.x - arrow_length, arrow_y);
                d.draw_line_ex(arrow_start, arrow_end, 4.0, arrow_color);
                // Arrowhead
                d.draw_triangle(
                    Vector2::new(arrow_end.x - 8.0, arrow_y),
                    Vector2::new(arrow_end.x + 4.0, arrow_y - 6.0),
                    Vector2::new(arrow_end.x + 4.0, arrow_y + 6.0),
                    arrow_color,
                );
            }
        }

        // Draw label below
        let label_y = self.center.y as i32 + self.radius as i32 + 5;
        let text_size = 16;
        let text_width = d.measure_text(&self.label, text_size);
        d.draw_text(
            &self.label,
            self.center.x as i32 - text_width / 2,
            label_y,
            text_size,
            Color::LIGHTGRAY,
        );

        // Border - highlight when dragging
        let border_color = if self.is_dragging {
            Color::new(100, 200, 255, 255) // Cyan when dragging
        } else if self.pressed {
            Color::WHITE
        } else {
            Color::GRAY
        };
        d.draw_circle_lines(
            self.center.x as i32,
            self.center.y as i32,
            self.radius,
            border_color,
        );
    }

    /// Turn the knob (positive = clockwise, negative = counter-clockwise)
    pub fn turn(&mut self, delta: f32) {
        self.rotation = (self.rotation + delta) % 360.0;
        if self.rotation < 0.0 {
            self.rotation += 360.0;
        }
    }

    /// Start a drag operation
    pub fn start_drag(&mut self, x: f32) {
        self.is_dragging = true;
        self.drag_start_x = x;
        self.drag_last_x = x;
        self.drag_indicator = 0.0;
    }

    /// Update drag and return the number of steps to send (if threshold crossed)
    /// Each step represents ~20 pixels of horizontal drag
    pub fn update_drag(&mut self, x: f32) -> i8 {
        if !self.is_dragging {
            return 0;
        }

        let delta = x - self.drag_last_x;
        self.drag_indicator += delta;

        // Calculate steps based on accumulated movement since last step sent
        // ~20 pixels per step
        const PIXELS_PER_STEP: f32 = 20.0;
        let accumulated = x - self.drag_start_x;
        let total_steps = (accumulated / PIXELS_PER_STEP) as i32;
        let last_steps = ((self.drag_last_x - self.drag_start_x) / PIXELS_PER_STEP) as i32;
        let new_steps = (total_steps - last_steps) as i8;

        if new_steps != 0 {
            self.drag_last_x = x;
            // Update visual rotation
            self.turn(new_steps as f32 * 15.0); // 15 degrees per step
        }

        new_steps
    }

    /// End a drag operation, returns true if it was a short tap (no significant movement)
    pub fn end_drag(&mut self) -> bool {
        let was_tap = self.is_dragging && (self.drag_last_x - self.drag_start_x).abs() < 10.0;
        self.is_dragging = false;
        self.drag_indicator = 0.0;
        was_tap
    }
}
