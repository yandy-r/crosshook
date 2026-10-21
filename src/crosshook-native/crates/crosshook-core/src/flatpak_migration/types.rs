use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum FlatpakMigrationError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    SourceMissing(PathBuf),
    DestinationNotEmpty(PathBuf),
    HomeDirectoryUnavailable,
}

impl fmt::Display for FlatpakMigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "io error at {}: {source}", path.display()),
            Self::SourceMissing(p) => write!(f, "source path missing: {}", p.display()),
            Self::DestinationNotEmpty(p) => write!(f, "destination not empty: {}", p.display()),
            Self::HomeDirectoryUnavailable => write!(f, "HOME directory unavailable"),
        }
    }
}

impl std::error::Error for FlatpakMigrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MigrationOutcome {
    pub imported_config: bool,
    pub imported_subtrees: Vec<&'static str>,
    pub skipped_subtrees: Vec<&'static str>,
}

pub const CONFIG_ROOT_SEGMENT: &str = "crosshook";
pub const DATA_INCLUDE_SUBTREES: &[&str] = &[
    "crosshook/community",
    "crosshook/media",
    "crosshook/launchers",
];
/// Metadata DB trio, copied as one all-or-nothing unit with the `.db` first.
pub const DATA_INCLUDE_FILES: &[&str] = &[
    "crosshook/metadata.db",
    "crosshook/metadata.db-wal",
    "crosshook/metadata.db-shm",
];
/// Config-history HMAC key (ADR-0005). Copied only alongside a DB import and
/// only when the sandbox has no key, so imported signatures keep verifying.
/// Never part of the "already imported" check.
pub const DATA_INCLUDE_DB_COMPANION_FILE: &str = "crosshook/config_history_hmac.key";
pub const DATA_SKIP_SUBTREES: &[&str] = &[
    "crosshook/prefixes",
    "crosshook/artifacts",
    "crosshook/cache",
    "crosshook/logs",
    "crosshook/runtime-helpers",
];
