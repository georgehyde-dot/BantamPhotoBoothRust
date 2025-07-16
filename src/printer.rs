use async_trait::async_trait;
use printers::{
    common::base::job::PrinterJobOptions, common::base::printer::Printer as PrintersCratePrinter,
    get_printers,
};
use std::error::Error;
use std::fmt;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub enum PaperSize {
    Letter,
    A4,
    Photo4x6,
    Photo5x7,
}

#[derive(Debug, Clone)]
pub enum PrintQuality {
    Draft,
    Normal,
    High,
}

#[derive(Debug, Clone)]
pub struct PrintJob {
    pub file_path: String,
    pub copies: u32,
    pub paper_size: PaperSize,
    pub quality: PrintQuality,
}

#[derive(Debug, Clone)]
pub struct PrinterStatus {
    pub is_online: bool,
    pub paper_level: Option<u32>,
    pub toner_level: Option<u32>,
    pub error_message: Option<String>,
}

#[derive(Debug)]
pub enum PrinterError {
    NotFound(String),
    NotReady(String),
    PrintFailed(String),
    IoError(String),
}

impl fmt::Display for PrinterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrinterError::NotFound(msg) => write!(f, "Printer not found: {}", msg),
            PrinterError::NotReady(msg) => write!(f, "Printer not ready: {}", msg),
            PrinterError::PrintFailed(msg) => write!(f, "Print failed: {}", msg),
            PrinterError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl Error for PrinterError {}

#[async_trait]
pub trait Printer {
    async fn print_photo(&self, job: PrintJob) -> Result<String, PrinterError>;
    async fn is_ready(&self) -> bool;
    async fn get_status(&self) -> Result<PrinterStatus, PrinterError>;
    fn type_name(&self) -> &'static str;
}

pub struct BrotherPrinter {
    printer_name: String,
    cups_printer: Option<PrintersCratePrinter>,
}

impl BrotherPrinter {
    pub async fn new(printer_name: &str) -> Result<Self, PrinterError> {
        info!("Initializing Brother printer: {}", printer_name);

        // Get all available printers
        let printers = get_printers();
        info!("Found {} printers", printers.len());

        // Log all available printers for debugging
        for printer in &printers {
            info!("Available printer: {}", printer.name);
        }

        // Find the specific printer (try exact match first, then partial match)
        let cups_printer = printers
            .iter()
            .find(|p| p.name.to_lowercase() == printer_name.to_lowercase())
            .or_else(|| {
                printers
                    .iter()
                    .find(|p| p.name.to_lowercase().contains("brother"))
            })
            .cloned();

        match cups_printer {
            Some(printer) => {
                info!("Found printer: {}", printer.name);
                Ok(BrotherPrinter {
                    printer_name: printer.name.clone(),
                    cups_printer: Some(printer),
                })
            }
            None => {
                error!(
                    "Printer '{}' not found. Available printers: {:?}",
                    printer_name,
                    printers.iter().map(|p| &p.name).collect::<Vec<_>>()
                );
                Err(PrinterError::NotFound(format!(
                    "Printer '{}' not found in CUPS",
                    printer_name
                )))
            }
        }
    }
}

