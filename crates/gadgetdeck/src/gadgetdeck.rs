//! GadgetDeck - Main entry point for USB gadget Stream Deck emulation
//!
//! This module provides the `GadgetDeck` struct which wraps up all the
//! initialization and thread management for emulating a Stream Deck device.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use usb_gadget::{default_udc, Config, Gadget, RegGadget};

use crate::device::{ButtonState, ImageStore, PlusInputState};
use crate::usb::{CustomHid, StreamDeckModel, run_input_report_sender, run_output_report_receiver, run_plus_input_report_sender};

/// Error type for GadgetDeck operations
#[derive(Debug)]
pub enum GadgetDeckError {
    /// Failed to find a USB Device Controller (UDC)
    NoUdc(std::io::Error),
    /// Failed to bind the gadget to the UDC
    BindFailed(std::io::Error),
    /// Already started
    AlreadyStarted,
    /// Not started yet
    NotStarted,
}

impl std::fmt::Display for GadgetDeckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GadgetDeckError::NoUdc(e) => write!(f, "Failed to find UDC: {}", e),
            GadgetDeckError::BindFailed(e) => write!(f, "Failed to bind gadget: {}", e),
            GadgetDeckError::AlreadyStarted => write!(f, "GadgetDeck already started"),
            GadgetDeckError::NotStarted => write!(f, "GadgetDeck not started yet"),
        }
    }
}

impl std::error::Error for GadgetDeckError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GadgetDeckError::NoUdc(e) => Some(e),
            GadgetDeckError::BindFailed(e) => Some(e),
            _ => None,
        }
    }
}

/// Thread handles for the running GadgetDeck
struct GadgetDeckThreads {
    /// Thread that sends input reports (button states to host)
    input_thread: JoinHandle<()>,
    /// Thread that receives output reports (images from host)
    output_thread: JoinHandle<()>,
    /// Thread that processes USB control transfers
    event_thread: JoinHandle<()>,
}

/// Configuration for creating a GadgetDeck instance
pub struct GadgetDeckConfig {
    /// The Stream Deck model to emulate
    pub model: StreamDeckModel,
    /// The device serial number
    pub serial: String,
}

impl GadgetDeckConfig {
    /// Create a new configuration with the given model and serial
    pub fn new(model: StreamDeckModel, serial: impl Into<String>) -> Self {
        Self {
            model,
            serial: serial.into(),
        }
    }
    
    /// Create a configuration for Stream Deck Mini with the given serial
    pub fn mini(serial: impl Into<String>) -> Self {
        Self::new(StreamDeckModel::Mini, serial)
    }
    
    /// Create a configuration for Stream Deck Pedal with the given serial
    pub fn pedal(serial: impl Into<String>) -> Self {
        Self::new(StreamDeckModel::Pedal, serial)
    }
    
    /// Create a configuration for Stream Deck Plus with the given serial
    pub fn plus(serial: impl Into<String>) -> Self {
        Self::new(StreamDeckModel::Plus, serial)
    }
}

/// Main struct for managing a USB gadget Stream Deck emulation
///
/// This struct wraps up all the initialization, thread management, and
/// state for emulating a Stream Deck device over USB.
///
/// # Example
///
/// ```no_run
/// use gadgetdeck::{GadgetDeck, GadgetDeckConfig, StreamDeckModel};
/// use std::sync::atomic::Ordering;
///
/// let config = GadgetDeckConfig::new(StreamDeckModel::Mini, "ZZZZZZZZZZZZZZ");
/// let mut deck = GadgetDeck::new(config).expect("Failed to create GadgetDeck");
///
/// // Start the USB gadget and processing threads
/// deck.start().expect("Failed to start");
///
/// // Access button state
/// let buttons = deck.button_state();
/// buttons.press(0);
///
/// // Subscribe to image events
/// let image_rx = deck.subscribe_images();
///
/// // Run until shutdown
/// while deck.is_running() {
///     std::thread::sleep(std::time::Duration::from_millis(100));
/// }
///
/// // Stop and clean up
/// deck.stop();
/// ```
pub struct GadgetDeck {
    /// The Stream Deck model being emulated
    model: StreamDeckModel,
    /// Device serial number
    serial: String,
    /// Running flag shared with all threads
    running: Arc<AtomicBool>,
    /// Button state shared with input thread
    button_state: Arc<ButtonState>,
    /// Plus-specific input state (touchscreen/knobs) - only used for Plus model
    plus_state: Option<Arc<PlusInputState>>,
    /// Image store shared with output thread
    image_store: ImageStore,
    /// The registered gadget (dropped to clean up)
    gadget_reg: Option<RegGadget>,
    /// Thread handles (None until started)
    threads: Option<GadgetDeckThreads>,
    /// Custom HID instance (consumed when starting)
    custom_hid: Option<CustomHid>,
}

