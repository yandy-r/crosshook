//! Launch-optimization and optimization-preset mutation rules.

use crate::launch::is_known_launch_optimization_id;
use crate::profile::{GameProfile, ProfileStoreError};

/// A validated `[launch.presets.<key>]` write: trimmed key plus known, trimmed option ids.
struct PresetSelection {
    key: String,
    enabled_option_ids: Vec<String>,
}

/// Trims and drops blank option ids, then rejects any id missing from the optimization catalog.
fn normalize_option_ids(enabled_option_ids: Vec<String>) -> Result<Vec<String>, ProfileStoreError> {
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

    Ok(enabled_option_ids)
}

/// Validates a preset key and option ids.
fn validate_preset_selection(
    preset_key: &str,
    enabled_option_ids: Vec<String>,
) -> Result<PresetSelection, ProfileStoreError> {
    let key = preset_key.trim();
    if key.is_empty() {
        return Err(ProfileStoreError::LaunchPresetNotFound(
            preset_key.to_string(),
        ));
    }

    Ok(PresetSelection {
        key: key.to_string(),
        enabled_option_ids: normalize_option_ids(enabled_option_ids)?,
    })
}

/// Trims a user preset display name and rejects empty names and the reserved `bundled/` prefix.
fn validate_manual_launch_preset_name(raw: &str) -> Result<String, ProfileStoreError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(ProfileStoreError::InvalidLaunchPresetName(
            "preset name must not be empty".to_string(),
        ));
    }
    if name.starts_with("bundled/") {
        return Err(ProfileStoreError::ReservedLaunchPresetName(
            name.to_string(),
        ));
    }
    Ok(name.to_string())
}

impl GameProfile {
    /// Replaces the launch optimizations in memory.
    ///
    /// With `switch_active_preset`, activates that existing preset (its ids must be known) and
    /// keeps the active section's unknown keys. Otherwise stores the trimmed, known
    /// `enabled_option_ids` and mirrors them into the active preset entry, if any.
    pub fn apply_launch_optimizations(
        &mut self,
        enabled_option_ids: Vec<String>,
        switch_active_preset: Option<String>,
    ) -> Result<(), ProfileStoreError> {
        if let Some(raw) = switch_active_preset {
            let key = raw.trim();
            if key.is_empty() {
                return Err(ProfileStoreError::LaunchPresetNotFound(raw));
            }

            let section = self
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

            self.launch.active_preset = key.to_string();
            let extra = std::mem::take(&mut self.launch.optimizations.extra);
            self.launch.optimizations = section;
            // Preset unknowns stay in the preset table, matching load normalization.
            self.launch.optimizations.extra = extra;
        } else {
            self.launch.optimizations.enabled_option_ids =
                normalize_option_ids(enabled_option_ids)?;

            let ap = self.launch.active_preset.trim();
            if !ap.is_empty() {
                if let Some(slot) = self.launch.presets.get_mut(ap) {
                    slot.enabled_option_ids = self.launch.optimizations.enabled_option_ids.clone();
                }
            }
        }

        Ok(())
    }

    /// Writes or overwrites `launch.presets.<preset_key>` in memory and optionally activates it.
    /// All `enabled_option_ids` must be known catalog ids.
    pub fn materialize_launch_optimization_preset(
        &mut self,
        preset_key: &str,
        enabled_option_ids: Vec<String>,
        set_as_active: bool,
    ) -> Result<(), ProfileStoreError> {
        let selection = validate_preset_selection(preset_key, enabled_option_ids)?;
        self.apply_preset_selection(selection, set_as_active);
        Ok(())
    }

    /// Stores the selection under a new user preset name and activates it, in memory.
    pub fn apply_manual_optimization_preset(
        &mut self,
        preset_display_name: &str,
        enabled_option_ids: Vec<String>,
    ) -> Result<(), ProfileStoreError> {
        let name = validate_manual_launch_preset_name(preset_display_name)?;
        self.materialize_launch_optimization_preset(&name, enabled_option_ids, true)
    }

    /// Applies an already validated preset selection.
    fn apply_preset_selection(&mut self, selection: PresetSelection, set_as_active: bool) {
        let PresetSelection {
            key,
            enabled_option_ids,
        } = selection;

        self.launch
            .presets
            .entry(key.clone())
            .or_default()
            .enabled_option_ids = enabled_option_ids.clone();

        if set_as_active {
            self.launch.active_preset = key;
            self.launch.optimizations.enabled_option_ids = enabled_option_ids;
        }
    }
}
