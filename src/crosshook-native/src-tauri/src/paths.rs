use std::path::{Path, PathBuf};

const HELPER_SCRIPTS_DIR: &str = "../runtime-helpers";

pub fn resolve_script_path(app: &tauri::AppHandle, script_name: &str) -> PathBuf {
    resolve_bundled_script_path(app, script_name)
        .unwrap_or_else(|| development_script_path(script_name))
}

pub fn ensure_development_scripts_executable() -> std::io::Result<()> {
    for script_name in [
        "steam-launch-helper.sh",
        "steam-launch-trainer.sh",
        "steam-host-trainer-runner.sh",
    ] {
        ensure_executable(&development_script_path(script_name))?;
    }

    Ok(())
}

fn resolve_bundled_script_path(app: &tauri::AppHandle, script_name: &str) -> Option<PathBuf> {
    use tauri::path::BaseDirectory;
    use tauri::Manager;

    app.path()
        .resolve(script_name, BaseDirectory::Resource)
        .ok()
}

fn development_script_path(script_name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(HELPER_SCRIPTS_DIR)
        .join(script_name)
}

#[cfg(unix)]
fn ensure_executable(path: &Path) -> std::io::Result<()> {
    use std::fs;
    use std::io::ErrorKind;
    use std::os::unix::fs::PermissionsExt;

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut permissions = metadata.permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn ensure_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}
