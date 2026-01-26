//! Custom HID implementation using FunctionFS
//!
//! This provides full control over HID control transfers, allowing us to
//! respond to GET_REPORT feature report requests.

use bytes::{Bytes, BytesMut};
use std::io::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use usb_gadget::function::custom::{
    Custom, CustomDesc, Endpoint, EndpointDirection,
    EndpointReceiver, EndpointSender, Event, Interface, TransferType,
};
use usb_gadget::Class;

use crate::device::ButtonState;
use crate::device::ImageStore;
use crate::device::PlusInputState;
use super::descriptors::StreamDeckModel;

// HID Class-Specific Request Codes
const HID_REQ_GET_REPORT: u8 = 0x01;
const HID_REQ_GET_IDLE: u8 = 0x02;
const HID_REQ_GET_PROTOCOL: u8 = 0x03;
const HID_REQ_SET_REPORT: u8 = 0x09;
const HID_REQ_SET_IDLE: u8 = 0x0A;
const HID_REQ_SET_PROTOCOL: u8 = 0x0B;

// HID Report Types (in wValue high byte)
#[allow(dead_code)]
const HID_REPORT_TYPE_INPUT: u8 = 1;
#[allow(dead_code)]
const HID_REPORT_TYPE_OUTPUT: u8 = 2;
const HID_REPORT_TYPE_FEATURE: u8 = 3;

// USB Request Types
const USB_TYPE_CLASS: u8 = 0x20;
const USB_RECIP_INTERFACE: u8 = 0x01;
#[allow(dead_code)]
const USB_DIR_IN: u8 = 0x80;

// HID Descriptor Type
const HID_DESCRIPTOR_TYPE_HID: u8 = 0x21;
const HID_DESCRIPTOR_TYPE_REPORT: u8 = 0x22;

/// Custom HID device using FunctionFS
pub struct CustomHid {
    custom: Custom,
    model: StreamDeckModel,
    serial: String,
    ep_in: Option<EndpointSender>,
    ep_out: Option<EndpointReceiver>,
}

impl CustomHid {
    /// Build a custom HID function for the given Stream Deck model
    pub fn build(model: StreamDeckModel, serial: String) -> (Self, usb_gadget::function::Handle) {
        let hid_config = model.hid_config();
        
        // Create HID descriptor data (excluding bLength and bDescriptorType which are added by CustomDesc)
        // Full HID descriptor is 9 bytes, minus 2 for length/type = 7 bytes of data
        let report_desc_len = hid_config.report_descriptor.len() as u16;
        let hid_descriptor_data = vec![
            0x11, 0x01,                 // bcdHID (1.11)
            0x00,                       // bCountryCode
            0x01,                       // bNumDescriptors
            HID_DESCRIPTOR_TYPE_REPORT, // bDescriptorType (Report)
            (report_desc_len & 0xFF) as u8,         // wDescriptorLength low
            ((report_desc_len >> 8) & 0xFF) as u8,  // wDescriptorLength high
        ];
        
        // Create endpoints
        let (ep_in, ep_in_dir) = EndpointDirection::device_to_host();
        let (ep_out, ep_out_dir) = EndpointDirection::host_to_device();
        
        // Create interrupt endpoints
        let mut ep_in_endpoint = Endpoint::custom(ep_in_dir, TransferType::Interrupt);
        ep_in_endpoint.interval = hid_config.interval;
        
        let mut ep_out_endpoint = Endpoint::custom(ep_out_dir, TransferType::Interrupt);
        ep_out_endpoint.interval = hid_config.interval;
        
        // HID class: 0x03, no subclass, no protocol (or boot protocol)
        let hid_class = Class::new(0x03, hid_config.sub_class, hid_config.protocol);
        
        let interface = Interface::new(hid_class, model.product_name())
            .with_custom_desc(CustomDesc::new(HID_DESCRIPTOR_TYPE_HID, hid_descriptor_data))
            .with_endpoint(ep_in_endpoint)
            .with_endpoint(ep_out_endpoint);
        
        let mut builder = Custom::builder();
        builder.all_ctrl_recipient = true;  // Receive all control requests
        
        let (custom, handle) = builder
            .with_interface(interface)
            .build();
        
        (
            Self {
                custom,
                model,
                serial,
                ep_in: Some(ep_in),
                ep_out: Some(ep_out),
            },
            handle,
        )
    }
    
