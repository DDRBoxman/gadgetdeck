# Stream Deck USB Gadget Emulation Research

## Project Overview

This project emulates Elgato Stream Deck devices on a Raspberry Pi using the Linux USB Gadget subsystem. The goal is to have the official Stream Deck software on macOS recognize the Pi as a real Stream Deck device.

## Key Findings

### USB Identification

**Vendor ID**: `0x0fd9` (Elgato Systems GmbH)

#### Complete Product ID List

| Constant | Product ID | Model |
|----------|------------|-------|
| USB_PID_STREAMDECK_ORIGINAL | 0x0060 | Stream Deck Original |
| USB_PID_STREAMDECK_MINI | 0x0063 | Stream Deck Mini |
| USB_PID_STREAMDECK_XL | 0x006c | Stream Deck XL (Legacy) |
| USB_PID_STREAMDECK_ORIGINAL_V2 | 0x006d | Stream Deck Original V2 |
| USB_PID_STREAMDECK_MK2 | 0x0080 | Stream Deck MK.2 |
| USB_PID_STREAMDECK_PLUS | 0x0084 | Stream Deck + |
| USB_PID_STREAMDECK_PEDAL | 0x0086 | Stream Deck Pedal |
| USB_PID_STREAMDECK_XL_V2 | 0x008f | Stream Deck XL V2 |
| USB_PID_STREAMDECK_MINI_MK2 | 0x0090 | Stream Deck Mini MK.2 |
| USB_PID_STREAMDECK_NEO | 0x009a | Stream Deck Neo |
| USB_PID_STREAMDECK_MK2_SCISSOR | 0x00a5 | Stream Deck MK.2 (Scissor) |
| USB_PID_STREAMDECK_STUDIO | 0x00aa | Stream Deck Studio |
| USB_PID_STREAMDECK_MINI_MK2_MODULE | 0x00b8 | Stream Deck Mini MK.2 (Module) |
| USB_PID_STREAMDECK_MK2_MODULE | 0x00b9 | Stream Deck MK.2 (Module 15) |
| USB_PID_STREAMDECK_MK2_V2 | 0x00b9 | Stream Deck MK.2 V2 (same as Module) |
| USB_PID_STREAMDECK_XL_V2_MODULE | 0x00ba | Stream Deck XL V2 (Module 32) |

#### Device Capabilities Summary

| Model | Vendor ID | Product ID | Keys | Layout | Notes |
|-------|-----------|------------|------|--------|-------|
| Stream Deck Mini | 0x0fd9 | 0x0063 | 6 | 3×2 | 80×80 BMP images |
| Stream Deck Mini MK.2 | 0x0fd9 | 0x0090 | 6 | 3×2 | 80×80 JPEG images |
| Stream Deck Original | 0x0fd9 | 0x0060 | 15 | 5×3 | Legacy BMP |
| Stream Deck Original V2 | 0x0fd9 | 0x006d | 15 | 5×3 | JPEG images |
| Stream Deck MK.2 (Module 15) | 0x0fd9 | 0x00b9 | 15 | 5×3 | 72×72 JPEG images |
| Stream Deck XL (Legacy) | 0x0fd9 | 0x006c | 32 | 8×4 | Legacy |
| Stream Deck XL V2 (Module 32) | 0x0fd9 | 0x00ba | 32 | 8×4 | 96×96 JPEG images |
| Stream Deck Pedal | 0x0fd9 | 0x0086 | 3 | 3×1 | No display |
| Stream Deck Plus | 0x0fd9 | 0x0084 | 8 | 4×2 | 120×120 JPEG, 800×100 screen, 4 knobs |
| Stream Deck Neo | 0x0fd9 | 0x009a | 8 | 4×2 | Touchscreen info bar |
| Stream Deck Studio | 0x0fd9 | 0x00aa | 15 | 5×3 | Pro model with additional features |

### Device Capabilities (from Official Elgato Docs)

| Model | LCD Size | Key Image Size | Image Format | Rotation |
|-------|----------|----------------|--------------|----------|
| Module 6 (Mini) | 320×240 px | 80×80 px | BMP | 90° clockwise |
| Module 15 (MK.2) | 480×272 px | 72×72 px | JPEG | 180° |
| Module 32 (XL) | 1024×600 px | 96×96 px | JPEG | 180° |
| Pedal | N/A | N/A | N/A | N/A |

