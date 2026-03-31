//! MangoHud companion config file generation for CrossHook profiles.
//!
//! Each profile can carry an optional MangoHud overlay configuration. When a profile is
//! saved, this module writes a `<profile_name>.mangohud.conf` file alongside the TOML.
//! Deleting or renaming a profile also removes/renames its companion config file.

use std::io;
use std::path::{Path, PathBuf};

use super::models::{MangoHudConfig, MangoHudPosition};

fn position_to_str(pos: &MangoHudPosition) -> &'static str {
    match pos {
        MangoHudPosition::TopLeft => "top-left",
        MangoHudPosition::TopRight => "top-right",
        MangoHudPosition::BottomLeft => "bottom-left",
        MangoHudPosition::BottomRight => "bottom-right",
        MangoHudPosition::TopCenter => "top-center",
        MangoHudPosition::BottomCenter => "bottom-center",
    }
}

/// Renders a MangoHud `.conf` file body from a [`MangoHudConfig`].
///
/// Returns an empty string when `config.enabled` is `false`.
///
/// MangoHud's config format uses bare keys for boolean options (presence = enabled;
/// absence = disabled). Numeric and string options use `key=value` syntax.
pub fn render_mangohud_conf(config: &MangoHudConfig) -> String {
    if !config.enabled {
        return String::new();
    }

    let mut out = String::new();

    if config.gpu_stats {
        out.push_str("gpu_stats\n");
    }
    if config.cpu_stats {
        out.push_str("cpu_stats\n");
    }
    if config.ram {
        out.push_str("ram\n");
    }
    if config.frametime {
        out.push_str("frametime\n");
    }
    if config.battery {
        out.push_str("battery\n");
    }
    if config.watt {
        out.push_str("watt\n");
    }

    if let Some(limit) = config.fps_limit {
        if limit > 0 {
            out.push_str(&format!("fps_limit={limit}\n"));
        }
    }

    if let Some(ref pos) = config.position {
        out.push_str(&format!("position={}\n", position_to_str(pos)));
    }

    out
}

/// Returns the path for a profile's MangoHud companion config file.
///
/// The companion file lives alongside the profile TOML as
/// `<base_path>/<profile_name>.mangohud.conf`.
pub fn mangohud_conf_path(base_path: &Path, profile_name: &str) -> PathBuf {
    base_path.join(format!("{profile_name}.mangohud.conf"))
}

/// Writes or removes the MangoHud companion config file for a profile.
///
/// - When `config.enabled` is `true`: renders and writes the config, returning `Some(path)`.
/// - When `config.enabled` is `false`: removes the file if it exists (ignores `NotFound`),
///   returning `None`.
pub fn write_mangohud_conf(
    base_path: &Path,
    profile_name: &str,
    config: &MangoHudConfig,
) -> io::Result<Option<PathBuf>> {
    let path = mangohud_conf_path(base_path, profile_name);

    if config.enabled {
        let content = render_mangohud_conf(config);
        std::fs::write(&path, content)?;
        Ok(Some(path))
    } else {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::models::MangoHudPosition;

    fn enabled_config() -> MangoHudConfig {
        MangoHudConfig {
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn render_returns_empty_when_disabled() {
        let config = MangoHudConfig::default();
        assert_eq!(render_mangohud_conf(&config), "");
    }

    #[test]
    fn render_omits_false_booleans() {
        let config = enabled_config();
        let result = render_mangohud_conf(&config);
        // No boolean keys should appear since all are false
        assert!(!result.contains("gpu_stats"));
        assert!(!result.contains("cpu_stats"));
        assert!(!result.contains("ram"));
        assert!(!result.contains("frametime"));
        assert!(!result.contains("battery"));
        assert!(!result.contains("watt"));
    }

    #[test]
    fn render_includes_bare_keys_for_true_booleans() {
        let config = MangoHudConfig {
            enabled: true,
            gpu_stats: true,
            cpu_stats: true,
            ram: true,
            frametime: true,
            battery: true,
            watt: true,
            ..Default::default()
        };
        let result = render_mangohud_conf(&config);
        assert!(result.contains("gpu_stats\n"));
        assert!(result.contains("cpu_stats\n"));
        assert!(result.contains("ram\n"));
        assert!(result.contains("frametime\n"));
        assert!(result.contains("battery\n"));
        assert!(result.contains("watt\n"));
        // Must NOT use key=true syntax
        assert!(!result.contains("gpu_stats=true"));
    }

    #[test]
    fn render_includes_fps_limit_when_nonzero() {
        let config = MangoHudConfig {
            enabled: true,
            fps_limit: Some(60),
            ..Default::default()
        };
        let result = render_mangohud_conf(&config);
        assert!(result.contains("fps_limit=60\n"));
    }

    #[test]
    fn render_omits_fps_limit_when_zero() {
        let config = MangoHudConfig {
            enabled: true,
            fps_limit: Some(0),
            ..Default::default()
        };
        let result = render_mangohud_conf(&config);
        assert!(!result.contains("fps_limit"));
    }

    #[test]
    fn render_omits_fps_limit_when_none() {
        let config = enabled_config();
        let result = render_mangohud_conf(&config);
        assert!(!result.contains("fps_limit"));
    }

    #[test]
    fn render_includes_position_as_kebab_case() {
        let config = MangoHudConfig {
            enabled: true,
            position: Some(MangoHudPosition::TopLeft),
            ..Default::default()
        };
        assert!(render_mangohud_conf(&config).contains("position=top-left\n"));

        let config2 = MangoHudConfig {
            enabled: true,
            position: Some(MangoHudPosition::BottomRight),
            ..Default::default()
        };
        assert!(render_mangohud_conf(&config2).contains("position=bottom-right\n"));
    }

    #[test]
    fn render_omits_position_when_none() {
        let config = enabled_config();
        assert!(!render_mangohud_conf(&config).contains("position"));
    }

    #[test]
    fn mangohud_conf_path_uses_expected_filename() {
        let base = Path::new("/home/user/.config/crosshook/profiles");
        let path = mangohud_conf_path(base, "MyGame");
        assert_eq!(
            path,
            base.join("MyGame.mangohud.conf")
        );
    }

    #[test]
    fn write_mangohud_conf_creates_file_when_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let config = MangoHudConfig {
            enabled: true,
            gpu_stats: true,
            fps_limit: Some(120),
            position: Some(MangoHudPosition::TopRight),
            ..Default::default()
        };

        let result = write_mangohud_conf(dir.path(), "TestProfile", &config).unwrap();
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("gpu_stats\n"));
        assert!(content.contains("fps_limit=120\n"));
        assert!(content.contains("position=top-right\n"));
    }

    #[test]
    fn write_mangohud_conf_removes_file_when_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("TestProfile.mangohud.conf");
        std::fs::write(&path, "gpu_stats\n").unwrap();
        assert!(path.exists());

        let config = MangoHudConfig::default(); // disabled
        let result = write_mangohud_conf(dir.path(), "TestProfile", &config).unwrap();
        assert!(result.is_none());
        assert!(!path.exists());
    }

    #[test]
    fn write_mangohud_conf_disabled_tolerates_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = MangoHudConfig::default(); // disabled, no file exists
        let result = write_mangohud_conf(dir.path(), "NoSuchProfile", &config);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
