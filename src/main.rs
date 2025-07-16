use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};

use rust_embed::RustEmbed;

// Build timestamp to force rust_embed recompilation: BUILD_TIMESTAMP_PLACEHOLDER
#[derive(RustEmbed, Clone)]
#[folder = "frontend/"]
#[include = "*.html"]
#[include = "static/css/*.css"]
#[include = "static/js/*.js"]
#[include = "static/images/*.jpg"]
#[include = "static/images/*.png"]
struct Assets;

use axum_embed::ServeEmbed;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::fs;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// For HTTPS
use axum_server::tls_rustls::RustlsConfig;
use std::path::PathBuf;

mod camera;
mod handlers;
mod printer;
mod session;
mod session_logger;

/// AppState holds the shared state for our application
pub struct AppState {
    current_session: Mutex<Option<session::PhotoSession>>,
    active_camera: Arc<dyn camera::Camera + Send + Sync>,
    active_printer: Arc<dyn printer::Printer + Send + Sync>,
    session_logger: Arc<dyn session_logger::SessionLogger + Send + Sync>,
    session_count: Mutex<u32>,
}

async fn serve_latest_photo() -> Result<Response<Body>, StatusCode> {
    info!("Request for latest photo received");

    // Read the latest photo path
    match std::fs::read_to_string("./photos/latest.txt") {
        Ok(photo_path) => {
            info!("Latest photo path from file: {}", photo_path);

            // Check if file exists
            if !std::path::Path::new(&photo_path).exists() {
                error!("Photo file does not exist at path: {}", photo_path);
                return Err(StatusCode::NOT_FOUND);
            }

            match fs::read(&photo_path).await {
                Ok(contents) => {
                    info!(
                        "Successfully read photo file, size: {} bytes",
                        contents.len()
                    );
                    Ok(Response::builder()
                        .header(header::CONTENT_TYPE, "image/jpeg")
                        .header(header::CACHE_CONTROL, "no-cache")
                        .body(Body::from(contents))
                        .unwrap())
                }
                Err(e) => {
                    error!("Failed to read photo file: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            error!("Failed to read latest.txt file: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

// Debug endpoint to check photo status
async fn debug_photo_status() -> Json<serde_json::Value> {
    let mut response = serde_json::json!({
        "photos_dir_exists": std::path::Path::new("./photos").exists(),
        "latest_txt_exists": std::path::Path::new("./photos/latest.txt").exists(),
    });

    if let Ok(latest_path) = std::fs::read_to_string("./photos/latest.txt") {
        response["latest_photo_path"] = serde_json::Value::String(latest_path.clone());
        response["latest_photo_exists"] =
            serde_json::Value::Bool(std::path::Path::new(&latest_path).exists());

        if let Ok(metadata) = std::fs::metadata(&latest_path) {
            response["latest_photo_size"] =
                serde_json::Value::Number(serde_json::Number::from(metadata.len()));
        }
    }

    // List all files in photos directory
    if let Ok(entries) = std::fs::read_dir("./photos") {
        let files: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        response["photos_directory_files"] =
            serde_json::Value::Array(files.into_iter().map(serde_json::Value::String).collect());
    }

    Json(response)
}

// Debug endpoint to check printer status
async fn debug_printer_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let printer_ready = state.active_printer.is_ready().await;
    let printer_status = state.active_printer.get_status().await;

    let mut response = serde_json::json!({
        "printer_type": state.active_printer.type_name(),
        "is_ready": printer_ready,
    });

    match printer_status {
        Ok(status) => {
            response["status"] = serde_json::json!({
                "is_online": status.is_online,
                "paper_level": status.paper_level,
                "toner_level": status.toner_level,
                "error_message": status.error_message,
            });
        }
        Err(e) => {
            response["status_error"] = serde_json::Value::String(e.to_string());
        }
    }

    Json(response)
}

async fn serve_image(
    axum::extract::Path(filename): axum::extract::Path<String>,
) -> Result<Response<Body>, StatusCode> {
    let image_path = format!("static/images/{}", filename);

    match Assets::get(&image_path) {
        Some(content) => {
            let content_type = if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
                "image/jpeg"
            } else if filename.ends_with(".png") {
                "image/png"
            } else {
                "application/octet-stream"
            };

            // Clone the data to avoid lifetime issues
            let image_data = content.data.to_vec();

            Ok(Response::builder()
                .header(header::CONTENT_TYPE, content_type)
                .header(header::CACHE_CONTROL, "public, max-age=86400") // Cache for 24 hours
                .header("ETag", format!("\"{}\"", filename)) // Add ETag for better caching
                .body(Body::from(image_data))
                .unwrap())
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new("info,tower_http=debug"))
        .init();

    // Check if we're in kiosk mode
    let kiosk_mode = std::env::var("KIOSK_MODE").is_ok()
        || std::path::Path::new("/tmp/kiosk_mode").exists()
        || std::env::args().any(|arg| arg == "--kiosk");

    if kiosk_mode {
        info!("KIOSK MODE DETECTED - Will use HTTP only");
    }

    let cam = match camera::new_camera("libcamera").await {
        Ok(cam) => {
            info!("Using libcamera");
            cam
        }
        Err(_) => {
            warn!("Libcamera failed, using mock camera");
            camera::new_camera("mock")
                .await
                .expect("Failed to initialize mock camera")
        }
    };

    info!("Using camera type: {}", cam.type_name());

    let printer = match printer::new_printer("brother-hl-l2405w").await {
        Ok(printer) => {
            info!("Using Brother HL-L2405W printer");
            printer
        }
        Err(_) => {
            warn!("Brother printer failed to initialize, using mock printer");
            printer::new_printer("mock")
                .await
                .expect("Failed to initialize mock printer")
        }
    };

    info!("Using printer type: {}", printer.type_name());
    let logger = session_logger::new_logger("csv")
        .await
        .expect("Failed to initialize session logger");

    let shared_state = Arc::new(AppState {
        current_session: Mutex::new(None),
        active_camera: cam,
        active_printer: printer,
        session_count: Mutex::new(0),
        session_logger: logger,
    });

    let app = Router::new()
        // API routes first (more specific)
        .route("/api/session/start", post(handlers::start_session))
        .route("/api/session/submit_names", post(handlers::submit_names))
        .route("/api/session/select_weapon", post(handlers::select_weapon))
        .route("/api/session/select_land", post(handlers::select_land))
        .route(
            "/api/session/select_companion",
            post(handlers::select_companion),
        )
        .route("/api/session/submit_email", post(handlers::submit_email))
        .route(
            "/api/camera/start_countdown",
            post(handlers::start_countdown),
        )
        .route("/api/camera/retake", post(handlers::retake_photo))
        .route("/api/photo/latest", get(serve_latest_photo))
        .route("/api/session/status", get(handlers::get_session_status))
        .route("/api/printer/print_photo", post(handlers::print_photo))
        // Debug routes
        .route("/debug/files", get(handlers::list_embedded_files))
        .route("/debug/photo", get(debug_photo_status))
        .route("/debug/printer", get(debug_printer_status))
        // HTML page routes
        .route(
            "/start",
            get(|| handlers::serve_html("templates/start.html")),
        )
        .route(
            "/entry/names",
            get(|| handlers::serve_html("templates/name_entry.html")),
        )
        .route(
            "/select/weapon",
            get(|| handlers::serve_html("templates/weapon_select.html")),
        )
        .route(
            "/select/land",
            get(|| handlers::serve_html("templates/land_select.html")),
        )
        .route(
            "/select/companion",
            get(|| handlers::serve_html("templates/companion_select.html")),
        )
        .route(
            "/camera/countdown",
            get(|| handlers::serve_html("templates/countdown.html")),
        )
        .route(
            "/camera/retake",
            get(|| handlers::serve_html("templates/retake_preview.html")),
        )
        .route(
            "/entry/email",
            get(|| handlers::serve_html("templates/email_entry.html")),
        )
        // Photo serving
        .nest_service("/photos", ServeDir::new("photos"))
        // Root route (must be last)
        .route("/", get(|| handlers::serve_html("templates/start.html")))
        // Static Files, embedded in binary (catch-all, must be very last)
        .fallback_service(ServeEmbed::<Assets>::new())
        // Provide the shared state to all handlers.
        .with_state(shared_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));

    // Force HTTP in kiosk mode, otherwise try HTTPS then fallback to HTTP
    if kiosk_mode {
        info!(
            "Running in kiosk mode - using HTTP server on http://{}",
            addr
        );
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    } else if std::path::Path::new("cert.pem").exists() && std::path::Path::new("key.pem").exists()
    {
        info!("Setting up HTTPS server on https://{}", addr);

        let config =
            RustlsConfig::from_pem_file(PathBuf::from("cert.pem"), PathBuf::from("key.pem"))
                .await
                .expect("Failed to load TLS certificates");

        axum_server::bind_rustls(addr, config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        info!(
            "No TLS certificates found, starting HTTP server on http://{}",
            addr
        );
        info!("For camera access, generate certificates with:");
        info!("openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes -subj \"/C=US/ST=State/L=City/O=Organization/CN=BantamPhotoShop.local\"");

        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}
