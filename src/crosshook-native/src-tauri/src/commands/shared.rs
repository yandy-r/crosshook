use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn create_log_path(prefix: &str, target_slug: &str) -> Result<PathBuf, String> {
    let log_dir = PathBuf::from("/tmp/crosshook-logs");
    std::fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();

    let file_name = format!("{prefix}-{target_slug}-{timestamp}.log");
    let log_path = log_dir.join(file_name);
    std::fs::File::create(&log_path).map_err(|error| error.to_string())?;
    Ok(log_path)
}

pub fn slugify_target(name: &str, fallback: &str) -> String {
    let slug: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}
