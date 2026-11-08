use super::*;
use crosshook_core::settings::AppSettingsData;
use std::path::PathBuf;

fn ipc_from(data: AppSettingsData) -> AppSettingsIpcData {
    AppSettingsIpcData::from_snapshot(SettingsSnapshot {
        data,
        resolved_profiles_directory: PathBuf::from("/tmp/profiles"),
        active_profiles_directory: PathBuf::from("/tmp/profiles"),
        profiles_directory_requires_restart: false,
    })
}

#[test]
fn ipc_dto_clears_unknown_keys_and_hides_key() {
    let mut data: AppSettingsData =
        toml::from_str("future = 2026-10-08\nsteamgriddb_api_key = \"secret\"\n").unwrap();
    data.extra
        .insert("future_two".to_string(), toml::Value::Boolean(true));
    let ipc = ipc_from(data);
    assert!(ipc.has_steamgriddb_api_key);
    let json = serde_json::to_string(&ipc).unwrap();
    assert!(!json.contains("future"));
    assert!(!json.contains("secret"));
}

#[test]
fn ipc_dto_carries_restart_state_from_snapshot() {
    let ipc = AppSettingsIpcData::from_snapshot(SettingsSnapshot {
        data: AppSettingsData::default(),
        resolved_profiles_directory: PathBuf::from("/tmp/a"),
        active_profiles_directory: PathBuf::from("/tmp/b"),
        profiles_directory_requires_restart: true,
    });
    assert_eq!(ipc.resolved_profiles_directory, "/tmp/a");
    assert_eq!(ipc.active_profiles_directory, "/tmp/b");
    assert!(ipc.profiles_directory_requires_restart);
}

#[test]
fn ipc_dto_reduced_motion_serializes_as_string() {
    let data = AppSettingsData {
        reduced_motion: ReducedMotionPreference::Reduced,
        ..Default::default()
    };
    let ipc = ipc_from(data);
    let json = serde_json::to_string(&ipc).expect("serialization must not fail");
    assert!(
        json.contains("\"reduced_motion\":\"reduced\""),
        "serialized IPC DTO must carry the tri-state string form: {json}"
    );
}

#[test]
fn from_parts_surfaces_install_nag_dismissed_at() {
    let data = AppSettingsData {
        install_nag_dismissed_at: Some("2026-04-15T10:00:00Z".to_string()),
        ..Default::default()
    };
    let ipc = ipc_from(data);
    assert_eq!(
        ipc.install_nag_dismissed_at,
        Some("2026-04-15T10:00:00Z".to_string()),
        "from_parts must surface install_nag_dismissed_at to the IPC layer"
    );
}

#[test]
fn from_parts_install_nag_dismissed_at_none_when_unset() {
    let data = AppSettingsData {
        install_nag_dismissed_at: None,
        ..Default::default()
    };
    let ipc = ipc_from(data);
    assert!(
        ipc.install_nag_dismissed_at.is_none(),
        "from_parts must pass through None when field is unset"
    );
}

#[test]
fn settings_ipc_includes_install_nag_field_in_serialized_output() {
    let timestamp = "2026-04-15T10:00:00Z".to_string();
    let data = AppSettingsData {
        install_nag_dismissed_at: Some(timestamp.clone()),
        high_contrast: HighContrastPreference::On,
        ..Default::default()
    };
    let ipc = ipc_from(data);
    let json = serde_json::to_string(&ipc).expect("serialization must not fail");
    assert!(
        json.contains("install_nag_dismissed_at"),
        "serialized IPC DTO must contain the install_nag_dismissed_at key"
    );
    assert!(
        json.contains(&timestamp),
        "serialized IPC DTO must contain the timestamp value"
    );
    assert!(
        json.contains("high_contrast"),
        "serialized IPC DTO must include the high_contrast key"
    );
    assert!(
        json.contains("\"high_contrast\":\"on\""),
        "serialized IPC DTO must carry the tri-state string form"
    );
}

#[test]
fn settings_ipc_install_nag_null_serializes_as_json_null() {
    let data = AppSettingsData {
        install_nag_dismissed_at: None,
        ..Default::default()
    };
    let ipc = ipc_from(data);
    let json = serde_json::to_string(&ipc).expect("serialization must not fail");
    assert!(
        json.contains("install_nag_dismissed_at"),
        "serialized IPC DTO must always contain the install_nag_dismissed_at key (as null when unset)"
    );
}

#[test]
fn serialization_contains_steam_deck_caveats_dismissed_at() {
    let timestamp = "2026-04-15T12:00:00Z".to_string();
    let data = AppSettingsData {
        steam_deck_caveats_dismissed_at: Some(timestamp.clone()),
        ..Default::default()
    };
    let ipc = ipc_from(data);
    let json = serde_json::to_string(&ipc).expect("serialization must not fail");
    assert!(
        json.contains("steam_deck_caveats_dismissed_at"),
        "serialized IPC DTO must contain the steam_deck_caveats_dismissed_at key"
    );
    assert!(
        json.contains(&timestamp),
        "serialized IPC DTO must contain the timestamp value"
    );
}

#[test]
fn settings_ipc_includes_host_tool_dashboard_fields_in_serialized_output() {
    let data = AppSettingsData {
        host_tool_dashboard_dismissed_hints: vec!["gamescope".to_string()],
        host_tool_dashboard_default_category_filter: Some("runtime".to_string()),
        ..Default::default()
    };
    let ipc = ipc_from(data);
    assert_eq!(
        ipc.host_tool_dashboard_dismissed_hints,
        vec!["gamescope".to_string()]
    );
    assert_eq!(
        ipc.host_tool_dashboard_default_category_filter.as_deref(),
        Some("runtime")
    );
    let json = serde_json::to_string(&ipc).expect("serialization must not fail");
    assert!(
        json.contains("host_tool_dashboard_dismissed_hints"),
        "serialized IPC DTO must contain host_tool_dashboard_dismissed_hints"
    );
    assert!(
        json.contains("host_tool_dashboard_default_category_filter"),
        "serialized IPC DTO must contain host_tool_dashboard_default_category_filter"
    );
    assert!(json.contains("gamescope"));
    assert!(json.contains("runtime"));
}

#[test]
fn ipc_dto_serializes_ui_without_unknown_keys() {
    let data: AppSettingsData =
        toml::from_str("[ui]\nlibrary_view_mode = \"list\"\nfuture = 1\n").unwrap();
    let json = serde_json::to_string(&ipc_from(data)).unwrap();
    assert!(json.contains("\"library_view_mode\":\"list\""), "{json}");
    assert!(!json.contains("future"), "{json}");
}

#[test]
fn ipc_dto_default_ui_omits_library_view_mode() {
    let value = serde_json::to_value(ipc_from(AppSettingsData::default())).unwrap();
    let ui = serde_json::to_value(UiSettings::default()).unwrap();
    assert_eq!(value["ui"], ui);
    let ui = value["ui"].as_object().expect("ui must be an object");
    assert!(!ui.contains_key("library_view_mode"));
    for key in [
        "ui_scale",
        "density",
        "renderer",
        "controller_mode",
        "sidebar_collapsed",
        "inspector_width",
        "console_drawer_height",
        "palette_recent",
    ] {
        assert!(ui.contains_key(key), "missing default ui key {key}");
    }
}
