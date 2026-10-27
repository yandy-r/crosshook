use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::constants::{DRIVE_C_RELATIVE, STAGED_TRAINERS_RELATIVE};
use super::discovery::collect_referenced_profiles;
use super::types::{
    PrefixCleanupResult, PrefixCleanupSkipped, PrefixCleanupTarget, PrefixCleanupTargetKind,
    PrefixReference,
};
use super::utils::{
    dir_size_bytes, file_or_dir_size_bytes, has_crosshook_managed_marker, normalized_path_string,
};

#[cfg(test)]
mod tests;

#[derive(Debug)]
struct CleanupRejection {
    reason: String,
}

impl CleanupRejection {
    fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
        }
    }
}

fn skip(result: &mut PrefixCleanupResult, target: &PrefixCleanupTarget, reason: &str) {
    result.skipped.push(PrefixCleanupSkipped {
        target: target.clone(),
        reason: reason.to_string(),
    });
}

/// Rejects unless every component in `components` below `root`, walked in
/// order, is a plain directory (not a symlink) and `target` itself has
/// already been verified by the caller. Fails closed on metadata errors.
/// Returns the verified leaf directory (`root` joined with `components`).
fn ensure_no_follow_directories(
    root: &Path,
    components: &[&str],
    target: &Path,
) -> Result<PathBuf, CleanupRejection> {
    let mut current = root.to_path_buf();
    for component in components {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|_| {
            CleanupRejection::new(
                "intermediate path component could not be verified; refusing cleanup",
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(CleanupRejection::new(
                "intermediate path component is a symlink; refusing cleanup",
            ));
        }
        if !metadata.is_dir() {
            return Err(CleanupRejection::new(
                "intermediate path component is not a directory; refusing cleanup",
            ));
        }
    }
    // The target must be the directory reached through the verified
    // components or lie below it; lexical containment is re-checked
    // canonically by the callers.
    if !target.starts_with(&current) && target != current {
        // For orphan prefixes the target *is* the root, so the verified
        // chain lives below it; that case is fine.
        if !current.starts_with(target) {
            return Err(CleanupRejection::new(
                "target path is outside the resolved prefix; refusing cleanup",
            ));
        }
    }
    Ok(current)
}

/// Rejects the staged target unless it is an immediate child of the staged
/// root and both are strict descendants of the resolved prefix (never equal).
fn ensure_staged_target_layout(
    resolved_prefix: &Path,
    staged_root: &Path,
    target: &Path,
) -> Result<(), CleanupRejection> {
    if target == staged_root || target == resolved_prefix {
        return Err(CleanupRejection::new(
            "target is a cleanup root; refusing cleanup",
        ));
    }
    if staged_root == resolved_prefix || !staged_root.starts_with(resolved_prefix) {
        return Err(CleanupRejection::new(
            "target is a cleanup root; refusing cleanup",
        ));
    }
    if target.parent() != Some(staged_root) {
        return Err(CleanupRejection::new(
            "target is not an immediate child of the staged trainers root",
        ));
    }
    Ok(())
}

fn canonicalize_or_reject(path: &Path, reason: &str) -> Result<PathBuf, CleanupRejection> {
    fs::canonicalize(path).map_err(|_| CleanupRejection::new(reason))
}

/// Trusted roots from the reference inventory: the parent of each
/// referenced prefix, or the compatdata root (grandparent) when the
/// referenced prefix ends in `pfx`. Never derived from the request.
fn canonical_trusted_roots(
    referenced: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<PathBuf>, CleanupRejection> {
    let mut roots = Vec::new();
    let mut any_derived = false;
    for key in referenced.keys() {
        let referenced_path = Path::new(key);
        let derived = if referenced_path.file_name().and_then(|v| v.to_str()) == Some("pfx") {
            referenced_path.parent().and_then(Path::parent)
        } else {
            referenced_path.parent()
        };
        let Some(root) = derived else { continue };
        any_derived = true;
        if let Ok(canonical) = fs::canonicalize(root) {
            if !roots.contains(&canonical) {
                roots.push(canonical);
            }
        }
    }
    if any_derived && roots.is_empty() {
        return Err(CleanupRejection::new(
            "failed to canonicalize trusted prefix root",
        ));
    }
    Ok(roots)
}

fn is_strict_descendant(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

/// Authorizes a canonical prefix against the reference inventory: either it
/// is itself an explicitly referenced prefix, or it lies strictly inside a
/// trusted root derived from the inventory. The cleanup request is never
/// used to establish trust; an empty inventory authorizes nothing.
fn authorize_prefix_against_inventory(
    referenced: &BTreeMap<String, Vec<String>>,
    canonical_prefix: &Path,
) -> Result<(), CleanupRejection> {
    let explicitly_referenced = referenced.keys().any(|key| {
        fs::canonicalize(key)
            .map(|canonical| canonical == canonical_prefix)
            .unwrap_or(false)
    });
    if explicitly_referenced {
        return Ok(());
    }
    let trusted_roots = canonical_trusted_roots(referenced)?;
    if trusted_roots
        .iter()
        .any(|root| is_strict_descendant(canonical_prefix, root))
    {
        return Ok(());
    }
    Err(CleanupRejection::new(
        "resolved prefix is not strictly inside a trusted prefix root",
    ))
}

pub fn cleanup_prefix_storage(
    references: &[PrefixReference],
    targets: &[PrefixCleanupTarget],
) -> PrefixCleanupResult {
    let mut result = PrefixCleanupResult::default();
    let referenced = collect_referenced_profiles(references);

    for target in targets {
        match target.kind {
            PrefixCleanupTargetKind::OrphanPrefix => {
                cleanup_orphan_prefix(&referenced, target, &mut result);
            }
            PrefixCleanupTargetKind::StaleStagedTrainer => {
                cleanup_stale_staged_trainer(&referenced, target, &mut result);
            }
        }
    }

    result
}

fn cleanup_orphan_prefix(
    referenced: &BTreeMap<String, Vec<String>>,
    target: &PrefixCleanupTarget,
    result: &mut PrefixCleanupResult,
) {
    let resolved_prefix_key = normalized_path_string(Path::new(&target.resolved_prefix_path));
    if referenced.contains_key(&resolved_prefix_key) {
        skip(
            result,
            target,
            "prefix is still referenced by at least one profile",
        );
        return;
    }

    let target_path = PathBuf::from(&target.target_path);
    // No-follow check on the target itself, before any canonicalization.
    match fs::symlink_metadata(&target_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            skip(result, target, "target is a symlink; refusing cleanup");
            return;
        }
        Ok(metadata) if !metadata.is_dir() => {
            skip(
                result,
                target,
                "target is not a directory; refusing cleanup",
            );
            return;
        }
        Ok(_) => {}
        Err(_) => {
            skip(result, target, "target no longer exists");
            return;
        }
    }
    if !target_path.join(DRIVE_C_RELATIVE).is_dir() {
        skip(
            result,
            target,
            "target does not look like a wine prefix (missing drive_c)",
        );
        return;
    }
    if !has_crosshook_managed_marker(&target_path) {
        skip(result, target, "prefix is not marked as crosshook-managed");
        return;
    }

    let resolved_prefix_path = PathBuf::from(&target.resolved_prefix_path);
    if let Err(rejection) = ensure_no_follow_directories(
        &resolved_prefix_path,
        &["drive_c", "CrossHook"],
        &target_path,
    ) {
        skip(result, target, &rejection.reason);
        return;
    }

    let canonical_target =
        match canonicalize_or_reject(&target_path, "failed to canonicalize target prefix path") {
            Ok(value) => value,
            Err(rejection) => {
                skip(result, target, &rejection.reason);
                return;
            }
        };
    let canonical_resolved = match canonicalize_or_reject(
        &resolved_prefix_path,
        "failed to canonicalize resolved prefix path",
    ) {
        Ok(value) => value,
        Err(rejection) => {
            skip(result, target, &rejection.reason);
            return;
        }
    };
    if canonical_target != canonical_resolved {
        skip(
            result,
            target,
            "target path does not match resolved prefix path",
        );
        return;
    }

    let trusted_roots = match canonical_trusted_roots(referenced) {
        Ok(value) => value,
        Err(rejection) => {
            skip(result, target, &rejection.reason);
            return;
        }
    };
    let inside_trusted_root = trusted_roots.iter().any(|root| {
        is_strict_descendant(&canonical_target, root)
            && is_strict_descendant(&canonical_resolved, root)
    });
    if !inside_trusted_root {
        skip(
            result,
            target,
            "target is not strictly inside a trusted prefix root",
        );
        return;
    }

    let bytes_before = dir_size_bytes(&canonical_target);
    match fs::remove_dir_all(&canonical_target) {
        Ok(()) => {
            result.deleted.push(target.clone());
            result.reclaimed_bytes = result.reclaimed_bytes.saturating_add(bytes_before);
        }
        Err(error) => {
            tracing::warn!(error = %error, "failed to delete orphan prefix");
            skip(result, target, "failed to delete orphan prefix");
        }
    }
}

fn cleanup_stale_staged_trainer(
    referenced: &BTreeMap<String, Vec<String>>,
    target: &PrefixCleanupTarget,
    result: &mut PrefixCleanupResult,
) {
    let resolved_prefix_path = PathBuf::from(&target.resolved_prefix_path);
    let staged_root = resolved_prefix_path.join(STAGED_TRAINERS_RELATIVE);
    let target_path = PathBuf::from(&target.target_path);

    // No-follow check on the target itself, before any canonicalization.
    // Staged entries may be files or directories.
    match fs::symlink_metadata(&target_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            skip(result, target, "target is a symlink; refusing cleanup");
            return;
        }
        Ok(metadata) if !metadata.is_dir() && !metadata.is_file() => {
            skip(
                result,
                target,
                "target is not a file or directory; refusing cleanup",
            );
            return;
        }
        Ok(_) => {}
        Err(_) => {
            skip(result, target, "staged trainer entry no longer exists");
            return;
        }
    }
    // Reject a symlinked staged root with the stable legacy reason. A
    // missing/unverifiable root falls through to the no-follow walk below,
    // which fails closed with a fixed reason.
    if fs::symlink_metadata(&staged_root)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        skip(
            result,
            target,
            "staged trainers root is a symlink; refusing cleanup",
        );
        return;
    }

    let verified_staged_root = match ensure_no_follow_directories(
        &resolved_prefix_path,
        &["drive_c", "CrossHook", "StagedTrainers"],
        &target_path,
    ) {
        Ok(value) => value,
        Err(rejection) => {
            skip(result, target, &rejection.reason);
            return;
        }
    };
    if let Err(rejection) =
        ensure_staged_target_layout(&resolved_prefix_path, &verified_staged_root, &target_path)
    {
        skip(result, target, &rejection.reason);
        return;
    }

    let canonical_staged_root =
        match canonicalize_or_reject(&staged_root, "failed to canonicalize staged trainers root") {
            Ok(value) => value,
            Err(rejection) => {
                skip(result, target, &rejection.reason);
                return;
            }
        };
    let canonical_prefix = match canonicalize_or_reject(
        &resolved_prefix_path,
        "failed to canonicalize resolved prefix path",
    ) {
        Ok(value) => value,
        Err(rejection) => {
            skip(result, target, &rejection.reason);
            return;
        }
    };
    if !is_strict_descendant(&canonical_staged_root, &canonical_prefix) {
        skip(
            result,
            target,
            "staged trainers root escapes resolved prefix path",
        );
        return;
    }
    if let Err(rejection) = authorize_prefix_against_inventory(referenced, &canonical_prefix) {
        skip(result, target, &rejection.reason);
        return;
    }
    let canonical_target =
        match canonicalize_or_reject(&target_path, "failed to canonicalize staged trainer entry") {
            Ok(value) => value,
            Err(rejection) => {
                skip(result, target, &rejection.reason);
                return;
            }
        };

    if !is_strict_descendant(&canonical_target, &canonical_staged_root) {
        skip(
            result,
            target,
            "target path is outside staged trainers root",
        );
        return;
    }

    let bytes_before = file_or_dir_size_bytes(&canonical_target);
    let deletion = if canonical_target.is_dir() {
        fs::remove_dir_all(&canonical_target)
    } else {
        fs::remove_file(&canonical_target)
    };

    match deletion {
        Ok(()) => {
            result.deleted.push(target.clone());
            result.reclaimed_bytes = result.reclaimed_bytes.saturating_add(bytes_before);
        }
        Err(error) => {
            tracing::warn!(error = %error, "failed to delete staged trainer entry");
            skip(result, target, "failed to delete staged trainer entry");
        }
    }
}
