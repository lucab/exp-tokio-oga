//! Commands (guest-to-host messages).

use crate::errors::OgaError;
use serde::Serialize;

/// Supported protocol/API version.
const API_VERSION: u8 = 3;

/// Encode command as frame.
pub trait AsFrame: std::fmt::Debug + Send {
    fn as_frame(&self) -> Result<Vec<u8>, OgaError>;
}

/// Heartbeat.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "__name__")]
#[serde(rename(serialize = "heartbeat"))]
pub struct Heartbeat {
    #[serde(rename = "apiVersion")]
    api_version: u8,
    #[serde(rename = "free-ram")]
    pub free_ram: u64,
}

impl Default for Heartbeat {
    fn default() -> Self {
        Self {
            api_version: API_VERSION,
            free_ram: 0,
        }
    }
}

impl AsFrame for Heartbeat {
    fn as_frame(&self) -> Result<Vec<u8>, OgaError> {
        let mut msg =
            serde_json::to_vec(self).map_err(|e| format!("failed to encode frame: {}", e))?;
        msg.push(b'\n');
        Ok(msg)
    }
}

/// Guest system is started or restarted.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(tag = "__name__")]
#[serde(rename(serialize = "session-startup"))]
pub struct SessionStartup {}

impl AsFrame for SessionStartup {
    fn as_frame(&self) -> Result<Vec<u8>, OgaError> {
        let mut msg =
            serde_json::to_vec(self).map_err(|e| format!("failed to encode frame: {}", e))?;
        msg.push(b'\n');
        Ok(msg)
    }
}

/// Guest system shuts down.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(tag = "__name__")]
#[serde(rename(serialize = "session-shutdown"))]
pub struct SessionShutdown {}

impl AsFrame for SessionShutdown {
    fn as_frame(&self) -> Result<Vec<u8>, OgaError> {
        let mut msg =
            serde_json::to_vec(self).map_err(|e| format!("failed to encode frame: {}", e))?;
        msg.push(b'\n');
        Ok(msg)
    }
}

/// Guest agent was uninstalled.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(tag = "__name__")]
#[serde(rename(serialize = "uninstalled"))]
pub struct Uninstalled {}

impl AsFrame for Uninstalled {
    fn as_frame(&self) -> Result<Vec<u8>, OgaError> {
        let mut msg =
            serde_json::to_vec(self).map_err(|e| format!("failed to encode frame: {}", e))?;
        msg.push(b'\n');
        Ok(msg)
    }
}

/// Active user.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "__name__")]
#[serde(rename(serialize = "active-user"))]
pub struct ActiveUser {
    pub name: String,
}

impl AsFrame for ActiveUser {
    fn as_frame(&self) -> Result<Vec<u8>, OgaError> {
        let mut msg =
            serde_json::to_vec(self).map_err(|e| format!("failed to encode frame: {}", e))?;
        msg.push(b'\n');
        Ok(msg)
    }
}

impl Default for ActiveUser {
    fn default() -> Self {
        Self {
            name: "None".to_string(),
        }
    }
}
