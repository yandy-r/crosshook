/// A preference that can be persisted independently of other UI settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKey {
    /// Library layout preference.
    LibraryViewMode,
    /// Both saved window dimensions.
    WindowSize,
    /// UI scale factor.
    UiScale,
    /// Spacing density.
    Density,
    /// Rendering backend preference.
    Renderer,
    /// Controller navigation preference.
    ControllerMode,
    /// Sidebar collapsed state.
    SidebarCollapsed,
    /// Inspector width.
    InspectorWidth,
    /// Console drawer height.
    ConsoleDrawerHeight,
    /// Recent command palette identifiers.
    PaletteRecent,
}

const ALL_KEYS: [UiKey; 10] = [
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

/// Allocation-free set of independently persisted UI preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiKeySet(u16);

impl UiKeySet {
    /// Create an empty set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Mark a preference as present.
    pub fn insert(&mut self, key: UiKey) {
        self.0 |= 1 << key as u16;
    }

    /// Return whether a preference is present.
    pub fn contains(self, key: UiKey) -> bool {
        self.0 & (1 << key as u16) != 0
    }

    /// Return whether no preferences are present.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Iterate over present preferences in declaration order, once each.
    pub fn iter(self) -> impl Iterator<Item = UiKey> {
        ALL_KEYS.into_iter().filter(move |key| self.contains(*key))
    }
}
