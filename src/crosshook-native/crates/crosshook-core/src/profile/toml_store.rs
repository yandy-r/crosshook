use super::models::{LaunchOptimizationsSection, LocalOverrideSection};
use crate::launch::is_known_launch_optimization_id;
use crate::profile::{legacy, GameProfile};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
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
    AlreadyExists(String),
    InvalidLaunchOptimizationId(String),
    LaunchPresetNotFound(String),
    ReservedLaunchPresetName(String),
    InvalidLaunchPresetName(String),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl fmt::Display for ProfileStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(f, "invalid profile name: {name}"),
            Self::NotFound(path) => write!(f, "profile file not found: {}", path.display()),
            Self::AlreadyExists(name) => {
                write!(f, "a profile named '{name}' already exists")
            }
            Self::InvalidLaunchOptimizationId(id) => {
                write!(f, "unknown launch optimization id: {id}")
            }
            Self::LaunchPresetNotFound(name) => {
                write!(f, "launch optimization preset not found: {name}")
            }
            Self::ReservedLaunchPresetName(name) => {
                write!(
                    f,
                    "preset name is reserved for bundled presets (must not start with 'bundled/'): {name}"
                )
            }
            Self::InvalidLaunchPresetName(msg) => write!(f, "{msg}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::TomlDe(error) => write!(f, "{error}"),
            Self::TomlSer(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProfileStoreError {}

/// TOML key under `[launch.presets]` for a bundled catalog preset (`bundled/<preset_id>`).
pub fn bundled_optimization_preset_toml_key(preset_id: &str) -> String {
    format!("bundled/{}", preset_id.trim())
}

fn validate_manual_launch_preset_name(raw: &str) -> Result<String, ProfileStoreError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(ProfileStoreError::InvalidLaunchPresetName(
            "preset name must not be empty".to_string(),
        ));
    }
    if name.starts_with("bundled/") {
        return Err(ProfileStoreError::ReservedLaunchPresetName(name.to_string()));
    }
    Ok(name.to_string())
}

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

