use super::*;
use crate::settings::AppSettingsData;

fn populated_ui() -> UiSettings {
    UiSettings {
        library_view_mode: Some(LibraryViewMode::List),
        window_width: Some(1440),
        window_height: Some(900),
        ui_scale: 1.25,
        density: UiDensity::Compact,
        renderer: UiRenderer::TinySkia,
        controller_mode: ControllerMode::On,
        sidebar_collapsed: true,
        inspector_width: 480,
        console_drawer_height: 400,
        palette_recent: vec!["route:library".into(), "profile:launch-current".into()],
        extra: toml::Table::new(),
    }
}

#[test]
fn every_ui_key_round_trips_and_second_save_is_stable() -> Result<(), Box<dyn std::error::Error>> {
    let mut settings = AppSettingsData {
        ui: populated_ui(),
        ..Default::default()
    };
    settings.ui.extra.insert("future".into(), true.into());

    let serialized = toml::to_string_pretty(&settings)?;
    let restored: AppSettingsData = toml::from_str(&serialized)?;

    assert_eq!(restored, settings);
    assert_eq!(toml::to_string_pretty(&restored)?, serialized);
    assert!(serialized.contains("renderer = \"tiny-skia\""));
    assert!(!serialized.contains("tiny_skia"));
    Ok(())
}

#[test]
fn unset_options_are_omitted_and_effective_library_mode_is_grid(
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = UiSettings::default();
    let serialized = toml::to_string_pretty(&settings)?;

    for key in ["library_view_mode", "window_width", "window_height"] {
        assert!(!serialized.contains(key), "unexpected optional key: {key}");
    }
    assert_eq!(toml::from_str::<UiSettings>(&serialized)?, settings);
    assert_eq!(
        settings.library_view_mode_or_default(),
        LibraryViewMode::Grid
    );
    assert_eq!(
        populated_ui().library_view_mode_or_default(),
        LibraryViewMode::List
    );
    Ok(())
}

#[test]
fn all_enum_values_have_exact_persisted_spellings() -> Result<(), Box<dyn std::error::Error>> {
    macro_rules! check_values {
        ($field:ident, $ty:ident, [$($variant:ident),+]) => {
            for value in [$($ty::$variant),+] {
                assert_eq!($ty::parse(value.as_str()), Some(value));
                assert_eq!($ty::parse(&format!(" {}", value.as_str())), None);
                assert_eq!($ty::parse(&value.as_str().to_uppercase()), None);
                let settings = UiSettings { $field: value, ..Default::default() };
                let serialized = toml::to_string_pretty(&settings)?;
                assert_eq!(toml::from_str::<UiSettings>(&serialized)?, settings);
            }
        };
    }
    for mode in [LibraryViewMode::Grid, LibraryViewMode::List] {
        assert_eq!(LibraryViewMode::parse(mode.as_str()), Some(mode));
        let settings = UiSettings {
            library_view_mode: Some(mode),
            ..Default::default()
        };
        assert_eq!(
            toml::from_str::<UiSettings>(&toml::to_string_pretty(&settings)?)?,
            settings
        );
    }
    assert_eq!(LibraryViewMode::parse(" Grid"), None);
    assert_eq!(LibraryViewMode::parse("grid "), None);
    assert_eq!(UiRenderer::parse("tiny_skia"), None);
    check_values!(density, UiDensity, [Compact, Comfortable, Spacious]);
    check_values!(renderer, UiRenderer, [Auto, Wgpu, Gl, TinySkia]);
    check_values!(controller_mode, ControllerMode, [Auto, On, Off]);
    Ok(())
}