#[async_trait]
impl Printer for BrotherPrinter {
    async fn print_photo(&self, job: PrintJob) -> Result<String, PrinterError> {
        info!(
            "Printing photo: {} with {} copies",
            job.file_path, job.copies
        );

        let printer = self
            .cups_printer
            .as_ref()
            .ok_or_else(|| PrinterError::NotReady("Printer not initialized".to_string()))?;

        // Check if file exists
        if !std::path::Path::new(&job.file_path).exists() {
            return Err(PrinterError::IoError(format!(
                "File not found: {}",
                job.file_path
            )));
        }

        // Convert our job parameters to printer options
        let mut raw_properties = Vec::new();

        // Set number of copies
        raw_properties.push(("copies", job.copies.to_string()));

        // Set paper size
        let paper_size_str = match job.paper_size {
            PaperSize::Letter => "Letter",
            PaperSize::A4 => "A4",
            PaperSize::Photo4x6 => "4x6",
            PaperSize::Photo5x7 => "5x7",
        };
        raw_properties.push(("media", paper_size_str.to_string()));

        // Set print quality
        let quality_str = match job.quality {
            PrintQuality::Draft => "draft",
            PrintQuality::Normal => "normal",
            PrintQuality::High => "high",
        };
        raw_properties.push(("print-quality", quality_str.to_string()));

        // For photos, set some additional properties
        raw_properties.push(("media-type", "photo".to_string()));
        raw_properties.push(("print-color-mode", "color".to_string()));

        // Convert to the format expected by the printers crate
        let raw_props: Vec<(&str, &str)> = raw_properties
            .iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect();

        let job_name = format!("PhotoBooth-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));

        let options = PrinterJobOptions {
            name: Some(&job_name),
            raw_properties: &raw_props,
        };

        info!(
            "Sending print job to printer '{}' with options: {:?}",
            self.printer_name, raw_props
        );

        match printer.print_file(&job.file_path, options) {
            Ok(job_id) => {
                info!("Print job submitted successfully with ID: {}", job_id);
                Ok(job_id.to_string())
            }
            Err(e) => {
                error!("Failed to print file: {}", e);
                Err(PrinterError::PrintFailed(format!("CUPS error: {}", e)))
            }
        }
    }

    async fn is_ready(&self) -> bool {
        // For now, assume printer is ready if we have a CUPS printer object
        // In a real implementation, you might want to check printer status
        self.cups_printer.is_some()
    }

    async fn get_status(&self) -> Result<PrinterStatus, PrinterError> {
        // Basic status - the printers crate doesn't provide detailed status info
        // You could extend this by calling CUPS commands or checking printer state
        Ok(PrinterStatus {
            is_online: self.cups_printer.is_some(),
            paper_level: None, // Not available through printers crate
            toner_level: None, // Not available through printers crate
            error_message: None,
        })
    }

    fn type_name(&self) -> &'static str {
        "Brother CUPS Printer"
    }
}

pub struct MockPrinter;

#[async_trait]
impl Printer for MockPrinter {
    async fn print_photo(&self, job: PrintJob) -> Result<String, PrinterError> {
        info!(
            "Mock printer: Would print {} with {} copies",
            job.file_path, job.copies
        );

        // Simulate some processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Generate a mock job ID
        let job_id = format!("mock-job-{}", uuid::Uuid::new_v4().as_simple());
        info!("Mock print job created: {}", job_id);

        Ok(job_id)
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn get_status(&self) -> Result<PrinterStatus, PrinterError> {
        Ok(PrinterStatus {
            is_online: true,
            paper_level: Some(85),
            toner_level: Some(60),
            error_message: None,
        })
    }

    fn type_name(&self) -> &'static str {
        "Mock Printer"
    }
}

pub async fn new_printer(
    printer_type: &str,
) -> Result<std::sync::Arc<dyn Printer + Send + Sync>, PrinterError> {
    match printer_type {
        "brother-hl-l2405w" => {
            // Try different possible names for the Brother printer
            let possible_names = ["brother", "Brother", "HL-L2405W", "Brother_HL-L2405W"];

            for name in &possible_names {
                match BrotherPrinter::new(name).await {
                    Ok(printer) => {
                        info!(
                            "Successfully initialized Brother printer with name: {}",
                            name
                        );
                        return Ok(std::sync::Arc::new(printer));
                    }
                    Err(e) => {
                        warn!("Failed to initialize printer with name '{}': {}", name, e);
                    }
                }
            }

            Err(PrinterError::NotFound(
                "Brother printer not found with any expected name".to_string(),
            ))
        }
        "mock" => Ok(std::sync::Arc::new(MockPrinter)),
        _ => Err(PrinterError::NotFound(format!(
            "Unknown printer type: {}",
            printer_type
        ))),
    }
}
