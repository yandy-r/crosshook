//! Focus targets and zones (same set as the iced spike, so focus coverage is comparable).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    Sidebar,
    Content,
    Inspector,
    Overlay,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Nav(u8),
    Back,
    Forward,
    Search,
    Card(usize),
    Favorite(usize),
    /// Gamescope form: 0..=5 number inputs, 6 enabled switch, 7 filter select,
    /// 8..=11 flags, 12 launch optimization, 13 save, 14 pick file (same numbering as iced).
    Field(u8),
    InspectorAction(u8),
    Overlay(u8),
    Palette,
}

impl Target {
    pub fn zone(&self) -> Zone {
        match self {
            Self::Nav(_) | Self::Back | Self::Forward => Zone::Sidebar,
            Self::Search | Self::Card(_) | Self::Favorite(_) | Self::Field(_) => Zone::Content,
            Self::InspectorAction(_) => Zone::Inspector,
            Self::Overlay(_) | Self::Palette => Zone::Overlay,
        }
    }
}
