use std::fs;
use std::path::Path;

/// Best-effort total size of files under `path` (recursive).
pub fn directory_size_bytes(path: &Path) -> u64 {
    fn walk(dir: &Path) -> std::io::Result<u64> {
        let mut sum = 0u64;
        let Ok(read_dir) = fs::read_dir(dir) else {
            return Ok(0);
        };
        for entry in read_dir.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                sum += walk(&entry.path()).unwrap_or(0);
            } else {
                sum += meta.len();
            }
        }
        Ok(sum)
    }
    walk(path).unwrap_or(0)
}

pub(crate) fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}
