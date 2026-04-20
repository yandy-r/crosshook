//! Launch-session registry — coordinates teardown between linked game and
//! trainer launches without touching each other's process trees.
//!
//! See [`LaunchSessionRegistry`] for the public API. The split between game
//! and trainer cleanup lives at the `cancel_linked_children` boundary:
//! tearing down a child launch is always driven through its owned broadcast
//! channel, so watchdogs are responsible for their own process trees. The
//! registry never touches PIDs directly.

mod registry;
mod types;

pub use registry::LaunchSessionRegistry;
pub use types::{LinkError, SessionId, SessionKind, TeardownReason, WatchdogOutcome};
