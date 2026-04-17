//! Host capability map and derived capability state.
//!
//! Mirrors [`super::catalog`] patterns for embedded TOML, optional user
//! overrides, merge behavior, and process-global access via [`OnceLock`].

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::{
    global_readiness_catalog, HostDistroFamily, HostToolCheckResult, HostToolInstallCommand,
    ReadinessCatalog, ReadinessCheckResult,
};
use crate::launch::runtime_helpers::resolve_umu_run_path;
use crate::profile::health::HealthIssueSeverity;

/// Embedded default capability map TOML (compile time).
pub const DEFAULT_CAPABILITY_MAP_TOML: &str =
    include_str!("../../../../assets/default_capability_map.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Available,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub label: String,
    pub category: String,
    pub state: CapabilityState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default)]
    pub required_tool_ids: Vec<String>,
    #[serde(default)]
    pub optional_tool_ids: Vec<String>,
    #[serde(default)]
    pub missing_required: Vec<HostToolCheckResult>,
    #[serde(default)]
    pub missing_optional: Vec<HostToolCheckResult>,
    #[serde(default)]
    pub install_hints: Vec<HostToolInstallCommand>,
}

/// Validated capability definition loaded from TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDefinition {
    pub id: String,
    pub label: String,
    pub category: String,
    pub required_tools: Vec<String>,
    pub optional_tools: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawCapabilityMapFile {
    #[serde(default)]
    catalog_version: u32,
    #[serde(rename = "capability", default)]
    capabilities: Vec<RawCapabilityDefinition>,
}

#[derive(Debug, Deserialize)]
struct RawCapabilityDefinition {
    id: String,
    label: String,
    category: String,
    #[serde(default)]
    required_tools: Vec<String>,
    #[serde(default)]
    optional_tools: Vec<String>,
}

/// Validated in-memory capability map.
#[derive(Debug, Clone)]
pub struct CapabilityMap {
    pub catalog_version: u32,
    pub entries: Vec<CapabilityDefinition>,
}