    /// Take ownership of the endpoint sender (for use in a separate thread)
    pub fn take_ep_in(&mut self) -> Option<EndpointSender> {
        self.ep_in.take()
    }
    
    /// Take ownership of the endpoint receiver (for use in a separate thread)
    pub fn take_ep_out(&mut self) -> Option<EndpointReceiver> {
        self.ep_out.take()
    }
    
    /// Get the model
    pub fn model(&self) -> StreamDeckModel {
        self.model
    }
    
    /// Process events from the USB host
    pub fn process(&mut self, running: Arc<AtomicBool>) -> Result<()> {
        loop {
            // Check if we should shut down
            if !running.load(Ordering::SeqCst) {
                log::info!("Shutdown requested, stopping event processing");
                break;
            }
            
            // Use try_event which properly clears previous events, then sleep if no event
            let event = match self.custom.try_event() {
                Ok(Some(e)) => e,
                Ok(None) => {
                    // No event available, sleep briefly and check running flag
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    continue;
                }
                Err(e) => {
                    // Level 2 halted (51) or Broken pipe typically means USB disconnected
                    if e.raw_os_error() == Some(51) || e.kind() == std::io::ErrorKind::BrokenPipe {
                        // Check if shutdown was requested
                        if !running.load(Ordering::SeqCst) {
                            log::info!("Shutdown requested during disconnect");
                            break;
                        }
                        log::info!("USB disconnected, waiting for reconnection...");
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        continue;
                    }
                    return Err(e);
                }
            };
            
            match event {
                Event::Bind => {
                    log::info!("HID gadget bound");
                }
                Event::Unbind => {
                    log::info!("HID gadget unbound");
                    break;
                }
                Event::Enable => {
                    log::info!("HID gadget enabled");
                }
                Event::Disable => {
                    log::info!("HID gadget disabled");
                }
                Event::Suspend => {
                    log::debug!("HID gadget suspended");
                }
                Event::Resume => {
                    log::debug!("HID gadget resumed");
                }
                Event::SetupDeviceToHost(sender) => {
                    // Host is requesting data from device
                    let ctrl = sender.ctrl_req();
                    log::debug!(
                        "Setup D->H: type=0x{:02X} req=0x{:02X} value=0x{:04X} index=0x{:04X} len={}",
                        ctrl.request_type, ctrl.request, ctrl.value, ctrl.index, ctrl.length
                    );
                    
                    if Self::handle_get_request(&self.model, &self.serial, sender)? {
                        log::debug!("Request handled");
                    } else {
                        log::debug!("Request not handled, stalling");
                    }
                }
                Event::SetupHostToDevice(receiver) => {
                    // Host is sending data to device
                    let ctrl = receiver.ctrl_req();
                    log::debug!(
                        "Setup H->D: type=0x{:02X} req=0x{:02X} value=0x{:04X} index=0x{:04X} len={}",
                        ctrl.request_type, ctrl.request, ctrl.value, ctrl.index, ctrl.length
                    );
                    
                    if Self::handle_set_request(receiver)? {
                        log::debug!("Request handled");
                    } else {
                        log::debug!("Request not handled, stalling");
                    }
                }
                Event::Unknown(code) => {
                    log::warn!("Unknown event: {}", code);
                }
                _ => {
                    log::debug!("Unhandled event");
                }
            }
        }
        Ok(())
    }
    
