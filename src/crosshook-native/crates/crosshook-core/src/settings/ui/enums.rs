/// Layout used to display the library.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum LibraryViewMode {
    /// Display library entries as a grid.
    #[default]
    Grid,
    /// Display library entries as a list.
    List,
}

impl LibraryViewMode {
    /// Return the persisted string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
        }
    }

    /// Parse an exact persisted value, without trimming or case folding.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "grid" => Some(Self::Grid),
            "list" => Some(Self::List),
            _ => None,
        }
    }
}

/// Spacing density of the UI.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum UiDensity {
    /// Use compact spacing.
    Compact,
    /// Use standard spacing.
    #[default]
    Comfortable,
    /// Use generous spacing.
    Spacious,
}

impl UiDensity {
    /// Return the persisted string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Comfortable => "comfortable",
            Self::Spacious => "spacious",
        }
    }

    /// Parse an exact persisted value, without trimming or case folding.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "compact" => Some(Self::Compact),
            "comfortable" => Some(Self::Comfortable),
            "spacious" => Some(Self::Spacious),
            _ => None,
        }
    }
}

/// Preferred backend for rendering the native UI.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum UiRenderer {
    /// Select a backend automatically.
    #[default]
    Auto,
    /// Use the wgpu backend.
    Wgpu,
    /// Use the OpenGL backend.
    Gl,
    /// Use the software tiny-skia backend.
    #[serde(rename = "tiny-skia")]
    TinySkia,
}

impl UiRenderer {
    /// Return the persisted string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Wgpu => "wgpu",
            Self::Gl => "gl",
            Self::TinySkia => "tiny-skia",
        }
    }

    /// Parse an exact persisted value, without trimming or case folding.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "wgpu" => Some(Self::Wgpu),
            "gl" => Some(Self::Gl),
            "tiny-skia" => Some(Self::TinySkia),
            _ => None,
        }
    }
}

/// Preference for controller navigation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ControllerMode {
    /// Enable controller navigation automatically.
    #[default]
    Auto,
    /// Always enable controller navigation.
    On,
    /// Disable controller navigation.
    Off,
}

impl ControllerMode {
    /// Return the persisted string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }

    /// Parse an exact persisted value, without trimming or case folding.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "auto" => Some(Self::Auto),
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}