### USB Gadget Driver Selection

**Problem**: The Linux kernel's `f_hid` driver does NOT support `GET_REPORT` control transfers. When the host sends a `GET_REPORT` request, `f_hid` stalls the endpoint, causing the host to think the device is malfunctioning.

**Solution**: Use **FunctionFS** (`usb_gadget::function::Custom`) to implement a custom HID class device with full control over USB control transfers.

### HID Class Configuration

```
bInterfaceClass:    0x03 (HID)
bInterfaceSubClass: 0x00 (No subclass)
bInterfaceProtocol: 0x00 (None)
```

### HID Report Descriptor

The HID report descriptor defines the structure of all reports.

#### Module 6 (Mini) - 221 bytes

| Report ID | Type | Size (bytes) | Purpose |
|-----------|------|--------------|---------|
| 0x01 | Input | 64 | Button states (first 6 bytes used) |
| 0x02 | Output | 1023 | Image data packets |
| 0x03 | Feature | 31 | Serial number |
| 0x05 | Feature | 16 | Brightness control |
| 0x0B | Feature | 16 | Commands |
| 0xA0 | Feature | 16 | LD firmware version |
| 0xA1 | Feature | 16 | AP2 (primary) firmware version |
| 0xA2 | Feature | 16 | AP1 (backup) firmware version |
| 0xA3 | Feature | 16 | Idle time before sleep |

#### Module 15/32 (MK.2/XL) - Per Official Elgato Docs

| Report ID | Type | Max Size | Purpose |
|-----------|------|----------|---------|
| 0x01 | Input | 512 | Key states [Report ID, Command, Length(2), Payload] |
| 0x02 | Output | 1024 | Image upload [Report ID, Command, Payload] |
| 0x03 | Feature | 32 | Setters (brightness, fill color, sleep, etc.) |
| 0x04 | Feature | 32 | LD firmware version |
| 0x05 | Feature | 32 | AP2 (primary) firmware version |
| 0x06 | Feature | 32 | Serial number |
| 0x07 | Feature | 32 | AP1 (backup) firmware version |
| 0x08 | Feature | 32 | Unit information |
| 0x0A | Feature | 32 | Idle time before sleep |

### Feature Report Formats

#### Serial Number

| Model | Report ID | Format |
|-------|-----------|--------|
| Module 6 (Mini) | 0x03 | 32 bytes, serial at offset [5:] |
| Module 15/32 | 0x06 | 32 bytes, [Report ID, Data Length, Serial String] |
| Pedal | 0x06 | 32 bytes, serial at offset [2:] |

#### Firmware Version

| Model | Report ID | Format |
|-------|-----------|--------|
| Module 6 (Mini) | 0xA1 (AP2) | 32 bytes, version at offset [5:], 12 chars max |
| Module 15/32 | 0x05 (AP2) | 32 bytes, [Report ID, Length(0x0C), Checksum(4), Version(8)] |
| Pedal | 0x05 | 32 bytes, version at offset [6:] |

Example version strings: `"1.0.170602"`, `"1.0.0"`

#### Unit Information (Module 15/32 only, Report ID 0x08)

Response format:
```
[0x00] Report ID (0x08)
[0x01] Keypad Matrix Rows
[0x02] Keypad Matrix Columns
[0x03-0x04] Key Width (u16 LE)
[0x05-0x06] Key Height (u16 LE)
[0x07-0x08] LCD Width (u16 LE)
[0x09-0x0A] LCD Height (u16 LE)
[0x0B] Image BPP
[0x0C] Color Scheme
[0x0D] Number of key images in Gallery
[0x0E] Number of LCD images in Gallery
[0x0F] Number of frames for DEMO
```

### SET_REPORT Commands from Host

#### Module 6 (Mini)

| Report ID | Command | Description |
|-----------|---------|-------------|
| 0x0B | 0x63 | Show boot logo |
| 0x0B | 0xA2 | Set idle time before sleep |
| 0x05 | 0x55 0xAA 0xD1 0x01 [%] | Set brightness (0-100) |

#### Module 15/32 (MK.2/XL) - Report ID 0x03

| Command | Description | Payload |
|---------|-------------|---------|
| 0x02 | Show boot logo | - |
| 0x05 | Fill LCD with color | RGB triplet |
| 0x06 | Fill key with color | Key index, RGB triplet |
| 0x08 | Set brightness | Brightness (0x00-0x64) |
| 0x0D | Set sleep timeout | INT32 seconds (0 = disable) |
| 0x13 | Show background by index | Background index |

