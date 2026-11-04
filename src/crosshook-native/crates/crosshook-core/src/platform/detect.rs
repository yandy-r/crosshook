use std::ffi::{CStr, CString};
use std::path::Path;

const FLATPAK_ID_ENV: &str = "FLATPAK_ID";
const FLATPAK_INFO_PATH: &str = "/.flatpak-info";
/// Mount point of the host filesystem inside the Flatpak sandbox.
pub(crate) const FLATPAK_HOST_ROOT: &str = "/run/host";
/// Per-user document-portal mount: `/run/user/<uid>/doc`.
const RUN_USER_PREFIX: &str = "/run/user/";
const RUN_USER_DOCUMENT_PORTAL_DIR: &str = "doc";
/// Document-portal mount that Flatpak binds into every sandbox; newer
/// xdg-desktop-portal releases hand out paths in this form.
const FLATPAK_DOCUMENT_PORTAL_ROOT: &str = "/run/flatpak/doc";
pub(crate) const DOCUMENT_PORTAL_HOST_PATH_XATTR: &[u8] = b"user.document-portal.host-path\0";

/// Returns `true` when running inside a Flatpak sandbox.
///
/// Detection uses the two signals documented by the Flatpak runtime:
/// the `FLATPAK_ID` environment variable (set automatically by `flatpak run`)
/// and the presence of `/.flatpak-info` (always mounted inside the sandbox).
pub fn is_flatpak() -> bool {
    is_flatpak_with(FLATPAK_ID_ENV, Path::new(FLATPAK_INFO_PATH))
}

/// Normalizes a Flatpak host-mount path like `/run/host/usr/bin/foo` back to
/// the corresponding host path (`/usr/bin/foo`).
///
/// This repair is applied unconditionally so paths persisted by the Flatpak
/// build continue to work when reused later by host/dev tooling.
/// Non-Unix paths (for example `C:\Games\foo.exe`) and relative paths are
/// returned unchanged aside from trimming outer whitespace.
///
/// # vs `resolve_picked_host_path_default`
///
/// This function is a best-effort, infallible repair for paths that were
/// already persisted: a document-portal path whose xattr cannot be read is
/// returned unchanged. Fresh picks from a file dialog go through
/// [`resolve_picked_host_path_default`](super::resolve_picked_host_path_default)
/// instead, which also asks the document portal over D-Bus and returns an
/// error rather than handing a sandbox-only path to the host.
pub fn normalize_flatpak_host_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed == FLATPAK_HOST_ROOT {
        return "/".to_string();
    }

    if let Some(stripped) = trimmed.strip_prefix(&format!("{FLATPAK_HOST_ROOT}/")) {
        return format!("/{}", stripped.trim_start_matches('/'));
    }

    if let Some(host_path) = read_document_portal_host_path(trimmed) {
        return host_path;
    }

    trimmed.to_string()
}

/// The document id and the components after the entry name of a
/// document-portal path (`<mount>/<doc_id>/<entry>[/rest]`).
pub(crate) struct DocumentPortalPath<'a> {
    pub(crate) doc_id: &'a str,
    /// Components below the entry, with empty and `.` components dropped.
    pub(crate) rest: Vec<&'a str>,
}

/// Returns the part of `path` after its document-portal mount (empty or
/// starting with `/`), or `None` when `path` is not inside a document-portal
/// mount. Recognises `/run/user/<digits>/doc` and `/run/flatpak/doc`.
pub(crate) fn document_portal_tail(path: &str) -> Option<&str> {
    let tail = match path.strip_prefix(FLATPAK_DOCUMENT_PORTAL_ROOT) {
        Some(tail) => tail,
        None => {
            let (uid, after_uid) = path.strip_prefix(RUN_USER_PREFIX)?.split_once('/')?;
            if uid.is_empty() || !uid.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            after_uid.strip_prefix(RUN_USER_DOCUMENT_PORTAL_DIR)?
        }
    };
    (tail.is_empty() || tail.starts_with('/')).then_some(tail)
}

/// `true` when `path` lies inside a document-portal mount (well-formed or not).
pub(crate) fn is_document_portal_path(path: &str) -> bool {
    document_portal_tail(path).is_some()
}

/// Parses a document-portal tail returned by [`document_portal_tail`].
/// Returns `None` for an empty or dot document id or entry, or for `..`
/// anywhere below the entry.
pub(crate) fn parse_document_portal_tail(tail: &str) -> Option<DocumentPortalPath<'_>> {
    let mut segments = tail.split('/').skip(1);
    let doc_id = segments.next().filter(|segment| is_name(segment))?;
    segments.next().filter(|segment| is_name(segment))?;

    let mut rest = Vec::new();
    for segment in segments {
        match segment {
            "" | "." => {}
            ".." => return None,
            name => rest.push(name),
        }
    }
    Some(DocumentPortalPath { doc_id, rest })
}

fn is_name(segment: &str) -> bool {
    !matches!(segment, "" | "." | "..")
}

fn read_document_portal_host_path(path: &str) -> Option<String> {
    if !is_document_portal_path(path) {
        return None;
    }

    read_document_portal_host_path_xattr(path)
}

/// Lossy, trimmed string form of [`read_document_portal_host_path_xattr_bytes`]
/// used by the best-effort [`normalize_flatpak_host_path`] repair.
pub(crate) fn read_document_portal_host_path_xattr(path: &str) -> Option<String> {
    let bytes = read_document_portal_host_path_xattr_bytes(path)?;
    Some(String::from_utf8_lossy(&bytes).trim().to_string())
}

/// Raw value of the `user.document-portal.host-path` xattr on `path`, or
/// `None` when it is absent, empty or unreadable.
pub(crate) fn read_document_portal_host_path_xattr_bytes(path: &str) -> Option<Vec<u8>> {
    #[cfg(target_os = "linux")]
    {
        let c_path = CString::new(path.as_bytes()).ok()?;
        let attr_name = CStr::from_bytes_with_nul(DOCUMENT_PORTAL_HOST_PATH_XATTR).ok()?;

        // SAFETY: `c_path` and `attr_name` are NUL-terminated and live across
        // both libc calls. We first probe for the required buffer size, then
        // allocate exactly that many bytes before reading the xattr value.
        unsafe {
            let size =
                nix::libc::getxattr(c_path.as_ptr(), attr_name.as_ptr(), std::ptr::null_mut(), 0);
            if size <= 0 {
                return None;
            }

            let mut buffer = vec![0u8; size as usize];
            let written = nix::libc::getxattr(
                c_path.as_ptr(),
                attr_name.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            );
            if written <= 0 {
                return None;
            }

            buffer.truncate(written as usize);
            Some(buffer)
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

pub(crate) fn is_flatpak_with(env_key: &str, info_path: &Path) -> bool {
    std::env::var_os(env_key).is_some() || info_path.exists()
}
