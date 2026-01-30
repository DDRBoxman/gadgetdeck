# GadgetDeck Server

A web server that exposes a USB gadget emulating an Elgato Stream Deck. It provides a REST API and web interface for interacting with the emulated device, including real-time image updates via WebSocket.

## Overview

GadgetDeck Server creates a USB gadget that appears to the host computer as a real Stream Deck device. Applications like the Elgato Stream Deck software or other compatible tools can communicate with it just like they would with physical hardware. The server provides:

- **USB Gadget Emulation** – Emulates various Stream Deck models (Mini, MK.2, XL, Plus, Pedal)
- **REST API** – Control buttons, knobs, and touchscreen programmatically
- **Web UI** – Visual interface showing button images in real-time
- **WebSocket** – Real-time updates for button images and LCD content

## Supported Devices

| Device | Keys | Layout | Special Features |
|--------|------|--------|------------------|
| Mini | 6 | 3×2 | – |
| MK.2 | 15 | 5×3 | – |
| XL | 32 | 8×4 | – |
| Plus | 8 | 4×2 | 4 rotary knobs, LCD touchscreen |
| Neo | 8+2 | 4×2 | 2 extra buttons with RGB LEDs, info bar LCD |
| Pedal | 3 | 3×1 | Foot pedals (no display) |

## Installation

```bash
cargo build --release -p gadgetdeck-server
```

The binary will be at `target/release/gadgetdeck-server`.

## Usage

```bash
gadgetdeck-server [OPTIONS]
```

### Command Line Options

| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--device <TYPE>` | `-d` | `mini` | Device type to emulate: `mini`, `mk2`, `xl`, `plus`, `neo`, `pedal` |
| `--serial <STRING>` | `-s` | `ZZZZZZZZZZZZZZ` | USB serial number |
| `--bind <ADDR>` | `-b` | `0.0.0.0:3000` | Web server bind address |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `GADGETDECK_SERIAL` | Serial number (overridden by `--serial` flag) |
| `GADGETDECK_BIND` | Bind address (overridden by `--bind` flag) |
| `RUST_LOG` | Logging level (e.g., `debug`, `info`, `warn`) |

### Examples

```bash
# Emulate a Stream Deck Mini on default port
gadgetdeck-server

# Emulate a Stream Deck XL on port 8080
gadgetdeck-server --device xl --bind 0.0.0.0:8080

# Emulate a Stream Deck Plus with custom serial
gadgetdeck-server -d plus -s MY_SERIAL_123

# Emulate a Stream Deck Neo
gadgetdeck-server -d neo

# Enable debug logging
RUST_LOG=debug gadgetdeck-server
```

## Web Interface

Open `http://<host>:3000/` in a browser to view the web UI. It displays:

- Button grid with live images from the host application
- Visual button press/release states
- LCD touchscreen content (Plus model only)

The UI updates in real-time via WebSocket connection.

## REST API

All API endpoints are under `/api/`.

### Status

#### `GET /api/status`

Returns device status.

**Response:**
```json
{
  "model": "Stream Deck Mini",
  "num_buttons": 6,
  "running": true
}
```

### Buttons

#### `GET /api/buttons`

Returns all button states.

**Response:**
```json
{
  "buttons": [
    { "id": 0, "pressed": false },
    { "id": 1, "pressed": true },
    ...
  ]
}
```

#### `POST /api/buttons/{id}/press`

Press and hold a button.

#### `POST /api/buttons/{id}/release`

Release a pressed button.

#### `POST /api/buttons/{id}/click`

Press and release a button (simulates a click).

### Images

#### `GET /api/images`

Returns image statistics and available button images.

**Response:**
```json
{
  "packets_received": 1024,
  "images_completed": 6,
  "bytes_received": 262144,
  "available_images": [0, 1, 2, 3, 4, 5]
}
```

#### `GET /api/images/{id}`

Returns the raw image data for a specific button. Content-Type is `image/jpeg` or `image/bmp` depending on the device model.

### Knobs (Plus Only)

