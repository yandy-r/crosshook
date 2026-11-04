//! Strict resolution of paths returned by a native file or folder picker.
//!
//! Under Flatpak, a pick can come back as a sandbox-only path: a `/run/host`
//! mount or a document-portal FUSE path (`/run/user/<uid>/doc/<id>/<entry>` or
//! `/run/flatpak/doc/<id>/<entry>`). Host-executed processes cannot open
//! those, so a pick meant for host use is resolved to the real host path or
//! rejected with a [`HostPathError`]. A pick meant for sandbox use keeps its
//! document-portal path, which is the one the sandbox can read.
//!
//! Compare [`normalize_flatpak_host_path`], the best-effort, infallible repair
//! applied to already-persisted paths.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::detect::{
    document_portal_tail, is_document_portal_path, nested_document_portal_host_path,
    normalize_flatpak_host_path, parse_document_portal_tail,
    read_document_portal_host_path_xattr_bytes, DocumentPortalPath, FLATPAK_HOST_ROOT,
};
use super::portals::documents::{DocumentsPortal, ZbusDocumentsPortal};

/// Upper bound for reading the document-portal xattr, which goes through the
/// portal's FUSE daemon and can stall when that daemon hangs.
const XATTR_READ_TIMEOUT: Duration = Duration::from_secs(1);

/// Maximum number of xattr reads that may be outstanding at once. A read that
/// times out keeps its blocking thread until `getxattr(2)` returns, so a hung
/// portal could otherwise fill the shared blocking pool with repeated picks.
const MAX_IN_FLIGHT_XATTR_READS: usize = 2;

static XATTR_READ_PERMITS: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(MAX_IN_FLIGHT_XATTR_READS);

/// What a picked path will be used for. Sent by the frontend as `pathUse`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PickedPathUse {
    /// Consumed by a host-executed process or the host desktop; resolved with
    /// [`resolve_picked_host_path_default`].
    Host,
    /// Read or written by CrossHook itself inside the sandbox; resolved with
    /// [`resolve_picked_sandbox_path`].
    Sandbox,
}

/// Why a picked path could not be turned into a host path.
///
/// Variants carry only the file name (last path component), never the full
/// sandbox path, so messages stay readable and do not leak portal internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPathError {
    /// The picked path is empty or whitespace only.
    Empty,
    /// A document-portal path that neither the xattr nor the portal mapped to
    /// a host path, or one that is malformed (no document id, no entry, or
    /// `..` components).
    UnresolvedDocumentPortalPath {
        /// Last component of the picked path.
        file_name: String,
    },
    /// The document portal's `GetHostPaths` call failed or timed out.
    DocumentPortalUnavailable {
        /// Last component of the picked path.
        file_name: String,
    },
    /// The document portal answered with a host path that is not absolute,
    /// contains NUL bytes, or is not UTF-8.
    InvalidPortalReply {
        /// Last component of the picked path.
        file_name: String,
    },
}

impl fmt::Display for HostPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("the selected path is empty"),
            Self::UnresolvedDocumentPortalPath { file_name } => write!(
                f,
                "CrossHook could not find the real location of \"{file_name}\". Grant CrossHook \
                 access to its folder (for example with Flatseal, or `flatpak override --user \
                 --filesystem=<folder> dev.crosshook.CrossHook`) and pick it again."
            ),
            Self::DocumentPortalUnavailable { file_name } => write!(
                f,
                "CrossHook could not find the real location of \"{file_name}\" because the \
                 desktop document portal did not answer (it needs xdg-desktop-portal 1.19 or \
                 newer). Update xdg-desktop-portal, or grant CrossHook access to the file's \
                 folder (for example with Flatseal) and pick it again."
            ),
            Self::InvalidPortalReply { file_name } => write!(
                f,
                "the desktop document portal returned an invalid location for \"{file_name}\""
            ),
        }
    }
}

impl std::error::Error for HostPathError {}

/// Reads the `user.document-portal.host-path` xattr of a document-portal file.
pub(crate) trait XattrReader: Send + Sync {
    /// Returns the raw xattr value stored on `path`, or `None` when it is
    /// absent or unreadable.
    fn document_portal_host_path(&self, path: &str) -> Option<Vec<u8>>;
}

/// Serves an xattr value read ahead of time on a blocking thread. Answers
/// only for the path it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrefetchedXattr {
    pub(crate) path: String,
    pub(crate) value: Option<Vec<u8>>,
}

