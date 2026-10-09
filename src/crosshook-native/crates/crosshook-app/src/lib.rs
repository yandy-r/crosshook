//! Application scaffold: an owner handle ([`App`]) over an explicit Tokio
//! runtime and a bounded event bus. Runtime-only; persists nothing.
//!
//! No Tauri or CLI wiring lives here. See the crate `README.md`.

#![deny(missing_docs)]

mod app;
mod error;
mod events;
pub mod presenter;
mod runtime;

pub use app::App;
pub use error::{AppError, AppErrorKind};
pub use events::{AppEvent, EventBus, EventReceiver, DEFAULT_EVENT_CAPACITY, MAX_EVENT_CAPACITY};