    /// Handle GET requests (device to host)
    fn handle_get_request(model: &StreamDeckModel, serial: &str, sender: usb_gadget::function::custom::CtrlSender) -> Result<bool> {
        let ctrl = sender.ctrl_req();
        let request_type = ctrl.request_type;
        let request = ctrl.request;
        
        // Check if this is a class request to interface
        if (request_type & 0x60) == USB_TYPE_CLASS && (request_type & 0x1F) == USB_RECIP_INTERFACE {
            match request {
                HID_REQ_GET_REPORT => {
                    let report_type = ((ctrl.value >> 8) & 0xFF) as u8;
                    let report_id = (ctrl.value & 0xFF) as u8;
                    let length = ctrl.length as usize;
                    
                    log::info!("GET_REPORT: type={} id=0x{:02X} len={}", report_type, report_id, length);
                    
                    if report_type == HID_REPORT_TYPE_FEATURE {
                        if let Some(mut data) = model.get_feature_report(report_id, serial) {
                            // Pad to requested length if needed
                            if data.len() < length {
                                data.resize(length, 0);
                            }
                            log::info!("Sending feature report 0x{:02X} ({} bytes)", report_id, data.len());
                            sender.send(&data)?;
                            return Ok(true);
                        }
                        // Return zeroed report for unknown feature report IDs
                        // This prevents stalling which disconnects the device
                        log::info!("Sending empty feature report 0x{:02X} ({} bytes)", report_id, length);
                        let empty_report = vec![0u8; length];
                        sender.send(&empty_report)?;
                        return Ok(true);
                    }
                    // Stall for non-feature reports we don't handle
                    sender.halt()?;
                    return Ok(true);
                }
                HID_REQ_GET_IDLE => {
                    // Return idle rate (0 = report only when changed)
                    sender.send(&[0])?;
                    return Ok(true);
                }
                HID_REQ_GET_PROTOCOL => {
                    // Return protocol (1 = report protocol)
                    sender.send(&[1])?;
                    return Ok(true);
                }
                _ => {}
            }
        }
        
        // Check if this is a standard GET_DESCRIPTOR for HID report descriptor
        if request_type == 0x81 && request == 0x06 {
            let desc_type = ((ctrl.value >> 8) & 0xFF) as u8;
            if desc_type == HID_DESCRIPTOR_TYPE_REPORT {
                let report_desc = model.hid_config().report_descriptor;
                log::info!("Sending HID report descriptor ({} bytes)", report_desc.len());
                sender.send(&report_desc)?;
                return Ok(true);
            }
        }
        
        // Not handled - will stall
        Ok(false)
    }
    
    /// Handle SET requests (host to device)
    fn handle_set_request(receiver: usb_gadget::function::custom::CtrlReceiver) -> Result<bool> {
        let ctrl = receiver.ctrl_req();
        let request_type = ctrl.request_type;
        let request = ctrl.request;
        let length = ctrl.length;
        
        // Check if this is a class request to interface
        if (request_type & 0x60) == USB_TYPE_CLASS && (request_type & 0x1F) == USB_RECIP_INTERFACE {
            match request {
                HID_REQ_SET_REPORT => {
                    let report_type = ((receiver.ctrl_req().value >> 8) & 0xFF) as u8;
                    let report_id = (receiver.ctrl_req().value & 0xFF) as u8;
                    
                    log::info!("SET_REPORT: type={} id=0x{:02X}", report_type, report_id);
                    
                    // Read the data
                    let data = receiver.recv_all()?;
                    // Log the first 16 bytes of data for debugging
                    let display_len = data.len().min(16);
                    let hex_str: String = data[..display_len].iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    log::info!("SET_REPORT data[0..{}]: {}", display_len, hex_str);
                    
                    // TODO: Handle output reports (e.g., setting brightness, images)
                    return Ok(true);
                }
                HID_REQ_SET_IDLE => {
                    // Accept but ignore idle rate - may have no data
                    if length > 0 {
                        let _ = receiver.recv_all()?;
                    }
                    return Ok(true);
                }
                HID_REQ_SET_PROTOCOL => {
                    // Accept but ignore protocol changes - may have no data
                    if length > 0 {
                        let _ = receiver.recv_all()?;
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }
        
        // Not handled
        Ok(false)
    }
    
    /// Send an input report (e.g., button press)
    #[allow(dead_code)]
    pub fn send_input_report(&mut self, data: &[u8]) -> Result<()> {
        if let Some(ref mut ep_in) = self.ep_in {
            ep_in.send_and_flush(Bytes::copy_from_slice(data))
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "EP IN not available"))
        }
    }
    
    /// Receive an output report
    #[allow(dead_code)]
    pub fn recv_output_report(&mut self, capacity: usize) -> Result<BytesMut> {
        if let Some(ref mut ep_out) = self.ep_out {
            let buf = BytesMut::with_capacity(capacity);
            ep_out.recv_and_fetch(buf)
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "EP OUT not available"))
        }
    }
}

