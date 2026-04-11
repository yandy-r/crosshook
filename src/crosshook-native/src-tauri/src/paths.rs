use std::path::{Path, PathBuf};

const HELPER_SCRIPTS_DIR: &str = "../runtime-helpers";

pub fn resolve_script_path(app: &tauri::AppHandle, script_name: &str) -> PathBuf {
    if let Some(bundled) = resolve_bundled_script_path(app, script_name) {
        tracing::debug!(path = %bundled.display(), script_name, "resolved bundled script");
        return bundled;
    }

    let dev_path = development_script_path(script_name);
    tracing::debug!(path = %dev_path.display(), script_name, "falling back to development script path");
    dev_path
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

    if let Ok(path) = app.path().resolve(script_name, BaseDirectory::Resource) {
        return Some(path);
    }

    // Flatpak fallback: Tauri's BaseDirectory::Resource resolution may fail
    // inside the Flatpak sandbox where bundled resources live at /app/resources/.
    // Only return Some when the file actually exists so a missing script
    // surfaces as a clear ENOENT at spawn time rather than a silently bogus path.
    // This branch is covered end-to-end by the Flatpak build smoke test under
    // #69; the inner is_flatpak() decision is unit-tested in crosshook-core.
    if crosshook_core::platform::is_flatpak() {
        let flatpak_path = PathBuf::from("/app/resources").join(script_name);
        if flatpak_path.exists() {
            tracing::debug!(
                path = %flatpak_path.display(),
                script_name,
                "resolved bundled script via Flatpak /app/resources fallback"
            );
            return Some(flatpak_path);
        }
    }

    None
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
