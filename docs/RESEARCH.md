# Stream Deck USB Gadget Emulation Research

## Project Overview

This project emulates Elgato Stream Deck devices on a Raspberry Pi using the Linux USB Gadget subsystem. The goal is to have the official Stream Deck software on macOS recognize the Pi as a real Stream Deck device.

## Key Findings

### USB Identification

| Model | Vendor ID | Product ID | Keys | Layout | Notes |
|-------|-----------|------------|------|--------|-------|
| Stream Deck Mini (Module 6) | 0x0fd9 | 0x0063 | 6 | 3×2 | 80×80 BMP images |
| Stream Deck Pedal | 0x0fd9 | 0x0086 | 3 | 3×1 | No display |
| Stream Deck MK.2 (Module 15) | 0x0fd9 | 0x00B9 | 15 | 5×3 | 72×72 JPEG images |
| Stream Deck XL (Module 32) | 0x0fd9 | 0x00BA | 32 | 8×4 | 96×96 JPEG images |
| Stream Deck Original | 0x0fd9 | 0x0060 | 15 | 5×3 | Legacy |
| Stream Deck Original V2 | 0x0fd9 | 0x006d | 15 | 5×3 | JPEG images |
| Stream Deck XL (Legacy) | 0x0fd9 | 0x006c | 32 | 8×4 | Legacy |

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

## References

- [Elgato Stream Deck Module 6 HID Docs](https://docs.elgato.com/streamdeck/hid/module-6) - Official documentation
- [Elgato Stream Deck Module 15/32 HID Docs](https://docs.elgato.com/streamdeck/hid/module-15_32) - Official documentation
- [python-elgato-streamdeck](https://github.com/abcminiuser/python-elgato-streamdeck) - Python library for Stream Deck
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