### Mystery Report 0xA1

The Stream Deck software queries Feature Report ID 0xA1 (161) during device initialization. This is NOT defined in the public HID report descriptor but the software still requests it.

**Observation**: Returning 32 bytes of zeros works - the software continues initialization.

**Theory**: This might be:
- Device authentication/validation
- Extended device information
- Reserved for future use

### Device Recognition Flow

When the Stream Deck software detects the USB device, it performs these queries in order:

1. **GET_DESCRIPTOR (HID Report)** - Retrieves the 221-byte HID report descriptor
2. **GET_REPORT 0xA1** - Mystery feature report (we return zeros)
3. **GET_REPORT 0x03** - Serial number query
4. **SET_REPORT 0x0B** - Reset command
5. **SET_REPORT 0x05** - Set brightness

After this sequence, the device appears in the Stream Deck software UI.

### Software Crash Issue

**Problem**: After initial recognition, the Stream Deck software crashes/locks up.

**Likely Causes**:
1. **No input reports** - Software polls for button state on interrupt IN endpoint
2. **No output report handling** - Software tries to send button images on interrupt OUT endpoint
3. **Timeout on HID reads** - Blocking reads with no data cause software to hang

### Endpoint Configuration

```
EP1 IN  (Interrupt): Button input reports - Report ID 0x01, 16 bytes
EP2 OUT (Interrupt): Image output data - Report ID 0x02, up to 1024 bytes per packet
```

### Image Protocol

#### Module 6 (Mini) - BMP Format

Images are sent as BMP format, 80×80 pixels, rotated 90° clockwise, via output reports:

```
Header (16 bytes):
  [0] 0x02        - Report ID
  [1] 0x01        - Command (write image)
  [2] chunk_index - Packet sequence (0, 1, 2, ...)
  [3] 0x00        - Reserved
  [4] show_image  - 1 to display immediately, 0 to buffer
  [5] key_index   - Button number (0-5)
  [6-15] padding  - Fill with 0x00

Payload: Up to 1008 bytes of image data per packet
```

#### Module 15/32 (MK.2/XL) - JPEG Format

Images are sent as JPEG format, rotated 180°, via output reports:

**Update Key Image (Command 0x07)**
```
[0x00] Report ID (0x02)
[0x01] Command (0x07)
[0x02] Key Index
[0x03] Transfer Done flag (0x01 = last chunk)
[0x04-0x05] Chunk Contents Size (u16 LE)
[0x06-0x07] Chunk Index (u16 LE, zero-based)
[0x08+] Chunk Data (fill to end of 1024-byte report)
```

**Update Full Screen Image (Command 0x08)** - Same format as key image

**Update Boot Logo (Command 0x09)**
```
[0x00] Report ID (0x02)
[0x01] Command (0x09)
[0x02] Reserved
[0x03] Transfer Done flag
[0x04-0x05] Chunk Index (u16 LE)
[0x06-0x07] Chunk Contents Size (u16 LE)
[0x08+] Chunk Data
```

### Button Input Report Format

#### Module 6 (Mini)
```
Report ID: 0x01
Data: 64 bytes, each byte is button state (0=released, 1=pressed)
      Only first 6 bytes are used for Mini's 6 buttons
```

#### Module 15/32 (MK.2/XL)
```
[0x00] Report ID (0x01)
[0x01] Command (0x00 for key press state change)
[0x02-0x03] Payload data length (u16 LE) = number of keys
[0x04+] Key states: 0x00 = released, 0x01 = pressed
```

Polling: Recommended polling interval is 50ms. HID READ returns TIMEOUT if no state change.

## Stream Deck Plus (Module 8)

