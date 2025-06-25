use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn, error};
use thiserror::Error;
use tokio::process::Command;
use std::path::Path;

#[derive(Debug, Error)]
pub enum PrinterError {
    #[error("Unknown printer type requested: {0}")]
    UnknownType(String),
    #[error("Printer not found or not connected")]
    NotFound,
    #[error("Print job failed: {0}")]
    PrintFailed(String),
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Image processing error: {0}")]
    ImageError(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, PrinterError>;

#[derive(Debug, Clone)]
pub struct PrintJob {
    pub file_path: String,
    pub copies: u32,
    pub paper_size: PaperSize,
    pub quality: PrintQuality,
}

#[derive(Debug, Clone)]
pub enum PaperSize {
    Letter,
    A4,
    Photo4x6,
}

#[derive(Debug, Clone)]
pub enum PrintQuality {
    Draft,
    Normal,
    High,
}

impl Default for PrintJob {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            copies: 1,
            paper_size: PaperSize::Letter,
            quality: PrintQuality::Normal,
        }
    }
}

#[async_trait]
pub trait Printer {
    /// Print a photo from file path
    async fn print_photo(&self, job: PrintJob) -> Result<String>;
    
    /// Check if printer is connected and ready
    async fn is_ready(&self) -> bool;
    
    /// Get printer status information
    async fn get_status(&self) -> Result<PrinterStatus>;
    
    /// Get printer type name
    fn type_name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct PrinterStatus {
    pub is_online: bool,
    pub paper_level: Option<String>,
    pub toner_level: Option<String>,
    pub error_message: Option<String>,
}

pub async fn new_printer(printer_type: &str) -> Result<Arc<dyn Printer + Send + Sync>> {
    match printer_type {
        "brother-hl-l2405w" => Ok(Arc::new(BrotherHLL2405W::new().await?)),
        "mock" => Ok(Arc::new(MockPrinter::new())),
        _ => Err(PrinterError::UnknownType(printer_type.to_string())),
    }
}

// Brother HL-L2405W Implementation
pub struct BrotherHLL2405W {
    printer_name: String,
}

impl BrotherHLL2405W {
    pub async fn new() -> Result<Self> {
        info!("Initializing Brother HL-L2405W printer...");
        
        // Check if CUPS is available
        let cups_check = Command::new("lpstat")
            .arg("-p")
            .output()
            .await?;
        
        if !cups_check.status.success() {
            return Err(PrinterError::NotFound);
        }
        
        // Look for Brother printer in CUPS
        let output = String::from_utf8_lossy(&cups_check.stdout);
        let printer_name = if output.contains("Brother") || output.contains("HL-L2405W") {
            // Extract printer name from lpstat output
            output.lines()
                .find(|line| line.contains("Brother") || line.contains("HL-L2405W"))
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("Brother_HL-L2405W")
                .to_string()
        } else {
            warn!("Brother printer not found in CUPS, using default name");
            "Brother_HL-L2405W".to_string()
        };
        
        info!("Brother printer initialized with name: {}", printer_name);
        
        Ok(BrotherHLL2405W { printer_name })
    }
    
