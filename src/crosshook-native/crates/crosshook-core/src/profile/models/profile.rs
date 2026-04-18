use serde::{Deserialize, Serialize};

use super::game_meta::{GameSection, InjectionSection, SteamSection};
use super::launch::{CollectionDefaultsSection, LaunchSection};
use super::local_override::LocalOverrideSection;
use super::runtime::RuntimeSection;
use super::trainer::TrainerSection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameProfile {
    #[serde(default)]
    pub game: GameSection,
    #[serde(default)]
    pub trainer: TrainerSection,
    #[serde(default)]
    pub injection: InjectionSection,
    #[serde(default)]
    pub steam: SteamSection,
    #[serde(default, skip_serializing_if = "RuntimeSection::is_empty")]
    pub runtime: RuntimeSection,
    #[serde(default)]
    pub launch: LaunchSection,
    #[serde(default, skip_serializing_if = "LocalOverrideSection::is_empty")]
    pub local_override: LocalOverrideSection,
}

impl GameProfile {
    /// Returns the effective profile used at runtime where local overrides take precedence
    /// over portable base values.
    ///
    /// Backward-compat shim: forwards to [`Self::effective_profile_with`] with no
    /// collection-defaults layer. Existing call sites that don't know about per-collection
    /// launch defaults continue to get base + `local_override` only.
    pub fn effective_profile(&self) -> Self {
        self.effective_profile_with(None)
    }

    /// Returns the effective profile, optionally merging a collection-defaults layer
    /// between the base profile and the local override layer.
    ///
    /// Precedence (lowest → highest):
    ///   1. Base profile (`self`)
    ///   2. Collection defaults (if `Some`) — per-collection overrides from
    ///      [`CollectionDefaultsSection`]. `custom_env_vars` is an additive merge
    ///      where collection keys win on collision; all other fields are replacement.
    ///      Whitespace-only `method` is ignored so it cannot accidentally clobber
    ///      a profile's launch method.
    ///   3. `local_override.*` — machine-specific paths always win last so a power
    ///      user's portable executable_path is never trampled by collection defaults.
    ///
    /// # Runtime precedence after [`crate::profile::ProfileStore::load`]
    ///
    /// The 3-layer precedence above applies when this method is called on a
    /// profile that still carries a populated `local_override` section (e.g.,
    /// a freshly-constructed fixture in tests or a raw-storage profile read from
    /// TOML without going through the store loader).
    ///
    /// In production, the only caller that threads collection defaults is
    /// `profile_load`, which first calls `ProfileStore::load`. That loader
    /// already collapses `local_override` into layer 1 (baking the overrides
    /// into the base profile fields) and clears `self.local_override` to
    /// `LocalOverrideSection::default()`. By the time this method runs on a
    /// post-load profile, layer 3 is a no-op — the `local_override`-guarded
    /// branches below all see empty strings. The effective precedence at that
    /// call site is therefore:
    ///
    /// ```text
    /// (base ⊕ local_override, baked into layer 1)  →  collection defaults  →  ∅
    /// ```
    ///
    /// Today this has no user-visible effect because `CollectionDefaultsSection`
    /// and `LocalOverrideSection` have zero field overlap (collection = launch
    /// subset; local_override = machine-specific paths). If a future contributor
    /// adds overlapping fields, the "local_override always wins" guarantee still
    /// holds at the call site because layer 1 already contains the override,
    /// but any new fields must be audited here to preserve that invariant.
    pub fn effective_profile_with(&self, defaults: Option<&CollectionDefaultsSection>) -> Self {
        let mut merged = self.clone();

        // ── Layer 2: collection defaults ────────────────────────────────────
        if let Some(d) = defaults {
            if let Some(ref method) = d.method {
                if !method.trim().is_empty() {
                    merged.launch.method = method.clone();
                }
            }
            if let Some(ref opts) = d.optimizations {
                merged.launch.optimizations = opts.clone();
            }
            if !d.custom_env_vars.is_empty() {
                // Additive merge — collection keys win on collision, profile keys
                // without a collision are preserved.
                for (k, v) in &d.custom_env_vars {
                    merged.launch.custom_env_vars.insert(k.clone(), v.clone());
                }
            }
            if let Some(ni) = d.network_isolation {
                merged.launch.network_isolation = ni;
            }
            if let Some(ref gs) = d.gamescope {
                merged.launch.gamescope = gs.clone();
            }
            if let Some(ref tgs) = d.trainer_gamescope {
                merged.launch.trainer_gamescope = tgs.clone();
            }
            if let Some(ref mh) = d.mangohud {
                merged.launch.mangohud = mh.clone();
            }
        }

        // ── Layer 3: local_override (unchanged) ─────────────────────────────
        if !self.local_override.game.executable_path.trim().is_empty() {
            merged.game.executable_path = self.local_override.game.executable_path.clone();
        }
        if !self
            .local_override
            .game
            .custom_cover_art_path
            .trim()
            .is_empty()
        {
            merged.game.custom_cover_art_path =
                self.local_override.game.custom_cover_art_path.clone();
        }
        if !self
            .local_override
            .game
            .custom_portrait_art_path
            .trim()
            .is_empty()
        {
            merged.game.custom_portrait_art_path =
                self.local_override.game.custom_portrait_art_path.clone();
        }
        if !self
            .local_override
            .game
            .custom_background_art_path
            .trim()
            .is_empty()
        {
            merged.game.custom_background_art_path =
                self.local_override.game.custom_background_art_path.clone();
        }
        if !self.local_override.trainer.path.trim().is_empty() {
            merged.trainer.path = self.local_override.trainer.path.clone();
        }
        if !self.local_override.trainer.extra_protontricks.is_empty() {
            merged
                .trainer
                .required_protontricks
                .extend(self.local_override.trainer.extra_protontricks.clone());
        }
        if !self.local_override.steam.compatdata_path.trim().is_empty() {
            merged.steam.compatdata_path = self.local_override.steam.compatdata_path.clone();
        }
        if !self.local_override.steam.proton_path.trim().is_empty() {
            merged.steam.proton_path = self.local_override.steam.proton_path.clone();
        }
        if !self.local_override.runtime.prefix_path.trim().is_empty() {
            merged.runtime.prefix_path = self.local_override.runtime.prefix_path.clone();
        }
        if !self.local_override.runtime.proton_path.trim().is_empty() {
            merged.runtime.proton_path = self.local_override.runtime.proton_path.clone();
        }

        merged
    }

