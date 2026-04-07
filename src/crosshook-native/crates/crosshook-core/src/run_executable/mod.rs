//! Ad-hoc Windows executable runner contracts and shared data models.
//!
//! Mirrors the [`crate::update`] module shape: a thin façade re-exporting
//! request/result/error types and the service entry points used by the Tauri
//! command layer. The runner is profile-less by design — it accepts an
//! arbitrary `.exe` or `.msi`, optionally auto-resolves a throwaway prefix
//! under `_run-adhoc/<slug>`, and never persists state.

mod models;
mod service;

pub use models::{
    RunExecutableError, RunExecutableRequest, RunExecutableResult, RunExecutableValidationError,
};
pub use service::{
    build_run_executable_command, is_throwaway_prefix_path, resolve_default_adhoc_prefix_path,
    run_executable, validate_run_executable_request,
};
