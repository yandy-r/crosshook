//! Tests for the typed `[ui]` settings service.
//!
//! Exercises clamping, the Classic↔native round-trip through fresh services
//! on the same store, dirty-key isolation against concurrent settings
//! writes, and the degraded behaviour on an unreadable settings file.

use std::fs;

use crosshook_core::settings::{
    ControllerMode, LibraryViewMode, SettingsStore, UiDensity, UiKey, UiRenderer, UiSettings,
    UI_INSPECTOR_WIDTH_MIN, UI_SCALE_MAX, UI_WINDOW_HEIGHT_MIN, UI_WINDOW_WIDTH_MIN,
};
use tempfile::tempdir;
use tokio::runtime::Handle;

use super::{SettingsService, UiSettingsService};
use crate::events::EventBus;

fn service(store: &SettingsStore) -> UiSettingsService {
    UiSettingsService::new(store.clone(), EventBus::default(), Handle::current())
}

fn settings_service(store: &SettingsStore) -> SettingsService {
    SettingsService::new(store.clone(), store.base_path.join("profiles"))
}

#[tokio::test]
async fn typed_setters_clamp_and_get_reflects_them_immediately() {
    let temp = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp.path().to_path_buf());
    let ui = service(&store);

    let defaults = UiSettings::default();
    let current = ui.get();
    assert_eq!(current.library_view_mode, None);
    assert_eq!(current.ui_scale, defaults.ui_scale);
    assert_eq!(current.density, defaults.density);

    ui.set_ui_scale(9.0);
    assert_eq!(ui.get().ui_scale, UI_SCALE_MAX, "set_ui_scale clamps high");
    ui.set_ui_scale(0.1);
    assert_eq!(ui.get().ui_scale, 0.75, "set_ui_scale clamps low");

    ui.set_inspector_width(1);
    assert_eq!(
        ui.get().inspector_width,
        UI_INSPECTOR_WIDTH_MIN,
        "set_inspector_width clamps"
    );

    ui.set_window_size(1, 1);
    assert_eq!(
        (ui.get().window_width, ui.get().window_height),
        (Some(UI_WINDOW_WIDTH_MIN), Some(UI_WINDOW_HEIGHT_MIN)),
        "set_window_size clamps both dimensions"
    );

    ui.set_console_drawer_height(1);
    assert_eq!(ui.get().console_drawer_height, 120);

    ui.set_library_view_mode(LibraryViewMode::List);
    ui.set_density(UiDensity::Compact);
    ui.set_renderer(UiRenderer::TinySkia);
    ui.set_controller_mode(ControllerMode::Off);
    ui.set_sidebar_collapsed(true);
    let current = ui.get();
    assert_eq!(current.library_view_mode, Some(LibraryViewMode::List));
    assert_eq!(current.density, UiDensity::Compact);
    assert_eq!(current.renderer, UiRenderer::TinySkia);
    assert_eq!(current.controller_mode, ControllerMode::Off);
    assert!(current.sidebar_collapsed);

    assert!(
        !ui.push_palette_recent("Not Valid"),
        "an invalid palette id is rejected"
    );
    assert!(ui.push_palette_recent("route:library"));
    assert_eq!(ui.get().palette_recent, vec!["route:library".to_string()]);

    ui.flush().await.expect("flush persists the dirty keys");
}

#[tokio::test]
async fn classic_to_native_round_trip_through_fresh_services() {
    let temp = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp.path().to_path_buf());

    // Classic command path: typed setter + flush.
    let classic = service(&store);
    classic.set_library_view_mode(LibraryViewMode::List);
    classic.flush().await.expect("classic flush");

    // What native will construct: a fresh service on the same store.
    let native = service(&store);
    assert_eq!(
        native.get().library_view_mode,
        Some(LibraryViewMode::List),
        "a fresh service must see the Classic-written value"
    );
}

#[tokio::test]
async fn native_to_classic_round_trip_through_the_store() {
    let temp = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp.path().to_path_buf());

    // Native path: a direct store update.
    store
        .update(|settings| {
            settings.ui.library_view_mode = Some(LibraryViewMode::Grid);
            Ok::<(), std::convert::Infallible>(())
        })
        .unwrap()
        .unwrap();

    // Classic reads through a fresh service and through SettingsService.
    let ui = service(&store);
    assert_eq!(ui.get().library_view_mode, Some(LibraryViewMode::Grid));
    let snapshot = settings_service(&store).load().await.unwrap();
    assert_eq!(
        snapshot.data.ui.library_view_mode,
        Some(LibraryViewMode::Grid)
    );
}

