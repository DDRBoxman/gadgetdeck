# GadgetDeck

A Rust library for emulating Elgato Stream Deck devices as USB gadgets on Linux.

## Overview

GadgetDeck provides the core functionality for creating USB gadgets that appear to host computers as real Stream Deck devices. Applications like the Elgato Stream Deck software can communicate with the emulated device just like physical hardware.

- **Multiple Device Models** – Emulates Mini, MK.2, XL, Plus, and Pedal
- **USB HID Implementation** – Full HID protocol support for input/output reports
- **Thread-Safe State Management** – Concurrent access to button states and images
- **Image Store** – Receives and caches button images from host software
- **Plus Support** – Touchscreen and rotary encoder events for Stream Deck Plus

## Quick Start

```rust
use gadgetdeck::{GadgetDeck, GadgetDeckConfig, StreamDeckModel};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a Stream Deck Mini emulator
    let config = GadgetDeckConfig::new(StreamDeckModel::Mini, "MYSERIAL123");
    let mut deck = GadgetDeck::new(config)?;
    
    // Start USB processing threads
    deck.start()?;
    
    // Access button state
    let buttons = deck.button_state();
    buttons.press(0);  // Press button 0
    buttons.release(0);
    
    // Subscribe to image updates
    let image_rx = deck.subscribe_images();
    
    // Run until stopped
    while deck.is_running() {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    deck.stop();
    Ok(())
}
```

## Supported Devices

| Model | Keys | Layout | Special Features |
|-------|------|--------|------------------|
| `Mini` | 6 | 3×2 | – |
| `Mk2` | 15 | 5×3 | – |
| `Xl` | 32 | 8×4 | – |
| `Plus` | 8 | 4×2 | 4 rotary knobs, LCD touchscreen |
| `Pedal` | 3 | 3×1 | Foot pedals (no display) |

## API Reference

### GadgetDeck

The main struct for managing USB gadget emulation.

```rust
use gadgetdeck::{GadgetDeck, GadgetDeckConfig, StreamDeckModel};

// Create with specific model
let config = GadgetDeckConfig::new(StreamDeckModel::Xl, "SERIAL");
let mut deck = GadgetDeck::new(config)?;

// Or use convenience constructors
let config = GadgetDeckConfig::mini("SERIAL");
let config = GadgetDeckConfig::plus("SERIAL");
let config = GadgetDeckConfig::pedal("SERIAL");
```

#### Methods

| Method | Description |
|--------|-------------|
| `new(config)` | Create a new GadgetDeck instance |
| `start()` | Start USB processing threads |
| `stop()` | Stop and clean up |
| `is_running()` | Check if still running |
| `signal_stop()` | Signal stop (non-blocking) |
| `running_flag()` | Get `Arc<AtomicBool>` for signal handlers |
| `model()` | Get the Stream Deck model |
| `serial()` | Get the device serial number |
| `button_state()` | Get button state manager |
| `plus_state()` | Get Plus-specific state (if Plus model) |
| `image_store()` | Get image store |
| `subscribe_images()` | Subscribe to image update events |

### ButtonState

Thread-safe button state manager.

```rust
let buttons = deck.button_state();

// Query state
let num = buttons.num_buttons();  // e.g., 6 for Mini
let pressed = buttons.is_pressed(0);

// Update state (automatically sent to host)
buttons.press(0);
buttons.release(0);
buttons.click(0);  // Press, wait 50ms, release
```

### ImageStore

Stores button images received from the host.

```rust
let images = deck.image_store();

// Get image for a button
if let Some(image) = images.get_image(0) {
    let bytes: &[u8] = image.as_bytes();
    // Process image data (JPEG or BMP depending on model)
}

// Get statistics
let stats = images.stats();
println!("Received {} images", stats.images_completed);
```

### Image Events

Subscribe to real-time image updates.

```rust
use gadgetdeck::ImageEvent;

let rx = deck.subscribe_images();

loop {
    match rx.recv_timeout(Duration::from_millis(100)) {
        Ok(ImageEvent::Updated { key_index, image }) => {
            println!("Button {} image updated", key_index);
        }
        Ok(ImageEvent::LcdUpdated { x_offset, y_offset, width, height, image }) => {
            println!("LCD segment at ({}, {}) updated", x_offset, y_offset);
        }
        Err(_) => continue,
    }
}
```

### PlusInputState (Plus Only)

For Stream Deck Plus touchscreen and knob events.

```rust
use gadgetdeck::KnobIndex;

if let Some(plus) = deck.plus_state() {
    // Touchscreen events
    plus.tap(200, 50);                    // Tap at coordinates
    plus.long_press(200, 50);             // Long press
    plus.swipe(100, 50, 700, 50);         // Swipe gesture
    plus.swipe_horizontal(100, 700);      // Horizontal swipe (y=50)
    
    // Knob events
    plus.press_knob(KnobIndex::A);
    plus.release_knob(KnobIndex::A);
    plus.turn_knob(KnobIndex::B, 3);      // Clockwise 3 steps
    plus.turn_knob(KnobIndex::C, -2);     // Counter-clockwise 2 steps
}
```

## Module Structure

```
gadgetdeck/
├── lib.rs              # Re-exports and crate documentation
├── gadgetdeck.rs       # GadgetDeck main struct
├── device/
│   ├── buttons.rs      # ButtonState management
│   ├── image.rs        # ImageStore and image handling
│   └── plus.rs         # PlusInputState (touch/knobs)
└── usb/
    ├── custom_hid.rs   # HID endpoint handling
    └── descriptors.rs  # USB/HID descriptors per model
```

## Requirements

- Linux with USB gadget support (ConfigFS)
- Root privileges or appropriate permissions
- USB OTG-capable port

### Kernel Modules

```bash
sudo modprobe libcomposite
sudo modprobe dwc2
```

## License

MIT OR Apache-2.0
