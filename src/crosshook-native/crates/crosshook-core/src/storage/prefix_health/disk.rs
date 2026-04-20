use std::path::Path;

use nix::sys::statvfs::statvfs;

use crate::launch::runtime_helpers::resolve_wine_prefix_path;

use super::types::LowDiskWarning;
use super::utils::normalized_path_string;

pub fn check_low_disk_warning(
    prefix_path: &Path,
    threshold_mb: u64,
) -> Result<Option<LowDiskWarning>, String> {
    let resolved_prefix = resolve_wine_prefix_path(prefix_path);
    let statvfs_target = if resolved_prefix.exists() {
        resolved_prefix.clone()
    } else {
        let mut ancestor = resolved_prefix.as_path();
        loop {
            match ancestor.parent() {
                Some(parent) if parent.exists() => break parent.to_path_buf(),
                Some(parent) => ancestor = parent,
                None => return Ok(None),
            }
        }
    };

    let stats = statvfs(&statvfs_target).map_err(|error| {
        format!(
            "failed to query disk usage for {}: {error}",
            resolved_prefix.display()
        )
    })?;
    let available_bytes = stats
        .fragment_size()
        .saturating_mul(stats.blocks_available());
    let threshold_bytes = threshold_mb.saturating_mul(1024 * 1024);

    if available_bytes >= threshold_bytes {
        return Ok(None);
    }

    Ok(Some(LowDiskWarning {
        mount_path: normalized_path_string(&resolved_prefix),
        available_bytes,
        threshold_bytes,
    }))
}
