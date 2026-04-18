use std::ffi::{CStr, CString};
use std::path::Path;

const FLATPAK_ID_ENV: &str = "FLATPAK_ID";
const FLATPAK_INFO_PATH: &str = "/.flatpak-info";
const FLATPAK_HOST_ROOT_PREFIX: &str = "/run/host";
const FLATPAK_DOCUMENT_PORTAL_PREFIX: &str = "/run/user/";
const FLATPAK_DOCUMENT_PORTAL_SEGMENT: &str = "/doc/";
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
/// build continue to work when reused later by the native/AppImage build.
/// Non-Unix paths (for example `C:\Games\foo.exe`) and relative paths are
/// returned unchanged aside from trimming outer whitespace.
pub fn normalize_flatpak_host_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed == FLATPAK_HOST_ROOT_PREFIX {
        return "/".to_string();
    }

    if let Some(stripped) = trimmed.strip_prefix(&format!("{FLATPAK_HOST_ROOT_PREFIX}/")) {
        return format!("/{}", stripped.trim_start_matches('/'));
    }

    if let Some(host_path) = read_document_portal_host_path(trimmed) {
        return host_path;
    }

    path.to_string()
}

fn looks_like_document_portal_path(path: &str) -> bool {
    path.starts_with(FLATPAK_DOCUMENT_PORTAL_PREFIX)
        && path.contains(FLATPAK_DOCUMENT_PORTAL_SEGMENT)
}

fn read_document_portal_host_path(path: &str) -> Option<String> {
    if !looks_like_document_portal_path(path) {
        return None;
    }

    read_document_portal_host_path_xattr(path)
}

pub(crate) fn read_document_portal_host_path_xattr(path: &str) -> Option<String> {
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
            Some(String::from_utf8_lossy(&buffer).trim().to_string())
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