/// Result of a successful profile duplication, returned across the Tauri IPC boundary.
///
/// The frontend receives this as the TypeScript `DuplicateProfileResult` interface
/// (see `src/types/profile.ts`) and uses `name` to navigate to the newly created profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateProfileResult {
    /// The generated name for the duplicate profile (e.g. "MyGame (Copy)" or "MyGame (Copy 2)").
    pub name: String,
    /// A byte-for-byte clone of the source profile's `GameProfile` data.
    pub profile: GameProfile,
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
        let profile: GameProfile = toml::from_str(&content)?;
        let mut effective = profile.effective_profile();
        effective.local_override = LocalOverrideSection::default();
        effective.launch.normalize_preset_selection();
        Ok(effective)
    }

    pub fn save(&self, name: &str, profile: &GameProfile) -> Result<(), ProfileStoreError> {
        let path = self.profile_path(name)?;
        fs::create_dir_all(&self.base_path)?;
        let storage_profile = profile.storage_profile();
        fs::write(path, toml::to_string_pretty(&storage_profile)?)?;
        Ok(())
    }

    /// Loads the profile, replaces launch optimizations, and saves. Concurrent `save` or
    /// `save_launch_optimizations` calls for the same profile are not synchronized; the last
    /// completed write wins.
    pub fn save_launch_optimizations(
        &self,
        name: &str,
        enabled_option_ids: Vec<String>,
        switch_active_preset: Option<String>,
    ) -> Result<(), ProfileStoreError> {
        let mut profile = self.load(name)?;

        if let Some(raw) = switch_active_preset {
            let key = raw.trim();
            if key.is_empty() {
                return Err(ProfileStoreError::LaunchPresetNotFound(raw));
            }

            let section = profile
                .launch
                .presets
                .get(key)
                .ok_or_else(|| ProfileStoreError::LaunchPresetNotFound(key.to_string()))?
                .clone();

            for id in &section.enabled_option_ids {
                if !is_known_launch_optimization_id(id) {
                    return Err(ProfileStoreError::InvalidLaunchOptimizationId(id.clone()));
                }
            }

            profile.launch.active_preset = key.to_string();
            profile.launch.optimizations = section;
        } else {
            let enabled_option_ids: Vec<String> = enabled_option_ids
                .into_iter()
                .filter_map(|raw| {
                    let id = raw.trim();
                    (!id.is_empty()).then(|| id.to_string())
                })
                .collect();

            for id in &enabled_option_ids {
                if !is_known_launch_optimization_id(id) {
                    return Err(ProfileStoreError::InvalidLaunchOptimizationId(id.clone()));
                }
            }

            profile.launch.optimizations = LaunchOptimizationsSection { enabled_option_ids };

            let ap = profile.launch.active_preset.trim();
            if !ap.is_empty() {
                if let Some(slot) = profile.launch.presets.get_mut(ap) {
                    slot.enabled_option_ids = profile.launch.optimizations.enabled_option_ids.clone();
                }
            }
        }

        self.save(name, &profile)
    }

    /// Writes or overwrites `launch.presets.<preset_key>` and optionally sets it as the active preset.
    /// All `enabled_option_ids` must be known catalog ids.
    pub fn materialize_launch_optimization_preset(
        &self,
        profile_name: &str,
        preset_key: &str,
        enabled_option_ids: Vec<String>,
        set_as_active: bool,
    ) -> Result<(), ProfileStoreError> {
        let key = preset_key.trim();
        if key.is_empty() {
            return Err(ProfileStoreError::LaunchPresetNotFound(preset_key.to_string()));
        }

        let enabled_option_ids: Vec<String> = enabled_option_ids
            .into_iter()
            .filter_map(|raw| {
                let id = raw.trim();
                (!id.is_empty()).then(|| id.to_string())
            })
            .collect();

        for id in &enabled_option_ids {
            if !is_known_launch_optimization_id(id) {
                return Err(ProfileStoreError::InvalidLaunchOptimizationId(id.clone()));
            }
        }

        let mut profile = self.load(profile_name)?;
        profile
            .launch
            .presets
            .insert(key.to_string(), LaunchOptimizationsSection {
                enabled_option_ids: enabled_option_ids.clone(),
            });

        if set_as_active {
            profile.launch.active_preset = key.to_string();
            profile.launch.optimizations = LaunchOptimizationsSection { enabled_option_ids };
        }

        self.save(profile_name, &profile)
    }

    /// Saves the current optimization selection under a new user preset name and activates it.
    pub fn save_manual_launch_optimization_preset(
        &self,
        profile_name: &str,
        preset_display_name: &str,
        enabled_option_ids: Vec<String>,
    ) -> Result<(), ProfileStoreError> {
        let name = validate_manual_launch_preset_name(preset_display_name)?;
        self.materialize_launch_optimization_preset(
            profile_name,
            &name,
            enabled_option_ids,
            true,
        )
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

    pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
        let old_name = old_name.trim();
        let new_name = new_name.trim();
        validate_name(old_name)?;
        validate_name(new_name)?;
        let old_path = self.profile_path(old_name)?;
        let new_path = self.profile_path(new_name)?;
        if !old_path.exists() {
            return Err(ProfileStoreError::NotFound(old_path));
        }
        if old_name == new_name {
            return Ok(()); // no-op
        }
        if new_path.exists() {
            return Err(ProfileStoreError::AlreadyExists(new_name.to_string()));
        }
        fs::rename(&old_path, &new_path)?;
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

    /// Duplicates an existing profile under a new, unique copy name.
    ///
    /// Loads the source profile, generates a collision-free name via
    /// [`generate_unique_copy_name`](Self::generate_unique_copy_name), and saves the cloned
    /// `GameProfile` to disk. The source profile is never modified.
    ///
    /// # Safety constraints
    /// - The generated name is always validated through [`validate_name`] before saving,
    ///   preventing path traversal or filesystem-unsafe characters.
    /// - `save()` will overwrite an existing file if one exists at the target path, but
    ///   `generate_unique_copy_name` ensures the name is not already present in the store,
    ///   so overwrites cannot occur under normal operation.
    ///
    /// # Errors
    /// - `ProfileStoreError::InvalidName` if `source_name` fails validation.
    /// - `ProfileStoreError::NotFound` if no profile file exists for `source_name`.
    /// - `ProfileStoreError::Io` or `ProfileStoreError::TomlSer` on filesystem/serialization failure.
    pub fn duplicate(
        &self,
        source_name: &str,
    ) -> Result<DuplicateProfileResult, ProfileStoreError> {
        validate_name(source_name)?;
        let profile = self.load(source_name)?;
        let existing_names = self.list()?;
        let new_name = Self::generate_unique_copy_name(source_name, &existing_names)?;
        self.save(&new_name, &profile)?;
        Ok(DuplicateProfileResult {
            name: new_name,
            profile,
        })
    }

    /// Generates a unique copy name that does not collide with any existing profile.
    ///
    /// # Algorithm
    /// 1. Strip any existing `(Copy)` or `(Copy N)` suffix from `source_name` via
    ///    [`strip_copy_suffix`] to recover the original base name. If stripping produces
    ///    an empty string (e.g. source is literally `"(Copy)"`), the full source name is
    ///    used as the base to guarantee a non-empty result.
    /// 2. Try `"{base} (Copy)"` first.
    /// 3. If that collides, iterate `"{base} (Copy 2)"` through `"{base} (Copy 1000)"`.
    /// 4. If all 1000 candidates collide, return `InvalidName`.
    ///
    /// This means duplicating `"MyGame (Copy)"` produces `"MyGame (Copy 2)"` rather than
    /// `"MyGame (Copy) (Copy)"`, keeping names clean across repeated duplications.
    fn generate_unique_copy_name(
        source_name: &str,
        existing_names: &[String],
    ) -> Result<String, ProfileStoreError> {
        let stripped_base = strip_copy_suffix(source_name);
        let base = if stripped_base.is_empty() {
            source_name.trim()
        } else {
            stripped_base
        };
        let candidate = format!("{base} (Copy)");
        if !existing_names.iter().any(|n| n == &candidate) {
            validate_name(&candidate)?;
            return Ok(candidate);
        }

        for i in 2..=1000 {
            let candidate = format!("{base} (Copy {i})");
            if !existing_names.iter().any(|n| n == &candidate) {
                validate_name(&candidate)?;
                return Ok(candidate);
            }
        }

        Err(ProfileStoreError::InvalidName(format!(
            "cannot generate unique copy name for '{source_name}'"
        )))
    }

    fn profile_path(&self, name: &str) -> Result<PathBuf, ProfileStoreError> {
        validate_name(name)?;
        Ok(self.base_path.join(format!("{name}.toml")))
    }
}

