use std::fs;
use std::path::{Path, PathBuf};

use crate::launch::runtime_helpers::resolve_wine_prefix_path;
use crate::launch::trainer_paths::build_staged_trainer_path;

const STAGED_TRAINER_ROOT: &str = "CrossHook/StagedTrainers";
const SUPPORT_DIRECTORIES: [&str; 9] = [
    "assets",
    "data",
    "lib",
    "bin",
    "runtimes",
    "plugins",
    "locales",
    "cef",
    "resources",
];
const SHARED_DEPENDENCY_EXTENSIONS: [&str; 7] =
    ["dll", "json", "config", "ini", "pak", "dat", "bin"];

pub(super) fn stage_trainer_into_prefix(
    prefix_path: &Path,
    trainer_host_path: &Path,
) -> std::io::Result<String> {
    let trainer_file_name = trainer_host_path
        .file_name()
        .ok_or_else(|| io_error("trainer host path is missing a file name"))?;
    let trainer_base_name = trainer_host_path
        .file_stem()
        .ok_or_else(|| io_error("trainer host path is missing a file stem"))?;
    let trainer_source_dir = trainer_host_path
        .parent()
        .ok_or_else(|| io_error("trainer host path is missing a parent directory"))?;

    let wine_prefix_path = resolve_wine_prefix_path(prefix_path);
    let staged_root = wine_prefix_path
        .join("drive_c")
        .join(PathBuf::from(STAGED_TRAINER_ROOT));
    let staged_directory = staged_root.join(trainer_base_name);
    let staged_host_path = staged_directory.join(trainer_file_name);

    if staged_directory.exists() {
        fs::remove_dir_all(&staged_directory)?;
    }

    fs::create_dir_all(&staged_directory)?;
    fs::copy(trainer_host_path, &staged_host_path)?;
    stage_trainer_support_files(trainer_source_dir, &staged_directory, trainer_file_name)?;

    build_staged_trainer_path(&trainer_host_path.to_string_lossy())
        .ok_or_else(|| io_error("trainer host path cannot be staged"))
}

fn stage_trainer_support_files(
    trainer_source_dir: &Path,
    staged_target_dir: &Path,
    trainer_file_name: &std::ffi::OsStr,
) -> std::io::Result<()> {
    for entry in fs::read_dir(trainer_source_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();

        if file_name == trainer_file_name {
            continue;
        }

        if path.is_file() && should_stage_support_file(&file_name) {
            fs::copy(&path, staged_target_dir.join(&file_name))?;
        }
    }

    for directory in SUPPORT_DIRECTORIES {
        let source = trainer_source_dir.join(directory);
        if source.is_dir() {
            copy_dir_all(&source, &staged_target_dir.join(directory))?;
        }
    }

    Ok(())
}

fn should_stage_support_file(file_name: &std::ffi::OsStr) -> bool {
    let file_name = file_name.to_string_lossy();
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();

    SHARED_DEPENDENCY_EXTENSIONS.contains(&extension.as_str())
}

fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_symlink() {
            tracing::debug!(path = %source_path.display(), "skipping symlink during trainer staging");
            continue;
        }

        if source_path.is_dir() {
            copy_dir_all(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

fn io_error(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    #[cfg(unix)]
    fn copy_dir_all_skips_symlinks() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let destination_dir = temp_dir.path().join("destination");

        fs::create_dir_all(&source_dir).expect("source dir");
        fs::write(source_dir.join("real.dll"), b"content").expect("real file");

        let external_target = temp_dir.path().join("external_target.dll");
        fs::write(&external_target, b"external").expect("external file");
        symlink(&external_target, source_dir.join("link.dll")).expect("symlink");

        super::copy_dir_all(&source_dir, &destination_dir).expect("copy_dir_all");

        assert!(
            destination_dir.join("real.dll").exists(),
            "regular file should be copied"
        );
        assert!(
            !destination_dir.join("link.dll").exists(),
            "symlink should be skipped"
        );
    }
}
