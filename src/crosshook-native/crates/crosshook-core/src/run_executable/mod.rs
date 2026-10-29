//! Ad-hoc Windows executable runner contracts and shared data models.
//!
//! Mirrors the [`crate::update`] module shape: a thin façade re-exporting
//! request/result/error types and the service entry points used by the app
//! layer. The runner is profile-less by design — it accepts an arbitrary
//! `.exe` or `.msi`, optionally auto-resolves a unique throwaway prefix under
//! `_run-adhoc/<slug>-<suffix>`, and never persists state. Core only prepares
//! the command; the caller spawns and supervises it.

mod models;
mod service;

pub use models::{
    RunExecutableError, RunExecutableRequest, RunExecutableResult, RunExecutableValidationError,
};
pub use service::{
    adhoc_prefix_root, build_run_executable_command, is_throwaway_prefix_path,
    is_throwaway_prefix_path_in, prepare_run_executable, resolve_adhoc_prefix_path,
    resolve_default_adhoc_prefix_path, run_executable, validate_run_executable_request,
    PreparedRunExecutable,
};
