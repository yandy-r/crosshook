use std::error::Error;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use crate::settings::{AppSettingsData, SettingsStore};

use super::*;

static WARNING_PARSE_LOCK: Mutex<()> = Mutex::new(());

fn parse_ui(body: &str) -> AppSettingsData {
    let _guard = WARNING_PARSE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    parse_ui_unlocked(body)
}

fn parse_ui_unlocked(body: &str) -> AppSettingsData {
    let source = format!("offline_mode = true\n[ui]\n{body}\n");
    let settings: AppSettingsData =
        toml::from_str(&source).expect("invalid [ui] values must not fail the load");
    assert!(settings.offline_mode);
    settings
}

#[test]
fn invalid_known_values_reset_or_clamp_without_affecting_other_settings() {
    let settings = parse_ui(
        r#"
library_view_mode = "kanban"
window_width = -5
window_height = 99999
ui_scale = "big"
density = 4
renderer = "vulkan"
controller_mode = true
sidebar_collapsed = "yes"
inspector_width = 1.5
console_drawer_height = 900
palette_recent = "x"
"#,
    );

    assert_eq!(settings.ui.library_view_mode, None);
    assert_eq!(settings.ui.window_width, Some(UI_WINDOW_WIDTH_MIN));
    assert_eq!(settings.ui.window_height, Some(UI_WINDOW_HEIGHT_MAX));
    assert_eq!(settings.ui.ui_scale, UI_SCALE_DEFAULT);
    assert_eq!(settings.ui.density, UiDensity::Comfortable);
    assert_eq!(settings.ui.renderer, UiRenderer::Auto);
    assert_eq!(settings.ui.controller_mode, ControllerMode::Auto);
    assert!(!settings.ui.sidebar_collapsed);
    assert_eq!(settings.ui.inspector_width, UI_INSPECTOR_WIDTH_DEFAULT);
    assert_eq!(
        settings.ui.console_drawer_height,
        UI_CONSOLE_DRAWER_HEIGHT_MAX
    );
    assert!(settings.ui.palette_recent.is_empty());
    assert!(settings.ui.extra.is_empty());
}

#[test]
fn non_table_ui_resets_to_defaults_and_keeps_top_level_values() {
    for raw in ["3", r#""string""#] {
        let source = format!("offline_mode = true\nui = {raw}\n");
        let _guard = WARNING_PARSE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let settings: AppSettingsData =
            toml::from_str(&source).expect("non-table [ui] must not fail the load");
        assert!(settings.offline_mode);
        assert_eq!(settings.ui, UiSettings::default());
    }
}

#[test]
fn window_integers_are_read_as_i64_then_clamped() {
    for (raw, expected) in [
        ("-5", UI_WINDOW_WIDTH_MIN),
        ("99999", UI_WINDOW_WIDTH_MAX),
        ("9223372036854775807", UI_WINDOW_WIDTH_MAX),
        ("-9223372036854775808", UI_WINDOW_WIDTH_MIN),
    ] {
        let settings = parse_ui(&format!("window_width = {raw}"));
        assert_eq!(settings.ui.window_width, Some(expected), "raw = {raw}");
        assert_eq!(settings.ui.density, UiDensity::Comfortable);
        assert_eq!(settings.ui.ui_scale, UI_SCALE_DEFAULT);
    }
}

#[test]
fn ui_scale_accepts_integers_and_resets_non_finite_values() {
    assert_eq!(parse_ui("ui_scale = 1").ui.ui_scale, 1.0);
    assert_eq!(parse_ui("ui_scale = 0").ui.ui_scale, UI_SCALE_MIN);
    assert_eq!(parse_ui("ui_scale = 3.5").ui.ui_scale, UI_SCALE_MAX);

    for raw in ["nan", "inf", "-inf"] {
        assert_eq!(
            parse_ui(&format!("ui_scale = {raw}")).ui.ui_scale,
            UI_SCALE_DEFAULT,
            "raw = {raw}"
        );
    }
}