/// Serializes a `GameProfile` to a valid TOML string with comment headers
/// indicating where to save the file for sharing.
///
/// The returned string is valid TOML — comment headers use `#` syntax and are
/// ignored by TOML parsers, so the output can be saved directly as a `.toml` profile.
pub fn profile_to_shareable_toml(
    name: &str,
    profile: &GameProfile,
) -> Result<String, toml::ser::Error> {
    let toml_body = toml::to_string_pretty(profile)?;
    Ok(format!(
        "# CrossHook Profile: {name}\n\
         # https://github.com/yandy-r/crosshook\n\
         #\n\
         # To use this profile, save this file as:\n\
         #   ~/.config/crosshook/profiles/{name}.toml\n\
         #\n\
         # Then select the profile in CrossHook.\n\
         \n\
         {toml_body}"
    ))
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

/// Strips a trailing `(Copy)` or `(Copy N)` suffix from a profile name, returning
/// the base name. Non-copy parenthesized suffixes (e.g. `"Game (Special Edition)"`)
/// are left intact.
///
/// Returns the full trimmed input if no copy suffix is detected.
///
/// # Examples (from tests)
/// - `"Name (Copy)"` -> `"Name"`
/// - `"Name (Copy 3)"` -> `"Name"`
/// - `"Game (Special Edition)"` -> `"Game (Special Edition)"` (unchanged)
/// - `"(Copy)"` -> `""` (empty -- caller must handle)
fn strip_copy_suffix(name: &str) -> &str {
    let trimmed = name.trim_end();

    if let Some(before_paren) = trimmed.strip_suffix(')') {
        if let Some(pos) = before_paren.rfind('(') {
            let inside = before_paren[pos + 1..].trim();
            if inside == "Copy"
                || inside
                    .strip_prefix("Copy ")
                    .is_some_and(|n| n.parse::<u32>().is_ok())
            {
                return trimmed[..pos].trim_end();
            }
        }
    }

    trimmed
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
                loading_mode: crate::profile::TrainerLoadingMode::SourceDirectory,
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
                ..Default::default()
            },
            local_override: crate::profile::LocalOverrideSection::default(),
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

    #[test]
    fn test_rename_success() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("old-name", &profile).unwrap();
        assert!(store.profile_path("old-name").unwrap().exists());

        store.rename("old-name", "new-name").unwrap();
        assert!(!store.profile_path("old-name").unwrap().exists());
        assert!(store.profile_path("new-name").unwrap().exists());

        let loaded = store.load("new-name").unwrap();
        assert_eq!(loaded, profile);
    }

    #[test]
    fn test_rename_not_found() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        fs::create_dir_all(&store.base_path).unwrap();

        let result = store.rename("nonexistent", "new-name");
        assert!(result.is_err());
    }

    #[test]
    fn test_rename_same_name() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("same-name", &profile).unwrap();

        let result = store.rename("same-name", "same-name");
        assert!(result.is_ok());
        assert!(store.profile_path("same-name").unwrap().exists());
    }

    #[test]
    fn test_rename_preserves_content() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("original", &profile).unwrap();
        let original_content = fs::read_to_string(store.profile_path("original").unwrap()).unwrap();

        store.rename("original", "renamed").unwrap();
        let renamed_content = fs::read_to_string(store.profile_path("renamed").unwrap()).unwrap();

        assert_eq!(original_content, renamed_content);
    }

    #[test]
    fn test_rename_rejects_existing_target_profile() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let source_profile = sample_profile();
        let mut target_profile = sample_profile();
        target_profile.game.name = "Different Game".to_string();

        store.save("source", &source_profile).unwrap();
        store.save("target", &target_profile).unwrap();

        let result = store.rename("source", "target");

        assert!(matches!(
            result,
            Err(ProfileStoreError::AlreadyExists(ref name)) if name == "target"
        ));
        assert!(store.profile_path("source").unwrap().exists());
        assert!(store.profile_path("target").unwrap().exists());
        assert_eq!(store.load("source").unwrap(), source_profile);
        assert_eq!(store.load("target").unwrap(), target_profile);
    }

    #[test]
    fn save_launch_optimizations_merges_only_launch_section() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("elden-ring", &profile).unwrap();

        let optimizations = LaunchOptimizationsSection {
            enabled_option_ids: vec![
                "disable_steam_input".to_string(),
                "use_gamemode".to_string(),
            ],
        };
        store
            .save_launch_optimizations(
                "elden-ring",
                optimizations.enabled_option_ids.clone(),
                None,
            )
            .unwrap();

        let loaded = store.load("elden-ring").unwrap();
        assert_eq!(loaded.game, profile.game);
        assert_eq!(loaded.trainer, profile.trainer);
        assert_eq!(loaded.injection, profile.injection);
        assert_eq!(loaded.steam, profile.steam);
        assert_eq!(loaded.runtime, profile.runtime);
        assert_eq!(loaded.launch.method, profile.launch.method);
        assert_eq!(loaded.launch.optimizations, optimizations);
    }

    #[test]
    fn save_launch_optimizations_rejects_missing_profiles() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

        let result = store.save_launch_optimizations(
            "missing-profile",
            vec!["use_gamemode".to_string()],
            None,
        );

        assert!(matches!(result, Err(ProfileStoreError::NotFound(_))));
    }

    #[test]
    fn save_launch_optimizations_rejects_unknown_option_ids() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("elden-ring", &profile).unwrap();

        let result = store.save_launch_optimizations(
            "elden-ring",
            vec!["not_a_real_launch_optimization".to_string()],
            None,
        );

        assert!(matches!(
            result,
            Err(ProfileStoreError::InvalidLaunchOptimizationId(id)) if id == "not_a_real_launch_optimization"
        ));
    }

    #[test]
    fn load_normalizes_optimizations_from_active_preset() {
        use std::collections::BTreeMap;

        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        fs::create_dir_all(&store.base_path).unwrap();

        let toml = r#"[game]
name = "Test"
executable_path = "/games/test.exe"

[trainer]
path = ""
type = ""
loading_mode = "source_directory"

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

[runtime]
prefix_path = ""
proton_path = ""
working_directory = ""

[launch]
method = "proton_run"
active_preset = "performance"

[launch.optimizations]
enabled_option_ids = ["enable_hdr"]

[launch.presets.performance]
enabled_option_ids = ["use_gamemode", "disable_steam_input"]

[launch.presets.quality]
enabled_option_ids = ["enable_hdr"]
"#;
        fs::write(store.profile_path("preset-test").unwrap(), toml).unwrap();

        let loaded = store.load("preset-test").unwrap();
        assert_eq!(loaded.launch.active_preset, "performance");
        assert_eq!(
            loaded.launch.optimizations.enabled_option_ids,
            vec!["use_gamemode".to_string(), "disable_steam_input".to_string()]
        );
        let mut expected = BTreeMap::new();
        expected.insert(
            "performance".to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: vec![
                    "use_gamemode".to_string(),
                    "disable_steam_input".to_string(),
                ],
            },
        );
        expected.insert(
            "quality".to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: vec!["enable_hdr".to_string()],
            },
        );
        assert_eq!(loaded.launch.presets, expected);
    }

    #[test]
    fn save_launch_optimizations_updates_active_preset_entry() {
        use std::collections::BTreeMap;

        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let mut profile = sample_profile();

        let mut presets = BTreeMap::new();
        presets.insert(
            "a".to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: vec!["use_gamemode".to_string()],
            },
        );
        profile.launch.presets = presets;
        profile.launch.active_preset = "a".to_string();
        profile.launch.optimizations = profile.launch.presets["a"].clone();

        store.save("p", &profile).unwrap();

        store
            .save_launch_optimizations(
                "p",
                vec!["use_ntsync".to_string(), "disable_esync".to_string()],
                None,
            )
            .unwrap();

        let loaded = store.load("p").unwrap();
        assert_eq!(loaded.launch.active_preset, "a");
        assert_eq!(
            loaded.launch.optimizations.enabled_option_ids,
            vec!["use_ntsync".to_string(), "disable_esync".to_string()]
        );
        assert_eq!(
            loaded.launch.presets["a"].enabled_option_ids,
            vec!["use_ntsync".to_string(), "disable_esync".to_string()]
        );
    }

    #[test]
    fn materialize_launch_optimization_preset_sets_active_and_presets() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();
        store.save("p", &profile).unwrap();

        let ids = vec![
            "use_gamemode".to_string(),
            "enable_nvapi".to_string(),
        ];
        store
            .materialize_launch_optimization_preset(
                "p",
                "bundled/nvidia_performance",
                ids.clone(),
                true,
            )
            .unwrap();

        let loaded = store.load("p").unwrap();
        assert_eq!(loaded.launch.active_preset, "bundled/nvidia_performance");
        assert_eq!(loaded.launch.optimizations.enabled_option_ids, ids);
        assert_eq!(
            loaded
                .launch
                .presets
                .get("bundled/nvidia_performance")
                .unwrap()
                .enabled_option_ids,
            ids
        );
    }

    #[test]
    fn save_manual_launch_optimization_preset_rejects_bundled_prefix() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();
        store.save("p", &profile).unwrap();

        let result = store.save_manual_launch_optimization_preset(
            "p",
            "bundled/foo",
            vec!["use_gamemode".to_string()],
        );
        assert!(matches!(
            result,
            Err(ProfileStoreError::ReservedLaunchPresetName(_))
        ));
    }

    #[test]
    fn save_launch_optimizations_switch_active_preset() {
        use std::collections::BTreeMap;

        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let mut profile = sample_profile();

        let mut presets = BTreeMap::new();
        presets.insert(
            "a".to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: vec!["use_gamemode".to_string()],
            },
        );
        presets.insert(
            "b".to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: vec!["enable_hdr".to_string()],
            },
        );
        profile.launch.presets = presets;
        profile.launch.active_preset = "a".to_string();
        profile.launch.optimizations = profile.launch.presets["a"].clone();

        store.save("p", &profile).unwrap();

        store
            .save_launch_optimizations("p", vec![], Some("b".to_string()))
            .unwrap();

        let loaded = store.load("p").unwrap();
        assert_eq!(loaded.launch.active_preset, "b");
        assert_eq!(
            loaded.launch.optimizations.enabled_option_ids,
            vec!["enable_hdr".to_string()]
        );
    }

    #[test]
    fn save_launch_optimizations_rejects_missing_preset_name() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        store.save("p", &sample_profile()).unwrap();

        let result = store.save_launch_optimizations("p", vec![], Some("nope".to_string()));

        assert!(matches!(
            result,
            Err(ProfileStoreError::LaunchPresetNotFound(name)) if name == "nope"
        ));
    }

    #[test]
    fn test_strip_copy_suffix() {
        assert_eq!(strip_copy_suffix("Name (Copy)"), "Name");
        assert_eq!(strip_copy_suffix("Name (Copy 3)"), "Name");
        assert_eq!(strip_copy_suffix("Name"), "Name");
        assert_eq!(strip_copy_suffix("Copy"), "Copy");
        assert_eq!(
            strip_copy_suffix("Game (Special Edition)"),
            "Game (Special Edition)"
        );
        assert_eq!(strip_copy_suffix("Name (Copy 0)"), "Name");
        assert_eq!(strip_copy_suffix("Name (Copy 99)"), "Name");
    }

    #[test]
    fn test_duplicate_basic() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("MyGame", &profile).unwrap();
        let result = store.duplicate("MyGame").unwrap();

        assert_eq!(result.name, "MyGame (Copy)");
        assert_eq!(result.profile, profile);
        assert!(store.profile_path("MyGame (Copy)").unwrap().exists());
    }

    #[test]
    fn test_duplicate_increments_on_conflict() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("MyGame", &profile).unwrap();
        store.save("MyGame (Copy)", &profile).unwrap();
        let result = store.duplicate("MyGame").unwrap();

        assert_eq!(result.name, "MyGame (Copy 2)");
    }

    #[test]
    fn test_duplicate_of_copy() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("MyGame", &profile).unwrap();
        store.save("MyGame (Copy)", &profile).unwrap();
        let result = store.duplicate("MyGame (Copy)").unwrap();

        assert_eq!(result.name, "MyGame (Copy 2)");
    }

    #[test]
    fn test_duplicate_copy_suffix_only_name_keeps_non_empty_base() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("(Copy)", &profile).unwrap();
        let result = store.duplicate("(Copy)").unwrap();

        assert_eq!(result.name, "(Copy) (Copy)");
        assert!(!result.name.starts_with(' '));
        assert_eq!(store.load(&result.name).unwrap(), profile);
    }

    #[test]
    fn test_duplicate_preserves_all_fields() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        let profile = sample_profile();

        store.save("FullProfile", &profile).unwrap();
        let result = store.duplicate("FullProfile").unwrap();

        let loaded_source = store.load("FullProfile").unwrap();
        let loaded_copy = store.load(&result.name).unwrap();
        assert_eq!(loaded_source, loaded_copy);
    }

    #[test]
    fn test_duplicate_source_not_found() {
        let temp_dir = tempdir().unwrap();
        let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
        fs::create_dir_all(&store.base_path).unwrap();

        let result = store.duplicate("nonexistent");
        assert!(matches!(result, Err(ProfileStoreError::NotFound(_))));
    }

    #[test]
    fn shareable_toml_starts_with_comment_header() {
        let profile = sample_profile();
        let toml = profile_to_shareable_toml("elden-ring", &profile).unwrap();
        assert!(toml.starts_with("# CrossHook Profile: elden-ring\n"));
        assert!(toml.contains("# To use this profile, save this file as:"));
        assert!(toml.contains("~/.config/crosshook/profiles/elden-ring.toml"));
    }

    #[test]
    fn shareable_toml_roundtrips_through_parser() {
        let profile = sample_profile();
        let toml = profile_to_shareable_toml("elden-ring", &profile).unwrap();
        let parsed: GameProfile = toml::from_str(&toml).unwrap();
        assert_eq!(parsed, profile);
    }

    #[test]
    fn shareable_toml_with_empty_name_still_valid() {
        let profile = GameProfile::default();
        let toml = profile_to_shareable_toml("", &profile).unwrap();
        assert!(toml.starts_with("# CrossHook Profile: \n"));
        let parsed: GameProfile = toml::from_str(&toml).unwrap();
        assert_eq!(parsed, profile);
    }
}
