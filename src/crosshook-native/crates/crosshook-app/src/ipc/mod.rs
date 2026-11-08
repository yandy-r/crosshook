//! The Classic (Tauri) wire contract, kept free of any UI toolkit.
//!
//! Command results, event payloads and the pure mapping from service events
//! to named wire events live here so they are testable without a webview.
//! `src-tauri` only forwards a [`WireEvent`] to its emitter and returns these
//! DTOs from its commands. Names, keys and JSON shapes are frozen; the module
//! leaves with `src-tauri` when the native UI replaces Classic.

pub mod fallback;
pub mod launch;
pub mod process;
pub mod protonup;
pub mod sessions;
pub mod settings;
pub mod startup;
mod wire;

pub use fallback::{cancel_accepted, unavailable_or};
pub use wire::WireEvent;
