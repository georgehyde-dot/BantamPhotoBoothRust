use async_trait::async_trait;
use image::{ImageBuffer, Rgb};
use std::sync::Arc;
use tracing::{info, warn, error};
use thiserror::Error;
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, Error)]
pub enum CameraError {
    #[error("Unknown camera type requested: {0}")]
    UnknownType(String),
    #[error("Failed to access camera device with rscam: {0}")]
    DeviceError(#[from] rscam::Error),
    #[error("Failed to decode image from camera frame: {0}")]
    ImageDecodeError(#[from] image::ImageError),
    #[error("Camera IO Error: {0}")]
    Io(#[from] std::io::Error), 
    #[error("Camera not found at device path: {0}")]
    NotFound(String),
    #[error("Command execution failed: {0}")]
    CommandFailed(String),
    #[error("Camera is busy, please try again")]
    Busy,
}

pub type Result<T> = std::result::Result<T, CameraError>;

#[async_trait]
pub trait Camera {
    async fn take_photo(&self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>>;
    async fn take_photo_to_file(&self) -> Result<String>;
    fn type_name(&self) -> &'static str;
}

pub async fn new_camera(cam_type: &str) -> Result<Arc<dyn Camera + Send + Sync>> {
    match cam_type {
        "libcamera" => Ok(Arc::new(LibcameraSystemCamera::new().await?)),
        "mock" => Ok(Arc::new(MockCamera)),
        _ => Err(CameraError::UnknownType(cam_type.to_string())),
    }
}

pub struct LibcameraSystemCamera {
    output_dir: String,
    camera_lock: Arc<Mutex<()>>,
}

impl LibcameraSystemCamera {
    pub async fn new() -> Result<Self> {
        // Always ensure camera is free on startup
        Self::free_camera().await?;
        
        // Test that libcamera-still is available
        let output = Command::new("libcamera-still")
            .arg("--help")
            .output()
            .await?;
        
        if !output.status.success() {
            return Err(CameraError::NotFound("libcamera-still command not found".to_string()));
        }
        
        // Use a permanent directory for photos
        let output_dir = "./photos".to_string();
        std::fs::create_dir_all(&output_dir)?;
        info!("LibcameraSystemCamera initialized, output dir: {}", output_dir);
        
        Ok(LibcameraSystemCamera { 
            output_dir,
            camera_lock: Arc::new(Mutex::new(())),
        })
    }

    // Comprehensive camera freeing function
    async fn free_camera() -> Result<()> {
        info!("Freeing camera from all processes...");
        
        // Stop PipeWire services
        let pipewire_services = ["pipewire-pulse", "wireplumber", "pipewire"];
        for service in &pipewire_services {
            let _ = Command::new("systemctl")
                .args(&["--user", "stop", service])
                .output()
                .await;
            info!("Stopped service: {}", service);
        }
        
        // Kill specific camera processes
        let camera_processes = ["libcamera-still", "libcamera-vid", "rpicam-still", "rpicam-vid"];
        for process in &camera_processes {
            let _ = Command::new("pkill")
                .args(&["-f", process])
                .output()
                .await;
        }
        
        // Kill any process using video devices
        let _ = Command::new("bash")
            .args(&["-c", "lsof /dev/video* 2>/dev/null | awk 'NR>1 {print $2}' | xargs -r kill -9"])
            .output()
            .await;
        
        // Wait for cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
        
        info!("Camera should now be free");
        Ok(())
    }

    async fn capture_with_libcamera(&self, filename: &str) -> Result<()> {
        // Acquire the camera lock to prevent concurrent access
        let _lock = self.camera_lock.lock().await;
        
        info!("LibcameraSystemCamera: Taking photo with libcamera-still to {}", filename);
        
        // Always try to free the camera before capture
        Self::free_camera().await?;
        
        let output = Command::new("libcamera-still")
            .args(&[
                "--output", filename,
                "--width", "1280",
                "--height", "720",
                "--quality", "95",
                "--timeout", "3000", // Increased timeout
                "--nopreview",
                "--immediate",
            ])
            .output()
            .await?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            error!("libcamera-still failed. STDERR: {}, STDOUT: {}", stderr, stdout);
            
            // Check for specific busy error
            if stderr.contains("Pipeline handler in use") || stderr.contains("Device or resource busy") {
                return Err(CameraError::Busy);
            }
            
            return Err(CameraError::CommandFailed(format!("libcamera-still failed: {}", stderr)));
        }
        
        info!("Photo captured successfully to: {}", filename);
        Ok(())
    }
}

#[async_trait]
impl Camera for LibcameraSystemCamera {
    async fn take_photo(&self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("{}/photo_{}.jpg", self.output_dir, timestamp);
        
        // Try up to 5 times if camera is busy
        for attempt in 1..=5 {
            match self.capture_with_libcamera(&filename).await {
                Ok(_) => break,
                Err(CameraError::Busy) if attempt < 5 => {
                    warn!("Camera busy, attempt {} of 5. Freeing camera and retrying...", attempt);
                    Self::free_camera().await?;
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        let image = image::open(&filename)?
            .to_rgb8();
        
        Ok(image)
    }

    async fn take_photo_to_file(&self) -> Result<String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("{}/photo_{}.jpg", self.output_dir, timestamp);
        
        // Try up to 5 times if camera is busy
        for attempt in 1..=5 {
            match self.capture_with_libcamera(&filename).await {
                Ok(_) => break,
                Err(CameraError::Busy) if attempt < 5 => {
                    warn!("Camera busy, attempt {} of 5. Freeing camera and retrying...", attempt);
                    Self::free_camera().await?;
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        // Write the latest photo path to a file for easy access
        std::fs::write("./photos/latest.txt", &filename)?;
        
        Ok(filename)
    }

    fn type_name(&self) -> &'static str {
        "libcamera-system"
    }
}

pub struct MockCamera;

#[async_trait]
impl Camera for MockCamera {
    async fn take_photo(&self) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        info!("MockCamera: Simulating photo capture.");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let img = ImageBuffer::from_pixel(800, 600, image::Rgb([50u8, 50u8, 50u8]));
        Ok(img)
    }

    async fn take_photo_to_file(&self) -> Result<String> {
        info!("MockCamera: Simulating photo capture to file.");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        
        // Create photos directory
        std::fs::create_dir_all("./photos")?;
        
        // Create a mock image
        let img = ImageBuffer::from_pixel(800, 600, image::Rgb([50u8, 50u8, 50u8]));
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("./photos/photo_{}.jpg", timestamp);
        
        img.save(&filename)?;
        
        // Write the latest photo path
        std::fs::write("./photos/latest.txt", &filename)?;
        
        Ok(filename)
    }

    fn type_name(&self) -> &'static str {
        "mock"
    }
}