impl CapabilityMap {
    pub fn from_entries(catalog_version: u32, entries: Vec<CapabilityDefinition>) -> Self {
        Self {
            catalog_version: catalog_version.max(1),
            entries,
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&CapabilityDefinition> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}

/// Parse TOML; returns valid entries, warnings for skipped rows, and `catalog_version`.
pub fn parse_capability_map_toml(
    toml_text: &str,
    source_label: &str,
) -> (Vec<CapabilityDefinition>, Vec<String>, u32) {
    let mut warnings: Vec<String> = Vec::new();

    let raw: RawCapabilityMapFile = match toml::from_str(toml_text) {
        Ok(raw) => raw,
        Err(err) => {
            warnings.push(format!(
                "capability map '{source_label}' failed to parse: {err}"
            ));
            return (Vec::new(), warnings, 1);
        }
    };
    let catalog_version = raw.catalog_version.max(1);

    let mut valid = Vec::new();
    let mut seen_ids = HashSet::new();

    for entry in raw.capabilities {
        if entry.id.trim().is_empty() {
            warnings.push("skipping capability entry with empty id".to_string());
            continue;
        }
        if !seen_ids.insert(entry.id.clone()) {
            warnings.push(format!("skipping duplicate capability id: {}", entry.id));
            continue;
        }
        if entry.label.trim().is_empty() {
            warnings.push(format!("skipping capability '{}': empty label", entry.id));
            continue;
        }
        if entry.category.trim().is_empty() {
            warnings.push(format!(
                "skipping capability '{}': empty category",
                entry.id
            ));
            continue;
        }

        let required_tools = sanitize_tool_list(&entry.required_tools);
        let mut optional_tools = sanitize_tool_list(&entry.optional_tools);
        optional_tools.retain(|tool_id| !required_tools.contains(tool_id));

        if required_tools.is_empty() && optional_tools.is_empty() {
            warnings.push(format!(
                "skipping capability '{}': at least one required_tools or optional_tools entry is required",
                entry.id
            ));
            continue;
        }

        valid.push(CapabilityDefinition {
            id: entry.id,
            label: entry.label,
            category: entry.category,
            required_tools,
            optional_tools,
        });
    }

    (valid, warnings, catalog_version)
}

pub fn merge_capability_maps(
    default_entries: Vec<CapabilityDefinition>,
    override_entries: Vec<CapabilityDefinition>,
) -> Vec<CapabilityDefinition> {
    let mut result = default_entries;
    for over in override_entries {
        if let Some(pos) = result.iter().position(|entry| entry.id == over.id) {
            result[pos] = over;
        } else {
            result.push(over);
        }
    }
    result
}

/// Load embedded default → optional user `host_capability_map.toml`.
pub fn load_capability_map(user_config_dir: Option<&Path>) -> CapabilityMap {
    let (default_entries, default_warnings, catalog_version) =
        parse_capability_map_toml(DEFAULT_CAPABILITY_MAP_TOML, "embedded default");
    for warning in &default_warnings {
        tracing::warn!(warning = %warning, "default capability map warning");
    }

    let mut merged = default_entries;

    let (user_entries, user_catalog_version_override) = user_config_dir
        .map(|dir| dir.join("host_capability_map.toml"))
        .filter(|path| path.exists())
        .and_then(|path| {
            std::fs::read_to_string(&path)
                .map_err(|err| {
                    tracing::warn!(
                        path = %path.display(),
                        %err,
                        "failed to read user capability map"
                    );
                })
                .ok()
        })
        .map(|text| {
            let (entries, warnings, user_ver) = parse_capability_map_toml(&text, "user override");
            for warning in &warnings {
                tracing::warn!(warning = %warning, "user capability map warning");
            }
            let parse_failed = warnings
                .iter()
                .any(|warning| warning.contains("failed to parse:"));
            let version_override = if parse_failed { None } else { Some(user_ver) };
            (entries, version_override)
        })
        .unwrap_or((Vec::new(), None));

    merged = merge_capability_maps(merged, user_entries);

    CapabilityMap::from_entries(
        user_catalog_version_override.unwrap_or(catalog_version),
        merged,
    )
}

static GLOBAL_CAPABILITY_MAP: OnceLock<CapabilityMap> = OnceLock::new();

pub fn initialize_capability_map(capability_map: CapabilityMap) -> bool {
    let ok = GLOBAL_CAPABILITY_MAP.set(capability_map).is_ok();
    if !ok {
        tracing::warn!("capability map was already initialized; ignoring duplicate set");
    }
    ok
}

pub fn global_capability_map() -> &'static CapabilityMap {
    GLOBAL_CAPABILITY_MAP.get_or_init(|| load_capability_map(None))
}

pub fn derive_capabilities(result: &ReadinessCheckResult) -> Vec<Capability> {
    derive_capabilities_with_map(result, global_capability_map(), global_readiness_catalog())
}

fn derive_capabilities_with_map(
    result: &ReadinessCheckResult,
    capability_map: &CapabilityMap,
    readiness_catalog: &ReadinessCatalog,
) -> Vec<Capability> {
    capability_map
        .entries
        .iter()
        .map(|definition| {
            let missing_required = definition
                .required_tools
                .iter()
                .filter_map(|tool_id| {
                    let check = resolve_tool_check(result, readiness_catalog, tool_id, true);
                    (!check.is_available).then_some(check)
                })
                .collect::<Vec<_>>();

            let missing_optional = definition
                .optional_tools
                .iter()
                .filter_map(|tool_id| {
                    let check = resolve_tool_check(result, readiness_catalog, tool_id, false);
                    (!check.is_available).then_some(check)
                })
                .collect::<Vec<_>>();

            let state = if !missing_required.is_empty() {
                CapabilityState::Unavailable
            } else if !missing_optional.is_empty() {
                CapabilityState::Degraded
            } else {
                CapabilityState::Available
            };

            Capability {
                id: definition.id.clone(),
                label: definition.label.clone(),
                category: definition.category.clone(),
                state,
                rationale: capability_rationale(
                    &definition.label,
                    state,
                    &missing_required,
                    &missing_optional,
                ),
                required_tool_ids: definition.required_tools.clone(),
                optional_tool_ids: definition.optional_tools.clone(),
                install_hints: collect_install_hints(
                    missing_required.iter().chain(missing_optional.iter()),
                ),
                missing_required,
                missing_optional,
            }
        })
        .collect()
}

fn sanitize_tool_list(tool_ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut sanitized = Vec::new();

    for tool_id in tool_ids {
        let trimmed = tool_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            sanitized.push(trimmed.to_string());
        }
    }