    /// Returns the storage representation where machine-specific paths are moved into
    /// the local override section and portable base fields are path-free.
    pub fn storage_profile(&self) -> Self {
        let effective = self.effective_profile();
        let mut storage = effective.clone();

        storage.local_override.game.executable_path = effective.game.executable_path.clone();
        storage.local_override.game.custom_cover_art_path =
            effective.game.custom_cover_art_path.clone();
        storage.local_override.game.custom_portrait_art_path =
            effective.game.custom_portrait_art_path.clone();
        storage.local_override.game.custom_background_art_path =
            effective.game.custom_background_art_path.clone();
        storage.local_override.trainer.path = effective.trainer.path.clone();
        storage.local_override.steam.compatdata_path = effective.steam.compatdata_path.clone();
        storage.local_override.steam.proton_path = effective.steam.proton_path.clone();
        storage.local_override.runtime.prefix_path = effective.runtime.prefix_path.clone();
        storage.local_override.runtime.proton_path = effective.runtime.proton_path.clone();

        storage.game.executable_path.clear();
        storage.game.custom_cover_art_path.clear();
        storage.game.custom_portrait_art_path.clear();
        storage.game.custom_background_art_path.clear();
        storage.trainer.path.clear();
        storage.steam.compatdata_path.clear();
        storage.steam.proton_path.clear();
        storage.runtime.prefix_path.clear();
        storage.runtime.proton_path.clear();

        storage
    }

    /// Returns the portable profile representation with all local machine-specific data removed.
    pub fn portable_profile(&self) -> Self {
        let mut portable = self.storage_profile();
        portable.local_override = LocalOverrideSection::default();
        portable
    }
}
