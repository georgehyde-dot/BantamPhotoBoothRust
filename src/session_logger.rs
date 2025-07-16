use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, error};
use thiserror::Error;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Error)]
pub enum LoggerError {
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("CSV error: {0}")]
    Csv(String),
    #[error("Database error: {0}")]
    Database(String),
}

pub type Result<T> = std::result::Result<T, LoggerError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub names: Vec<String>,
    pub weapon_choice: Option<String>,
    pub land_choice: Option<String>,
    pub companion_choice: Option<String>,
    pub photo_path: Option<String>,
    pub email: Option<String>,
    pub print_job_id: Option<String>,
    pub session_duration_seconds: Option<u64>,
}

impl SessionRecord {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            timestamp: Utc::now(),
            names: Vec::new(),
            weapon_choice: None,
            land_choice: None,
            companion_choice: None,
            photo_path: None,
            email: None,
            print_job_id: None,
            session_duration_seconds: None,
        }
    }
    
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{}",
            self.session_id,
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.names.join(";"), // Use semicolon to separate multiple names
            self.weapon_choice.as_deref().unwrap_or(""),
            self.land_choice.as_deref().unwrap_or(""),
            self.companion_choice.as_deref().unwrap_or(""),
            self.photo_path.as_deref().unwrap_or(""),
            self.email.as_deref().unwrap_or(""),
            self.print_job_id.as_deref().unwrap_or(""),
            self.session_duration_seconds.unwrap_or(0)
        )
    }
    
    pub fn csv_header() -> &'static str {
        "session_id,timestamp,names,weapon_choice,land_choice,companion_choice,photo_path,email,print_job_id,duration_seconds"
    }
}

#[async_trait]
pub trait SessionLogger {
    async fn log_session(&self, record: &SessionRecord) -> Result<()>;
    async fn get_session_count(&self) -> Result<u64>;
    fn logger_type(&self) -> &'static str;
}

pub async fn new_logger(logger_type: &str) -> Result<Arc<dyn SessionLogger + Send + Sync>> {
    match logger_type {
        "csv" => Ok(Arc::new(CsvLogger::new("./sessions.csv").await?)),
        "sqlite" => Ok(Arc::new(SqliteLogger::new("./sessions.db").await?)),
        _ => Ok(Arc::new(CsvLogger::new("./sessions.csv").await?)), // Default to CSV
    }
}

// CSV Logger Implementation
pub struct CsvLogger {
    file_path: String,
}

impl CsvLogger {
    pub async fn new(file_path: &str) -> Result<Self> {
        let logger = CsvLogger {
            file_path: file_path.to_string(),
        };
        
        // Create file with header if it doesn't exist
        if !std::path::Path::new(file_path).exists() {
            logger.create_csv_file().await?;
        }
        
        info!("CSV logger initialized: {}", file_path);
        Ok(logger)
    }
    
    async fn create_csv_file(&self) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.file_path)
            .await?;
        
        file.write_all(SessionRecord::csv_header().as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        
        info!("Created CSV file with header: {}", self.file_path);
        Ok(())
    }
}

#[async_trait]
impl SessionLogger for CsvLogger {
    async fn log_session(&self, record: &SessionRecord) -> Result<()> {
        info!("Logging session to CSV: {}", record.session_id);
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?;
        
        let csv_row = record.to_csv_row();
        file.write_all(csv_row.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        
        info!("Session logged successfully: {}", record.session_id);
        Ok(())
    }
    
    async fn get_session_count(&self) -> Result<u64> {
        match tokio::fs::read_to_string(&self.file_path).await {
            Ok(content) => {
                let line_count = content.lines().count();
                // Subtract 1 for header row
                Ok(if line_count > 0 { line_count as u64 - 1 } else { 0 })
            }
            Err(_) => Ok(0),
        }
    }
    
    fn logger_type(&self) -> &'static str {
        "csv"
    }
}

// SQLite Logger Implementation (for future use)
pub struct SqliteLogger {
    db_path: String,
}

impl SqliteLogger {
    pub async fn new(db_path: &str) -> Result<Self> {
        let logger = SqliteLogger {
            db_path: db_path.to_string(),
        };
        
        // For now, just create the struct - we'll implement SQLite later
        info!("SQLite logger initialized (not yet implemented): {}", db_path);
        Ok(logger)
    }
}

#[async_trait]
impl SessionLogger for SqliteLogger {
    async fn log_session(&self, record: &SessionRecord) -> Result<()> {
        // TODO: Implement SQLite logging
        info!("SQLite logging not yet implemented, session: {}", record.session_id);
        Ok(())
    }
    
    async fn get_session_count(&self) -> Result<u64> {
        // TODO: Implement SQLite count
        Ok(0)
    }
    
    fn logger_type(&self) -> &'static str {
        "sqlite"
    }
}