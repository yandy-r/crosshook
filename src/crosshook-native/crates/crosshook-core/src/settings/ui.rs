use std::fmt;

mod enums;
mod keys;
mod lenient;
mod palette;

pub use enums::*;
pub use keys::{UiKey, UiKeySet};
pub use palette::is_valid_palette_id;

#[cfg(test)]
mod lenient_tests;
#[cfg(test)]
mod tests;

/// Minimum saved window width, in logical pixels.
pub const UI_WINDOW_WIDTH_MIN: u32 = 800;
/// Maximum saved window width, in logical pixels.
pub const UI_WINDOW_WIDTH_MAX: u32 = 16384;
/// Minimum saved window height, in logical pixels.
pub const UI_WINDOW_HEIGHT_MIN: u32 = 600;
/// Maximum saved window height, in logical pixels.
pub const UI_WINDOW_HEIGHT_MAX: u32 = 16384;
/// Minimum UI scale factor.
pub const UI_SCALE_MIN: f64 = 0.75;
/// Maximum UI scale factor.
pub const UI_SCALE_MAX: f64 = 2.0;
/// Default UI scale factor.
pub const UI_SCALE_DEFAULT: f64 = 1.0;
/// Minimum inspector width, in logical pixels.
pub const UI_INSPECTOR_WIDTH_MIN: u32 = 240;
/// Maximum inspector width, in logical pixels.
pub const UI_INSPECTOR_WIDTH_MAX: u32 = 640;
/// Default inspector width, in logical pixels.
pub const UI_INSPECTOR_WIDTH_DEFAULT: u32 = 320;
/// Minimum console drawer height, in logical pixels.
pub const UI_CONSOLE_DRAWER_HEIGHT_MIN: u32 = 120;
/// Maximum console drawer height, in logical pixels.
pub const UI_CONSOLE_DRAWER_HEIGHT_MAX: u32 = 800;
/// Default console drawer height, in logical pixels.
pub const UI_CONSOLE_DRAWER_HEIGHT_DEFAULT: u32 = 280;
/// Maximum number of recent command palette entries.
pub const PALETTE_RECENT_MAX: usize = 20;
/// Maximum byte length of a command palette identifier.
pub const PALETTE_ID_MAX_BYTES: usize = 98;

/// Clamp a saved window width to its supported range.
pub fn clamp_window_width(n: u32) -> u32 {
    n.clamp(UI_WINDOW_WIDTH_MIN, UI_WINDOW_WIDTH_MAX)
}

/// Clamp a saved window height to its supported range.
pub fn clamp_window_height(n: u32) -> u32 {
    n.clamp(UI_WINDOW_HEIGHT_MIN, UI_WINDOW_HEIGHT_MAX)
}

/// Clamp a finite scale to its supported range; reset non-finite input to the default.
pub fn clamp_ui_scale(n: f64) -> f64 {
    if n.is_finite() {
        n.clamp(UI_SCALE_MIN, UI_SCALE_MAX)
    } else {
        UI_SCALE_DEFAULT
    }
}

/// Clamp an inspector width to its supported range.
pub fn clamp_inspector_width(n: u32) -> u32 {
    n.clamp(UI_INSPECTOR_WIDTH_MIN, UI_INSPECTOR_WIDTH_MAX)
}

/// Clamp a console drawer height to its supported range.
pub fn clamp_console_drawer_height(n: u32) -> u32 {
    n.clamp(UI_CONSOLE_DRAWER_HEIGHT_MIN, UI_CONSOLE_DRAWER_HEIGHT_MAX)
}