#[test]
fn numeric_clamps_preserve_boundaries_and_clamp_outside_them() {
    type ClampCase = (fn(u32) -> u32, u32, u32);
    let cases: [ClampCase; 4] = [
        (clamp_window_width, UI_WINDOW_WIDTH_MIN, UI_WINDOW_WIDTH_MAX),
        (
            clamp_window_height,
            UI_WINDOW_HEIGHT_MIN,
            UI_WINDOW_HEIGHT_MAX,
        ),
        (
            clamp_inspector_width,
            UI_INSPECTOR_WIDTH_MIN,
            UI_INSPECTOR_WIDTH_MAX,
        ),
        (
            clamp_console_drawer_height,
            UI_CONSOLE_DRAWER_HEIGHT_MIN,
            UI_CONSOLE_DRAWER_HEIGHT_MAX,
        ),
    ];
    for (clamp, min, max) in cases {
        for (input, expected) in [(min - 1, min), (min, min), (max, max), (max + 1, max)] {
            assert_eq!(clamp(input), expected, "input: {input}");
        }
        assert_eq!(clamp(0), min);
        assert_eq!(clamp(u32::MAX), max);
    }
    for (input, expected) in [
        (UI_SCALE_MIN - 0.25, UI_SCALE_MIN),
        (UI_SCALE_MIN, UI_SCALE_MIN),
        (UI_SCALE_MAX, UI_SCALE_MAX),
        (UI_SCALE_MAX + 0.25, UI_SCALE_MAX),
        (f64::NAN, UI_SCALE_DEFAULT),
        (f64::INFINITY, UI_SCALE_DEFAULT),
        (f64::NEG_INFINITY, UI_SCALE_DEFAULT),
    ] {
        assert_eq!(clamp_ui_scale(input), expected, "input: {input}");
    }
}

#[test]
fn normalize_clamps_every_number_sanitizes_palette_and_reports_only_changes() {
    let mut settings = UiSettings {
        window_width: Some(0),
        window_height: Some(u32::MAX),
        ui_scale: f64::NAN,
        inspector_width: 0,
        console_drawer_height: u32::MAX,
        palette_recent: vec![
            "route:library".into(),
            "invalid id".into(),
            "route:library".into(),
        ],
        ..Default::default()
    };

    assert!(settings.normalize());
    assert_eq!(settings.window_width, Some(UI_WINDOW_WIDTH_MIN));
    assert_eq!(settings.window_height, Some(UI_WINDOW_HEIGHT_MAX));
    assert_eq!(settings.ui_scale, UI_SCALE_DEFAULT);
    assert_eq!(settings.inspector_width, UI_INSPECTOR_WIDTH_MIN);
    assert_eq!(settings.console_drawer_height, UI_CONSOLE_DRAWER_HEIGHT_MAX);
    assert_eq!(settings.palette_recent, ["route:library"]);
    assert!(!settings.normalize());
    assert!(!UiSettings::default().normalize());

    let mut palette_only = UiSettings {
        palette_recent: vec!["route:library".into(), "route:library".into()],
        ..Default::default()
    };
    assert!(palette_only.normalize());
    assert!(!palette_only.normalize());
}

#[test]
fn palette_normalization_keeps_first_occurrence_and_caps_at_twenty() {
    let expected: Vec<String> = (0..PALETTE_RECENT_MAX)
        .map(|n| format!("route:item-{n}"))
        .collect();
    let mut settings = UiSettings::default();
    for n in 0..25 {
        let id = format!("route:item-{n}");
        settings.palette_recent.push(id.clone());
        settings.palette_recent.push(id);
    }

    assert!(settings.normalize());
    assert_eq!(settings.palette_recent, expected);
    assert!(!settings.normalize());
}

#[test]
fn palette_push_moves_to_front_deduplicates_and_caps_at_twenty() {
    let mut settings = UiSettings::default();
    for n in 0..25 {
        assert!(settings.push_palette_recent(&format!("route:item-{n}")));
    }
    let expected: Vec<String> = (5..25).rev().map(|n| format!("route:item-{n}")).collect();
    assert_eq!(settings.palette_recent, expected);

    assert!(settings.push_palette_recent("route:item-12"));
    let mut moved = expected;
    moved.retain(|id| id != "route:item-12");
    moved.insert(0, "route:item-12".into());
    assert_eq!(settings.palette_recent, moved);
    assert!(settings.push_palette_recent("route:item-12"));
    assert_eq!(settings.palette_recent, moved);
}

