//! Typed traversal keeps unknown TOML values intact without a JSON round trip.

use std::collections::BTreeMap;

use super::launch::LaunchCommandArgumentsSection;
use super::*;

trait ExtraFields {
    const HAS_IDENTITY: bool = false;

    fn clear_extra(&mut self);
    fn preserve_extra_from(&mut self, existing: &Self);

    fn identity(&self) -> Option<&str> {
        None
    }
}

macro_rules! extra_fields {
    ($type:ty $(, $child:ident)* $(;)?) => {
        impl ExtraFields for $type {
            fn clear_extra(&mut self) {
                self.extra.clear();
                $(self.$child.clear_extra();)*
            }

            fn preserve_extra_from(&mut self, existing: &Self) {
                self.extra.clone_from(&existing.extra);
                $(self.$child.preserve_extra_from(&existing.$child);)*
            }
        }
    };
}

extra_fields!(GameSection);
extra_fields!(TrainerSection);
extra_fields!(LauncherSection);
extra_fields!(RuntimeSection);
extra_fields!(LaunchOptimizationsSection);
extra_fields!(LaunchCommandArgumentsSection);
extra_fields!(GamescopeConfig);
extra_fields!(MangoHudConfig);
extra_fields!(LocalOverrideGameSection);
extra_fields!(LocalOverrideTrainerSection);
extra_fields!(LocalOverrideSteamSection);
extra_fields!(LocalOverrideRuntimeSection);
extra_fields!(SteamSection, launcher);
extra_fields!(InjectionSection, loaded_hooks);
extra_fields!(
    LaunchSection,
    optimizations,
    command_arguments,
    presets,
    gamescope,
    trainer_gamescope,
    mangohud
);
extra_fields!(LocalOverrideSection, game, trainer, steam, runtime);
extra_fields!(
    CollectionDefaultsSection,
    optimizations,
    gamescope,
    trainer_gamescope,
    mangohud
);
extra_fields!(
    GameProfile,
    game,
    trainer,
    injection,
    steam,
    runtime,
    launch,
    local_override,
    pre_launch_hooks,
    post_exit_hooks
);

// Hook IDs are stable across editor reorder/removal; positional matching would
// attach another hook's future settings to the wrong script or DLL.
macro_rules! hook_extra_fields {
    ($type:ty) => {
        impl ExtraFields for $type {
            const HAS_IDENTITY: bool = true;

            fn clear_extra(&mut self) {
                self.extra.clear();
            }

            fn preserve_extra_from(&mut self, existing: &Self) {
                self.extra.clone_from(&existing.extra);
            }

            fn identity(&self) -> Option<&str> {
                (!self.id.is_empty()).then_some(self.id.as_str())
            }
        }
    };
}

hook_extra_fields!(LaunchHook);
hook_extra_fields!(LoadedDllHook);

impl<T: ExtraFields> ExtraFields for Vec<T> {
    fn clear_extra(&mut self) {
        for value in self {
            value.clear_extra();
        }
    }

    fn preserve_extra_from(&mut self, existing: &Self) {
        for (index, value) in self.iter_mut().enumerate() {
            let source = match value.identity() {
                Some(id) => existing.iter().find(|item| item.identity() == Some(id)),
                None if T::HAS_IDENTITY => None,
                None => existing.get(index),
            };
            if let Some(source) = source {
                value.preserve_extra_from(source);
            }
        }
    }
}

impl<T: ExtraFields> ExtraFields for BTreeMap<String, T> {
    fn clear_extra(&mut self) {
        for value in self.values_mut() {
            value.clear_extra();
        }
    }

    fn preserve_extra_from(&mut self, existing: &Self) {
        for (key, value) in self {
            if let Some(source) = existing.get(key) {
                value.preserve_extra_from(source);
            }
        }
    }
}

impl<T: ExtraFields> ExtraFields for Option<T> {
    fn clear_extra(&mut self) {
        if let Some(value) = self {
            value.clear_extra();
        }
    }

    fn preserve_extra_from(&mut self, existing: &Self) {
        if let (Some(value), Some(source)) = (self, existing) {
            value.preserve_extra_from(source);
        }
    }
}

impl GameProfile {
    /// Removes unknown fields recursively before crossing an IPC/export boundary.
    pub fn clear_extra(&mut self) {
        ExtraFields::clear_extra(self);
    }

    /// Restores stored unknown fields without converting TOML values to JSON.
    /// Hooks match by stable ID, presets by name, other arrays by index.
    /// Removed hooks/presets stay removed; known edited fields stay unchanged.
    pub fn preserve_extra_from(&mut self, existing: &Self) {
        ExtraFields::preserve_extra_from(self, existing);
    }
}

impl CollectionDefaultsSection {
    /// Removes unknown fields recursively before crossing an IPC/export boundary.
    pub fn clear_extra(&mut self) {
        ExtraFields::clear_extra(self);
    }

    /// Restores unknown fields for surviving collection-default sections.
    pub fn preserve_extra_from(&mut self, existing: &Self) {
        ExtraFields::preserve_extra_from(self, existing);
    }
}

impl LocalOverrideSection {
    /// Clears known machine-local fields while retaining unrecognized settings.
    pub(crate) fn extra_only(&self) -> Self {
        let mut result = Self::default();
        ExtraFields::preserve_extra_from(&mut result, self);
        result
    }
}