#[test]
fn present_but_invalid_library_view_mode_becomes_none() {
    for raw in [r#""kanban""#, "4", "true"] {
        let settings = parse_ui(&format!("library_view_mode = {raw}"));
        assert_eq!(settings.ui.library_view_mode, None, "raw = {raw}");
    }

    let valid = parse_ui(r#"library_view_mode = "list""#);
    assert_eq!(valid.ui.library_view_mode, Some(LibraryViewMode::List));
}

#[test]
fn palette_entries_are_sanitized_deduplicated_and_capped() {
    let mut entries = vec![
        "../secret".to_string(),
        "/home/user/palette".to_string(),
        "Route:Library".to_string(),
        "route:library".to_string(),
        "route:library".to_string(),
    ];
    entries.extend((1..25).map(|index| format!("route:item-{index}")));
    let source_entries = entries
        .iter()
        .map(|entry| format!(r#""{entry}""#))
        .collect::<Vec<_>>()
        .join(", ");
    let settings = parse_ui(&format!("palette_recent = [{source_entries}]"));

    let expected = std::iter::once("route:library".to_string())
        .chain((1..20).map(|index| format!("route:item-{index}")))
        .collect::<Vec<_>>();
    assert_eq!(settings.ui.palette_recent, expected);
}

#[test]
fn palette_sanitizer_reports_only_invalid_and_truncated_counts() {
    let mut entries = vec![
        "../secret".to_string(),
        "/home/user/palette".to_string(),
        "Route:Library".to_string(),
        "route:library".to_string(),
        "route:library".to_string(),
    ];
    entries.extend((0..26).map(|index| format!("route:item-{index}")));

    let counts = palette::sanitize_palette_recent(&mut entries);
    assert_eq!(counts, (3, 7));
    assert_eq!(entries.len(), PALETTE_RECENT_MAX);
}

#[test]
fn unknown_ui_keys_and_datetime_survive_settings_store_save() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let store = SettingsStore::with_base_path(directory.path().join("crosshook"));
    std::fs::create_dir_all(&store.base_path)?;
    std::fs::write(
        store.settings_path(),
        r#"
offline_mode = true
[ui]
library_view_mode = "list"
future_datetime = 2026-10-26T10:20:30Z
future_flag = true
[ui.future_nested]
preference = "preserve-me"
"#,
    )?;

    let settings = store.load()?;
    assert!(settings.offline_mode);
    assert!(matches!(
        settings.ui.extra.get("future_datetime"),
        Some(toml::Value::Datetime(_))
    ));
    assert_eq!(
        settings
            .ui
            .extra
            .get("future_nested")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("preference"))
            .and_then(toml::Value::as_str),
        Some("preserve-me")
    );

    store.save(&settings)?;
    let saved = std::fs::read_to_string(store.settings_path())?;
    assert!(saved.contains("future_datetime = 2026-10-26T10:20:30Z"));
    assert!(saved.contains("[ui.future_nested]"));
    let reloaded = store.load()?;
    assert_eq!(reloaded.ui.extra, settings.ui.extra);
    assert!(reloaded.offline_mode);
    Ok(())
}

#[test]
fn settings_backward_compat_without_ui() -> Result<(), Box<dyn Error>> {
    let settings: AppSettingsData = toml::from_str("offline_mode = true\n")?;
    assert!(settings.offline_mode);
    assert_eq!(settings.ui, UiSettings::default());
    assert_eq!(settings.ui.library_view_mode, None);
    Ok(())
}

#[test]
fn settings_outside_ui_remain_strict() {
    assert!(toml::from_str::<AppSettingsData>(r#"high_contrast = "yes""#).is_err());
}

#[test]
fn palette_id_grammar_enforces_segments_and_ascii_case() {
    let first = format!("a{}", "a".repeat(31));
    let second = format!("0{}", "b".repeat(31));
    let third = format!("0{}", "c".repeat(31));
    let longest = format!("{first}:{second}:{third}");
    assert_eq!(longest.len(), PALETTE_ID_MAX_BYTES);
    for id in ["a", "a:0", "route:library", &longest] {
        assert!(is_valid_palette_id(id), "rejected valid id: {id}");
    }

    let long_segment = format!("a{}", "a".repeat(32));
    for id in [
        "",
        "0route:library",
        "Route:Library",
        "route:/library",
        "route:library:settings:extra",
        "route::library",
        "é",
        &long_segment,
    ] {
        assert!(!is_valid_palette_id(id), "accepted invalid id: {id}");
    }
}

#[derive(Clone, Default)]
struct SharedLogBuffer(Arc<Mutex<Vec<u8>>>);

struct SharedLogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedLogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("log buffer mutex poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLogBuffer {
    type Writer = SharedLogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        SharedLogWriter(Arc::clone(&self.0))
    }
}

#[test]
fn warnings_name_keys_without_logging_values() {
    let buffer = SharedLogBuffer::default();
    let _guard = WARNING_PARSE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(buffer.clone())
        .finish();
    let dispatcher = tracing::Dispatch::new(subscriber);
    // Dispatch registers this subscriber's callsite interest; with_default
    // scopes its writer to this test thread.
    let settings = tracing::dispatcher::with_default(&dispatcher, || {
        tracing::callsite::rebuild_interest_cache();
        parse_ui_unlocked(
            r#"
renderer = "SECRET-VALUE"
palette_recent = ["/home/secret/x"]
"#,
        )
    });
    assert_eq!(settings.ui.renderer, UiRenderer::Auto);
    assert!(settings.ui.palette_recent.is_empty());

    let bytes = buffer.0.lock().expect("log buffer mutex poisoned").clone();
    let output = String::from_utf8_lossy(&bytes);
    assert!(output.contains("renderer"), "logs: {output}");
    assert!(output.contains("palette_recent"), "logs: {output}");
    assert!(!output.contains("SECRET-VALUE"), "logs: {output}");
    assert!(!output.contains("/home/secret/x"), "logs: {output}");
    assert_eq!(
        output.matches("palette_recent").count(),
        1,
        "logs: {output}"
    );
}