#[test]
fn palette_push_rejects_invalid_identifiers_without_changing_the_list() {
    let mut settings = UiSettings::default();
    for id in [
        "route:library",
        "profile:launch-current",
        "settings:ui:density",
    ] {
        assert!(settings.push_palette_recent(id));
        assert_eq!(
            settings.palette_recent.first().map(String::as_str),
            Some(id)
        );
    }
    let before = settings.palette_recent.clone();
    let oversized = "a".repeat(PALETTE_ID_MAX_BYTES + 1);
    for id in [
        "/etc/passwd",
        "~/x",
        r"C:\x",
        "Route:Library",
        "has space",
        &oversized,
        "a:b:c:d",
    ] {
        assert!(
            !settings.push_palette_recent(id),
            "accepted invalid id: {id}"
        );
        assert_eq!(settings.palette_recent, before);
    }
}

#[test]
fn debug_reports_counts_without_palette_ids_or_unknown_keys_and_values() {
    let mut settings = AppSettingsData {
        steamgriddb_api_key: Some("debug-api-key-sentinel".into()),
        ui: populated_ui(),
        ..Default::default()
    };
    settings
        .extra
        .insert("top-secret-key".into(), "top-secret-value".into());
    settings
        .ui
        .extra
        .insert("ui-secret-key".into(), "ui-secret-value".into());

    for debug in [format!("{:?}", settings.ui), format!("{settings:?}")] {
        for secret in [
            "debug-api-key-sentinel",
            "top-secret-key",
            "top-secret-value",
            "ui-secret-key",
            "ui-secret-value",
            "route:library",
            "profile:launch-current",
        ] {
            assert!(!debug.contains(secret), "Debug leaked {secret}");
        }
        assert!(debug.contains("palette_recent: 2"));
        assert!(debug.contains("extra: 1"));
        assert!(debug.contains("inspector_width: 480"));
    }
}

#[test]
fn key_sets_iterate_each_inserted_key_once_and_copy_only_that_preference(
) -> Result<(), Box<dyn std::error::Error>> {
    let cases: [(UiKey, &[&str]); 10] = [
        (UiKey::LibraryViewMode, &["library_view_mode"]),
        (UiKey::WindowSize, &["window_width", "window_height"]),
        (UiKey::UiScale, &["ui_scale"]),
        (UiKey::Density, &["density"]),
        (UiKey::Renderer, &["renderer"]),
        (UiKey::ControllerMode, &["controller_mode"]),
        (UiKey::SidebarCollapsed, &["sidebar_collapsed"]),
        (UiKey::InspectorWidth, &["inspector_width"]),
        (UiKey::ConsoleDrawerHeight, &["console_drawer_height"]),
        (UiKey::PaletteRecent, &["palette_recent"]),
    ];
    let mut source = populated_ui();
    source.extra.insert("source-extra".into(), true.into());
    let source_table = toml::Table::try_from(&source)?;
    let mut keys = UiKeySet::empty();
    assert!(keys.is_empty());
    assert_eq!(keys.iter().count(), 0);
    assert_eq!(keys, UiKeySet::default());

    for (key, fields) in cases {
        let mut destination = UiSettings::default();
        destination
            .extra
            .insert("destination-extra".into(), false.into());
        let mut expected = toml::Table::try_from(&destination)?;
        for field in fields {
            expected.insert((*field).into(), source_table[*field].clone());
        }

        destination.copy_key_from(&source, key);
        assert_eq!(toml::Table::try_from(&destination)?, expected);
        assert!(!keys.contains(key));
        keys.insert(key);
        keys.insert(key);
        assert!(keys.contains(key));
    }
    assert!(!keys.is_empty());
    assert_eq!(keys.iter().collect::<Vec<_>>(), cases.map(|(key, _)| key));
    Ok(())
}