/// Layout and display preferences persisted in the `[ui]` settings table.
#[derive(Clone, PartialEq, serde::Serialize)]
pub struct UiSettings {
    /// Preferred library layout; absent until explicitly selected or imported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_view_mode: Option<LibraryViewMode>,
    /// Saved window width, in logical pixels; absent uses the centred default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_width: Option<u32>,
    /// Saved window height, in logical pixels; absent uses the centred default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_height: Option<u32>,
    /// UI scale factor.
    pub ui_scale: f64,
    /// Spacing density.
    pub density: UiDensity,
    /// Preferred rendering backend.
    pub renderer: UiRenderer,
    /// Controller navigation preference.
    pub controller_mode: ControllerMode,
    /// Whether the sidebar is collapsed.
    pub sidebar_collapsed: bool,
    /// Inspector width, in logical pixels.
    pub inspector_width: u32,
    /// Console drawer height, in logical pixels.
    pub console_drawer_height: u32,
    /// Recent command palette identifiers, most recent first.
    pub palette_recent: Vec<String>,
    /// Unknown `[ui]` keys, preserved verbatim.
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            library_view_mode: None,
            window_width: None,
            window_height: None,
            ui_scale: UI_SCALE_DEFAULT,
            density: UiDensity::default(),
            renderer: UiRenderer::default(),
            controller_mode: ControllerMode::default(),
            sidebar_collapsed: false,
            inspector_width: UI_INSPECTOR_WIDTH_DEFAULT,
            console_drawer_height: UI_CONSOLE_DRAWER_HEIGHT_DEFAULT,
            palette_recent: Vec::new(),
            extra: toml::Table::new(),
        }
    }
}

impl fmt::Debug for UiSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UiSettings")
            .field("library_view_mode", &self.library_view_mode)
            .field("window_width", &self.window_width)
            .field("window_height", &self.window_height)
            .field("ui_scale", &self.ui_scale)
            .field("density", &self.density)
            .field("renderer", &self.renderer)
            .field("controller_mode", &self.controller_mode)
            .field("sidebar_collapsed", &self.sidebar_collapsed)
            .field("inspector_width", &self.inspector_width)
            .field("console_drawer_height", &self.console_drawer_height)
            .field("palette_recent", &self.palette_recent.len())
            .field("extra", &self.extra.len())
            .finish()
    }
}

impl UiSettings {
    /// Resolve the library layout, defaulting to grid when unset.
    pub fn library_view_mode_or_default(&self) -> LibraryViewMode {
        self.library_view_mode.unwrap_or_default()
    }

    /// Silently clamp numbers and sanitize recent commands, returning whether anything changed.
    pub fn normalize(&mut self) -> bool {
        let window_width = self.window_width.map(clamp_window_width);
        let window_height = self.window_height.map(clamp_window_height);
        let ui_scale = clamp_ui_scale(self.ui_scale);
        let inspector_width = clamp_inspector_width(self.inspector_width);
        let console_drawer_height = clamp_console_drawer_height(self.console_drawer_height);
        let changed = self.window_width != window_width
            || self.window_height != window_height
            || self.ui_scale != ui_scale
            || self.inspector_width != inspector_width
            || self.console_drawer_height != console_drawer_height;
        self.window_width = window_width;
        self.window_height = window_height;
        self.ui_scale = ui_scale;
        self.inspector_width = inspector_width;
        self.console_drawer_height = console_drawer_height;
        let recent_len = self.palette_recent.len();
        palette::sanitize_palette_recent(&mut self.palette_recent);
        changed || self.palette_recent.len() != recent_len
    }

    /// Move a valid command identifier to the front, deduplicating and capping at 20.
    /// Invalid identifiers return `false` without changing the list.
    pub fn push_palette_recent(&mut self, id: &str) -> bool {
        palette::push_recent(&mut self.palette_recent, id)
    }

    /// Copy one preference from `src`; `WindowSize` copies both dimensions.
    pub fn copy_key_from(&mut self, src: &UiSettings, key: UiKey) {
        match key {
            UiKey::LibraryViewMode => self.library_view_mode = src.library_view_mode,
            UiKey::WindowSize => {
                self.window_width = src.window_width;
                self.window_height = src.window_height;
            }
            UiKey::UiScale => self.ui_scale = src.ui_scale,
            UiKey::Density => self.density = src.density,
            UiKey::Renderer => self.renderer = src.renderer,
            UiKey::ControllerMode => self.controller_mode = src.controller_mode,
            UiKey::SidebarCollapsed => self.sidebar_collapsed = src.sidebar_collapsed,
            UiKey::InspectorWidth => self.inspector_width = src.inspector_width,
            UiKey::ConsoleDrawerHeight => self.console_drawer_height = src.console_drawer_height,
            UiKey::PaletteRecent => self.palette_recent.clone_from(&src.palette_recent),
        }
    }
}
