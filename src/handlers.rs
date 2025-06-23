use crate::{session::PhotoSession, AppState, Assets};
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

pub async fn serve_html(filename: &str) -> impl IntoResponse {
    match Assets::get(filename) {
        Some(content) => (StatusCode::OK, Html(std::str::from_utf8(&content.data).unwrap().to_string())).into_response(),
        None => (StatusCode::NOT_FOUND, Html("Page not found".to_string())).into_response(),
    }
}

// For debugging routes/ embedded files
pub async fn list_embedded_files() -> impl IntoResponse {
    
    let mut files = Vec::new();
    for file_path in Assets::iter() {
        files.push(file_path.to_string());
    }
    
    Json(serde_json::json!({
        "embedded_files": files,
        "total_count": files.len()
    }))
}


/// Starts a new photo booth session, replacing any existing one.
pub async fn start_session(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut session_guard = state.current_session.lock().unwrap();
    *session_guard = Some(PhotoSession::new());
    drop(session_guard);

    let mut count_guard = state.session_count.lock().unwrap();
    *count_guard += 1;
    let current_count = *count_guard;
    drop(count_guard);

    info!("New session started. Current session count: {}", current_count);

    (StatusCode::OK, Json(serde_json::json!({
        "status": "ok",
        "message": "New session created",
        "redirect": "/entry/names",
        "session_count": current_count})))
}

/// A generic payload for simple string selections (weapon, land, companion).
#[derive(Deserialize)]
pub struct SelectionPayload {
    id: String,
}

/// Helper function to handle a generic selection.
fn handle_selection(
    session_guard: &Mutex<Option<PhotoSession>>,
    update_fn: impl FnOnce(&mut PhotoSession, String),
    payload: SelectionPayload,
    log_message: &str,
) -> StatusCode {
    let mut session_guard = session_guard.lock().unwrap();
    if let Some(session) = session_guard.as_mut() {
        update_fn(session, payload.id.clone());
        info!("{}: {}", log_message, payload.id);
        StatusCode::OK
    } else {
        error!("Action attempted with no active session.");
        StatusCode::CONFLICT
    }
}

pub async fn select_weapon(State(state): State<Arc<AppState>>, Json(payload): Json<SelectionPayload>) -> StatusCode {
    handle_selection(&state.current_session, |s, id| s.weapon = Some(id), payload, "Weapon selected")
}
pub async fn select_land(State(state): State<Arc<AppState>>, Json(payload): Json<SelectionPayload>) -> StatusCode {
    handle_selection(&state.current_session, |s, id| s.land = Some(id), payload, "Land selected")
}

pub async fn select_companion(State(state): State<Arc<AppState>>, Json(payload): Json<SelectionPayload>) -> StatusCode {
    handle_selection(&state.current_session, |s, id| s.companion = Some(id), payload, "Companion selected")
}

#[derive(Deserialize)]
pub struct NamesPayload {
    names: Vec<String>,
}

pub async fn submit_names(State(state): State<Arc<AppState>>, Json(payload): Json<NamesPayload>) -> StatusCode {
    let mut session_guard = state.current_session.lock().unwrap();
    if let Some(session) = session_guard.as_mut() {
        session.user_names = Some(payload.names.clone());
        info!("Names submitted: {:?}", payload.names);
        StatusCode::OK
    } else {
        error!("Name submission attempted with no active session.");
        StatusCode::CONFLICT
    }
}

/// This handler takes a photo and returns a URL to it.
pub async fn start_countdown(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("API: Start photo countdown received.");
    
    match state.active_camera.take_photo_to_file().await {
        Ok(photo_path) => {
            info!("Photo captured successfully to: {}", photo_path);
            
            // Verify the file exists
            if std::path::Path::new(&photo_path).exists() {
                info!("Photo file verified to exist at: {}", photo_path);
            } else {
                error!("Photo file does not exist at: {}", photo_path);
            }
            
            Ok(Json(serde_json::json!({
                "status": "success",
                "photo_path": photo_path
            })))
        }
        Err(e) => {
            error!("Failed to take photo: {}", e);
            
            // Return more specific error messages
            let error_message = match e.to_string().as_str() {
                s if s.contains("busy") => "Camera is busy, please try again in a moment",
                s if s.contains("Pipeline handler in use") => "Camera is being used by another process",
                _ => "Failed to take photo, please try again"
            };
            
            Ok(Json(serde_json::json!({
                "status": "error",
                "message": error_message
            })))
        }
    }
}

pub async fn retake_photo(state: State<Arc<AppState>>) -> impl IntoResponse {
    info!("Retaking photo.");
    start_countdown(state).await
}

#[derive(Deserialize)]
pub struct EmailPayload {
    email: String,
}

pub async fn submit_email(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    info!("Email submitted: {:?}", payload);
    
    // Here you would typically:
    // 1. Save the email to your session
    // 2. Send the photo via email
    // 3. Clean up and prepare for next session
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Photo will be sent to your email!"
    })))
}
