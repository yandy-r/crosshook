use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::launch::is_known_launch_optimization_id;
use crate::profile::models::{LaunchOptimizationsSection, LocalOverrideSection};
use crate::profile::{legacy, GameProfile};
use crate::settings::{resolve_profiles_directory_from_config, AppSettingsData};

use super::error::ProfileStoreError;
use super::utils::{strip_copy_suffix, validate_manual_launch_preset_name, validate_name};
use crate::profile::mangohud;

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

#[derive(Debug, Clone)]
pub struct ProfileStore {
    pub base_path: PathBuf,
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileStore {
    pub fn try_new() -> Result<Self, String> {
        let config_dir = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .config_dir()
            .join("crosshook");
        let settings_path = config_dir.join("settings.toml");
        let settings: AppSettingsData = if settings_path.exists() {
            fs::read_to_string(&settings_path)
                .ok()
                .and_then(|s| toml::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            AppSettingsData::default()
        };
        Self::try_new_with_settings_data(&settings, &config_dir)
    }

    /// Profiles directory from `settings` relative to `crosshook_config_dir` (`~/.config/crosshook`).
    pub fn try_new_with_settings_data(
        settings: &AppSettingsData,
        crosshook_config_dir: &std::path::Path,
    ) -> Result<Self, String> {
        let base_path = resolve_profiles_directory_from_config(settings, crosshook_config_dir)?;
        fs::create_dir_all(&base_path).map_err(|e| format!("profiles directory: {e}"))?;
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

        if let Err(err) =
            mangohud::write_mangohud_conf(&self.base_path, name, &profile.launch.mangohud)
        {
            tracing::warn!(
                profile = name,
                error = %err,
                "failed to write MangoHud companion config; profile save succeeded"
            );
        }

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
                    slot.enabled_option_ids =
                        profile.launch.optimizations.enabled_option_ids.clone();
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
            return Err(ProfileStoreError::LaunchPresetNotFound(
                preset_key.to_string(),
            ));
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
        profile.launch.presets.insert(
            key.to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: enabled_option_ids.clone(),
            },
        );

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
        self.materialize_launch_optimization_preset(profile_name, &name, enabled_option_ids, true)
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

        let mangohud_path = mangohud::mangohud_conf_path(&self.base_path, name);
        if mangohud_path.exists() {
            if let Err(err) = fs::remove_file(&mangohud_path) {
                tracing::warn!(
                    profile = name,
                    path = %mangohud_path.display(),
                    error = %err,
                    "failed to remove MangoHud companion config"
                );
            }
        }

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

        let old_mangohud = mangohud::mangohud_conf_path(&self.base_path, old_name);
        if old_mangohud.exists() {
            let new_mangohud = mangohud::mangohud_conf_path(&self.base_path, new_name);
            if let Err(err) = fs::rename(&old_mangohud, &new_mangohud) {
                tracing::warn!(
                    old_profile = old_name,
                    new_profile = new_name,
                    error = %err,
                    "failed to rename MangoHud companion config; attempting copy fallback"
                );
                if let Err(copy_err) = fs::copy(&old_mangohud, &new_mangohud) {
                    tracing::warn!(
                        old_profile = old_name,
                        new_profile = new_name,
                        error = %copy_err,
                        "failed to copy MangoHud companion config during rename fallback"
                    );
                } else if let Err(remove_err) = fs::remove_file(&old_mangohud) {
                    tracing::warn!(
                        old_profile = old_name,
                        new_profile = new_name,
                        error = %remove_err,
                        "failed to remove old MangoHud companion config after copy fallback"
                    );
                }
            }
        }

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

    pub(crate) fn profile_path(&self, name: &str) -> Result<PathBuf, ProfileStoreError> {
        validate_name(name)?;
        Ok(self.base_path.join(format!("{name}.toml")))
    }

    /// Returns whether a profile TOML already exists for `name`.
    pub fn profile_exists(&self, name: &str) -> bool {
        match self.profile_path(name) {
            Ok(path) => path.exists(),
            Err(_) => false,
        }
    }
}
