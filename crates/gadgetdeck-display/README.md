# GadgetDeck Display

A graphical display application that emulates Stream Deck USB devices with a touchscreen interface. Built with Raylib and DRM/KMS support for running directly on Raspberry Pi without X11/Wayland.

## Overview

GadgetDeck Display creates a USB gadget that appears to the host computer as a real Stream Deck device, while rendering the button images and handling touch input on a local display. This allows you to use a Raspberry Pi with a touchscreen as a fully functional Stream Deck.

- **USB Gadget Emulation** – Emulates various Stream Deck models (Mini, MK.2, XL, Plus, Pedal)
- **Image Rendering** – Displays button images sent from Stream Deck software
- **Touch Input** – Touch buttons to send HID button press events to the host
- **Framebuffer Rendering** – Runs directly on the console without X11/Wayland
- **Plus Support** – Includes 4 rotary knob UI and 800×100 touchscreen strip display

## Supported Devices

| Device | Keys | Layout | Special Features |
|--------|------|--------|------------------|
| Mini | 6 | 3×2 | – |
| MK.2 | 15 | 5×3 | – |
| XL | 32 | 8×4 | – |
| Plus | 8 | 4×2 | 4 rotary knobs, LCD touchscreen |
| Pedal | 3 | 3×1 | Foot pedals (no display) |

## Dependencies

```bash
sudo apt-get install -y \
    cmake libclang-dev \
    libglfw3-dev libxi-dev libxcursor-dev \
    libx11-dev libxrandr-dev libxinerama-dev \
    libgl1-mesa-dev libgles2-mesa-dev \
    libdrm-dev libgbm-dev
```

<details>
<summary>Package breakdown</summary>

| Package | Purpose |
|---------|---------|
| `cmake` | Build system for raylib |
| `libclang-dev` | Required by bindgen for Rust FFI |
| `libglfw3-dev` | Window/input handling |
| `libxi-dev` | X11 input extension |
| `libxcursor-dev` | X11 cursor support |
| `libx11-dev` | X11 core library |
| `libxrandr-dev` | X11 resize/rotate extension |
| `libxinerama-dev` | X11 multi-monitor support |
| `libgl1-mesa-dev` | OpenGL development files |
| `libgles2-mesa-dev` | OpenGL ES 2.0 development files |
| `libdrm-dev` | Direct Rendering Manager |
| `libgbm-dev` | Generic Buffer Management |

</details>

## Installation

```bash
cargo build --release -p gadgetdeck-display
```

The binary will be at `target/release/gadgetdeck-display`.

## Usage

```bash
gadgetdeck-display [OPTIONS]
```

### Command Line Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--device <TYPE>` | `-d` | `mini` | Device type: `mini`, `mk2`, `xl`, `plus`, `pedal` |
| `--serial <STRING>` | `-s` | `ZZZZZZZZZZZZZZ` | USB serial number |
| `--width <PIXELS>` | `-W` | `1600` | Screen width |
| `--height <PIXELS>` | `-H` | `600` | Screen height |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `GADGETDECK_SERIAL` | Serial number (overridden by `--serial` flag) |
| `RUST_LOG` | Logging level (e.g., `debug`, `info`, `warn`) |

### Examples

```bash
# Emulate Stream Deck Mini (default)
sudo ./target/release/gadgetdeck-display

# Emulate Stream Deck XL
sudo ./target/release/gadgetdeck-display -d xl

# Emulate Stream Deck Plus with custom serial
sudo ./target/release/gadgetdeck-display -d plus -s MY_SERIAL_123

# Custom screen resolution
sudo ./target/release/gadgetdeck-display -W 1920 -H 1080

# Enable debug logging
sudo RUST_LOG=debug ./target/release/gadgetdeck-display
```

## Prerequisites

### USB Gadget Support

Ensure your device supports USB gadget mode (Raspberry Pi 4/5 with USB-C):

```bash
# Load required kernel modules
sudo modprobe libcomposite
sudo modprobe dwc2
```

### User Permissions

```bash
# Add user to required groups
sudo usermod -a -G video,input,plugdev $USER

# Install udev rules (optional, for non-root access)
sudo cp scripts/gadgetdeck.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
```

Log out and back in for group changes to take effect.

### Touchscreen Calibration

If using a touchscreen, you may need tslib for calibration:

```bash
sudo apt-get install -y libts-dev
ts_calibrate
```

## How It Works

```
┌─────────────────────────────────────────────────────────────┐
│                     Host Computer                           │
│  ┌─────────────────────────────────────────────────────┐    │
│  │         Stream Deck Software / Application          │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                  │
│                         USB                                  │
└─────────────────────────────│────────────────────────────────┘
                              │
┌─────────────────────────────│────────────────────────────────┐
│                    Raspberry Pi + Touchscreen                │
│  ┌──────────────────────────┴───────────────────────────┐   │
│  │              GadgetDeck Display Binary               │   │
│  │                                                       │   │
│  │  ┌─────────────┐    ┌─────────────┐    ┌──────────┐  │   │
│  │  │ USB Gadget  │◄──►│ Image Store │───►│  Raylib  │  │   │
│  │  │  (HID I/O)  │    └─────────────┘    │ Renderer │  │   │
│  │  └──────┬──────┘                       └────┬─────┘  │   │
│  │         │                                   │        │   │
│  │         ▼                                   ▼        │   │
│  │  ┌─────────────┐                    ┌────────────┐   │   │
│  │  │Button State │◄───────────────────│Touch Input │   │   │
│  │  └─────────────┘                    └────────────┘   │   │
│  └──────────────────────────────────────────────────────┘   │
│                              │                               │
│                    ┌─────────┴─────────┐                    │
│                    │   Touchscreen     │                    │
│                    │    (Display)      │                    │
│                    └───────────────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

1. **USB Gadget** – Receives image data from host, sends button press events
2. **Image Store** – Caches decoded button images
3. **Raylib Renderer** – Draws button grid to the framebuffer
4. **Touch Input** – Detects button presses and updates button state

## Compatible Software

Works with any software that supports Stream Deck devices:

- **Elgato Stream Deck** (Windows/macOS)
- **streamdeck-ui** (Linux) – https://github.com/streamdeck-ui/streamdeck-ui
- **python-elgato-streamdeck** – https://github.com/abcminiuser/python-elgato-streamdeck

## Troubleshooting

### USB gadget not found

- Verify USB gadget mode is enabled on your device
- Check kernel modules are loaded: `lsmod | grep dwc2`
- Run with `sudo` or install udev rules

### Display not rendering

- Run from console (TTY), not over SSH or in a terminal emulator
- Verify DRM device exists: `ls /dev/dri/card*`
- Check permissions: `sudo` may be required

### Touch not working

- Verify touch device: `cat /proc/bus/input/devices`
- Check group membership: `groups`
- Calibrate with `ts_calibrate`

### Images not appearing

- Connect Stream Deck software and set button images
- Enable debug logging: `RUST_LOG=debug`
- Check USB: `dmesg | tail`

## License

See the main repository LICENSE file.