#### `GET /api/knobs`

Returns available knobs.

**Response:**
```json
{
  "available": true,
  "knobs": [
    { "id": 0, "name": "A" },
    { "id": 1, "name": "B" },
    { "id": 2, "name": "C" },
    { "id": 3, "name": "D" }
  ]
}
```

#### `POST /api/knobs/{id}/press`

Press a knob.

#### `POST /api/knobs/{id}/release`

Release a knob.

#### `POST /api/knobs/{id}/click`

Click a knob (press and release).

#### `POST /api/knobs/{id}/turn`

Turn a knob.

**Request Body:**
```json
{
  "steps": 3
}
```

Positive values rotate clockwise, negative values rotate counter-clockwise.

### LCD Touchscreen (Plus Only)

#### `POST /api/lcd/tap`

Simulate a tap on the LCD touchscreen.

**Request Body:**
```json
{
  "x": 200,
  "y": 50
}
```

#### `POST /api/lcd/swipe`

Simulate a swipe gesture on the LCD touchscreen.

**Request Body:**
```json
{
  "start_x": 100,
  "start_y": 50,
  "end_x": 300,
  "end_y": 50
}
```

### Button LEDs (Neo Only)

Buttons 8 and 9 on the Neo have RGB LED strips that can be controlled.

#### `GET /api/buttons/leds`

Returns buttons that have LEDs and their current colors.

**Response:**
```json
{
  "available": true,
  "leds": [
    { "id": 8, "r": 255, "g": 0, "b": 0 },
    { "id": 9, "r": 0, "g": 255, "b": 0 }
  ]
}
```

#### `GET /api/buttons/{id}/led`

Get the current LED color for a button (only valid for buttons 8-9 on Neo).

**Response:**
```json
{
  "id": 8,
  "r": 255,
  "g": 128,
  "b": 0
}
```

#### `POST /api/buttons/{id}/led`

Set the LED color for a button (only valid for buttons 8-9 on Neo).

**Request Body:**
```json
{
  "r": 255,
  "g": 128,
  "b": 0
}
```

## WebSocket

Connect to `ws://<host>:3000/ws` for real-time updates.

### Message Types

#### Button Updates

```json
{
  "type": "buttons",
  "buttons": [
    { "id": 0, "pressed": false },
    ...
  ]
}
```

#### Image Updates

```json
{
  "type": "image",
  "button_id": 0,
  "image_data": "<base64-encoded-image>"
}
```

#### LCD Updates (Plus Only)

```json
{
  "type": "lcd",
  "x_offset": 0,
  "y_offset": 0,
  "width": 200,
  "height": 100,
  "image_data": "<base64-encoded-jpeg>"
}
```

### Connection Behavior

On connection, the server sends:
1. Current button states
2. Current button images (if available)
3. Current LCD segments (Plus model)

After that, updates are pushed as they occur.

## Architecture

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
│                      GadgetDeck Server                       │
│  ┌──────────────────────────┴───────────────────────────┐   │
│  │                   USB Gadget                         │   │
│  │   (Emulates Stream Deck Mini/MK.2/XL/Plus/Neo/Pedal) │   │
│  └──────────────────────────┬───────────────────────────┘   │
│                              │                               │
│  ┌─────────────┐  ┌─────────┴─────┐  ┌──────────────────┐   │
│  │Button State │  │ Image Store   │  │ Plus/Neo State   │   │
│  └─────────────┘  └───────────────┘  └──────────────────┘   │
│                              │                               │
│  ┌──────────────────────────┴───────────────────────────┐   │
│  │                   Web Server (Axum)                   │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐  │   │
│  │  │  REST API   │  │   Web UI    │  │  WebSocket   │  │   │
│  │  └─────────────┘  └─────────────┘  └──────────────┘  │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Requirements

- Linux with USB gadget support (ConfigFS)
- Root privileges or appropriate permissions for USB gadget configuration
- A USB OTG-capable port connected to the host

## License

See the main repository LICENSE file.