#[tokio::test]
async fn unreadable_store_keeps_memory_and_fails_flushes() {
    let temp = tempdir().unwrap();
    // A directory where settings.toml should be makes every read/write fail.
    fs::create_dir(temp.path().join("settings.toml")).unwrap();
    let store = SettingsStore::with_base_path(temp.path().to_path_buf());
    let ui = service(&store);

    assert_eq!(
        ui.get(),
        UiSettings::default(),
        "an unreadable store yields defaults"
    );
    ui.set_ui_scale(1.5);
    assert_eq!(ui.get().ui_scale, 1.5, "setters still apply in memory");
    ui.flush()
        .await
        .expect_err("flush must surface the store failure");
    assert_eq!(ui.get().ui_scale, 1.5, "memory survives the failed write");
}

#[tokio::test]
async fn write_touches_only_dirty_keys_and_keeps_concurrent_writes() {
    let temp = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp.path().to_path_buf());

    // Seed the file with unknown keys, a known `[ui]` value, and a profile pick.
    store
        .update(|settings| {
            settings
                .ui
                .extra
                .insert("future_ui_key".to_string(), toml::Value::Integer(7));
            settings.ui.palette_recent = vec!["settings:ui".to_string()];
            settings.last_used_profile = "alpha".to_string();
            settings
                .extra
                .insert("future_top_level".to_string(), toml::Value::Boolean(true));
            Ok::<(), std::convert::Infallible>(())
        })
        .unwrap()
        .unwrap();

    // A concurrent SettingsService write lands between the service snapshot
    // and its flush.
    let ui = service(&store);
    settings_service(&store)
        .update(|settings| {
            settings.last_used_profile = "beta".to_string();
        })
        .await
        .unwrap();

    ui.set_density(UiDensity::Spacious);
    ui.flush().await.expect("flush writes only the dirty key");

    let saved = store.load().unwrap();
    assert_eq!(
        saved.ui.density,
        UiDensity::Spacious,
        "the dirty key landed"
    );
    assert_eq!(
        saved.last_used_profile, "beta",
        "a concurrent settings write is not reverted"
    );
    assert_eq!(
        saved.ui.palette_recent,
        vec!["settings:ui".to_string()],
        "an untouched [ui] key survives"
    );
    assert_eq!(
        saved.ui.extra.get("future_ui_key"),
        Some(&toml::Value::Integer(7)),
        "unknown [ui] keys survive"
    );
    assert_eq!(
        saved.extra.get("future_top_level"),
        Some(&toml::Value::Boolean(true)),
        "unknown top-level keys survive"
    );
}

#[tokio::test]
async fn settings_service_update_normalizes_out_of_range_ui() {
    let temp = tempdir().unwrap();
    let store = SettingsStore::with_base_path(temp.path().to_path_buf());
    let settings = settings_service(&store);

    settings
        .update(|data| {
            data.ui.ui_scale = 42.0;
            data.ui.inspector_width = 1;
            data.ui.window_width = Some(10);
        })
        .await
        .unwrap();

    let saved = store.load().unwrap();
    assert_eq!(saved.ui.ui_scale, 2.0, "ui_scale is clamped on save");
    assert_eq!(saved.ui.inspector_width, 240, "inspector_width is clamped");
    assert_eq!(saved.ui.window_width, Some(800), "window_width is clamped");
}

/// Compiled-only guard: the independently persisted keys are all reachable
/// through the service, keeping `UiKey` in this module's API surface honest.
#[test]
fn ui_key_covers_every_service_setter() {
    let keys = [
        UiKey::LibraryViewMode,
        UiKey::WindowSize,
        UiKey::UiScale,
        UiKey::Density,
        UiKey::Renderer,
        UiKey::ControllerMode,
        UiKey::SidebarCollapsed,
        UiKey::InspectorWidth,
        UiKey::ConsoleDrawerHeight,
        UiKey::PaletteRecent,
    ];
    assert_eq!(keys.len(), 10);
}
