//! HTTP and WebSocket handlers

use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::sync::atomic::Ordering;
use tokio::sync::broadcast;

use crate::html::INDEX_HTML;
use crate::state::{AppState, ButtonStateInfo, WsMessage};

// ============================================================================
// Web UI Handler
// ============================================================================

/// Serve the web UI
pub async fn index_handler(State(state): State<AppState>) -> Html<String> {
    Html(
        INDEX_HTML
            .replace("{{KEY_COLS}}", &state.key_cols.to_string())
            .replace(
                "{{IMAGE_FORMAT}}",
                &state.model.image_format().to_lowercase(),
            ),
    )
}

// ============================================================================
// WebSocket Handler
// ============================================================================

/// WebSocket handler for real-time image updates
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to WebSocket updates
    let mut ws_rx = state.ws_tx.subscribe();

    // Send current state on connect
    {
        use base64::Engine;

        // Send current button states
        let buttons: Vec<ButtonStateInfo> = (0..state.button_state.num_buttons())
            .map(|i| ButtonStateInfo {
                id: i,
                pressed: state.button_state.is_pressed(i),
            })
            .collect();
        let update = WsMessage::ButtonsUpdate { buttons };
        if let Ok(json) = serde_json::to_string(&update) {
            if sender.send(Message::Text(json.into())).await.is_err() {
                return;
            }
        }

        // Send current button images
        for i in 0..state.button_state.num_buttons() {
            if let Some(image) = state.image_store.get_image(i) {
                let update = WsMessage::ImageUpdate {
                    button_id: i,
                    image_data: base64::engine::general_purpose::STANDARD.encode(image.as_bytes()),
                };
                if let Ok(json) = serde_json::to_string(&update) {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        return;
                    }
                }
            }
        }

        // Send current LCD segments (for Plus model)
        for segment in state.lcd_store.get_all() {
            let update = WsMessage::LcdUpdate {
                x_offset: segment.x_offset,
                y_offset: segment.y_offset,
                width: segment.width,
                height: segment.height,
                image_data: base64::engine::general_purpose::STANDARD.encode(&segment.image_data),
            };
            if let Ok(json) = serde_json::to_string(&update) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    return;
                }
            }
        }
    }

    // Handle incoming messages and broadcast updates
    loop {
        tokio::select! {
            // Forward updates to the client
            result = ws_rx.recv() => {
                match result {
                    Ok(update) => {
                        if let Ok(json) = serde_json::to_string(&update) {
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Missed some messages, continue
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            // Handle messages from the client (for keep-alive pings)
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        if sender.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
}

// ============================================================================
// Status API
// ============================================================================

/// Status response
#[derive(Serialize)]
pub struct StatusResponse {
    pub model: String,
    pub num_buttons: u8,
    pub running: bool,
}

pub async fn status_handler(State(state): State<AppState>) -> Json<StatusResponse> {
    Json(StatusResponse {
        model: state.model.product_name().to_string(),
        num_buttons: state.button_state.num_buttons(),
        running: state.running.load(Ordering::Relaxed),
    })
}

// ============================================================================
// Button API
// ============================================================================

/// Button state response
#[derive(Serialize)]
pub struct ButtonsResponse {
    pub buttons: Vec<ButtonInfo>,
}

#[derive(Serialize)]
pub struct ButtonInfo {
    pub id: u8,
    pub pressed: bool,
}

pub async fn buttons_handler(State(state): State<AppState>) -> Json<ButtonsResponse> {
    let num_buttons = state.button_state.num_buttons();
    let buttons = (0..num_buttons)
        .map(|i| ButtonInfo {
            id: i,
            pressed: state.button_state.is_pressed(i),
        })
        .collect();

    Json(ButtonsResponse { buttons })
}

/// Press a button
pub async fn press_button_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    if id >= state.button_state.num_buttons() {
        return (StatusCode::NOT_FOUND, "Button not found").into_response();
    }

    state.button_state.press(id);
    (StatusCode::OK, format!("Button {} pressed", id)).into_response()
}

/// Release a button
pub async fn release_button_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    if id >= state.button_state.num_buttons() {
        return (StatusCode::NOT_FOUND, "Button not found").into_response();
    }

    state.button_state.release(id);
    (StatusCode::OK, format!("Button {} released", id)).into_response()
}

/// Click a button (press and release)
pub async fn click_button_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    if id >= state.button_state.num_buttons() {
        return (StatusCode::NOT_FOUND, "Button not found").into_response();
    }

    // Spawn a blocking task for the click since it includes a sleep
    let button_state = state.button_state.clone();
    tokio::task::spawn_blocking(move || {
        button_state.click(id);
    })
    .await
    .ok();

    (StatusCode::OK, format!("Button {} clicked", id)).into_response()
}

// ============================================================================
// Button LED API (Neo only - buttons 8-9 have RGB LEDs)
// ============================================================================

use crate::state::SetLedColorRequest;
use gadgetdeck::{NeoInputState, RgbColor};

/// Get LED color for a button (Neo buttons 8-9 only)
pub async fn get_button_led_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    if id >= state.button_state.num_buttons() {
        return (StatusCode::NOT_FOUND, "Button not found").into_response();
    }

    match &state.neo_state {
        Some(neo) => match neo.get_led_color(id) {
            Some(color) => Json(serde_json::json!({
                "id": id,
                "r": color.r,
                "g": color.g,
                "b": color.b
            }))
            .into_response(),
            None => (
                StatusCode::BAD_REQUEST,
                format!("Button {} does not have an LED", id),
            )
                .into_response(),
        },
        None => (StatusCode::BAD_REQUEST, "LEDs only available on Neo device").into_response(),
    }
}

/// Set LED color for a button (Neo buttons 8-9 only)
pub async fn set_button_led_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
    Json(req): Json<SetLedColorRequest>,
) -> impl IntoResponse {
    if id >= state.button_state.num_buttons() {
        return (StatusCode::NOT_FOUND, "Button not found").into_response();
    }

    match &state.neo_state {
        Some(neo) => {
            let color = RgbColor::new(req.r, req.g, req.b);
            if neo.set_led_color(id, color) {
                (
                    StatusCode::OK,
                    format!(
                        "Button {} LED set to RGB({}, {}, {})",
                        id, req.r, req.g, req.b
                    ),
                )
                    .into_response()
            } else {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Button {} does not have an LED", id),
                )
                    .into_response()
            }
        }
        None => (StatusCode::BAD_REQUEST, "LEDs only available on Neo device").into_response(),
    }
}

