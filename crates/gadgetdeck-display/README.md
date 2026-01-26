# GadgetDeck Display

Raylib-based display binary that emulates Stream Deck USB devices with a graphical touchscreen interface.
**Built with DRM/KMS support for running without X11/Wayland on Raspberry Pi.**

## Features

- **Multiple Stream Deck models** - Emulates Mini, MK.2, XL, and Plus devices
- **Image rendering** - Displays button images sent from Stream Deck software
- **Touch input** - Touch buttons to send HID button press events to the host
- **Framebuffer rendering** - Runs directly on the console without X11/Wayland
- **Plus support** - Includes 4 rotary knob UI and 800x100 touchscreen strip display

## Supported Devices

| Device | Buttons | Layout | Features |
|--------|---------|--------|----------|
| Mini | 6 | 3x2 | Basic button grid |
| MK.2 | 15 | 5x3 | Button grid |
| XL | 32 | 8x4 | Large button grid |
| Plus | 8 | 4x2 | Buttons + 4 knobs + touchscreen |
| Pedal | 3 | 3x1 | Foot pedals (no display) |

## Dependencies

Install all required packages:

```bash
sudo apt-get install -y cmake libclang-dev \
    libglfw3-dev libxi-dev libxcursor-dev \
    libx11-dev libxrandr-dev libxinerama-dev \
    libgl1-mesa-dev libgles2-mesa-dev \
    libdrm-dev libgbm-dev
```

### Package breakdown:

**Build tools:**
- `cmake` - Build system for raylib
- `libclang-dev` - Required by bindgen for Rust FFI generation

**X11 (required for build, even with DRM):**
- `libglfw3-dev` - Window/input handling
- `libxi-dev` - X11 input extension
- `libxcursor-dev` - X11 cursor support
- `libx11-dev` - X11 core library
- `libxrandr-dev` - X11 resize and rotate extension
- `libxinerama-dev` - X11 multi-monitor support

**Graphics:**
- `libgl1-mesa-dev` - OpenGL development files
- `libgles2-mesa-dev` - OpenGL ES 2.0 development files

**DRM/KMS (for framebuffer without X11):**
- `libdrm-dev` - Direct Rendering Manager
- `libgbm-dev` - Generic Buffer Management

## Building

```bash
cargo build --bin display --features display --release
```

## Running on Raspberry Pi

### Prerequisites

1. **USB gadget support** - Ensure your device supports USB gadget mode (e.g., Raspberry Pi 4/5 with USB-C)

2. **Kernel modules** - Load the required modules:
   ```bash
   sudo modprobe libcomposite
   sudo modprobe dwc2
   ```

3. **User permissions** - Add user to required groups:
   ```bash
   sudo usermod -a -G video,input,plugdev $USER
   # Log out and back in for changes to take effect
   ```

4. **Udev rules** (optional) - Install the provided rules for non-root access:
   ```bash
   sudo cp scripts/gadgetdeck.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules
   ```

### Running

```bash
# From console (not X11 terminal) - typically requires root for USB gadget
sudo ./target/release/gadgetdeck-display

# Specify device type (default: mini)
sudo ./target/release/gadgetdeck-display --device mini
sudo ./target/release/gadgetdeck-display --device mk2
sudo ./target/release/gadgetdeck-display --device xl
sudo ./target/release/gadgetdeck-display --device pedal

# Short form
sudo ./target/release/gadgetdeck-display -d xl

# Custom screen resolution (default: 1600x600)
sudo ./target/release/gadgetdeck-display --width 1920 --height 1080
sudo ./target/release/gadgetdeck-display -W 800 -H 480

# With custom serial number
sudo ./target/release/gadgetdeck-display --serial MYSERIAL123

# Or via environment variable
sudo GADGETDECK_SERIAL=MYSERIAL123 ./target/release/gadgetdeck-display

# Enable debug logging
sudo RUST_LOG=debug ./target/release/gadgetdeck-display

# View all options
./target/release/gadgetdeck-display --help
```

### Supported Device Types

| Device | Keys | Layout | Description |
|--------|------|--------|-------------|
| `mini` | 6 | 3×2 | Stream Deck Mini (default) |
| `mk2` | 15 | 5×3 | Stream Deck MK.2 |
| `xl` | 32 | 8×4 | Stream Deck XL |
| `pedal` | 3 | 3×1 | Stream Deck Pedal |

### Touch input setup

If using a touchscreen, you may need `tslib` for calibration:

```bash
sudo apt-get install -y libts-dev
```

## Configuration

Edit constants in `main.rs` to match your display:

```rust
// Screen dimensions
const SCREEN_WIDTH: i32 = 1600;
const SCREEN_HEIGHT: i32 = 600;

// Button layout (Stream Deck Mini = 2x3)
const BUTTON_COLS: usize = 3;
const BUTTON_ROWS: usize = 2;

// Button appearance
const BUTTON_SIZE: i32 = 200;
const BUTTON_SPACING: i32 = 50;
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `GADGETDECK_SERIAL` | USB device serial number | `GDECK0000001` |
| `RUST_LOG` | Log level (error, warn, info, debug, trace) | (none) |

## How It Works

1. **USB Gadget Setup** - Creates a USB HID device that emulates a Stream Deck Mini
2. **Image Reception** - Receives BMP images from host software via USB output reports
3. **Display Rendering** - Renders received images on the touchscreen using Raylib
4. **Touch Handling** - Detects touch/click on buttons and sends HID input reports to host

### Stream Deck Mini Protocol

- **Button grid**: 2 rows × 3 columns (6 buttons)
- **Image format**: 80×80 pixels, 24-bit BMP, rotated 90° counter-clockwise
- **Input reports**: Button states sent as HID Report ID 0x01
- **Output reports**: Image data received as HID Report ID 0x02

## Compatible Software

The display should work with any software that supports Stream Deck Mini:

- **Elgato Stream Deck software** (Windows/macOS)
- **streamdeck-ui** (Linux) - https://github.com/streamdeck-ui/streamdeck-ui
- **python-elgato-streamdeck** - https://github.com/elgato/python-elgato-streamdeck

## Troubleshooting

### USB gadget not found
- Ensure USB gadget mode is enabled in your device
- Check that `libcomposite` and `dwc2` modules are loaded
- Verify you're running as root or have proper udev rules

### Display not rendering
- Ensure you're running from console, not over SSH
- Check that `/dev/dri/card*` exists and is accessible
- Try running with `sudo` if permission denied

### Touch not working
- Verify touch device exists: `cat /proc/bus/input/devices`
- Check input group membership: `groups`
- Try calibrating with `ts_calibrate` (tslib)

### Images not appearing
- Connect Stream Deck software and configure button images
- Enable debug logging: `RUST_LOG=debug`
- Check USB connection with `dmesg | tail`
