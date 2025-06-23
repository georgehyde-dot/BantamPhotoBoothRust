use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PhotoSession {
    pub start_time: Option<SystemTime>,
    pub weapon: Option<String>,
    pub land: Option<String>,
    pub companion: Option<String>,
    pub user_names: Option<Vec<String>>,
    pub email: Option<String>,
    pub last_photo_path: Option<String>,
}

impl PhotoSession {
    pub fn new() -> Self {
        PhotoSession {
            start_time: Some(SystemTime::now()),
            ..Default::default()
        }
    }
}
