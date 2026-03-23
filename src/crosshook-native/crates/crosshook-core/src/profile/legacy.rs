use crate::profile::LegacyProfileData;
use std::fs;
use std::io::{self, Write};
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
            "SteamCompatDataPath" => data.steam_compat_data_path = normalize_legacy_windows_path(value),
            "SteamProtonPath" => data.steam_proton_path = normalize_legacy_windows_path(value),
            "SteamLauncherIconPath" => data.steam_launcher_icon_path = normalize_legacy_windows_path(value),
            _ => {}
        }
    }

    Ok(data)
}

pub fn save(profiles_dir: &Path, name: &str, data: &LegacyProfileData) -> io::Result<()> {
    validate_name(name)?;
    fs::create_dir_all(profiles_dir)?;

    let mut file = fs::File::create(profile_path_unchecked(profiles_dir, name))?;
    writeln!(file, "GamePath={}", data.game_path)?;
    writeln!(file, "TrainerPath={}", data.trainer_path)?;
    writeln!(file, "Dll1Path={}", data.dll1_path)?;
    writeln!(file, "Dll2Path={}", data.dll2_path)?;
    writeln!(file, "LaunchInject1={}", data.launch_inject1)?;
    writeln!(file, "LaunchInject2={}", data.launch_inject2)?;
    writeln!(file, "LaunchMethod={}", data.launch_method)?;
    writeln!(file, "UseSteamMode={}", data.use_steam_mode)?;
    writeln!(file, "SteamAppId={}", data.steam_app_id)?;
    writeln!(file, "SteamCompatDataPath={}", data.steam_compat_data_path)?;
    writeln!(file, "SteamProtonPath={}", data.steam_proton_path)?;
    writeln!(file, "SteamLauncherIconPath={}", data.steam_launcher_icon_path)?;
    Ok(())
}

pub fn list(profiles_dir: &Path) -> io::Result<Vec<String>> {
    if !profiles_dir.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in fs::read_dir(profiles_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some(PROFILE_EXTENSION) {
            continue;
        }

        if let Some(name) = path.file_stem().and_then(|value| value.to_str()) {
            names.push(name.to_string());
        }
    }

    names.sort();
    Ok(names)
}

pub fn delete(profiles_dir: &Path, name: &str) -> io::Result<()> {
    let path = profile_path(profiles_dir, name)?;
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "profile file not found",
        ));
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn validate_name(name: &str) -> io::Result<()> {
    const WINDOWS_RESERVED_PATH_CHARACTERS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

    if name.trim().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "profile name cannot be empty"));
    }

    if name == "." || name == ".." {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "profile name cannot be a relative path segment",
        ));
    }

    if Path::new(name).is_absolute() || name.contains('/') || name.contains('\\') || name.contains(':') {
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

    fn sample_profile() -> LegacyProfileData {
        LegacyProfileData {
            game_path: "/games/example.exe".to_string(),
            trainer_path: "/trainers/example.exe".to_string(),
            dll1_path: "/dlls/one.dll".to_string(),
            dll2_path: "/dlls/two.dll".to_string(),
            launch_inject1: true,
            launch_inject2: false,
            launch_method: "proton_run".to_string(),
            use_steam_mode: true,
            steam_app_id: "12345".to_string(),
            steam_compat_data_path: "/compat/12345".to_string(),
            steam_proton_path: "/proton/proton".to_string(),
            steam_launcher_icon_path: "/icons/example.png".to_string(),
        }
    }

    #[test]
    fn save_and_load_round_trip() {
        let temp_dir = tempdir().unwrap();
        let profile = sample_profile();

        save(temp_dir.path(), "example", &profile).unwrap();

        let loaded = load(temp_dir.path(), "example").unwrap();
        assert_eq!(loaded, profile);
    }

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
    fn list_and_delete_profiles() {
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("b.profile"), "").unwrap();
        fs::write(temp_dir.path().join("a.profile"), "").unwrap();
        fs::write(temp_dir.path().join("ignore.txt"), "").unwrap();

        assert_eq!(list(temp_dir.path()).unwrap(), vec!["a".to_string(), "b".to_string()]);

        delete(temp_dir.path(), "a").unwrap();
        assert!(!temp_dir.path().join("a.profile").exists());
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
}