/// Get all buttons that have LEDs
pub async fn button_leds_handler(State(state): State<AppState>) -> impl IntoResponse {
    match &state.neo_state {
        Some(neo) => {
            let leds: Vec<_> = NeoInputState::led_buttons()
                .iter()
                .filter_map(|&id| {
                    neo.get_led_color(id).map(|color| {
                        serde_json::json!({
                            "id": id,
                            "r": color.r,
                            "g": color.g,
                            "b": color.b
                        })
                    })
                })
                .collect();
            Json(serde_json::json!({
                "available": true,
                "leds": leds
            }))
            .into_response()
        }
        None => Json(serde_json::json!({
            "available": false,
            "leds": []
        }))
        .into_response(),
    }
}

// ============================================================================
// Image API
// ============================================================================

/// Image stats response
#[derive(Serialize)]
pub struct ImagesResponse {
    pub packets_received: u64,
    pub images_completed: u64,
    pub bytes_received: u64,
    pub available_images: Vec<u8>,
}

pub async fn images_handler(State(state): State<AppState>) -> Json<ImagesResponse> {
    let stats = state.image_store.stats();
    let available_images: Vec<u8> = (0..state.button_state.num_buttons())
        .filter(|&i| state.image_store.get_image(i).is_some())
        .collect();

    Json(ImagesResponse {
        packets_received: stats.packets_received,
        images_completed: stats.images_completed,
        bytes_received: stats.bytes_received,
        available_images,
    })
}

/// Get a specific button image
pub async fn get_image_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    match state.image_store.get_image(id) {
        Some(image) => {
            let bytes = image.as_bytes().to_vec();
            let content_type = match state.model.image_format() {
                "JPEG" => "image/jpeg",
                _ => "image/bmp",
            };
            (StatusCode::OK, [("content-type", content_type)], bytes).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Image not found").into_response(),
    }
}

// ============================================================================
// Plus-specific API: Knobs
// ============================================================================

use crate::state::{KnobInfo, SwipeRequest, TapRequest, TurnKnobRequest};
use gadgetdeck::KnobIndex;

/// Knobs response
#[derive(Serialize)]
pub struct KnobsResponse {
    pub available: bool,
    pub knobs: Vec<KnobInfo>,
}

