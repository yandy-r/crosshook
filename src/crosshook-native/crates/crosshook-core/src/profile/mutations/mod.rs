//! In-memory profile mutation rules (D8).
//!
//! Each rule is a `GameProfile` method that validates and mutates a loaded profile without
//! touching disk, so a caller can load once, apply, and persist exactly once through
//! `ProfileStore::save` (the app's `ProfileWriter` owns that sequence).

mod command_arguments;
mod optimizations;