Sources:
- [Reverse Engineering The Stream Deck Plus](https://den.dev/blog/reverse-engineer-stream-deck-plus/) by Den Delimarsky
- [python-elgato-streamdeck](https://github.com/abcminiuser/python-elgato-streamdeck) StreamDeckPlus.py

### Device Overview

| Property | Value |
|----------|-------|
| Vendor ID | 0x0fd9 |
| Product ID | 0x0084 |
| KEY_COUNT | 8 |
| KEY_COLS | 4 |
| KEY_ROWS | 2 |
| DIAL_COUNT | 4 |
| KEY_PIXEL_WIDTH | 120 |
| KEY_PIXEL_HEIGHT | 120 |
| KEY_IMAGE_FORMAT | JPEG |
| KEY_FLIP | (False, False) |
| KEY_ROTATION | 0° |
| TOUCHSCREEN_PIXEL_WIDTH | 800 |
| TOUCHSCREEN_PIXEL_HEIGHT | 100 |
| TOUCHSCREEN_IMAGE_FORMAT | JPEG |
| TOUCHSCREEN_FLIP | (False, False) |
| TOUCHSCREEN_ROTATION | 0° |
| DECK_VISUAL | True |
| DECK_TOUCH | True |

### Packet Sizes

| Constant | Value |
|----------|-------|
| _IMG_PACKET_LEN | 1024 bytes |
| _KEY_PACKET_HEADER | 8 bytes |
| _LCD_PACKET_HEADER | 16 bytes |
| _KEY_PACKET_PAYLOAD_LEN | 1016 bytes (1024 - 8) |
| _LCD_PACKET_PAYLOAD_LEN | 1008 bytes (1024 - 16) |

### Features

1. **8 Buttons** - Same behavior as other Stream Deck products, supports 120×120 color JPEG images
2. **Narrow Screen** - 800×100 color touchscreen for auxiliary information
3. **4 Dials/Knobs** - Each can turn right/left unlimited times and can be pressed (clicked)

### Button Image Protocol

Images are JPEG-encoded, sent via output reports. Packet header (8 bytes):

```
+-------+----+----+----+----+----+----+----+----+
| Byte  |  0 |  1 |  2 |  3 |  4 |  5 |  6 |  7 |
+-------+----+----+----+----+----+----+----+----+
| Value | 02 | 07 | ?? | ?? | ?? | ?? | ?? | ?? |
+-------+----+----+----+----+----+----+----+----+
```

| Byte | Description |
|------|-------------|
| 0 | Always `0x02` |
| 1 | Always `0x07` |
| 2 | Button index (zero-indexed, 0x00-0x07) |
| 3 | Final packet flag: `0x00` = more data, `0x01` = last packet |
| 4-5 | Payload length (u16 Little Endian) |
| 6-7 | Chunk/page index (u16 Little Endian, zero-based) |

Packets are 1,051 bytes total (8-byte header + up to 1,016 bytes payload, padded to 1,024).

### Button Input Report Format

Filter: `URB_INTERRUPT in` from device

```
[0x00] 0x01 - Report ID
[0x01] 0x00
[0x02] Number of buttons (0x08 for Plus)
[0x03] 0x00
[0x04+] Button states (8 bytes, one per button: 0x00=released, 0x01=pressed)
```

Example (button 4 pressed):
```
01 00 08 00 00 00 00 01 00 00 00 00 ...
```

### Screen Image Protocol

Screen is 800×100 pixels. Can be set as full image or per-segment (200×100 each).

#### Screen Header Format (16 bytes)

```
+-------+----+----+-------+-------+-------+-------+-------+-------+----+-------+----+-------+-------+----+
| Byte  |  0 |  1 |  2-3  |  4-5  |  6-7  |  8-9  | 10    | 11-12 | 13-14    | 15 |
+-------+----+----+-------+-------+-------+-------+-------+-------+----+-------+----+-------+-------+----+
| Value | 02 | 0C | X off | 00 00 | Width | Height| Final | Chunk | Payload Len | 00 |
+-------+----+----+-------+-------+-------+-------+-------+-------+----+-------+----+-------+-------+----+
```

| Byte | Description |
|------|-------------|
| 0 | Always `0x02` |
| 1 | Always `0x0C` |
| 2-3 | X offset from left (u16 LE): 0=seg A, 200=seg B, 400=seg C, 600=seg D |
| 4-5 | Always `0x00 0x00` |
| 6-7 | Image width (u16 LE): `0xC8 0x00` = 200 (segment), `0x20 0x03` = 800 (full) |
| 8-9 | Image height (u16 LE): `0x64 0x00` = 100 |
| 10 | Final chunk flag: `0x00` = more, `0x01` = last |
| 11-12 | Chunk index (u16 LE, zero-based) |
| 13-14 | Payload length (u16 LE) |
| 15 | Always `0x00` |

#### Screen Segment Offsets

| Segment | X Offset (hex) | X Offset (decimal) |
|---------|----------------|--------------------|
| A (leftmost) | `0x00 0x00` | 0 |
| B | `0xC8 0x00` | 200 |
| C | `0x90 0x01` | 400 |
| D (rightmost) | `0x58 0x02` | 600 |

### Touchscreen Input Report Format

Filter: `URB_INTERRUPT in` from device

The touchscreen input report is 14 bytes (read from device).

```
+-------+----+----+----+----+----+----+---------+---------+---------+---------+
| Byte  |  0 |  1 |  2 |  3 |  4 |  5 |   6-7   |   8-9   |  10-11  |  12-13  |
+-------+----+----+----+----+----+----+---------+---------+---------+---------+
| Value | 01 | 02 | 0E | 00 | ET | 01 | X coord | Y coord | X_out   | Y_out   |
+-------+----+----+----+----+----+----+---------+---------+---------+---------+
```

| Byte | Description |
|------|-------------|
| 0 | Report ID (0x01) |
| 1 | Event Type indicator (0x02 = touchscreen) |
| 2-3 | Payload length (0x0E 0x00 = 14) |
| 4 | Touch Event Type: 1=SHORT, 2=LONG, 3=DRAG |
| 5 | Always 0x01 |
| 6-7 | X coordinate (u16 LE) |
| 8-9 | Y coordinate (u16 LE) |
| 10-11 | X_out coordinate (u16 LE) - only for DRAG events |
| 12-13 | Y_out coordinate (u16 LE) - only for DRAG events |

#### Touch Event Types

| Value | Type | Description |
|-------|------|-------------|
| 0x01 | SHORT | Short tap |
| 0x02 | LONG | Long press |
| 0x03 | DRAG | Drag gesture (includes start and end coordinates) |

- X and Y coordinates are u16 Little Endian (0-799 for X, 0-99 for Y)
- DRAG events include both start (x, y) and end (x_out, y_out) coordinates
- Use X coordinates to determine which screen segment was tapped (0-199=A, 200-399=B, 400-599=C, 600-799=D)

### Knob/Dial Input Report Format

Filter: `URB_INTERRUPT in` from device

```
+-------+----+----+----+----+----------+--------+--------+--------+--------+
| Byte  |  0 |  1 |  2 |  3 |     4    |    5   |    6   |    7   |    8   |
+-------+----+----+----+----+----------+--------+--------+--------+--------+
| Value | 01 | 03 | 05 | 00 | IsTurn   | Knob A | Knob B | Knob C | Knob D |
+-------+----+----+----+----+----------+--------+--------+--------+--------+
```

| Byte | Description |
|------|-------------|
| 4 | `0x01` = turning, `0x00` = pressing |
| 5-8 | Knob action values (one per knob A-D) |

#### Knob Turn Events (Byte 4 = 0x01)
- `0x01` = Turn right
- `0xFF` = Turn left

#### Knob Press Events (Byte 4 = 0x00)
- `0x01` = Pressed
- `0x00` = Released

### Example Knob Data

| Knob | Right Turn | Left Turn | Press | Release |
|------|------------|-----------|-------|---------|
| A | `01 03 05 00 01 01 00 00 00` | `01 03 05 00 01 ff 00 00 00` | `01 03 05 00 00 01 00 00 00` | `01 03 05 00 00 00 00 00 00` |
| B | `01 03 05 00 01 00 01 00 00` | `01 03 05 00 01 00 ff 00 00` | `01 03 05 00 00 00 01 00 00` | `01 03 05 00 00 00 00 00 00` |
| C | `01 03 05 00 01 00 00 01 00` | `01 03 05 00 01 00 00 ff 00` | `01 03 05 00 00 00 00 01 00` | `01 03 05 00 00 00 00 00 00` |
| D | `01 03 05 00 01 00 00 00 01` | `01 03 05 00 01 00 00 00 ff` | `01 03 05 00 00 00 00 00 01` | `01 03 05 00 00 00 00 00 00` |

#### Dial Rotation Value Transform

For dial turn events, values are interpreted as:
- `0x01` to `0x7F`: Clockwise rotation (positive values 1-127)
- `0x80` to `0xFF`: Counter-clockwise rotation (negative values, calculated as `-(0x100 - value)`)

### Feature Reports (Stream Deck Plus)

| Report ID | Direction | Size | Purpose |
|-----------|-----------|------|---------|
| 0x03 | SET | 32 bytes | Commands (reset, brightness) |
| 0x05 | GET | 32 bytes | Firmware version |
| 0x06 | GET | 32 bytes | Serial number |

#### Reset Command
```
Payload: [0x03, 0x02, 0x00, ...]  (32 bytes)
```

#### Set Brightness Command
```
Payload: [0x03, 0x08, brightness, 0x00, ...]  (32 bytes)
brightness: 0x00 to 0x64 (0-100%)
```

#### Get Serial Number
```
Request: read_feature(0x06, 32)
Response: Serial number string at offset [5:]
```

#### Get Firmware Version
```
Request: read_feature(0x05, 32)
Response: Version string at offset [5:]
```

### Related Projects

- [DeckSurf SDK](https://github.com/dend/decksurf-sdk) - C# library supporting Stream Deck Plus
- [DeckSurf Docs](https://docs.deck.surf/) - Documentation for the SDK

## Stream Deck Neo

Sources:
- [python-elgato-streamdeck](https://github.com/abcminiuser/python-elgato-streamdeck) StreamDeckNeo.py
- [rust-elgato-streamdeck](https://github.com/OpenActionAPI/rust-elgato-streamdeck) - Rust library for Stream Deck

### Device Overview

| Property | Value |
|----------|-------|
| Vendor ID | 0x0fd9 |
| Product ID | 0x009a |
| KEY_COUNT | 8 |
| KEY_COLS | 4 |
| KEY_ROWS | 2 |
| TOUCHPOINT_COUNT | 2 |
| KEY_PIXEL_WIDTH | 96 |
| KEY_PIXEL_HEIGHT | 96 |
| KEY_IMAGE_FORMAT | JPEG |
| KEY_FLIP | (True, True) |
| KEY_ROTATION | 0° |
| TOUCHSCREEN_PIXEL_WIDTH | 248 |
| TOUCHSCREEN_PIXEL_HEIGHT | 58 |
| TOUCHSCREEN_IMAGE_FORMAT | JPEG |
| TOUCHSCREEN_FLIP | (False, False) |
| TOUCHSCREEN_ROTATION | 180° |
| DECK_VISUAL | True |
| DECK_TOUCH | True |

### Unique Features

1. **8 LCD Buttons** - 96×96 pixel JPEG images (same as XL), keys need 180° rotation (flipped on both X and Y)
2. **Info Bar Screen** - 248×58 pixel LCD touchscreen between buttons and touch points
3. **2 Touch Points** - Left and Right touch-sensitive buttons below the info bar (no encoders/knobs)
4. **LED Touch Points** - Touch points have controllable RGB LED strips

### Packet Sizes

| Constant | Value |
|----------|-------|
| _IMG_PACKET_LEN | 1024 bytes |
| _KEY_PACKET_HEADER | 8 bytes |
| _LCD_PACKET_HEADER | 8 bytes |
| _KEY_PACKET_PAYLOAD_LEN | 1016 bytes (1024 - 8) |
| _LCD_PACKET_PAYLOAD_LEN | 1016 bytes (1024 - 8) |

### Button Image Protocol

Images are JPEG-encoded, 96×96 pixels, rotated 180° (flipped on X and Y axes). Uses standard MK.2 packet format:

```
Header (8 bytes):
[0x00] Report ID (0x02)
[0x01] Command (0x07)
[0x02] Button index (0x00-0x07)
[0x03] Final chunk flag: 0x00 = more data, 0x01 = last packet
[0x04-0x05] Payload length (u16 Little Endian)
[0x06-0x07] Chunk index (u16 Little Endian, zero-based)

Payload: Up to 1016 bytes of JPEG data
```

### Info Bar Screen Protocol

The Neo's info bar screen uses a simpler protocol than the Plus. Full-screen writes only (no partial/region updates).

**LCD Fill Command (0x0B)**

```
Header (8 bytes):
[0x00] Report ID (0x02)
[0x01] Command (0x0B)
[0x02] Reserved (0x00)
[0x03] Final chunk flag: 0x00 = more, 0x01 = last
[0x04-0x05] Payload length (u16 Little Endian)
[0x06-0x07] Chunk index (u16 Little Endian, zero-based)

Payload: Up to 1016 bytes of JPEG data
```

Image requirements:
- Size: 248×58 pixels
- Format: JPEG
- Rotation: 180° (upside down)
- No mirroring needed

### Button Input Report Format

Uses standard Module 15/32 format:

```
[0x00] Report ID (0x01)
[0x01] Command (0x00 for key press state change)
[0x02-0x03] Payload data length (u16 LE) = number of keys
[0x04+] Key states: 0x00 = released, 0x01 = pressed
```

Button states include both the 8 keys and 2 touch points (total 10 elements).
- Keys: indices 0-7
- Touch Points: indices 8-9 (left=8, right=9)

### Touch Point Input

Touch points are NOT encoders - they're simple buttons. Their state is reported together with the key states in the button input report.

To distinguish:
- `key_count` = 8 (actual LCD buttons)
- `touchpoint_count` = 2 (touch-sensitive areas)
- Button report contains states for all 10 (8 keys + 2 touch points)

### Touch Point LED Control

Each touch point has an RGB LED strip that can be controlled:

**Set Touch Point Color (Report ID 0x03, Command 0x06)**

```
[0x00] Report ID (0x03)
[0x01] Command (0x06)
[0x02] Touch Point Index (key_count + point_index: 8 for left, 9 for right)
[0x03] Red (0x00-0xFF)
[0x04] Green (0x00-0xFF)
[0x05] Blue (0x00-0xFF)
```

### Feature Reports

Uses same format as Module 15/32:

| Report ID | Direction | Size | Purpose |
|-----------|-----------|------|---------|
| 0x03 | SET | 32 bytes | Commands (reset, brightness, touch point color) |
| 0x05 | GET | 32 bytes | Firmware version |
| 0x06 | GET | 32 bytes | Serial number |

#### Reset Command
```
Payload: [0x03, 0x02, 0x00, ...]  (32 bytes)
```

#### Set Brightness Command
```
Payload: [0x03, 0x08, brightness, 0x00, ...]  (32 bytes)
brightness: 0x00 to 0x64 (0-100%)
```

#### Get Serial Number
```
Request: get_feature_report(0x06, 32)
Response: [Report ID, Length, Serial String...]
Serial at offset [2:]
```

#### Get Firmware Version
```
Request: get_feature_report(0x05, 32)
Response: [Report ID, Length, Checksum(4), Version(8)]
Version string at offset [6:]
```

### Key Differences from Stream Deck Plus

| Feature | Stream Deck Neo | Stream Deck Plus |
|---------|-----------------|------------------|
| Product ID | 0x009a | 0x0084 |
| Key count | 8 | 8 |
| Key image size | 96×96 | 120×120 |
| Key rotation | 180° (flip X+Y) | None |
| Screen size | 248×58 | 800×100 |
| Screen rotation | 180° | None |
| Screen protocol | 0x0B (fill only) | 0x0C (region) |
| Encoders/Knobs | 0 | 4 |
| Touch points | 2 (simple buttons) | 0 |
| Touch screen | Info bar only | Full 800×100 |
| LED control | Touch point RGB | None |

## References

- [Elgato Stream Deck Module 6 HID Docs](https://docs.elgato.com/streamdeck/hid/module-6) - Official documentation
- [Elgato Stream Deck Module 15/32 HID Docs](https://docs.elgato.com/streamdeck/hid/module-15_32) - Official documentation
- [Reverse Engineering The Stream Deck Plus](https://den.dev/blog/reverse-engineer-stream-deck-plus/) - Den Delimarsky's blog post
- [python-elgato-streamdeck](https://github.com/abcminiuser/python-elgato-streamdeck) - Python library for Stream Deck
- [rust-elgato-streamdeck](https://github.com/OpenActionAPI/rust-elgato-streamdeck) - Rust library for Stream Deck
- [streamdeck-linux-gui](https://github.com/streamdeck-linux-gui/streamdeck-linux-gui) - Linux GUI for Stream Deck
- [USB HID Specification](https://www.usb.org/hid)
- [Linux USB Gadget Documentation](https://www.kernel.org/doc/html/latest/usb/gadget.html)
- [usb-gadget Rust crate](https://crates.io/crates/usb-gadget)

## Test Environment

- **Device**: Raspberry Pi 5
- **OS**: Raspberry Pi OS (Linux)
- **UDC**: `1000480000.usb` (dwc2)
- **Host**: macOS with official Stream Deck software
- **Connection**: USB-C (Pi) to USB-A (Mac)
