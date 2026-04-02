use std::path::{Path, PathBuf};

use crate::metadata::sha256_hex;

use super::client::validate_image_bytes;

/// Base directory for user-imported media files.
///
/// Resolves to `~/.local/share/crosshook/media` on Linux.
fn media_base_dir() -> Result<PathBuf, String> {
    directories::BaseDirs::new()
        .ok_or_else(|| "home directory not found".to_string())
        .map(|dirs| dirs.data_local_dir().join("crosshook").join("media"))
}

/// Return `true` when `path` is already inside the managed media directory.
pub fn is_in_managed_media_dir(path: &str) -> bool {
    let Ok(base) = media_base_dir() else {
        return false;
    };
    Path::new(path.trim()).starts_with(&base)
}

/// Copy a user-selected cover art image into the managed media directory.
///
/// The file is validated (magic bytes, size limit, MIME allow-list) before
/// copying.  The destination filename is derived from the content hash so
/// repeated imports of the same file are idempotent — the existing copy is
/// returned without re-writing.
///
/// Returns the absolute path of the imported file.
pub fn import_custom_cover_art(source_path: &str) -> Result<String, String> {
    let source = Path::new(source_path.trim());
    if !source.exists() {
        return Err(format!(
            "source file does not exist: {}",
            source.display()
        ));
    }

    let bytes = std::fs::read(source)
        .map_err(|e| format!("failed to read source file: {e}"))?;

    let mime = validate_image_bytes(&bytes)
        .map_err(|e| format!("image validation failed: {e}"))?;
    let ext = mime_extension(mime);

    let dest_dir = media_base_dir()?.join("covers");
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("failed to create media directory: {e}"))?;

    let hash = sha256_hex(&bytes);
    let dest_path = dest_dir.join(format!("{}.{ext}", &hash[..16]));

    // Idempotent: skip write if the content-addressed file already exists.
    if !dest_path.exists() {
        std::fs::write(&dest_path, &bytes)
            .map_err(|e| format!("failed to write imported cover art: {e}"))?;
    }

    dest_path
        .to_str()
        .ok_or_else(|| "media path contains non-UTF-8 characters".to_string())
        .map(String::from)
}

fn mime_extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => "img",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_in_managed_media_dir_detects_managed_paths() {
        let base = media_base_dir().unwrap();
        let inside = base.join("covers").join("abc.jpg");
        assert!(is_in_managed_media_dir(inside.to_str().unwrap()));

        assert!(!is_in_managed_media_dir("/tmp/random.jpg"));
        assert!(!is_in_managed_media_dir(""));
    }

    #[test]
    fn import_rejects_missing_source() {
        let result = import_custom_cover_art("/nonexistent/path.jpg");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }
}