impl GadgetDeck {
    /// Create a new GadgetDeck instance with the given configuration
    ///
    /// This sets up the USB gadget and prepares for starting, but does not
    /// start the processing threads yet. Call `start()` to begin operation.
    pub fn new(config: GadgetDeckConfig) -> Result<Self, GadgetDeckError> {
        let model = config.model;
        let serial = config.serial;
        
        log::info!("Creating GadgetDeck: model={:?}, serial={}", model, serial);
        
        // Create custom HID function using FunctionFS
        let (custom_hid, hid_handle) = CustomHid::build(model, serial.clone());
        
        // Create configuration
        let usb_config = Config::new("Configuration 1")
            .with_function(hid_handle);
        
        // Find UDC
        let udc = default_udc().map_err(GadgetDeckError::NoUdc)?;
        log::info!("Found UDC: {:?}", udc);
        
        // Build and bind the USB gadget
        let gadget_reg = Gadget::new(
            model.device_class(),
            model.usb_id(),
            model.usb_strings(&serial),
        )
        .with_config(usb_config)
        .bind(&udc)
        .map_err(GadgetDeckError::BindFailed)?;
        
        log::info!("USB {} gadget registered!", model.product_name());
        
        // Create shared state
        let running = Arc::new(AtomicBool::new(true));
        let button_state = ButtonState::new(model);
        let image_store = ImageStore::new();
        
        // Create Plus-specific state if needed
        let plus_state = if model == StreamDeckModel::Plus {
            Some(PlusInputState::new())
        } else {
            None
        };
        
        Ok(Self {
            model,
            serial,
            running,
            button_state,
            plus_state,
            image_store,
            gadget_reg: Some(gadget_reg),
            threads: None,
            custom_hid: Some(custom_hid),
        })
    }
    
    /// Start the USB gadget processing threads
    ///
    /// This spawns threads for:
    /// - Input report sender (button states to host)
    /// - Output report receiver (images from host)
    /// - USB control transfer event processing
    pub fn start(&mut self) -> Result<(), GadgetDeckError> {
        if self.threads.is_some() {
            return Err(GadgetDeckError::AlreadyStarted);
        }
        
        let mut custom_hid = self.custom_hid.take()
            .ok_or(GadgetDeckError::AlreadyStarted)?;
        
        // Take ownership of endpoints for separate threads
        let ep_in = custom_hid.take_ep_in().expect("EP IN should be available");
        let ep_out = custom_hid.take_ep_out().expect("EP OUT should be available");
        let ep_model = custom_hid.model();
        
        // Spawn input report sender thread
        // Use Plus-specific sender for Plus model (handles touchscreen/knobs)
        let running_input = self.running.clone();
        let button_state_input = self.button_state.clone();
        let plus_state_input = self.plus_state.clone();
        let input_thread = if ep_model == StreamDeckModel::Plus {
            let plus_state = plus_state_input.expect("Plus state should exist for Plus model");
            thread::spawn(move || {
                run_plus_input_report_sender(ep_in, running_input, button_state_input, plus_state);
            })
        } else {
            thread::spawn(move || {
                run_input_report_sender(ep_in, ep_model, running_input, button_state_input);
            })
        };
        
        // Spawn output report receiver thread
        // For Pedal mode, we still need to drain the output endpoint even though there's no display
        // Otherwise the host may timeout waiting for the endpoint to be read
        let running_output = self.running.clone();
        let output_image_store = self.image_store.clone();
        let output_thread = thread::spawn(move || {
            run_output_report_receiver(ep_out, ep_model, running_output, output_image_store);
        });
        
        // Spawn USB control transfer event processing thread
        let running_event = self.running.clone();
        let event_thread = thread::spawn(move || {
            log::info!("Starting USB event processing...");
            if let Err(e) = custom_hid.process(running_event.clone()) {
                if running_event.load(Ordering::SeqCst) {
                    log::error!("USB event processing error: {}", e);
                }
            }
            log::info!("USB event processing stopped");
        });
        
        self.threads = Some(GadgetDeckThreads {
            input_thread,
            output_thread,
            event_thread,
        });
        
        log::info!("GadgetDeck started");
        Ok(())
    }
    