/// Input report sender thread - sends periodic button state reports
pub fn run_input_report_sender(
    mut ep_in: EndpointSender,
    model: StreamDeckModel,
    running: Arc<AtomicBool>,
    button_state: Arc<ButtonState>,
) {
    log::info!("Starting input report sender thread");
    
    let num_buttons = button_state.num_buttons();
    
    // Initial delay to let device fully enumerate
    thread::sleep(Duration::from_millis(100));
    
    // Send initial button state
    let input_report = button_state.build_input_report(model);
    log::info!("Sending initial button state (all {} buttons released)", num_buttons);
    if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&input_report)) {
        log::warn!("Failed to send initial input report: {}", e);
    }
    
    // Continue sending periodic reports while running
    let mut report_count = 0u64;
    while running.load(Ordering::Relaxed) {
        // Check for state changes or send periodic keepalive
        let send_report = button_state.take_changed();
        
        if send_report {
            // Build and send updated button state immediately
            let input_report = button_state.build_input_report(model);
            
            // Log button states for debugging
            let states: Vec<String> = (0..num_buttons)
                .map(|i| if button_state.is_pressed(i) { "1" } else { "0" }.to_string())
                .collect();
            log::debug!("Button states: [{}]", states.join(", "));
            
            if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&input_report)) {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                
                let os_error = e.raw_os_error();
                if os_error == Some(51) || e.kind() == std::io::ErrorKind::BrokenPipe {
                    log::debug!("EP IN disconnected, waiting...");
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                
                log::warn!("Failed to send input report: {} (os_error: {:?})", e, os_error);
            } else {
                report_count += 1;
            }
        } else {
            // Send periodic keepalive report every 100ms
            thread::sleep(Duration::from_millis(100));
            
            let input_report = button_state.build_input_report(model);
            if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&input_report)) {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                
                let os_error = e.raw_os_error();
                if os_error == Some(51) || e.kind() == std::io::ErrorKind::BrokenPipe {
                    log::debug!("EP IN disconnected, waiting...");
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                
                log::warn!("Failed to send input report: {} (os_error: {:?})", e, os_error);
                thread::sleep(Duration::from_millis(100));
            } else {
                report_count += 1;
                if report_count % 100 == 0 {
                    log::debug!("Sent {} input reports", report_count);
                }
            }
        }
    }
    
    log::info!("Input report sender thread stopped (sent {} reports)", report_count);
}

/// Input report sender thread for Stream Deck Plus - handles buttons, touchscreen, and knobs
/// 
/// This is an enhanced version of `run_input_report_sender` that also sends
/// touchscreen touch/swipe events and rotary encoder (knob) events.
pub fn run_plus_input_report_sender(
    mut ep_in: EndpointSender,
    running: Arc<AtomicBool>,
    button_state: Arc<ButtonState>,
    plus_state: Arc<PlusInputState>,
) {
    log::info!("Starting Plus input report sender thread");
    
    let model = StreamDeckModel::Plus;
    let num_buttons = button_state.num_buttons();
    
    // Initial delay to let device fully enumerate
    thread::sleep(Duration::from_millis(100));
    
    // Send initial button state
    let input_report = button_state.build_input_report(model);
    log::info!("Sending initial button state (all {} buttons released)", num_buttons);
    if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&input_report)) {
        log::warn!("Failed to send initial input report: {}", e);
    }
    
    // Continue sending reports while running
    let mut report_count = 0u64;
    let poll_interval = Duration::from_millis(10);  // Fast polling for responsive touch/knob
    let keepalive_interval = Duration::from_millis(100);
    let mut last_keepalive = std::time::Instant::now();
    
    while running.load(Ordering::Relaxed) {
        let mut sent_event = false;
        
        // Priority 1: Check for touch events (touchscreen swipe/tap)
        if let Some(touch_event) = plus_state.take_touch_event() {
            let report = touch_event.build_input_report();
            log::info!(
                "Sending touch event: {:?} at ({}, {}) -> ({}, {})",
                touch_event.event_type, touch_event.x, touch_event.y,
                touch_event.x_end, touch_event.y_end
            );
            
            if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&report)) {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                log::warn!("Failed to send touch event: {}", e);
            } else {
                report_count += 1;
                sent_event = true;
            }
        }
        
        // Priority 2: Check for knob rotation events
        for knob_idx in 0..4u8 {
            let knob = crate::device::KnobIndex::from(knob_idx);
            let rotation = plus_state.take_knob_rotation(knob);
            if rotation != 0 {
                let report = plus_state.build_knob_turn_report(knob, rotation);
                log::info!("Sending knob {:?} rotation: {}", knob, rotation);
                
                if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&report)) {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    log::warn!("Failed to send knob turn event: {}", e);
                } else {
                    report_count += 1;
                    sent_event = true;
                }
            }
        }
        
        // Priority 3: Check for knob press state changes
        if plus_state.take_knob_changed() {
            let report = plus_state.build_knob_press_report();
            log::debug!("Sending knob press state update");
            
            if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&report)) {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                log::warn!("Failed to send knob press event: {}", e);
            } else {
                report_count += 1;
                sent_event = true;
            }
        }
        
        // Priority 4: Check for button state changes
        if button_state.take_changed() {
            let input_report = button_state.build_input_report(model);
            
            // Log button states for debugging
            let states: Vec<String> = (0..num_buttons)
                .map(|i| if button_state.is_pressed(i) { "1" } else { "0" }.to_string())
                .collect();
            log::debug!("Button states: [{}]", states.join(", "));
            
            if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&input_report)) {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                
                let os_error = e.raw_os_error();
                if os_error == Some(51) || e.kind() == std::io::ErrorKind::BrokenPipe {
                    log::debug!("EP IN disconnected, waiting...");
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                
                log::warn!("Failed to send input report: {} (os_error: {:?})", e, os_error);
            } else {
                report_count += 1;
                sent_event = true;
            }
        }
        
        // Send periodic keepalive if no events sent recently
        if !sent_event && last_keepalive.elapsed() >= keepalive_interval {
            let input_report = button_state.build_input_report(model);
            if let Err(e) = ep_in.send_and_flush(Bytes::copy_from_slice(&input_report)) {
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                
                let os_error = e.raw_os_error();
                if os_error == Some(51) || e.kind() == std::io::ErrorKind::BrokenPipe {
                    log::debug!("EP IN disconnected, waiting...");
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                
                log::warn!("Failed to send keepalive: {} (os_error: {:?})", e, os_error);
            } else {
                report_count += 1;
                if report_count % 100 == 0 {
                    log::debug!("Sent {} input reports", report_count);
                }
            }
            last_keepalive = std::time::Instant::now();
        }
        
        // Small sleep to avoid busy-waiting
        thread::sleep(poll_interval);
    }
    
    log::info!("Plus input report sender thread stopped (sent {} reports)", report_count);
}