    sanitized
}

fn resolve_tool_check(
    result: &ReadinessCheckResult,
    readiness_catalog: &ReadinessCatalog,
    tool_id: &str,
    capability_requires_tool: bool,
) -> HostToolCheckResult {
    if let Some(existing) = result
        .tool_checks
        .iter()
        .find(|check| check.tool_id == tool_id)
    {
        let mut enriched = existing.clone();
        enriched.is_required = capability_requires_tool;
        if !enriched.is_available && enriched.install_guidance.is_none() {
            enriched.install_guidance = install_hint_for_tool(result, readiness_catalog, tool_id);
        }
        return enriched;
    }

    if tool_id == "umu_run" {
        return synthesize_umu_run_check(result, readiness_catalog, capability_requires_tool);
    }

    let catalog_entry = readiness_catalog.find_by_id(tool_id);
    HostToolCheckResult {
        tool_id: tool_id.to_string(),
        display_name: catalog_entry
            .map(|entry| entry.display_name.clone())
            .unwrap_or_else(|| tool_id.to_string()),
        is_available: false,
        is_required: capability_requires_tool,
        category: catalog_entry
            .map(|entry| entry.category.clone())
            .unwrap_or_default(),
        docs_url: catalog_entry
            .map(|entry| entry.docs_url.clone())
            .unwrap_or_default(),
        tool_version: None,
        resolved_path: None,
        install_guidance: install_hint_for_tool(result, readiness_catalog, tool_id),
    }
}

fn synthesize_umu_run_check(
    result: &ReadinessCheckResult,
    readiness_catalog: &ReadinessCatalog,
    capability_requires_tool: bool,
) -> HostToolCheckResult {
    let catalog_entry = readiness_catalog.find_by_id("umu_run");

    // Prefer the HealthIssue emitted by a fresh `check_system_readiness` run.
    // When capabilities are derived from a cached SQLite snapshot the
    // ReadinessCheckResult is rebuilt without HealthIssues, so the lookup
    // returns None — fall back to a live host probe so we never report umu-run
    // as missing when it is actually installed. `resolve_umu_run_path` already
    // handles both native PATH walks and Flatpak host probing.
    let probed_umu_path = match result
        .checks
        .iter()
        .find(|issue| issue.field == "umu_run_available")
    {
        Some(issue) if matches!(issue.severity, HealthIssueSeverity::Info) => {
            // Available per HealthIssue; reuse the path it recorded if any.
            let recorded = issue.path.trim();
            if recorded.is_empty() {
                resolve_umu_run_path()
            } else {
                Some(recorded.to_string())
            }
        }
        Some(_) => None, // Explicit Warning/Error severity → not available.
        None => resolve_umu_run_path(),
    };

    let available = probed_umu_path.is_some();

    let install_guidance = if available {
        None
    } else if let Some(guidance) = &result.umu_install_guidance {
        Some(HostToolInstallCommand {
            distro_family: if result.detected_distro_family.trim().is_empty() {
                HostDistroFamily::Unknown.as_str().to_string()
            } else {
                result.detected_distro_family.clone()
            },
            command: guidance.install_command.clone(),
            alternatives: guidance.description.clone(),
        })
    } else {
        install_hint_for_tool(result, readiness_catalog, "umu_run")
    };

    HostToolCheckResult {
        tool_id: "umu_run".to_string(),
        display_name: catalog_entry
            .map(|entry| entry.display_name.clone())
            .unwrap_or_else(|| "umu-launcher".to_string()),
        is_available: available,
        is_required: capability_requires_tool,
        category: catalog_entry
            .map(|entry| entry.category.clone())
            .unwrap_or_else(|| "runtime".to_string()),
        docs_url: catalog_entry
            .map(|entry| entry.docs_url.clone())
            .unwrap_or_default(),
        tool_version: None,
        resolved_path: probed_umu_path,
        install_guidance,
    }
}

