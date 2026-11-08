//! A named, already serialized event ready to be emitted by a shell.

use serde::Serialize;

/// One wire event: the event name and its JSON payload.
///
/// The payload is the exact string a shell hands to its emitter (Tauri's
/// `emit_str`); serializing here instead of at emit time yields the same bytes
/// `emit` would produce, so the wire stays byte-identical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireEvent {
    /// Event name listeners subscribe to.
    pub name: &'static str,
    /// JSON payload.
    pub payload: String,
}

impl WireEvent {
    /// Serializes `payload` for event `name`. A payload that cannot be
    /// serialized is logged and dropped (`None`), never emitted half-formed.
    pub fn new<T: Serialize + ?Sized>(name: &'static str, payload: &T) -> Option<Self> {
        match serde_json::to_string(payload) {
            Ok(payload) => Some(Self { name, payload }),
            Err(error) => {
                tracing::warn!(%error, event = name, "failed to serialize wire event");
                None
            }
        }
    }

    /// The payload parsed back into a JSON value (tests).
    #[cfg(test)]
    pub(crate) fn value(&self) -> serde_json::Value {
        serde_json::from_str(&self.payload).expect("wire payload is JSON")
    }
}