/// Output report receiver thread - receives image data and other output reports
pub fn run_output_report_receiver(
    mut ep_out: EndpointReceiver,
    model: StreamDeckModel,
    running: Arc<AtomicBool>,
    image_store: ImageStore,
) {
    log::info!("Starting output report receiver thread");
    
    let hid_config = model.hid_config();
    let max_packet_size = hid_config.out_max_packet_size as usize;
    
    let mut report_count = 0u64;
    let mut total_bytes = 0u64;
    
    while running.load(Ordering::Relaxed) {
        // Create buffer for receiving
        let buf = BytesMut::with_capacity(max_packet_size);
        
        match ep_out.recv_and_fetch(buf) {
            Ok(data) => {
                report_count += 1;
                total_bytes += data.len() as u64;
                
                if !data.is_empty() {
                    let report_id = data[0];
                    
                    // Log all incoming output reports for debugging
                    if report_count <= 20 || report_count % 100 == 0 {
                        let display_len = data.len().min(16);
                        let hex_str: String = data[..display_len].iter()
                            .map(|b| format!("{:02X}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        log::info!(
                            "Output report #{}: id=0x{:02X} len={} data[0..{}]: {}",
                            report_count, report_id, data.len(), display_len, hex_str
                        );
                    }
                    
                    match report_id {
                        0x02 => {
                            // Image data - process through ImageStore
                            match image_store.process_packet(&data) {
                                Ok(Some(key_index)) => {
                                    // Image complete!
                                    if let Some(image) = image_store.get_image(key_index) {
                                        log::info!(
                                            "Button {} image received: {} bytes (total images: {})",
                                            key_index,
                                            image.len(),
                                            image_store.stats().images_completed
                                        );
                                    }
                                }
                                Ok(None) => {
                                    // More packets needed
                                }
                                Err(e) => {
                                    log::warn!("Image packet error: {}", e);
                                }
                            }
                        }
                        _ => {
                            // Non-image output report
                        }
                    }
                }
            }
            Err(e) => {
                // Check if we should still be running
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                
                let os_error = e.raw_os_error();
                if os_error == Some(51) || e.kind() == std::io::ErrorKind::BrokenPipe {
                    log::debug!("EP OUT disconnected, waiting...");
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                
                // EAGAIN means no data available
                if os_error == Some(11) {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                
                log::warn!("Failed to receive output report: {} (os_error: {:?})", e, os_error);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
    
    // Log final stats
    let stats = image_store.stats();
    log::info!(
        "Output report receiver thread stopped (received {} reports, {} bytes total, {} images completed)",
        report_count, total_bytes, stats.images_completed
    );
}
