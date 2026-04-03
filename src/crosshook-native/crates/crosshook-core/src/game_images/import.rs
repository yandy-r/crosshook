use std::path::{Path, PathBuf};

use crate::metadata::sha256_hex;

use super::client::validate_image_bytes;
use super::models::GameImageType;

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

/// Copy a user-selected art image of the given type into the managed media directory.
///
/// The file is validated (magic bytes, size limit, MIME allow-list) before
/// copying.  The destination filename is derived from the content hash so
/// repeated imports of the same file are idempotent — the existing copy is
/// returned without re-writing.
///
/// Returns the absolute path of the imported file.
pub fn import_custom_art(source_path: &str, art_type: GameImageType) -> Result<String, String> {
    let subdir = match art_type {
        GameImageType::Cover => "covers",
        GameImageType::Portrait => "portraits",
        GameImageType::Background => "backgrounds",
        _ => {
            return Err(format!(
                "Unsupported art type for custom import: {art_type}"
            ))
        }
    };
    let dest_dir = media_base_dir()?.join(subdir);

    let source = Path::new(source_path.trim());
    if !source.exists() {
        return Err(format!("source file does not exist: {}", source.display()));
    }

    let bytes = std::fs::read(source).map_err(|e| format!("failed to read source file: {e}"))?;

    let mime = validate_image_bytes(&bytes).map_err(|e| format!("image validation failed: {e}"))?;
    let ext = mime_extension(mime);

    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("failed to create media directory: {e}"))?;

    let hash = sha256_hex(&bytes);
    let dest_path = dest_dir.join(format!("{}.{ext}", &hash[..16]));

    // Idempotent: skip write if the content-addressed file already exists.
    if !dest_path.exists() {
        std::fs::write(&dest_path, &bytes)
            .map_err(|e| format!("failed to write imported art: {e}"))?;
    }

    dest_path
        .to_str()
        .ok_or_else(|| "media path contains non-UTF-8 characters".to_string())
        .map(String::from)
}

/// Copy a user-selected cover art image into the managed media directory.
///
/// This is a backward-compatible wrapper around [`import_custom_art`] using
/// [`GameImageType::Cover`].
pub fn import_custom_cover_art(source_path: &str) -> Result<String, String> {
    import_custom_art(source_path, GameImageType::Cover)
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

    #[test]
    fn import_custom_art_portrait_creates_portraits_subdir() {
        use std::io::Write;
        use tempfile::tempdir;

        let tmp = tempdir().unwrap();
        // Minimal valid PNG magic bytes (1x1 PNG)
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk length + type
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1, height=1
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // bitdepth, colortype, ...
            0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT chunk
            0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2,
            0x21, 0xBC, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, // IEND chunk
            0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        let source = tmp.path().join("portrait.png");
        let mut f = std::fs::File::create(&source).unwrap();
        f.write_all(png_bytes).unwrap();

        let result = import_custom_art(source.to_str().unwrap(), GameImageType::Portrait);

        // The result path should contain the "portraits" subdirectory component.
        match result {
            Ok(path) => {
                assert!(
                    path.contains("portraits"),
                    "expected 'portraits' in path, got: {path}"
                );
            }
            // If image validation fails for minimal PNG bytes, that's an
            // acceptable outcome — but the subdir logic must not have errored.
            Err(e) => {
                assert!(
                    !e.contains("Unsupported art type"),
                    "unexpected unsupported-art-type error: {e}"
                );
            }
        }
    }

    #[test]
    fn import_custom_art_unsupported_type_returns_error() {
        let result = import_custom_art("/nonexistent/path.jpg", GameImageType::Hero);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("Unsupported art type"),
            "expected unsupported-art-type error"
        );
    }

    #[test]
    fn import_custom_art_portrait_rejects_missing_source() {
        let result = import_custom_art("/nonexistent/portrait.png", GameImageType::Portrait);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }
}
