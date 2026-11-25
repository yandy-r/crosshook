//! Focus targets and zones. A target is encoded in the widget `Id` string so the tree walk
//! (`collect`) can recover it: `<zone>:<kind>[:<n>]`.

use iced::widget::Id;

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

    pub fn key(&self) -> String {
        match self {
            Self::Nav(n) => format!("nav:{n}"),
            Self::Back => "back".into(),
            Self::Forward => "fwd".into(),
            Self::Search => "search".into(),
            Self::Card(i) => format!("card:{i}"),
            Self::Favorite(i) => format!("fav:{i}"),
            Self::Field(n) => format!("field:{n}"),
            Self::InspectorAction(n) => format!("insp:{n}"),
            Self::Overlay(n) => format!("ovl:{n}"),
            Self::Palette => "palette".into(),
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        let (kind, rest) = key.split_once(':').unwrap_or((key, ""));
        let n = || rest.parse().ok();
        Some(match kind {
            "nav" => Self::Nav(n()?),
            "back" => Self::Back,
            "fwd" => Self::Forward,
            "search" => Self::Search,
            "card" => Self::Card(rest.parse().ok()?),
            "fav" => Self::Favorite(rest.parse().ok()?),
            "field" => Self::Field(n()?),
            "insp" => Self::InspectorAction(n()?),
            "ovl" => Self::Overlay(n()?),
            "palette" => Self::Palette,
            _ => return None,
        })
    }

    pub fn id(&self) -> Id {
        Id::from(format!("{}{}", PREFIX, self.key()))
    }

    /// Text inputs keep iced's own focus; the app mirrors it.
    pub fn is_text_input(&self) -> bool {
        matches!(self, Self::Search | Self::Palette) || matches!(self, Self::Field(n) if *n < 6)
    }
}

pub const PREFIX: &str = "fz:";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_round_trips() {
        for t in [
            Target::Nav(2),
            Target::Back,
            Target::Card(499),
            Target::Favorite(3),
            Target::Field(7),
            Target::InspectorAction(1),
            Target::Overlay(0),
            Target::Palette,
            Target::Search,
        ] {
            assert_eq!(Target::parse(&t.key()), Some(t));
        }
    }
}
