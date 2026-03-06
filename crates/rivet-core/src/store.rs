use anyhow::{Context, Result};
use matrix_sdk::authentication::matrix::MatrixSession;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize)]
pub struct SessionData {
    pub homeserver_url: String,
    pub session: MatrixSession,
}

pub fn save_session(path: &str, data: &SessionData) -> Result<()> {
    let json = serde_json::to_string(data)?;
    fs::write(path, json).context("Failed to write session file")?;
    Ok(())
}

pub fn load_session(path: &str) -> Result<SessionData> {
    let json = fs::read_to_string(path).context("Failed to read session file")?;
    let data: SessionData = serde_json::from_str(&json)?;
    Ok(data)
}
