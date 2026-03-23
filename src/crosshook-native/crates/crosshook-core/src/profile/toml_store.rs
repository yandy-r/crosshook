use crate::profile::{legacy, GameProfile};
use directories::BaseDirs;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProfileStore {
    pub base_path: PathBuf,
}

#[derive(Debug)]
pub enum ProfileStoreError {
    InvalidName(String),
    NotFound(PathBuf),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl fmt::Display for ProfileStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(f, "invalid profile name: {name}"),
            Self::NotFound(path) => write!(f, "profile file not found: {}", path.display()),
            Self::Io(error) => write!(f, "{error}"),
            Self::TomlDe(error) => write!(f, "{error}"),
            Self::TomlSer(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProfileStoreError {}

impl From<std::io::Error> for ProfileStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ProfileStoreError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDe(value)
    }
}

impl From<toml::ser::Error> for ProfileStoreError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlSer(value)
    }
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileStore {
    pub fn try_new() -> Result<Self, String> {
        let base_path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .config_dir()
            .join("crosshook")
            .join("profiles");
        Ok(Self { base_path })
    }

    pub fn new() -> Self {
        Self::try_new().expect("home directory is required for CrossHook profile storage")
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn load(&self, name: &str) -> Result<GameProfile, ProfileStoreError> {
        let path = self.profile_path(name)?;
        if !path.exists() {
            return Err(ProfileStoreError::NotFound(path));
        }

        let content = fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, name: &str, profile: &GameProfile) -> Result<(), ProfileStoreError> {
        let path = self.profile_path(name)?;
        fs::create_dir_all(&self.base_path)?;
        fs::write(path, toml::to_string_pretty(profile)?)?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>, ProfileStoreError> {
        fs::create_dir_all(&self.base_path)?;

        let mut names = Vec::new();
        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|value| value.to_str()) != Some("toml") {
                continue;
            }

            if let Some(name) = path.file_stem().and_then(|value| value.to_str()) {
                names.push(name.to_string());
            }
        }

        names.sort_unstable();
        Ok(names)
    }

    pub fn delete(&self, name: &str) -> Result<(), ProfileStoreError> {
        let path = self.profile_path(name)?;
        if !path.exists() {
            return Err(ProfileStoreError::NotFound(path));
        }

        fs::remove_file(path)?;
        Ok(())
    }

    pub fn import_legacy(&self, legacy_path: &Path) -> Result<GameProfile, ProfileStoreError> {
        let profile_name = legacy_path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ProfileStoreError::InvalidName(legacy_path.display().to_string()))?;

        validate_name(profile_name)?;
        let legacy_profile = legacy::load(
            legacy_path.parent().unwrap_or_else(|| Path::new("")),
            profile_name,
        )?;
        let profile = GameProfile::from(legacy_profile);
        self.save(profile_name, &profile)?;
        Ok(profile)
    }

    fn profile_path(&self, name: &str) -> Result<PathBuf, ProfileStoreError> {
        validate_name(name)?;
        Ok(self.base_path.join(format!("{name}.toml")))
    }
}

pub fn validate_name(name: &str) -> Result<(), ProfileStoreError> {
    const WINDOWS_RESERVED_PATH_CHARACTERS: [char; 9] =
        ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err(ProfileStoreError::InvalidName(name.to_string()));
    }

    if Path::new(trimmed).is_absolute()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
    {
        return Err(ProfileStoreError::InvalidName(name.to_string()));
    }

    if trimmed
        .chars()
        .any(|character| WINDOWS_RESERVED_PATH_CHARACTERS.contains(&character))
    {
        return Err(ProfileStoreError::InvalidName(name.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_profile() -> GameProfile {
        GameProfile {
            game: crate::profile::GameSection {
                name: "Elden Ring".to_string(),
                executable_path: "/games/elden-ring/eldenring.exe".to_string(),
            },
            trainer: crate::profile::TrainerSection {
                path: "/trainers/elden-ring.exe".to_string(),
                kind: "fling".to_string(),
            },
            injection: crate::profile::InjectionSection {
                dll_paths: vec!["/dlls/a.dll".to_string(), "/dlls/b.dll".to_string()],
                inject_on_launch: vec![true, false],
            },
            steam: crate::profile::SteamSection {
                enabled: true,
                app_id: "1245620".to_string(),
                compatdata_path: "/steam/compatdata/1245620".to_string(),
                proton_path: "/steam/proton/proton".to_string(),
                launcher: crate::profile::LauncherSection {
                    icon_path: "/icons/elden-ring.png".to_string(),
                    display_name: "Elden Ring".to_string(),
                },
            },
            runtime: crate::profile::RuntimeSection {
                prefix_path: String::new(),
                proton_path: String::new(),
                working_directory: String::new(),
            },
            launch: crate::profile::LaunchSection {
                method: "steam_applaunch".to_string(),
            },
        }
    }

    #[test]
    fn save_load_list_and_delete_round_trip() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("elden-ring", &profile).unwrap();
        assert_eq!(store.list().unwrap(), vec!["elden-ring".to_string()]);
        assert_eq!(store.load("elden-ring").unwrap(), profile);

        store.delete("elden-ring").unwrap();
        assert!(store.load("elden-ring").is_err());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn import_legacy_converts_windows_paths_and_saves_toml() {
        let temp_dir = tempdir().unwrap();
        let legacy_path = temp_dir.path().join("elden-ring.profile");
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

        std::fs::write(
            &legacy_path,
            "GamePath=Z:\\games\\elden-ring\\eldenring.exe\nTrainerPath=Z:/trainers/elden-ring.exe\nDll1Path=\nDll2Path=\nLaunchInject1=True\nLaunchInject2=false\nLaunchMethod=proton_run\nUseSteamMode=True\nSteamAppId=1245620\nSteamCompatDataPath=Z:\\steam\\compatdata\\1245620\nSteamProtonPath=Z:/steam/proton/proton\nSteamLauncherIconPath=Z:\\icons\\elden-ring.png\n",
        )
        .unwrap();

        let imported = store.import_legacy(&legacy_path).unwrap();
        assert_eq!(
            imported.game.executable_path,
            "/games/elden-ring/eldenring.exe"
        );
        assert_eq!(imported.trainer.path, "/trainers/elden-ring.exe");
        assert_eq!(imported.steam.compatdata_path, "/steam/compatdata/1245620");
        assert_eq!(imported.steam.launcher.icon_path, "/icons/elden-ring.png");
        assert_eq!(imported.launch.method, "steam_applaunch");
        assert!(store.base_path.join("elden-ring.toml").exists());
    }

    #[test]
    fn load_defaults_runtime_when_runtime_section_is_missing() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile_path = store.base_path.join("legacy.toml");

        fs::create_dir_all(&store.base_path).unwrap();
        fs::write(
            &profile_path,
            r#"[game]
name = "Legacy"
executable_path = "/games/legacy.sh"

[trainer]
path = "/trainers/legacy"
type = "native"

[injection]
dll_paths = []
inject_on_launch = [false, false]

[steam]
enabled = false
app_id = ""
compatdata_path = ""
proton_path = ""

[steam.launcher]
icon_path = ""
display_name = ""

[launch]
method = "native"
"#,
        )
        .unwrap();

        let loaded = store.load("legacy").unwrap();
        assert!(loaded.runtime.is_empty());
        assert_eq!(loaded.launch.method, "native");
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
