//! Application layer: an owner handle ([`App`]) over an explicit Tokio
//! runtime, a bounded event bus, sanitized presenter errors, and
//! toolkit-independent [`services`] over `crosshook-core` stores.
//!
//! No UI toolkit dependency lives here; shells (Tauri, native UI) delegate to
//! the services. See the crate `README.md`.

#![deny(missing_docs)]

mod app;
mod error;
mod events;
pub mod jobs;
pub mod log_tail;
pub mod presenter;
mod runtime;
pub mod services;

pub use app::App;
pub use error::{AppError, AppErrorKind};
pub use events::{AppEvent, EventBus, EventReceiver, DEFAULT_EVENT_CAPACITY, MAX_EVENT_CAPACITY};
