//! Single choke point for XDG base-directory lookup.
//!
//! Production code calls [`base_dirs`] instead of `directories::BaseDirs::new()`
//! so every lookup is visible to the bootstrap ordering trace. The XDG override
//! in `crosshook_app::bootstrap::pre_runtime` must finish before the first call.

use directories::BaseDirs;

use super::trace;

/// Resolve the process XDG base directories.
///
/// Records a lookup, then delegates to `directories::BaseDirs::new()`.
/// Returns `None` when no home directory can be found.
#[must_use]
pub fn base_dirs() -> Option<BaseDirs> {
    trace::base_dirs_lookup();
    BaseDirs::new()
}
