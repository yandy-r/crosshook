use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameSection {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "executable_path", default)]
    pub executable_path: String,
    #[serde(
        rename = "custom_cover_art_path",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub custom_cover_art_path: String,
    #[serde(
        rename = "custom_portrait_art_path",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub custom_portrait_art_path: String,
    #[serde(
        rename = "custom_background_art_path",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub custom_background_art_path: String,
    /// Unknown TOML keys retained across read-modify-write operations.
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LoadedDllHook {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub path: String,
    pub enabled: bool,
    /// Unknown TOML keys retained across read-modify-write operations.
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectionMethod {
    #[default]
    Disabled,
    LoadLibrary,
    ManualMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectionStage {
    #[default]
    TrainerLaunch,
    GameProcessReady,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectionFallback {
    #[default]
    WarnAndContinue,
    DisableHook,
    AbortLaunch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct InjectionSection {
    #[serde(
        rename = "loaded_hooks",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub loaded_hooks: Vec<LoadedDllHook>,
    pub method: InjectionMethod,
    pub stage: InjectionStage,
    #[serde(rename = "timeout_ms", default)]
    pub timeout_ms: u64,
    pub fallback: InjectionFallback,
    #[serde(rename = "dll_paths", default)]
    pub dll_paths: Vec<String>,
    #[serde(rename = "inject_on_launch", default)]
    pub inject_on_launch: Vec<bool>,
    /// Unknown TOML keys retained across read-modify-write operations.
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

/// Compatibility policy for [`InjectionSection::reconcile`].
///
/// Storage reconciliation ([`crate::profile::GameProfile::normalize_injection`])
/// and Classic TypeScript editor normalization (`normalizeInjectionSection`)
/// share one canonical-vs-legacy reconciliation pipeline, parameterized only
/// where historical id/name/blank-path conventions differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InjectionNormalizePolicy {
    /// Storage load/save reconciliation: drops blank legacy `dll_paths`,
    /// synthesizes `legacy-dll-N` ids with `file_stem()` / `"DLL Hook N"`
    /// names, and drops `loaded_hooks` entries with blank ids.
    Storage,
    /// Classic TypeScript editor parity (`normalizeInjectionSection`): keeps
    /// legacy `dll_paths` entries, synthesizes `legacy-dll-hook-N` ids and
    /// `loaded-dll-hook-N` fallback ids, and derives missing names from the
    /// path basename / `"DLL hook N"`.
    ClassicEditor,
}

impl InjectionSection {
    /// Reconciles canonical `loaded_hooks` with legacy `dll_paths` /
    /// `inject_on_launch` arrays under `policy`, then refreshes the legacy
    /// mirror arrays from the reconciled `loaded_hooks`.
    ///
    /// Note: Rust's `Vec<LoadedDllHook>` cannot distinguish an omitted
    /// `loaded_hooks` field from an explicit empty array `[]`, so when
    /// `loaded_hooks` is empty and legacy `dll_paths` is populated, legacy
    /// entries are converted into canonical `loaded_hooks` rows.
    pub(crate) fn reconcile(&mut self, policy: InjectionNormalizePolicy) {
        if self.loaded_hooks.is_empty() && !self.dll_paths.is_empty() {
            self.loaded_hooks = self
                .dll_paths
                .iter()
                .enumerate()
                .filter_map(|(index, path)| {
                    let enabled = self.inject_on_launch.get(index).copied().unwrap_or(false);
                    reconcile_legacy_dll_hook(path, index, enabled, policy)
                })
                .collect();
        } else if !self.loaded_hooks.is_empty() {
            match policy {
                InjectionNormalizePolicy::Storage => {
                    self.loaded_hooks.retain(|hook| !hook.id.trim().is_empty());
                }
                InjectionNormalizePolicy::ClassicEditor => {
                    for (index, hook) in self.loaded_hooks.iter_mut().enumerate() {
                        let id = hook.id.trim();
                        hook.id = if id.is_empty() {
                            format!("loaded-dll-hook-{}", index + 1)
                        } else {
                            id.to_string()
                        };
                        let name = hook.name.trim();
                        hook.name = if name.is_empty() {
                            classic_dll_hook_name(&hook.path, index)
                        } else {
                            name.to_string()
                        };
                    }
                }
            }
        }

        self.dll_paths = self
            .loaded_hooks
            .iter()
            .map(|hook| hook.path.clone())
            .collect();
        self.inject_on_launch = self.loaded_hooks.iter().map(|hook| hook.enabled).collect();
    }
}

fn reconcile_legacy_dll_hook(
    path: &str,
    index: usize,
    enabled: bool,
    policy: InjectionNormalizePolicy,
) -> Option<LoadedDllHook> {
    match policy {
        InjectionNormalizePolicy::Storage => {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                return None;
            }
            Some(LoadedDllHook {
                id: format!("legacy-dll-{}", index + 1),
                name: storage_dll_hook_name(trimmed, index),
                path: trimmed.to_string(),
                enabled,
                extra: toml::Table::new(),
            })
        }
        InjectionNormalizePolicy::ClassicEditor => Some(LoadedDllHook {
            id: format!("legacy-dll-hook-{}", index + 1),
            name: classic_dll_hook_name(path, index),
            path: path.to_string(),
            enabled,
            extra: toml::Table::new(),
        }),
    }
}

fn storage_dll_hook_name(path: &str, index: usize) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("DLL Hook {}", index + 1))
}

fn classic_dll_hook_name(path: &str, index: usize) -> String {
    let basename = path.rsplit(['/', '\\']).next().map(str::trim).unwrap_or("");
    if basename.is_empty() {
        format!("DLL hook {}", index + 1)
    } else {
        basename.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SteamSection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "app_id", default)]
    pub app_id: String,
    #[serde(rename = "compatdata_path", default)]
    pub compatdata_path: String,
    #[serde(rename = "proton_path", default)]
    pub proton_path: String,
    #[serde(default)]
    pub launcher: LauncherSection,
    /// Unknown TOML keys retained across read-modify-write operations.
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LauncherSection {
    #[serde(rename = "icon_path", default)]
    pub icon_path: String,
    #[serde(rename = "display_name", default)]
    pub display_name: String,
    /// Unknown TOML keys retained across read-modify-write operations.
    #[serde(default, flatten)]
    pub extra: toml::Table,
}