fn install_hint_for_tool(
    result: &ReadinessCheckResult,
    readiness_catalog: &ReadinessCatalog,
    tool_id: &str,
) -> Option<HostToolInstallCommand> {
    let distro = HostDistroFamily::from_catalog_key(result.detected_distro_family.trim())
        .unwrap_or(HostDistroFamily::Unknown);
    readiness_catalog
        .find_by_id(tool_id)
        .and_then(|entry| ReadinessCatalog::install_for_distro(entry, distro))
}

fn capability_rationale(
    label: &str,
    state: CapabilityState,
    missing_required: &[HostToolCheckResult],
    missing_optional: &[HostToolCheckResult],
) -> Option<String> {
    match state {
        CapabilityState::Available => None,
        CapabilityState::Unavailable => {
            let tools = format_tool_list(missing_required);
            let noun = if missing_required.len() == 1 {
                "is"
            } else {
                "are"
            };
            Some(format!(
                "{label} is unavailable because {tools} {noun} not available on the host."
            ))
        }
        CapabilityState::Degraded => {
            let tools = format_tool_list(missing_optional);
            let noun = if missing_optional.len() == 1 {
                "is"
            } else {
                "are"
            };
            Some(format!(
                "{label} is degraded because optional tool {tools} {noun} not available on the host."
            ))
        }
    }
}

fn format_tool_list(tool_checks: &[HostToolCheckResult]) -> String {
    let labels = tool_checks
        .iter()
        .map(|check| check.display_name.as_str())
        .collect::<Vec<_>>();
    join_list(&labels)
}

fn join_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_string(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let mut combined = items[..items.len() - 1].join(", ");
            combined.push_str(", and ");
            combined.push_str(items[items.len() - 1]);
            combined
        }
    }
}