    // Convert image to black and white for laser printing
    async fn prepare_image_for_printing(&self, input_path: &str) -> Result<String> {
        info!("Converting image to B&W for laser printing: {}", input_path);
        
        let img = image::open(input_path)?;
        let gray_img = img.to_luma8();
        
        // Create output path
        let input_path_obj = Path::new(input_path);
        let output_path = input_path_obj
            .parent()
            .unwrap_or(Path::new("./"))
            .join(format!("print_{}", input_path_obj.file_name().unwrap().to_string_lossy()));
        
        gray_img.save(&output_path)?;
        
        info!("B&W image saved to: {}", output_path.display());
        Ok(output_path.to_string_lossy().to_string())
    }
}

#[async_trait]
impl Printer for BrotherHLL2405W {
    async fn print_photo(&self, job: PrintJob) -> Result<String> {
        info!("Brother HL-L2405W: Printing photo from {}", job.file_path);
        
        if !Path::new(&job.file_path).exists() {
            return Err(PrinterError::FileNotFound(job.file_path));
        }
        
        // Convert to B&W for better laser printing
        let print_ready_path = self.prepare_image_for_printing(&job.file_path).await?;
        
        // Build lp command
        let mut cmd = Command::new("lp");
        cmd.arg("-d").arg(&self.printer_name);
        cmd.arg("-n").arg(job.copies.to_string());
        
        // Set paper size
        match job.paper_size {
            PaperSize::Letter => cmd.arg("-o").arg("media=Letter"),
            PaperSize::A4 => cmd.arg("-o").arg("media=A4"),
            PaperSize::Photo4x6 => cmd.arg("-o").arg("media=4x6"),
        };
        
        // Set quality
        match job.quality {
            PrintQuality::Draft => cmd.arg("-o").arg("print-quality=3"),
            PrintQuality::Normal => cmd.arg("-o").arg("print-quality=4"),
            PrintQuality::High => cmd.arg("-o").arg("print-quality=5"),
        };
        
        // Add the file
        cmd.arg(&print_ready_path);
        
        info!("Executing print command: {:?}", cmd);
        let output = cmd.output().await?;
        
        if output.status.success() {
            let job_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("Print job submitted successfully: {}", job_id);
            
            // Clean up temporary B&W file
            if print_ready_path != job.file_path {
                let _ = tokio::fs::remove_file(&print_ready_path).await;
            }
            
            Ok(job_id)
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            error!("Print job failed: {}", error);
            Err(PrinterError::PrintFailed(error.to_string()))
        }
    }
    
    async fn is_ready(&self) -> bool {
        let output = Command::new("lpstat")
            .args(&["-p", &self.printer_name])
            .output()
            .await;
        
        match output {
            Ok(output) => {
                let status = String::from_utf8_lossy(&output.stdout);
                status.contains("enabled") && !status.contains("disabled")
            }
            Err(_) => false,
        }
    }
    
    async fn get_status(&self) -> Result<PrinterStatus> {
        let output = Command::new("lpstat")
            .args(&["-p", &self.printer_name, "-l"])
            .output()
            .await?;
        
        let status_text = String::from_utf8_lossy(&output.stdout);
        
        Ok(PrinterStatus {
            is_online: status_text.contains("enabled"),
            paper_level: None, // Brother printers don't typically report paper level via CUPS
            toner_level: None, // Would need SNMP or proprietary tools
            error_message: if status_text.contains("disabled") {
                Some("Printer is disabled or has an error".to_string())
            } else {
                None
            },
        })
    }
    
    fn type_name(&self) -> &'static str {
        "brother-hl-l2405w"
    }
}

// Mock Printer Implementation
pub struct MockPrinter;

impl MockPrinter {
    pub fn new() -> Self {
        info!("MockPrinter initialized");
        MockPrinter
    }
}

#[async_trait]
impl Printer for MockPrinter {
    async fn print_photo(&self, job: PrintJob) -> Result<String> {
        info!("MockPrinter: Simulating print of {} (copies: {})", job.file_path, job.copies);
        
        if !Path::new(&job.file_path).exists() {
            return Err(PrinterError::FileNotFound(job.file_path));
        }
        
        // Simulate print time
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        let mock_job_id = format!("mock-job-{}", 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        
        info!("MockPrinter: Print job completed with ID: {}", mock_job_id);
        Ok(mock_job_id)
    }
    
    async fn is_ready(&self) -> bool {
        true
    }
    
    async fn get_status(&self) -> Result<PrinterStatus> {
        Ok(PrinterStatus {
            is_online: true,
            paper_level: Some("Full".to_string()),
            toner_level: Some("75%".to_string()),
            error_message: None,
        })
    }
    
    fn type_name(&self) -> &'static str {
        "mock"
    }
}