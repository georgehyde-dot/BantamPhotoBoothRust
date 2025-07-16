use crate::{printer::PrintJob, session::PhotoSession, AppState, Assets};
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

/// This handler takes a photo and stores the path in the session
pub async fn start_countdown(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    info!("API: Start photo countdown received.");
    
    match state.active_camera.take_photo_to_file().await {
        Ok(photo_path) => {
            info!("Photo captured successfully to: {}", photo_path);
            
            // Store the photo path in the current session
            let mut session_guard = state.current_session.lock().unwrap();
            if let Some(session) = session_guard.as_mut() {
                session.photo_path = Some(photo_path.clone());
                info!("Photo path stored in session: {}", photo_path);
            } else {
                error!("No active session to store photo path");
                return (StatusCode::CONFLICT, Json(serde_json::json!({
                    "status": "error",
                    "message": "No active session"
                }))).into_response();
            }
            drop(session_guard);
            
            (StatusCode::OK, Json(serde_json::json!({
                "status": "success",
                "photo_path": photo_path
            }))).into_response()
        }
        Err(e) => {
            error!("Failed to take photo: {}", e);
            
            // Return more specific error messages
            let error_message = match e.to_string().as_str() {
                s if s.contains("busy") => "Camera is busy, please try again in a moment",
                s if s.contains("Pipeline handler in use") => "Camera is being used by another process",
                _ => "Failed to take photo, please try again"
            };
            
            (StatusCode::OK, Json(serde_json::json!({
                "status": "error",
                "message": error_message
            }))).into_response()
        }
    }
}

pub async fn retake_photo(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    info!("Retaking photo.");
    
    // Increment retake counter
    {
        let mut session_guard = state.current_session.lock().unwrap();
        if let Some(session) = session_guard.as_mut() {
            session.use_retake();
            info!("Retake used. Retakes used: {}/{}", session.retakes_used, session.max_retakes);
        }
    } // Drop the lock here
    
    // Call start_countdown and return its response
    start_countdown(State(state)).await
}

pub async fn get_session_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let session_guard = state.current_session.lock().unwrap();
    
    if let Some(session) = session_guard.as_ref() {
        Json(serde_json::json!({
            "has_session": true,
            "session_id": session.session_id,
            "has_photo": session.photo_path.is_some(),
            "can_retake": session.can_retake(),
            "retakes_used": session.retakes_used,
            "max_retakes": session.max_retakes,
            "is_complete": session.is_complete(),
            "email": session.email
        }))
    } else {
        Json(serde_json::json!({
            "has_session": false
        }))
    }
}

#[derive(Deserialize)]
pub struct EmailPayload {
    email: String,
}

pub async fn submit_email(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailPayload>,
) -> impl IntoResponse {
    info!("Email submitted: {}", payload.email);
    
    // Validate email format
    if payload.email.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "status": "error",
            "message": "Email cannot be empty"
        }))).into_response();
    }
    
    let session_record = {
        let mut session_guard = state.current_session.lock().unwrap();
        if let Some(session) = session_guard.as_mut() {
            session.email = Some(payload.email.clone());
            info!("Email stored in session: {}", payload.email);
            
            // Check if session is complete and get the record
            if session.is_complete() {
                info!("Session is complete, creating session record");
                Some(session.to_session_record())
            } else {
                info!("Session is not complete yet");
                None
            }
        } else {
            error!("No active session found");
            None
        }
    }; // Drop the lock here
    
    match session_record {
        Some(record) => {
            info!("Session Record created: {:?}", record);
            
            // Log the completed session
            match state.session_logger.log_session(&record).await {
                Ok(_) => {
                    info!("Session logged successfully: {}", record.session_id);
                }
                Err(e) => {
                    error!("Failed to log session: {}", e);
                    // Don't fail the request if logging fails, but log the error
                }
            }
            
            // Clear the session after logging
            {
                let mut session_guard = state.current_session.lock().unwrap();
                *session_guard = None;
                info!("Session cleared after completion");
            }
            
            (StatusCode::OK, Json(serde_json::json!({
                "status": "success",
                "message": "Photo session completed! Check your email.",
                "redirect": "/start"
            }))).into_response()
        }
        None => {
            // Check what's missing from the session
            let session_guard = state.current_session.lock().unwrap();
            if let Some(session) = session_guard.as_ref() {
                let missing_fields = vec![
                    if session.user_names.is_none() { Some("names") } else { None },
                    if session.photo_path.is_none() { Some("photo") } else { None },
                    if session.email.is_none() { Some("email") } else { None },
                ].into_iter().flatten().collect::<Vec<_>>();
                
                error!("Session is incomplete. Missing: {:?}", missing_fields);
                (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Session is incomplete. Missing: {}", missing_fields.join(", "))
                }))).into_response()
            } else {
                error!("No active session found");
                (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                    "status": "error",
                    "message": "No active session found"
                }))).into_response()
            }
        }
    }
}

pub async fn print_photo(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    info!("Print photo request received");
    
    let session_info = {
        let session_guard = state.current_session.lock().unwrap();
        if let Some(session) = session_guard.as_ref() {
            Some((
                session.photo_path.clone(),
                session.user_names.clone().unwrap_or_default(),
                session.session_id.clone()
            ))
        } else {
            None
        }
    }; // Drop the lock here
    
    match session_info {
        Some((Some(photo_path), names, session_id)) => {

            let print_job = crate::printer::PrintJob {
                file_path: photo_path,
                copies: 1,
                paper_size: crate::printer::PaperSize::Photo4x6,
                quality: crate::printer::PrintQuality::Normal,
            };
            
            match state.active_printer.print_photo(print_job).await {
                Ok(job_id) => {
                    info!("Print job submitted successfully: {}", job_id);
                    
                    // Store print job ID in session
                    {
                        let mut session_guard = state.current_session.lock().unwrap();
                        if let Some(session) = session_guard.as_mut() {
                            session.print_job_id = Some(job_id.clone());
                        }
                    }
                    
                    (StatusCode::OK, Json(serde_json::json!({
                        "status": "success",
                        "message": "Photo sent to printer!",
                        "job_id": job_id
                    }))).into_response()
                }
                Err(e) => {
                    error!("Failed to print photo: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                        "status": "error",
                        "message": format!("Failed to print: {}", e)
                    }))).into_response()
                }
            }
        }
        Some((None, _, _)) => {
            error!("No photo available to print");
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "status": "error",
                "message": "No photo available to print"
            }))).into_response()
        }
        None => {
            error!("No active session");
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "status": "error",
                "message": "No active session"
            }))).into_response()
        }
    }
}