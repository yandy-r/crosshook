//! Collection launch-defaults composition kept in the shell until M1-35.
//! Every profile write goes through `crosshook_app::services::profiles::ProfileWriter`.

use crosshook_core::metadata::{MetadataStore, MetadataStoreError};
use crosshook_core::profile::{CollectionDefaultsSection, GameProfile};

/// Merges the per-collection launch defaults layer into a profile that was just
/// returned from [`ProfileStore::load`].
///
/// Behaviour:
/// - `collection_id` is `None` or trims to empty → returns `profile` unchanged.
/// - `get_collection_defaults` returns `Ok(None)` (no defaults set on the
///   collection) → merges an empty layer, i.e. returns the profile unchanged.
/// - `get_collection_defaults` returns `Ok(Some(defaults))` → returns the
///   merged profile via [`GameProfile::effective_profile_with`].
/// - `get_collection_defaults` returns `Err(MetadataStoreError::Corrupt(_))`
///   → bubbles the error up so the frontend surfaces a loud failure. Corrupt
///   defaults are a data-integrity issue the user needs to fix, not a transient
///   glitch to silently swallow.
/// - `get_collection_defaults` returns any other `Err` (missing row via
///   `Validation`, `Database`, `Io`, …) → logs a `tracing::warn!` and falls
///   back to `Ok(profile)`. This preserves fail-open launch semantics so a
///   stale collection id or a transient SQLite glitch never hard-blocks a
///   game launch.
///
/// Note on layer precedence: `profile` here has already been flattened by
/// `ProfileStore::load`, which bakes `local_override` into layer 1 and clears
/// the section. That's why we can call `effective_profile_with` on it directly
/// — layer 3 is a no-op at this call site but layer 1 still carries every
/// machine-specific override, so the "local_override always wins" guarantee
/// holds at runtime. See the doc comment on `effective_profile_with` for the
/// full explanation.
///
/// [`ProfileStore::load`]: crosshook_core::profile::ProfileStore::load
pub(super) fn apply_collection_defaults(
    profile: GameProfile,
    metadata_store: &MetadataStore,
    collection_id: Option<&str>,
) -> Result<GameProfile, MetadataStoreError> {
    // Without defaults the merge is the identity on a loaded profile (its
    // `local_override` known fields are already baked in and cleared).
    Ok(
        match resolve_collection_defaults(metadata_store, collection_id)? {
            Some(defaults) => profile.effective_profile_with(Some(&defaults)),
            None => profile,
        },
    )
}

/// Resolves the launch-defaults layer of `collection_id` with the fail-open
/// policy documented on [`apply_collection_defaults`]: `Ok(None)` when there
/// is no collection context, no defaults, or a non-corrupt lookup failure;
/// `Err(Corrupt)` for corrupt defaults.
pub(super) fn resolve_collection_defaults(
    metadata_store: &MetadataStore,
    collection_id: Option<&str>,
) -> Result<Option<CollectionDefaultsSection>, MetadataStoreError> {
    let Some(cid) = collection_id.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    match metadata_store.get_collection_defaults(cid) {
        Ok(defaults) => Ok(defaults),
        Err(MetadataStoreError::Corrupt(msg)) => {
            tracing::warn!(
                collection_id = %cid,
                error = %msg,
                "collection defaults JSON is corrupt; surfacing error to launch entrypoint"
            );
            Err(MetadataStoreError::Corrupt(msg))
        }
        Err(other) => {
            tracing::warn!(
                collection_id = %cid,
                error = %other,
                "failed to load collection defaults; launching with raw profile"
            );
            Ok(None)
        }
    }
}
