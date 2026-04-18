//! ProtonUp install orchestration with progress, cancellation, and checksum dispatch.
//!
//! The public surface is:
//!   - [`install_version`] — backward-compatible entry point (no progress/cancel).
//!   - [`install_version_with_progress`] — full-featured orchestrator.
//!
//! All host-tool execution is in-process (tar/flate2/xz2); no `Command::new` calls
//! for blocked tools appear here (ADR-0001 compliance).
//!
//! Known limitation: archive extraction is offloaded to `spawn_blocking`, but the
//! synchronous tar loop in [`archive::extract_tar_read_sync`] does not observe
//! cancellation mid-extract. For large archives, cancellation is only seen
//! before or after extraction completes. A full fix requires a streaming async
//! tar extractor, or explicitly checking `cancel.is_cancelled()` between
//! archive entries.

mod archive;
mod download;
mod errors;
mod orchestrator;
mod validation;

#[cfg(test)]
mod tests;

pub use errors::InstallError;
pub use orchestrator::{install_version, install_version_with_progress};

#[cfg(test)]
use archive::{
    extract_tar_read_sync, first_normal_path_component, peek_tar_read_top_level_sync,
    validate_unpack_result,
};
#[cfg(test)]
use download::hex_encode;
#[cfg(test)]
use download::{fetch_sha256_manifest, fetch_sha512_sidecar};
#[cfg(test)]
use errors::err;
#[cfg(test)]
use validation::{validate_archive_filename, validate_install_destination, validate_release_url};