    /// Stop the GadgetDeck and wait for all threads to finish
    pub fn stop(&mut self) {
        log::info!("Stopping GadgetDeck...");
        
        // Signal all threads to stop
        self.running.store(false, Ordering::SeqCst);
        
        // Drop the gadget registration to trigger cleanup
        self.gadget_reg.take();
        
        // Wait for threads to finish
        if let Some(threads) = self.threads.take() {
            let _ = threads.input_thread.join();
            let _ = threads.output_thread.join();
            let _ = threads.event_thread.join();
        }
        
        log::info!("GadgetDeck stopped");
    }
    
    /// Check if the GadgetDeck is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
    
    /// Signal the GadgetDeck to stop (non-blocking)
    ///
    /// Use `stop()` if you want to wait for threads to finish.
    pub fn signal_stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
    
    /// Get a clone of the running flag for use in signal handlers
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }
    
    /// Get the Stream Deck model being emulated
    pub fn model(&self) -> StreamDeckModel {
        self.model
    }
    
    /// Get the device serial number
    pub fn serial(&self) -> &str {
        &self.serial
    }
    
    /// Get the button state manager
    ///
    /// Use this to read or update button states. Changes will be
    /// automatically sent to the host.
    pub fn button_state(&self) -> Arc<ButtonState> {
        self.button_state.clone()
    }
    
    /// Get the Plus-specific input state (touchscreen/knobs)
    ///
    /// Returns `None` if not emulating a Stream Deck Plus.
    /// Use this to send touchscreen touch/swipe events and knob events.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use gadgetdeck::{GadgetDeck, GadgetDeckConfig, StreamDeckModel};
    ///
    /// let config = GadgetDeckConfig::plus("SERIAL123");
    /// let deck = GadgetDeck::new(config).unwrap();
    ///
    /// if let Some(plus) = deck.plus_state() {
    ///     // Send a horizontal swipe from left to right
    ///     plus.swipe_horizontal(50, 750);
    ///     
    ///     // Send a tap on segment B
    ///     plus.tap(300, 50);
    /// }
    /// ```
    pub fn plus_state(&self) -> Option<Arc<PlusInputState>> {
        self.plus_state.clone()
    }
    
    /// Get the image store
    ///
    /// Use this to access received button images.
    pub fn image_store(&self) -> ImageStore {
        self.image_store.clone()
    }
    
    /// Subscribe to image update events
    ///
    /// Returns a receiver that will receive `ImageEvent` messages
    /// whenever a button image is updated by the host.
    pub fn subscribe_images(&self) -> crate::device::ImageEventReceiver {
        self.image_store.subscribe()
    }
}

impl Drop for GadgetDeck {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_creation() {
        let config = GadgetDeckConfig::new(StreamDeckModel::Mini, "TEST123");
        assert_eq!(config.model, StreamDeckModel::Mini);
        assert_eq!(config.serial, "TEST123");
    }
    
    #[test]
    fn test_config_mini() {
        let config = GadgetDeckConfig::mini("SERIAL");
        assert_eq!(config.model, StreamDeckModel::Mini);
    }
    
    #[test]
    fn test_config_pedal() {
        let config = GadgetDeckConfig::pedal("SERIAL");
        assert_eq!(config.model, StreamDeckModel::Pedal);
    }
    
    #[test]
    fn test_config_plus() {
        let config = GadgetDeckConfig::plus("SERIAL");
        assert_eq!(config.model, StreamDeckModel::Plus);
    }
}
