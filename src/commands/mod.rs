//! Commands (guest-to-host messages).

use crate::errors::OgaError;
use serde::Serialize;

const API_VERSION: u8 = 3;
const MEM_USAGE: u64 = 0;

/// Encode command as frame.
pub trait AsFrame: std::fmt::Debug + Send {
    fn as_frame(&self) -> Result<Vec<u8>, OgaError>;
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "__name__")]
#[serde(rename(serialize = "heartbeat"))]
pub struct Heartbeat {
    api_version: u8,
    pub mem_usage: u64,
}

impl Default for Heartbeat {
    fn default() -> Self {
        Self {
            api_version: API_VERSION,
            mem_usage: MEM_USAGE,
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
#[derive(Clone, Debug, Serialize)]
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
#[derive(Clone, Debug, Serialize)]
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
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "__name__")]
#[serde(rename(serialize = "session-shutdown"))]
pub struct Uninstalled {}

impl AsFrame for Uninstalled {
    fn as_frame(&self) -> Result<Vec<u8>, OgaError> {
        let mut msg =
            serde_json::to_vec(self).map_err(|e| format!("failed to encode frame: {}", e))?;
        msg.push(b'\n');
        Ok(msg)
    }
}