/// List available knobs (Plus only)
pub async fn knobs_handler(State(state): State<AppState>) -> Json<KnobsResponse> {
    let available = state.plus_state.is_some();
    let knobs = if available {
        vec![
            KnobInfo {
                id: 0,
                name: "A".to_string(),
            },
            KnobInfo {
                id: 1,
                name: "B".to_string(),
            },
            KnobInfo {
                id: 2,
                name: "C".to_string(),
            },
            KnobInfo {
                id: 3,
                name: "D".to_string(),
            },
        ]
    } else {
        Vec::new()
    };
    Json(KnobsResponse { available, knobs })
}

/// Press a knob
pub async fn press_knob_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    if id > 3 {
        return (StatusCode::NOT_FOUND, "Knob not found (0-3)").into_response();
    }

    match &state.plus_state {
        Some(plus) => {
            plus.press_knob(KnobIndex::from(id));
            (StatusCode::OK, format!("Knob {} pressed", id)).into_response()
        }
        None => (StatusCode::BAD_REQUEST, "Not a Plus device").into_response(),
    }
}

/// Release a knob
pub async fn release_knob_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    if id > 3 {
        return (StatusCode::NOT_FOUND, "Knob not found (0-3)").into_response();
    }

    match &state.plus_state {
        Some(plus) => {
            plus.release_knob(KnobIndex::from(id));
            (StatusCode::OK, format!("Knob {} released", id)).into_response()
        }
        None => (StatusCode::BAD_REQUEST, "Not a Plus device").into_response(),
    }
}

/// Click a knob (press and release)
pub async fn click_knob_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
) -> impl IntoResponse {
    if id > 3 {
        return (StatusCode::NOT_FOUND, "Knob not found (0-3)").into_response();
    }

    match &state.plus_state {
        Some(plus) => {
            let plus = plus.clone();
            tokio::task::spawn_blocking(move || {
                plus.press_knob(KnobIndex::from(id));
                std::thread::sleep(std::time::Duration::from_millis(50));
                plus.release_knob(KnobIndex::from(id));
            })
            .await
            .ok();
            (StatusCode::OK, format!("Knob {} clicked", id)).into_response()
        }
        None => (StatusCode::BAD_REQUEST, "Not a Plus device").into_response(),
    }
}

/// Turn a knob
pub async fn turn_knob_handler(
    State(state): State<AppState>,
    Path(id): Path<u8>,
    Json(req): Json<TurnKnobRequest>,
) -> impl IntoResponse {
    if id > 3 {
        return (StatusCode::NOT_FOUND, "Knob not found (0-3)").into_response();
    }

    match &state.plus_state {
        Some(plus) => {
            plus.turn_knob(KnobIndex::from(id), req.steps);
            let direction = if req.steps > 0 {
                "clockwise"
            } else {
                "counter-clockwise"
            };
            (
                StatusCode::OK,
                format!("Knob {} turned {} {} steps", id, direction, req.steps.abs()),
            )
                .into_response()
        }
        None => (StatusCode::BAD_REQUEST, "Not a Plus device").into_response(),
    }
}

// ============================================================================
// Plus-specific API: LCD Touchscreen
// ============================================================================

/// Tap the LCD touchscreen
pub async fn lcd_tap_handler(
    State(state): State<AppState>,
    Json(req): Json<TapRequest>,
) -> impl IntoResponse {
    match &state.plus_state {
        Some(plus) => {
            plus.tap(req.x, req.y);
            (StatusCode::OK, format!("LCD tap at ({}, {})", req.x, req.y)).into_response()
        }
        None => (StatusCode::BAD_REQUEST, "Not a Plus device").into_response(),
    }
}

/// Swipe on the LCD touchscreen
pub async fn lcd_swipe_handler(
    State(state): State<AppState>,
    Json(req): Json<SwipeRequest>,
) -> impl IntoResponse {
    match &state.plus_state {
        Some(plus) => {
            plus.swipe(req.start_x, req.start_y, req.end_x, req.end_y);
            (
                StatusCode::OK,
                format!(
                    "LCD swipe from ({}, {}) to ({}, {})",
                    req.start_x, req.start_y, req.end_x, req.end_y
                ),
            )
                .into_response()
        }
        None => (StatusCode::BAD_REQUEST, "Not a Plus device").into_response(),
    }
}
