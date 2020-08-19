//! Error handling.

use thiserror::Error;

/// Library errors.
#[derive(Error, Debug)]
#[error("tokio-oga error: {0}")]
pub struct OgaError(pub(crate) String);

impl From<&str> for OgaError {
    fn from(arg: &str) -> Self {
        Self(arg.to_string())
    }
}

impl From<String> for OgaError {
    fn from(arg: String) -> Self {
        Self(arg)
    }
}
