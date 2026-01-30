//! Device layout configuration for gadgetdeck-display

use gadgetdeck::StreamDeckModel;

/// Device layout configuration derived from StreamDeckModel
pub struct DeviceLayout {
    /// Number of button columns
    pub cols: usize,
    /// Number of button rows
    pub rows: usize,
    /// Total button count
    pub button_count: usize,
    /// Button size in pixels (for display)
    pub button_size: i32,
    /// Spacing between buttons
    pub button_spacing: i32,
    /// Corner radius for button rendering
    pub corner_radius: f32,
    /// Original image size from the device
    pub image_size: i32,
    /// Whether this device has knobs (Plus only)
    pub has_knobs: bool,
    /// Number of knobs (4 for Plus, 0 for others)
    pub knob_count: usize,
    /// Whether this device has a touchscreen strip (Plus only)
    pub has_touchscreen: bool,
    /// Touchscreen width in pixels (800 for Plus)
    pub touchscreen_width: i32,
    /// Touchscreen height in pixels (100 for Plus)
    pub touchscreen_height: i32,
    /// Whether this device has an info bar LCD (Neo only)
    pub has_info_bar: bool,
    /// Info bar width in pixels (248 for Neo)
    pub info_bar_width: i32,
    /// Info bar height in pixels (58 for Neo)
    pub info_bar_height: i32,
    /// Whether this device has LED buttons (Neo buttons 8-9)
    pub has_led_buttons: bool,
    /// Number of LED buttons (2 for Neo)
    pub led_button_count: usize,
}

impl DeviceLayout {
    pub fn from_model(model: StreamDeckModel, screen_width: i32, screen_height: i32) -> Self {
        let (cols, rows) = model.key_matrix();
        let cols = cols as usize;
        let rows = rows as usize;
        let button_count = model.key_count() as usize;
        let (img_w, _img_h) = model.key_image_size();
        let image_size = img_w as i32;

        // Plus-specific features
        let is_plus = matches!(model, StreamDeckModel::Plus);
        let has_knobs = is_plus;
        let knob_count = if is_plus { 4 } else { 0 };
        let has_touchscreen = is_plus;
        let touchscreen_width = if is_plus { 800 } else { 0 };
        let touchscreen_height = if is_plus { 100 } else { 0 };

        // Neo-specific features
        let is_neo = matches!(model, StreamDeckModel::Neo);
        let has_info_bar = is_neo;
        let info_bar_width = if is_neo { 248 } else { 0 };
        let info_bar_height = if is_neo { 58 } else { 0 };
        let has_led_buttons = is_neo;
        let led_button_count = if is_neo { 2 } else { 0 };

        // Calculate optimal button size based on screen dimensions and grid
        // Leave some margin for spacing and status bar
        // For Plus, also reserve space for knobs below buttons and touchscreen at bottom
        // For Neo, reserve space for info bar with LED buttons
        let knob_area_height = if has_knobs { 120 } else { 0 }; // Space for knob UI
        let touchscreen_area_height = if has_touchscreen { 120 } else { 0 }; // Space for touchscreen strip
        let info_bar_area_height = if has_info_bar { 100 } else { 0 }; // Space for info bar + LED buttons

        let available_width = screen_width - 100; // 50px margin on each side
        let available_height =
            screen_height - 140 - knob_area_height - touchscreen_area_height - info_bar_area_height; // 50px top, 40px status bar, 50px bottom

        // Calculate button size that fits the grid
        let max_button_width = (available_width - (cols as i32 - 1) * 30) / cols as i32;
        let max_button_height = (available_height - (rows as i32 - 1) * 30) / rows as i32;

        // Use the smaller dimension to keep buttons square
        let button_size = max_button_width.min(max_button_height).min(200); // Cap at 200px
        let button_spacing = (button_size / 4).max(20).min(50); // Proportional spacing

        Self {
            cols,
            rows,
            button_count,
            button_size,
            button_spacing,
            corner_radius: (button_size as f32 * 0.1).min(20.0),
            image_size,
            has_knobs,
            knob_count,
            has_touchscreen,
            touchscreen_width,
            touchscreen_height,
            has_info_bar,
            info_bar_width,
            info_bar_height,
            has_led_buttons,
            led_button_count,
        }
    }
}
