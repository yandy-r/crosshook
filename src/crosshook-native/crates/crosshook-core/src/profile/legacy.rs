use crate::launch::ANONYMOUS_PROFILE_KEY;
use crate::profile::LegacyProfileData;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PROFILE_EXTENSION: &str = "profile";

pub fn load(profiles_dir: &Path, name: &str) -> io::Result<LegacyProfileData> {
    let path = profile_path(profiles_dir, name)?;
    let content = fs::read_to_string(&path)?;
    let mut data = LegacyProfileData::default();

    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        match key {
            "GamePath" => data.game_path = normalize_legacy_windows_path(value),
            "TrainerPath" => data.trainer_path = normalize_legacy_windows_path(value),
            "Dll1Path" => data.dll1_path = normalize_legacy_windows_path(value),
            "Dll2Path" => data.dll2_path = normalize_legacy_windows_path(value),
            "LaunchInject1" => {
                if let Some(parsed) = parse_bool(value) {
                    data.launch_inject1 = parsed;
                }
            }
            "LaunchInject2" => {
                if let Some(parsed) = parse_bool(value) {
                    data.launch_inject2 = parsed;
                }
            }
            "LaunchMethod" => data.launch_method = value.to_string(),
            "UseSteamMode" => {
                if let Some(parsed) = parse_bool(value) {
                    data.use_steam_mode = parsed;
                }
            }
            "SteamAppId" => data.steam_app_id = value.trim().to_string(),
            "SteamCompatDataPath" => {
                data.steam_compat_data_path = normalize_legacy_windows_path(value);
            }
            "SteamProtonPath" => data.steam_proton_path = normalize_legacy_windows_path(value),
            "SteamLauncherIconPath" => {
                data.steam_launcher_icon_path = normalize_legacy_windows_path(value);
            }
            _ => {}
        }
    }

    Ok(data)
}

pub fn validate_name(name: &str) -> io::Result<()> {
    const WINDOWS_RESERVED_PATH_CHARACTERS: &[char] =
        &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

    if name.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile name cannot be empty",
        ));
    }

    if name.trim() == ANONYMOUS_PROFILE_KEY {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile name is reserved for anonymous launch sessions",
        ));
    }

    if name == "." || name == ".." {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile name cannot be a relative path segment",
        ));
    }

    if Path::new(name).is_absolute()
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile name cannot contain path separators or rooted paths",
        ));
    }

    if name.chars().any(|character| {
        WINDOWS_RESERVED_PATH_CHARACTERS.contains(&character)
            || std::path::MAIN_SEPARATOR == character
            || std::path::is_separator(character)
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile name contains invalid characters",
        ));
    }

    Ok(())
}

fn profile_path(profiles_dir: &Path, name: &str) -> io::Result<PathBuf> {
    validate_name(name)?;
    Ok(profile_path_unchecked(profiles_dir, name))
}

fn profile_path_unchecked(profiles_dir: &Path, name: &str) -> PathBuf {
    profiles_dir.join(format!("{name}.{PROFILE_EXTENSION}"))
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn normalize_legacy_windows_path(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed[..2].eq_ignore_ascii_case("z:") {
        let remainder = trimmed[2..].trim_start_matches(['\\', '/']);
        if remainder.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", remainder.replace('\\', "/"))
        }
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_normalizes_legacy_z_paths_and_case_insensitive_bools() {
        let temp_dir = tempdir().unwrap();
        fs::write(
            temp_dir.path().join("legacy.profile"),
            "GamePath=Z:\\games\\example.exe\nTrainerPath=Z:/trainers/example.exe\nDll1Path=/dlls/one.dll\nDll2Path=/dlls/two.dll\nLaunchInject1=TRUE\nLaunchInject2=false\nLaunchMethod=direct\nUseSteamMode=TrUe\nSteamAppId=Z:\\12345\nSteamCompatDataPath=Z:/compat/12345\nSteamProtonPath=Z:\\proton\\proton\nSteamLauncherIconPath=Z:/icons/example.png\n",
        )
        .unwrap();

        let loaded = load(temp_dir.path(), "legacy").unwrap();
        assert_eq!(loaded.game_path, "/games/example.exe");
        assert_eq!(loaded.trainer_path, "/trainers/example.exe");
        assert_eq!(loaded.steam_app_id, "Z:\\12345");
        assert!(loaded.launch_inject1);
        assert!(loaded.use_steam_mode);
        assert!(!loaded.launch_inject2);
    }

    #[test]
    fn validate_name_rejects_invalid_names() {
        assert!(validate_name("").is_err());
        assert!(validate_name(".").is_err());
        assert!(validate_name("..").is_err());
        assert!(validate_name("foo/bar").is_err());
        assert!(validate_name("foo\\bar").is_err());
        assert!(validate_name("foo:bar").is_err());
    }

    #[test]
    fn validate_name_rejects_the_anonymous_session_key_sentinel() {
        // A profile literally named the anonymous session key would collide
        // with `launch::session::ANONYMOUS_PROFILE_KEY`, letting
        // `launch_reset_sessions` cancel anonymous sessions it does not own.
        assert!(validate_name(ANONYMOUS_PROFILE_KEY).is_err());
        // Surrounding whitespace must not evade the reservation — validation
        // trims the same way `session_profile_key_for_name` does.
        assert!(validate_name(&format!("  {ANONYMOUS_PROFILE_KEY}  ")).is_err());
    }
}