impl PrefetchedXattr {
    /// Reads the xattr of `path` with `getxattr(2)` on a blocking thread,
    /// giving up after [`XATTR_READ_TIMEOUT`]. A timed-out read leaves its
    /// thread to finish in the background and yields `None`. At most
    /// [`MAX_IN_FLIGHT_XATTR_READS`] reads run at once; past that cap the read
    /// is skipped (yielding `None`, so resolution falls back to D-Bus).
    async fn read(path: &str) -> Self {
        let Ok(permit) = XATTR_READ_PERMITS.try_acquire() else {
            tracing::warn!(
                max_in_flight = MAX_IN_FLIGHT_XATTR_READS,
                "document-portal xattr reads still stalled; skipping xattr lookup"
            );
            return Self {
                path: path.to_string(),
                value: None,
            };
        };
        let owned = path.to_string();
        let read = tokio::task::spawn_blocking(move || {
            // Held until the syscall returns, even after the caller times out.
            let _permit = permit;
            read_document_portal_host_path_xattr_bytes(&owned)
        });
        let value = match tokio::time::timeout(XATTR_READ_TIMEOUT, read).await {
            Ok(Ok(value)) => value,
            Ok(Err(error)) => {
                tracing::debug!(%error, "document-portal xattr read task failed");
                None
            }
            Err(_) => {
                tracing::warn!(
                    timeout_ms = XATTR_READ_TIMEOUT.as_millis(),
                    "document-portal xattr read timed out"
                );
                None
            }
        };
        Self {
            path: path.to_string(),
            value,
        }
    }
}

impl XattrReader for PrefetchedXattr {
    fn document_portal_host_path(&self, path: &str) -> Option<Vec<u8>> {
        if path == self.path {
            self.value.clone()
        } else {
            None
        }
    }
}

/// A trimmed, non-empty pick, classified before any I/O.
enum ClassifiedPick<'a> {
    /// Already a host path (or Windows-style / relative input passed through).
    Resolved(PathBuf),
    /// A well-formed document-portal path that needs the xattr or the portal.
    DocumentPortal(DocumentPortalPath<'a>),
}

/// Applies every purely lexical rule: empty input, `/run/host` stripping,
/// malformed document-portal rejection and lexical cleanup.
fn classify_pick(trimmed: &str) -> Result<ClassifiedPick<'_>, HostPathError> {
    if trimmed.is_empty() {
        return Err(HostPathError::Empty);
    }

    if let Some(host_rest) = trimmed.strip_prefix(FLATPAK_HOST_ROOT) {
        if host_rest.is_empty() || host_rest.starts_with('/') {
            return Ok(ClassifiedPick::Resolved(clean_absolute(&format!(
                "/{host_rest}"
            ))));
        }
    }

    if let Some(tail) = document_portal_tail(trimmed) {
        return parse_document_portal_tail(tail)
            .map(ClassifiedPick::DocumentPortal)
            .ok_or_else(|| HostPathError::UnresolvedDocumentPortalPath {
                file_name: file_name(trimmed),
            });
    }

    // Any path starting with `/` is a POSIX path; Windows-style input
    // (`C:\…`, `Z:/…`) never does, so it falls through unchanged.
    if trimmed.starts_with('/') {
        return Ok(ClassifiedPick::Resolved(clean_absolute(trimmed)));
    }
    Ok(ClassifiedPick::Resolved(PathBuf::from(trimmed)))
}

