//! Hero Detail tabs and hero-image precedence (`hero-detail-model.ts`).

use crate::launch::LaunchMethod;

/// One Hero Detail tab, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(test, derive(serde::Deserialize), serde(rename_all = "kebab-case"))]
pub enum HeroTab {
    /// Overview (the default).
    #[default]
    Overview,
    /// Profiles.
    Profiles,
    /// Launch options.
    LaunchOptions,
    /// Trainer.
    Trainer,
    /// History.
    History,
    /// Compatibility.
    Compatibility,
}

impl HeroTab {
    /// Every tab, in display order.
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Profiles,
        Self::LaunchOptions,
        Self::Trainer,
        Self::History,
        Self::Compatibility,
    ];

    /// Stable id (`launch-options`, ...), as routes and saved state use it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Profiles => "profiles",
            Self::LaunchOptions => "launch-options",
            Self::Trainer => "trainer",
            Self::History => "history",
            Self::Compatibility => "compatibility",
        }
    }

    /// Tab label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Profiles => "Profiles",
            Self::LaunchOptions => "Launch options",
            Self::Trainer => "Trainer",
            Self::History => "History",
            Self::Compatibility => "Compatibility",
        }
    }

    /// The tab with exactly this id (`isHeroDetailTabId`; case-sensitive, no
    /// trimming).
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }

    /// Tab to open for a requested id: that tab when the id is valid and
    /// visible, else the default (Classic `AppShell` falls back to
    /// `overview`). Every tab is visible today (see [`visible_tabs`]), so the
    /// visibility filter only matters once the rules diverge from Classic.
    #[must_use]
    pub fn resolve(requested: Option<&str>, method: LaunchMethod, has_trainer: bool) -> Self {
        requested
            .and_then(Self::from_id)
            .filter(|tab| visible_tabs(method, has_trainer).contains(tab))
            .unwrap_or_default()
    }
}

/// Tabs shown for a profile. Classic shows all six for every launch method,
/// with or without a trainer (the Trainer tab then offers to set one up).
#[must_use]
pub const fn visible_tabs(method: LaunchMethod, has_trainer: bool) -> &'static [HeroTab] {
    let _ = (method, has_trainer);
    &HeroTab::ALL
}

/// A configured path for display, or `Not set` when blank (`displayPath`).
#[must_use]
pub fn display_path(value: Option<&str>) -> &str {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("Not set")
}

/// Load state of one hero-image source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct ImageSource<'a> {
    /// Resolved URL, if any.
    #[cfg_attr(test, serde(borrow))]
    pub url: Option<&'a str>,
    /// Still loading.
    pub loading: bool,
}

/// Inputs of [`resolve_hero_image`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct HeroImageInputs<'a> {
    /// The profile's custom background path, if any.
    #[cfg_attr(test, serde(rename = "customBgPath", borrow, default))]
    pub custom_bg_path: Option<&'a str>,
    /// Background art (custom or SteamGridDB).
    #[cfg_attr(test, serde(borrow))]
    pub bg: ImageSource<'a>,
    /// SteamGridDB hero art.
    #[cfg_attr(test, serde(borrow))]
    pub hero: ImageSource<'a>,
    /// Steam `header_image`.
    #[cfg_attr(test, serde(rename = "headerImage", borrow))]
    pub header_image: Option<&'a str>,
    /// Steam metadata still loading.
    #[cfg_attr(test, serde(rename = "metaLoading"))]
    pub meta_loading: bool,
}

/// Hero image to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct HeroImage<'a> {
    /// URL to show, if any.
    #[cfg_attr(test, serde(borrow))]
    pub url: Option<&'a str>,
    /// Show a skeleton instead.
    #[cfg_attr(test, serde(rename = "showSkeleton"))]
    pub show_skeleton: bool,
}

/// Hero image by precedence: custom background, then SteamGridDB background,
/// then SteamGridDB hero, then Steam `header_image`, then none
/// (`resolveGameDetailsHero`). A source still loading shows a skeleton.
#[must_use]
pub fn resolve_hero_image<'a>(input: &HeroImageInputs<'a>) -> HeroImage<'a> {
    let url = |url| HeroImage {
        url: Some(url),
        show_skeleton: false,
    };
    let skeleton = HeroImage {
        url: None,
        show_skeleton: true,
    };
    let set = |value: Option<&'a str>| value.filter(|v| !v.is_empty());
    let has_custom = input.custom_bg_path.is_some_and(|p| !p.trim().is_empty());
    if has_custom && (input.bg.loading || set(input.bg.url).is_some()) {
        return HeroImage {
            url: input.bg.url,
            show_skeleton: input.bg.loading,
        };
    }
    if input.bg.loading {
        return skeleton;
    }
    if let Some(bg) = set(input.bg.url) {
        return url(bg);
    }
    if input.hero.loading {
        return skeleton;
    }
    if let Some(hero) = set(input.hero.url) {
        return url(hero);
    }
    if input.meta_loading {
        return skeleton;
    }
    if let Some(header) = set(input.header_image) {
        return url(header);
    }
    HeroImage {
        url: None,
        show_skeleton: false,
    }
}

#[cfg(test)]
mod tests;
