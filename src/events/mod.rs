//! Events (host-to-guest messages).

use crate::errors::OgaError;
use serde::Deserialize;

// TODO(lucab): complete events with their args.

/// Event message from host.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "__name__")]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Event {
    ApiVersion(ApiVersion),
    Echo(Echo),
    Hibernate(Hibernate),
    Lifecycle(LifecycleEvent),
    Lock(LockScreen),
    Login(Login),
    Logoff(LogOff),
    Refresh(Refresh),
    SetCpus(SetNumberOfCpus),
    Shutdown(Shutdown),
}

impl Event {
    /// Try to parse an event from a protocol frame.
    pub fn parse_frame(data: &[u8]) -> Result<Self, OgaError> {
        serde_json::from_slice(data).map_err(|e| OgaError::from(e.to_string()))
    }
}

/// `api-version` event.
#[derive(Clone, Debug, Deserialize)]
pub struct ApiVersion {
    #[serde(rename = "apiVersion")]
    pub api_version: u8,
}

/// `echo` event.
#[derive(Clone, Debug, Deserialize)]
pub struct Echo {}

/// `hibernate` event.
#[derive(Clone, Debug, Deserialize)]
pub struct Hibernate {}

/// `lifecycle-event` event.
#[derive(Clone, Debug, Deserialize)]
pub struct LifecycleEvent {}

/// `lock-screen` event.
#[derive(Clone, Debug, Deserialize)]
pub struct LockScreen {}

/// `login` event.
#[derive(Clone, Debug, Deserialize)]
pub struct Login {}

/// `log-off` event.
#[derive(Clone, Debug, Deserialize)]
pub struct LogOff {}

/// `refresh` event.
#[derive(Clone, Debug, Deserialize)]
pub struct Refresh {
    #[serde(rename = "apiVersion")]
    pub api_version: u8,
}

/// `set-number-of-cpus` event.
#[derive(Clone, Debug, Deserialize)]
pub struct SetNumberOfCpus {}

/// `shutdown` event.
#[derive(Clone, Debug, Deserialize)]
pub struct Shutdown {
    pub message: Option<String>,
    pub timeout: Option<u64>,
    pub reboot: Option<String>,
}
