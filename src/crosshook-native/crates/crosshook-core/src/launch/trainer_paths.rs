use std::path::Path;

/// Returns the Windows-style staged trainer path used by copy-to-prefix mode,
/// or `None` if the host path is empty or malformed.
pub(crate) fn build_staged_trainer_path(trainer_host_path: &str) -> Option<String> {
    let trimmed = trainer_host_path.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.ends_with(['/', '\\']) {
        return None;
    }

    let path = Path::new(trimmed);
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let name = path.file_name()?.to_string_lossy().into_owned();
    if stem.is_empty() || name.is_empty() {
        return None;
    }

    Some(format!("C:\\CrossHook\\StagedTrainers\\{stem}\\{name}"))
}

#[cfg(test)]
mod tests {
    use super::build_staged_trainer_path;

    #[test]
    fn builds_staged_path_for_executable() {
        let path = "/home/user/trainers/MyTrainer.exe";
        let staged = build_staged_trainer_path(path);

        assert_eq!(
            staged.as_deref(),
            Some("C:\\CrossHook\\StagedTrainers\\MyTrainer\\MyTrainer.exe")
        );
    }

    #[test]
    fn returns_none_for_empty_input() {
        assert!(build_staged_trainer_path("   ").is_none());
    }

    #[test]
    fn supports_paths_without_extension() {
        let path = "/home/user/trainers/MyTrainer";
        let staged = build_staged_trainer_path(path);

        assert_eq!(
            staged.as_deref(),
            Some("C:\\CrossHook\\StagedTrainers\\MyTrainer\\MyTrainer")
        );
    }

    #[test]
    fn returns_none_for_trailing_separator() {
        assert!(build_staged_trainer_path("/home/user/trainers/").is_none());
    }
}
