//! In-memory profile mutation rules (D8).
//!
//! Each rule is a `GameProfile` method that validates and mutates a loaded profile without
//! touching disk, so a caller can load once, apply, and persist exactly once. The
//! `ProfileStore` convenience methods (`save_launch_optimizations`, `save_command_arguments`,
//! `materialize_launch_optimization_preset`, `save_manual_launch_optimization_preset`) are thin
//! `load → apply → save` wrappers over these rules.

mod command_arguments;
mod optimizations;

pub(crate) use optimizations::{validate_manual_launch_preset_name, validate_preset_selection};
