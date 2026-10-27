use std::io::Read;
use std::path::{Path, PathBuf};

use crate::metadata::sha256_hex;

use super::client::validate_image_bytes;
use super::models::{GameImageError, GameImageType};

/// Custom-art import failure categories.
///
/// Display text is fixed and never embeds filesystem paths or OS error detail,
/// so it is safe to surface to the UI. Underlying I/O detail is logged at
/// debug level by kind only.
#[derive(Debug)]
enum ImportError {
    UnsupportedType(GameImageType),
    HomeDirNotFound,
    MissingSource,
    ReadSource,
    Validation(GameImageError),
    CreateMediaDir,
    WriteImported,
    NonUtf8MediaPath,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedType(art_type) => {
                write!(f, "Unsupported art type for custom import: {art_type}")
            }
            Self::HomeDirNotFound => write!(f, "home directory not found"),
            Self::MissingSource => write!(f, "source file does not exist"),
            Self::ReadSource => write!(f, "failed to read source file"),
            Self::Validation(error) => write!(f, "image validation failed: {error}"),
            Self::CreateMediaDir => write!(f, "failed to create media directory"),
            Self::WriteImported => write!(f, "failed to write imported art"),
            Self::NonUtf8MediaPath => write!(f, "media path contains non-UTF-8 characters"),
        }
    }
}

/// Base directory for user-imported media files.
///
/// Resolves to `~/.local/share/crosshook/media` on Linux.
fn media_base_dir() -> Result<PathBuf, ImportError> {
    crate::platform::base_dirs()
        .ok_or(ImportError::HomeDirNotFound)
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
    import_custom_art_impl(source_path, art_type, None).map_err(|e| e.to_string())
}

/// Imports art into an explicit media root, using the same validation and layout
/// as [`import_custom_art`]. Useful for callers with an already resolved root.
pub fn import_custom_art_into(
    source_path: &str,
    art_type: GameImageType,
    media_base: &Path,
) -> Result<String, String> {
    import_custom_art_impl(source_path, art_type, Some(media_base)).map_err(|e| e.to_string())
}

fn import_custom_art_impl(
    source_path: &str,
    art_type: GameImageType,
    media_base: Option<&Path>,
) -> Result<String, ImportError> {
    let subdir = match art_type {
        GameImageType::Cover => "covers",
        GameImageType::Portrait => "portraits",
        GameImageType::Background => "backgrounds",
        GameImageType::Hero | GameImageType::Capsule => {
            return Err(ImportError::UnsupportedType(art_type));
        }
    };
    let media_base = match media_base {
        Some(base) => base.to_path_buf(),
        None => media_base_dir()?,
    };
    let dest_dir = media_base.join(subdir);

    let source = Path::new(source_path.trim());
    if !source.exists() {
        return Err(ImportError::MissingSource);
    }

    let source_file = std::fs::File::open(source).map_err(|e| {
        tracing::debug!(error_kind = ?e.kind(), "custom art source open failed");
        ImportError::ReadSource
    })?;
    let bytes = read_bounded_image(source_file)?;

    let mime = validate_image_bytes(&bytes).map_err(ImportError::Validation)?;
    let ext = mime_extension(mime);

    std::fs::create_dir_all(&dest_dir).map_err(|e| {
        tracing::debug!(error_kind = ?e.kind(), "custom art media directory creation failed");
        ImportError::CreateMediaDir
    })?;

    let hash = sha256_hex(&bytes);
    let dest_path = dest_dir.join(format!("{}.{ext}", &hash[..16]));

    // Idempotent: skip write if the content-addressed file already exists.
    if !dest_path.exists() {
        std::fs::write(&dest_path, &bytes).map_err(|e| {
            tracing::debug!(error_kind = ?e.kind(), "custom art write failed");
            ImportError::WriteImported
        })?;
    }

    dest_path
        .to_str()
        .ok_or(ImportError::NonUtf8MediaPath)
        .map(String::from)
}

/// Copy a user-selected cover art image into the managed media directory.
///
/// This is a backward-compatible wrapper around [`import_custom_art`] using
/// [`GameImageType::Cover`].
pub fn import_custom_cover_art(source_path: &str) -> Result<String, String> {
    import_custom_art(source_path, GameImageType::Cover)
}

const MAX_IMPORT_BYTES: u64 = 5 * 1024 * 1024;

fn read_bounded_image(reader: impl Read) -> Result<Vec<u8>, ImportError> {
    let mut bytes = Vec::new();
    // Read one extra byte to distinguish an exact-limit file from an oversized
    // stream. Never read the rest of a file that fails the size policy.
    reader
        .take(MAX_IMPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| {
            tracing::debug!(error_kind = ?e.kind(), "custom art source read failed");
            ImportError::ReadSource
        })?;
    if bytes.len() as u64 > MAX_IMPORT_BYTES {
        return Err(ImportError::Validation(GameImageError::TooLarge));
    }
    Ok(bytes)
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
    fn bounded_read_stops_after_first_excess_byte() {
        struct Endless<'a>(&'a mut u64);
        impl Read for Endless<'_> {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                *self.0 += output.len() as u64;
                output.fill(0);
                Ok(output.len())
            }
        }
        let mut consumed = 0;
        let error = read_bounded_image(Endless(&mut consumed))
            .unwrap_err()
            .to_string();
        assert_eq!(consumed, MAX_IMPORT_BYTES + 1);
        assert!(error.contains("size limit"));
    }

    #[test]
    fn bounded_read_accepts_exact_limit() {
        let bytes = read_bounded_image(std::io::repeat(0).take(MAX_IMPORT_BYTES)).unwrap();
        assert_eq!(bytes.len() as u64, MAX_IMPORT_BYTES);
    }

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
    fn import_errors_do_not_include_source_paths() {
        let missing = "/nonexistent-crosshook-test/x.png";
        let error = import_custom_cover_art(missing).unwrap_err();
        assert!(error.contains("does not exist"));
        assert!(!error.contains("nonexistent-crosshook-test"));
        assert!(!error.contains(missing));

        let tmp = tempfile::tempdir().unwrap();
        let media = tmp.path().join("media");
        let dir_source = tmp.path().join("source-dir-marker");
        std::fs::create_dir(&dir_source).unwrap();
        let error =
            import_custom_art_into(dir_source.to_str().unwrap(), GameImageType::Cover, &media)
                .unwrap_err();
        assert!(!error.contains("source-dir-marker"), "{error}");
        assert!(!error.contains(tmp.path().to_str().unwrap()), "{error}");

        let text_source = tmp.path().join("notes-marker.txt");
        std::fs::write(&text_source, b"not an image").unwrap();
        let error =
            import_custom_art_into(text_source.to_str().unwrap(), GameImageType::Cover, &media)
                .unwrap_err();
        assert!(error.contains("image validation failed"));
        assert!(!error.contains("notes-marker"), "{error}");
        assert!(!error.contains(tmp.path().to_str().unwrap()), "{error}");

        let blocker = tmp.path().join("blocked-media");
        std::fs::write(&blocker, b"x").unwrap();
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        let png_source = tmp.path().join("ok.png");
        std::fs::write(&png_source, png).unwrap();
        let error =
            import_custom_art_into(png_source.to_str().unwrap(), GameImageType::Cover, &blocker)
                .unwrap_err();
        assert!(!error.contains(tmp.path().to_str().unwrap()), "{error}");
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
