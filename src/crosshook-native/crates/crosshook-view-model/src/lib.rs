//! Pure, toolkit-agnostic view state and reducers for CrossHook screens.
//!
//! A screen's view-model owns its state and turns each [`ViewModel::Intent`]
//! (a user action or a service result) into a new state plus a list of
//! [`ViewModel::Effect`]s: plain data describing service calls. The UI bridge
//! (iced, gpui, or a test) executes the effects and feeds each result back as
//! another intent. Nothing here does I/O, reads the environment or a clock,
//! spawns work, or logs; see the crate `README.md`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

pub mod error;
pub mod form;
pub mod library;
pub mod settings;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use error::{ServiceError, ServiceErrorKind};
pub use form::{Form, FormError, FormModel, SubmitId, SubmitState};

/// A screen's state machine: applies one intent and returns the effects the
/// UI bridge must execute, in order.
///
/// Service results come back as intents. Effects are data (`Debug + Clone +
/// PartialEq`), never closures or futures, so tests can compare them.
pub trait ViewModel {
    /// User actions and service results this view-model accepts.
    type Intent;
    /// Service calls (as data) the bridge executes on the view-model's behalf.
    type Effect;

    /// Applies `intent` and returns the effects to run, in order. Returning an
    /// empty `Vec` does not allocate.
    fn reduce(&mut self, intent: Self::Intent) -> Vec<Self::Effect>;
}