/// Last non-empty `/` component of `path`, for user-facing messages.
fn file_name(path: &str) -> String {
    path.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// Collapses duplicate slashes and drops `.` components. `..` is kept as-is.
fn clean_absolute(path: &str) -> PathBuf {
    Path::new(path).components().collect()
}

/// Validates a raw host path from the xattr or a `GetHostPaths` reply: one
/// trailing NUL is stripped; the rest must be NUL-free, UTF-8 and absolute.
/// Nothing is trimmed, since whitespace is a legal file-name byte.
fn decode_portal_host_path(raw: &[u8]) -> Option<&str> {
    let raw = raw.strip_suffix(b"\0").unwrap_or(raw);
    if raw.contains(&0) {
        return None;
    }
    std::str::from_utf8(raw)
        .ok()
        .filter(|path| path.starts_with('/'))
}

/// Resolves a freshly picked path to a path valid on the host.
///
/// Order: trim → empty is [`HostPathError::Empty`] → `/run/host` is `/` →
/// `/run/host/…` is stripped → malformed document-portal paths are rejected →
/// document-portal paths resolve through the `user.document-portal.host-path`
/// xattr of the full path (appending the remainder inside an exported
/// directory when the value does not already end with it, see
/// [`nested_document_portal_host_path`]), then through `GetHostPaths`
/// (appending the remainder inside an exported directory) → anything else
/// passes through.
/// Absolute Unix results are lexically cleaned (duplicate slashes and `.`
/// removed, `..` kept); Windows-style and relative input is returned trimmed.
///
/// A document-portal path is never returned as-is: it resolves or fails with
/// a [`HostPathError`].
pub(crate) async fn resolve_picked_host_path(
    path: &str,
    xattr: &dyn XattrReader,
    portal: &dyn DocumentsPortal,
) -> Result<PathBuf, HostPathError> {
    let trimmed = path.trim();
    match classify_pick(trimmed)? {
        ClassifiedPick::Resolved(resolved) => Ok(resolved),
        ClassifiedPick::DocumentPortal(parsed) => {
            resolve_document_portal_path(trimmed, parsed, xattr, portal).await
        }
    }
}

async fn resolve_document_portal_path(
    path: &str,
    parsed: DocumentPortalPath<'_>,
    xattr: &dyn XattrReader,
    portal: &dyn DocumentsPortal,
) -> Result<PathBuf, HostPathError> {
    if let Some(raw) = xattr.document_portal_host_path(path) {
        match decode_portal_host_path(&raw) {
            Some(host_path) => {
                let host_path = clean_absolute(host_path);
                return Ok(nested_document_portal_host_path(&host_path, &parsed.rest).into_owned());
            }
            None => tracing::debug!(
                doc_id = parsed.doc_id,
                "document-portal xattr is not a valid host path; asking the portal"
            ),
        }
    }

    let doc_ids = [parsed.doc_id.to_string()];
    let mut replies = portal.get_host_paths(&doc_ids).await.map_err(|error| {
        tracing::warn!(
            doc_id = parsed.doc_id,
            %error,
            "document portal GetHostPaths failed"
        );
        HostPathError::DocumentPortalUnavailable {
            file_name: file_name(path),
        }
    })?;
    let raw = replies.remove(parsed.doc_id).ok_or_else(|| {
        tracing::debug!(
            doc_id = parsed.doc_id,
            "document portal does not know the document id"
        );
        HostPathError::UnresolvedDocumentPortalPath {
            file_name: file_name(path),
        }
    })?;
    let entry_host_path =
        decode_portal_host_path(&raw).ok_or_else(|| HostPathError::InvalidPortalReply {
            file_name: file_name(path),
        })?;

    let mut host_path = clean_absolute(entry_host_path);
    host_path.extend(parsed.rest);
    Ok(host_path)
}

/// [`resolve_picked_host_path`] with the real xattr and the session-bus
/// document portal.
///
/// Lexical rules run first, so non-portal and malformed portal paths make no
/// syscall. For a well-formed portal path the xattr is read on a blocking
/// thread with a 1 s timeout, because a FUSE `getxattr` can stall on the
/// portal daemon.
pub async fn resolve_picked_host_path_default(path: &str) -> Result<PathBuf, HostPathError> {
    let trimmed = path.trim();
    let xattr = match classify_pick(trimmed)? {
        ClassifiedPick::Resolved(resolved) => return Ok(resolved),
        ClassifiedPick::DocumentPortal(_) => PrefetchedXattr::read(trimmed).await,
    };
    // Re-classifying is a cheap string scan and keeps a single code path
    // shared with the unit-tested resolver.
    resolve_picked_host_path(trimmed, &xattr, &ZbusDocumentsPortal).await
}

/// Resolves a freshly picked path for use inside the sandbox.
///
/// Document-portal paths are returned trimmed but otherwise unchanged, since
/// that is the path the sandbox can read. Everything else gets the
/// [`normalize_flatpak_host_path`] repair (for example `/run/host` stripping).
pub fn resolve_picked_sandbox_path(path: &str) -> String {
    let trimmed = path.trim();
    if is_document_portal_path(trimmed) {
        return trimmed.to_string();
    }
    normalize_flatpak_host_path(trimmed)
}
