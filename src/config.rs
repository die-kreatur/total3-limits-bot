use std::fs;
use std::collections::HashSet;

use serde::Deserialize;
use teloxide::types::ChatId;

use crate::error::{ServiceError, Result};

const CONFIG_PATH: &str = "./configs/config.json";

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    pub redis_url: String,
    pub telegram_token: String,
    pub allowed_users: HashSet<ChatId>
}

impl ServiceConfig {
    pub fn read_from_file() -> Result<ServiceConfig> {
        let file = fs::read(CONFIG_PATH)?;
        serde_json::from_slice::<ServiceConfig>(&file).map_err(ServiceError::from)
    }
}