fn collect_install_hints<'a>(
    tool_checks: impl Iterator<Item = &'a HostToolCheckResult>,
) -> Vec<HostToolInstallCommand> {
    let mut hints = Vec::new();
    for tool_check in tool_checks {
        let Some(hint) = &tool_check.install_guidance else {
            continue;
        };
        if hint.command.trim().is_empty() && hint.alternatives.trim().is_empty() {
            continue;
        }
        if hints.iter().any(|existing| existing == hint) {
            continue;
        }
        hints.push(hint.clone());
    }
    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onboarding::HostToolEntry;
    use crate::profile::health::HealthIssue;

    fn sample_catalog_entry(
        tool_id: &str,
        display_name: &str,
        category: &str,
        command: &str,
    ) -> HostToolEntry {
        HostToolEntry {
            tool_id: tool_id.to_string(),
            binary_name: tool_id.to_string(),
            display_name: display_name.to_string(),
            description: format!("{display_name} description"),
            docs_url: format!("https://example.invalid/{tool_id}"),
            required: false,
            category: category.to_string(),
            install_commands: vec![HostToolInstallCommand {
                distro_family: HostDistroFamily::Unknown.as_str().to_string(),
                command: command.to_string(),
                alternatives: format!("Install {display_name} another way"),
            }],
        }
    }

    fn sample_capability_map() -> CapabilityMap {
        CapabilityMap::from_entries(
            1,
            vec![
                CapabilityDefinition {
                    id: "gamescope".to_string(),
                    label: "Gamescope".to_string(),
                    category: "performance".to_string(),
                    required_tools: vec!["gamescope".to_string()],
                    optional_tools: vec![],
                },
                CapabilityDefinition {
                    id: "mangohud".to_string(),
                    label: "MangoHud".to_string(),
                    category: "overlay".to_string(),
                    required_tools: vec!["mangohud".to_string()],
                    optional_tools: vec![],
                },
                CapabilityDefinition {
                    id: "gamemode".to_string(),
                    label: "GameMode".to_string(),
                    category: "performance".to_string(),
                    required_tools: vec!["gamemode".to_string()],
                    optional_tools: vec![],
                },
                CapabilityDefinition {
                    id: "prefix_tools".to_string(),
                    label: "Prefix tools".to_string(),
                    category: "prefix_tools".to_string(),
                    required_tools: vec![],
                    optional_tools: vec!["winetricks".to_string(), "protontricks".to_string()],
                },
                CapabilityDefinition {
                    id: "non_steam_launch".to_string(),
                    label: "Non-Steam launch".to_string(),
                    category: "runtime".to_string(),
                    required_tools: vec!["umu_run".to_string()],
                    optional_tools: vec![],
                },
            ],
        )
    }

    fn sample_readiness_catalog() -> ReadinessCatalog {
        ReadinessCatalog::from_entries(
            1,
            vec![
                sample_catalog_entry("gamescope", "Gamescope", "performance", "install gamescope"),
                sample_catalog_entry("mangohud", "MangoHud", "overlay", "install mangohud"),
                sample_catalog_entry("gamemode", "GameMode", "performance", "install gamemode"),
                sample_catalog_entry(
                    "winetricks",
                    "Winetricks",
                    "prefix_tools",
                    "install winetricks",
                ),
                sample_catalog_entry(
                    "protontricks",
                    "Protontricks",
                    "prefix_tools",
                    "install protontricks",
                ),
                sample_catalog_entry("umu_run", "umu-launcher", "runtime", "install umu"),
            ],
        )
    }

    fn issue(field: &str, severity: HealthIssueSeverity) -> HealthIssue {
        HealthIssue {
            field: field.to_string(),
            path: String::new(),
            message: field.to_string(),
            remediation: String::new(),
            severity,
        }
    }

    fn tool_check(
        tool_id: &str,
        display_name: &str,
        category: &str,
        is_available: bool,
    ) -> HostToolCheckResult {
        HostToolCheckResult {
            tool_id: tool_id.to_string(),
            display_name: display_name.to_string(),
            is_available,
            is_required: false,
            category: category.to_string(),
            docs_url: format!("https://example.invalid/{tool_id}"),
            tool_version: None,
            resolved_path: None,
            install_guidance: None,
        }
    }

    fn readiness_result(tool_checks: Vec<HostToolCheckResult>) -> ReadinessCheckResult {
        ReadinessCheckResult {
            checks: vec![issue("umu_run_available", HealthIssueSeverity::Info)],
            all_passed: true,
            critical_failures: 0,
            warnings: 0,
            umu_install_guidance: None,
            steam_deck_caveats: None,
            tool_checks,
            detected_distro_family: HostDistroFamily::Unknown.as_str().to_string(),
        }
    }

    #[test]
    fn parse_default_embedded_capability_map_has_expected_entries() {
        let (entries, warnings, _) =
            parse_capability_map_toml(DEFAULT_CAPABILITY_MAP_TOML, "embedded");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(entries.iter().any(|entry| entry.id == "gamescope"));
        assert!(entries.iter().any(|entry| entry.id == "prefix_tools"));
        assert!(entries.iter().any(|entry| entry.id == "non_steam_launch"));
    }

    #[test]
    fn merge_replaces_by_capability_id() {
        let merged = merge_capability_maps(
            vec![CapabilityDefinition {
                id: "gamescope".to_string(),
                label: "Gamescope".to_string(),
                category: "performance".to_string(),
                required_tools: vec!["gamescope".to_string()],
                optional_tools: vec![],
            }],
            vec![CapabilityDefinition {
                id: "gamescope".to_string(),
                label: "Patched".to_string(),
                category: "performance".to_string(),
                required_tools: vec!["gamescope".to_string()],
                optional_tools: vec![],
            }],
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].label, "Patched");
    }

    #[test]
    fn load_capability_map_user_override_replaces_default_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("host_capability_map.toml"),
            r#"
catalog_version = 7

