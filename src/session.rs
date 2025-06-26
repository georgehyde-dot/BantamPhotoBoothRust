use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::session_logger::SessionRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoSession {
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub user_names: Option<Vec<String>>,
    pub weapon: Option<String>,
    pub land: Option<String>,
    pub companion: Option<String>,
    pub photo_path: Option<String>,
    pub email: Option<String>,
    pub print_job_id: Option<String>,
    pub retakes_used: u32,
    pub max_retakes: u32,
}

impl PhotoSession {
    pub fn new() -> Self {
        let session_id = format!("session_{}", 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        
        Self {
            session_id,
            created_at: Utc::now(),
            user_names: None,
            weapon: None,
            land: None,
            companion: None,
            photo_path: None,
            email: None,
            print_job_id: None,
            retakes_used: 0,
            max_retakes: 1,
        }
    }
    
    pub fn is_complete(&self) -> bool {
        self.user_names.is_some() && 
        self.photo_path.is_some() && 
        self.email.is_some()
    }
    
    pub fn can_retake(&self) -> bool {
        self.retakes_used < self.max_retakes
    }
    
    pub fn use_retake(&mut self) {
        if self.can_retake() {
            self.retakes_used += 1;
        }
    }
    
    pub fn to_session_record(&self) -> SessionRecord {
        let duration = Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds() as u64;
        
        SessionRecord {
            session_id: self.session_id.clone(),
            timestamp: self.created_at,
            names: self.user_names.clone().unwrap_or_default(),
            weapon_choice: self.weapon.clone(),
            land_choice: self.land.clone(),
            companion_choice: self.companion.clone(),
            photo_path: self.photo_path.clone(),
            email: self.email.clone(),
            print_job_id: self.print_job_id.clone(),
            session_duration_seconds: Some(duration),
        }
    }
}
