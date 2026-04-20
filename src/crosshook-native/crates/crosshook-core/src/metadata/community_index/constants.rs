//! A6 string length bounds for community profile indexing.
//!
//! These constants enforce advisory security limits on text fields to prevent
//! unbounded memory consumption and database bloat from malformed or malicious
//! community profile manifests.

/// Maximum bytes for game name field.
pub(super) const MAX_GAME_NAME_BYTES: usize = 512;

/// Maximum bytes for description field.
pub(super) const MAX_DESCRIPTION_BYTES: usize = 4_096;

/// Maximum bytes for platform tags (space-joined).
pub(super) const MAX_PLATFORM_TAGS_BYTES: usize = 2_048;

/// Maximum bytes for trainer name field.
pub(super) const MAX_TRAINER_NAME_BYTES: usize = 512;

/// Maximum bytes for author field.
pub(super) const MAX_AUTHOR_BYTES: usize = 512;

/// Maximum bytes for version fields (game_version, trainer_version, proton_version).
pub(super) const MAX_VERSION_BYTES: usize = 256;

/// Maximum bytes for trainer source URL.
pub(super) const MAX_SOURCE_URL_BYTES: usize = 2_048;

/// Maximum bytes for trainer source name.
pub(super) const MAX_SOURCE_NAME_BYTES: usize = 512;

/// Maximum bytes for trainer source notes.
pub(super) const MAX_NOTES_BYTES: usize = 4_096;