[[capability]]
id = "gamescope"
label = "Gamescope override"
category = "performance"
required_tools = ["gamescope"]
optional_tools = []
"#,
        )
        .expect("write override");

        let loaded = load_capability_map(Some(dir.path()));
        let gamescope = loaded.find_by_id("gamescope").expect("gamescope entry");
        assert_eq!(gamescope.label, "Gamescope override");
        assert_eq!(loaded.catalog_version, 7);
    }

    #[test]
    fn load_capability_map_invalid_user_file_keeps_embedded_catalog_version() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("host_capability_map.toml"),
            "not valid toml [[[",
        )
        .expect("write invalid capability map");

        let loaded = load_capability_map(Some(dir.path()));
        let (_, _, embedded_ver) =
            parse_capability_map_toml(DEFAULT_CAPABILITY_MAP_TOML, "embedded");
        assert_eq!(loaded.catalog_version, embedded_ver.max(1));
    }

    #[test]
    fn derive_capabilities_all_available_fixture() {
        // Pin a path scope containing an umu-run stub so the live-probe
        // fallback in `synthesize_umu_run_check` deterministically returns Some,
        // independent of host state and concurrent tests that swap the scope.
        let umu_dir = tempfile::tempdir().expect("tempdir");
        let umu_stub = umu_dir.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").expect("write umu stub");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755))
            .expect("chmod umu stub");
        let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(umu_dir.path());

        let result = readiness_result(vec![
            tool_check("gamescope", "Gamescope", "performance", true),
            tool_check("mangohud", "MangoHud", "overlay", true),
            tool_check("gamemode", "GameMode", "performance", true),
            tool_check("winetricks", "Winetricks", "prefix_tools", true),
            tool_check("protontricks", "Protontricks", "prefix_tools", true),
        ]);

        let capabilities = derive_capabilities_with_map(
            &result,
            &sample_capability_map(),
            &sample_readiness_catalog(),
        );

        assert_eq!(capabilities.len(), 5);
        assert!(capabilities
            .iter()
            .all(|capability| capability.state == CapabilityState::Available));
        assert!(capabilities
            .iter()
            .all(|capability| capability.rationale.is_none()));
    }

    #[test]
    fn derive_capabilities_missing_required_fixture() {
        let mut result = readiness_result(vec![
            tool_check("gamescope", "Gamescope", "performance", true),
            tool_check("mangohud", "MangoHud", "overlay", true),
            tool_check("gamemode", "GameMode", "performance", true),
        ]);
        result.checks = vec![issue("umu_run_available", HealthIssueSeverity::Warning)];
        result.all_passed = false;
        result.warnings = 1;

        let capabilities = derive_capabilities_with_map(
            &result,
            &sample_capability_map(),
            &sample_readiness_catalog(),
        );
        let capability = capabilities
            .iter()
            .find(|capability| capability.id == "non_steam_launch")
            .expect("non_steam_launch");

        assert_eq!(capability.state, CapabilityState::Unavailable);
        assert_eq!(capability.missing_required.len(), 1);
        assert_eq!(capability.missing_required[0].tool_id, "umu_run");
        assert_eq!(
            capability.rationale.as_deref(),
            Some("Non-Steam launch is unavailable because umu-launcher is not available on the host.")
        );
        assert_eq!(capability.install_hints.len(), 1);
        assert_eq!(capability.install_hints[0].command, "install umu");
    }

    #[test]
    fn derive_capabilities_missing_optional_fixture() {
        // Pin a path scope containing umu-run so concurrent tests that swap the
        // scope to an empty dir cannot make `synthesize_umu_run_check` flip the
        // `non_steam_launch` capability state under us. This test only asserts
        // on `prefix_tools`, but holding the lock keeps the fixture isolated.
        let umu_dir = tempfile::tempdir().expect("tempdir");
        let umu_stub = umu_dir.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").expect("write umu stub");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755))
            .expect("chmod umu stub");
        let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(umu_dir.path());

        let result = readiness_result(vec![
            tool_check("gamescope", "Gamescope", "performance", true),
            tool_check("mangohud", "MangoHud", "overlay", true),
            tool_check("gamemode", "GameMode", "performance", true),
            tool_check("winetricks", "Winetricks", "prefix_tools", true),
            HostToolCheckResult {
                tool_id: "protontricks".to_string(),
                display_name: "Protontricks".to_string(),
                is_available: false,
                is_required: false,
                category: "prefix_tools".to_string(),
                docs_url: "https://example.invalid/protontricks".to_string(),
                tool_version: None,
                resolved_path: None,
                install_guidance: None,
            },
        ]);

        let capabilities = derive_capabilities_with_map(
            &result,
            &sample_capability_map(),
            &sample_readiness_catalog(),
        );
        let capability = capabilities
            .iter()
            .find(|capability| capability.id == "prefix_tools")
            .expect("prefix_tools");

        assert_eq!(capability.state, CapabilityState::Degraded);
        assert_eq!(capability.missing_optional.len(), 1);
        assert_eq!(capability.missing_optional[0].tool_id, "protontricks");
        assert_eq!(
            capability.rationale.as_deref(),
            Some("Prefix tools is degraded because optional tool Protontricks is not available on the host.")
        );
        assert_eq!(capability.install_hints.len(), 1);
        assert_eq!(capability.install_hints[0].command, "install protontricks");
    }

    #[test]
    fn derive_capabilities_empty_tool_checks_fixture() {
        // Pin the search PATH to an empty directory so the umu-run live-probe
        // fallback in `synthesize_umu_run_check` deterministically returns None,
        // independent of whether the dev/CI host has umu-run installed.
        let empty_dir = tempfile::tempdir().expect("tempdir");
        let _path_guard =
            crate::launch::test_support::ScopedCommandSearchPath::new(empty_dir.path());

        let result = ReadinessCheckResult {
            checks: Vec::new(),
            all_passed: false,
            critical_failures: 0,
            warnings: 0,
            umu_install_guidance: None,
            steam_deck_caveats: None,
            tool_checks: Vec::new(),
            detected_distro_family: HostDistroFamily::Unknown.as_str().to_string(),
        };

        let capabilities = derive_capabilities_with_map(
            &result,
            &sample_capability_map(),
            &sample_readiness_catalog(),
        );

        assert_eq!(
            capabilities
                .iter()
                .find(|capability| capability.id == "gamescope")
                .expect("gamescope")
                .state,
            CapabilityState::Unavailable
        );
        assert_eq!(
            capabilities
                .iter()
                .find(|capability| capability.id == "prefix_tools")
                .expect("prefix_tools")
                .state,
            CapabilityState::Degraded
        );
        assert_eq!(
            capabilities
                .iter()
                .find(|capability| capability.id == "non_steam_launch")
                .expect("non_steam_launch")
                .state,
            CapabilityState::Unavailable
        );
    }

    /// Regression: capabilities derived from a cached SQLite snapshot have an empty
    /// `checks` Vec (HealthIssues are not persisted). `synthesize_umu_run_check`
    /// must fall back to a live host probe in that case so umu-launcher is not
    /// reported as missing when it is actually installed on the host PATH.
    #[test]
    fn derive_capabilities_from_cached_snapshot_detects_umu_via_live_probe() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let umu_stub = tmp.path().join("umu-run");
        std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").expect("write umu stub");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755))
            .expect("chmod umu stub");
        let _path_guard = crate::launch::test_support::ScopedCommandSearchPath::new(tmp.path());

        // Mirror what `readiness_result_from_snapshot` produces for the
        // get_capabilities cached path: tool_checks present, but checks empty
        // (HealthIssues are not persisted in `host_readiness_snapshots`).
        let snapshot_result = ReadinessCheckResult {
            checks: Vec::new(),
            all_passed: false,
            critical_failures: 0,
            warnings: 0,
            umu_install_guidance: None,
            steam_deck_caveats: None,
            tool_checks: vec![
                tool_check("gamescope", "Gamescope", "performance", true),
                tool_check("mangohud", "MangoHud", "overlay", true),
                tool_check("gamemode", "GameMode", "performance", true),
            ],
            detected_distro_family: HostDistroFamily::Unknown.as_str().to_string(),
        };

        let capabilities = derive_capabilities_with_map(
            &snapshot_result,
            &sample_capability_map(),
            &sample_readiness_catalog(),
        );
        let non_steam = capabilities
            .iter()
            .find(|capability| capability.id == "non_steam_launch")
            .expect("non_steam_launch");

        assert_eq!(
            non_steam.state,
            CapabilityState::Available,
            "umu-run on PATH must be detected via live probe when cached snapshot omits HealthIssues"
        );
        assert!(
            non_steam.missing_required.is_empty(),
            "non_steam_launch should have no missing required tools when umu-run is on PATH"
        );
    }
}
