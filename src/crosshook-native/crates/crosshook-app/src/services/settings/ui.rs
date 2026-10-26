//! Typed `[ui]` settings service.
//!
//! Holds the current [`UiSettings`] in memory (loaded once at construction)
//! and applies typed setters immediately. Persistence is delegated to the
//! throttled [`UiWriter`](super::ui_writer::UiWriter), which writes only the
//! dirty keys through [`SettingsStore::update`].

use std::sync::Arc;

use crosshook_core::settings::{
    clamp_window_height, clamp_window_width, ControllerMode, LibraryViewMode, SettingsStore,
    UiDensity, UiKey, UiRenderer, UiSettings,
};
use tokio::runtime::Handle;

use super::run_blocking;
use super::ui_writer::{UiPatch, UiSink, UiWriter};
use crate::events::EventBus;
use crate::AppError;

/// Typed `[ui]` getters and setters over an in-memory copy, with a throttled
/// writer. Cheap to clone.
#[derive(Clone)]
pub struct UiSettingsService {
    writer: Arc<UiWriter>,
}

impl UiSettingsService {
    /// Creates the service, loading `[ui]` from `store` (defaults when the
    /// file is unreadable) and spawning the throttled writer on `runtime`.
    ///
    /// The production sink copies only the dirty keys onto the on-disk
    /// `[ui]` table through [`SettingsStore::update`], so unknown keys and
    /// concurrent settings writes are preserved.
    pub fn new(store: SettingsStore, events: EventBus, runtime: Handle) -> Self {
        let initial = store.load().map(|data| data.ui).unwrap_or_default();
        let sink: UiSink = Arc::new(move |patch: UiPatch| {
            let sink_store = store.clone();
            Box::pin(run_blocking(move || {
                sink_store
                    .update(|settings| {
                        for key in patch.keys.iter() {
                            settings.ui.copy_key_from(&patch.settings, key);
                        }
                        settings.ui.normalize();
                        Ok::<(), std::convert::Infallible>(())
                    })?
                    .unwrap_or_else(|never| match never {});
                Ok(())
            }))
        });
        Self {
            writer: Arc::new(UiWriter::spawn(initial, sink, events, &runtime)),
        }
    }

    /// Returns the current in-memory `[ui]` settings.
    pub fn get(&self) -> UiSettings {
        self.writer.snapshot()
    }

    /// Sets the preferred library layout.
    pub fn set_library_view_mode(&self, mode: LibraryViewMode) {
        self.writer.mutate(UiKey::LibraryViewMode, |ui| {
            ui.library_view_mode = Some(mode);
        });
    }

    /// Sets both saved window dimensions, clamped to their supported ranges.
    pub fn set_window_size(&self, width: u32, height: u32) {
        self.writer.mutate(UiKey::WindowSize, |ui| {
            ui.window_width = Some(clamp_window_width(width));
            ui.window_height = Some(clamp_window_height(height));
        });
    }

    /// Sets the UI scale factor; non-finite input resets to the default.
    pub fn set_ui_scale(&self, scale: f64) {
        self.writer.mutate(UiKey::UiScale, |ui| {
            ui.ui_scale = crosshook_core::settings::clamp_ui_scale(scale);
        });
    }

    /// Sets the spacing density.
    pub fn set_density(&self, density: UiDensity) {
        self.writer
            .mutate(UiKey::Density, |ui| ui.density = density);
    }

    /// Sets the preferred rendering backend.
    pub fn set_renderer(&self, renderer: UiRenderer) {
        self.writer
            .mutate(UiKey::Renderer, |ui| ui.renderer = renderer);
    }

    /// Sets the controller navigation preference.
    pub fn set_controller_mode(&self, mode: ControllerMode) {
        self.writer
            .mutate(UiKey::ControllerMode, |ui| ui.controller_mode = mode);
    }

    /// Sets whether the sidebar is collapsed.
    pub fn set_sidebar_collapsed(&self, collapsed: bool) {
        self.writer.mutate(UiKey::SidebarCollapsed, |ui| {
            ui.sidebar_collapsed = collapsed;
        });
    }

    /// Sets the inspector width, clamped to its supported range.
    pub fn set_inspector_width(&self, width: u32) {
        self.writer.mutate(UiKey::InspectorWidth, |ui| {
            ui.inspector_width = crosshook_core::settings::clamp_inspector_width(width);
        });
    }

    /// Sets the console drawer height, clamped to its supported range.
    pub fn set_console_drawer_height(&self, height: u32) {
        self.writer.mutate(UiKey::ConsoleDrawerHeight, |ui| {
            ui.console_drawer_height =
                crosshook_core::settings::clamp_console_drawer_height(height);
        });
    }

    /// Moves a valid command identifier to the front of the recent list,
    /// deduplicating and capping at 20 entries. Returns `false` when `id` is
    /// not a valid palette identifier.
    pub fn push_palette_recent(&self, id: &str) -> bool {
        self.writer.mutate(UiKey::PaletteRecent, |ui| {
            ui.push_palette_recent(id);
        })
    }

    /// Writes any pending dirty keys to disk immediately.
    ///
    /// # Errors
    /// The sanitized store error when the write fails; `Ok(())` when nothing
    /// is pending.
    pub async fn flush(&self) -> Result<(), AppError> {
        self.writer.flush().await
    }
}
