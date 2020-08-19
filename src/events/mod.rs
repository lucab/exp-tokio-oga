//! Events (host-to-guest messages).

use serde::Deserialize;

// TODO(lucab): complete list of events and their args.

/// Event message from host.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "__name__")]
pub enum Event {
    Echo(Echo),
    Refresh(Refresh),
}

/// `echo` event.
#[derive(Clone, Debug, Deserialize)]
pub struct Echo {}

/// `refresh` event.
#[derive(Clone, Debug, Deserialize)]
pub struct Refresh {}
