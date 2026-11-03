//! Library screen view-model: search, filter, sort and the Currently Playing count.
//!
//! Parity oracle: React `LibraryPage.tsx`, `useLibraryProfiles.ts` and
//! `runningSessionsStore.ts`. All state is runtime-only (nothing persists).
//!
//! ```
//! use crosshook_view_model::library::{LibraryEffect, LibraryIntent, LibraryViewModel, ProfileCard};
//! use crosshook_view_model::ViewModel;
//!
//! let mut vm = LibraryViewModel::default();
//! assert_eq!(vm.reduce(LibraryIntent::Refresh), [LibraryEffect::LoadProfiles]);
//! let card = |name: &str| ProfileCard {
//!     name: name.into(),
//!     game_name: String::new(),
//!     steam_app_id: String::new(),
//!     is_favorite: false,
//! };
//! vm.reduce(LibraryIntent::ProfilesLoaded(Ok(vec![card("alpha"), card("beta")])));
//! vm.reduce(LibraryIntent::SetQuery("  BE ".into()));
//! assert_eq!(vm.visible_names().collect::<Vec<_>>(), ["beta"]);
//! ```

use std::collections::{BTreeMap, VecDeque};

use crate::{ServiceError, ViewModel};

#[cfg(feature = "app")]
mod app;
pub mod select;
mod sessions;

#[cfg(feature = "app")]
pub use app::{running_snapshot, session_event};
pub use sessions::{RunningProfiles, RunningSession, SessionEvent, SessionId, SessionKind};

/// One library card, as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileCard {
    /// Profile name (file stem); unique within the library.
    pub name: String,
    /// Effective game display name; may be empty.
    pub game_name: String,
    /// Trimmed Steam App ID used for art; may be empty.
    pub steam_app_id: String,
    /// Whether the profile is a favorite.
    pub is_favorite: bool,
}

/// Which cards the list keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LibraryFilter {
    /// Every card.
    #[default]
    All,
    /// Favorites only.
    Favorites,
    /// Cards with a Steam App ID that is non-empty and not `"0"`. This means
    /// "has a Steam/art app id", not "installed on disk" (React parity).
    Installed,
    /// Cards whose exact name has a live game session.
    CurrentlyRunning,
}

/// List order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LibrarySort {
    /// Source order (the backend's profile-name byte order).
    #[default]
    Recent,
    /// By display name; see [`select::name_order`].
    Name,
}

/// User actions and service results the library accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryIntent {
    /// Result of [`LibraryEffect::LoadProfiles`]. `Ok` replaces the list and
    /// clears [`LibraryViewModel::load_error`]; `Err` keeps the previous list
    /// and sets it.
    ProfilesLoaded(Result<Vec<ProfileCard>, ServiceError>),
    /// Reload the profiles.
    Refresh,
    /// New search text (trimmed and lowercased when matching).
    SetQuery(String),
    /// New filter.
    SetFilter(LibraryFilter),
    /// New sort.
    SetSort(LibrarySort),
    /// Flip the favorite flag of the named card (optimistic).
    ToggleFavorite(String),
    /// Result of [`LibraryEffect::SetFavorite`]. `Err` reverts that card's
    /// flag and sets [`LibraryViewModel::load_error`].
    FavoriteSaved {
        /// Profile name the save was for.
        name: String,
        /// Save outcome.
        result: Result<(), ServiceError>,
    },
    /// Result of [`LibraryEffect::RequestRunningSessions`] (or the bridge's
    /// initial request, made after subscribing to session events).
    RunningSnapshot {
        /// Live sessions.
        sessions: Vec<RunningSession>,
        /// Registry change sequence the sessions reflect.
        seq: u64,
    },
    /// A session event from the app event bus.
    Session(SessionEvent),
}

/// Service calls the library asks the bridge to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryEffect {
    /// Load profile summaries and favorites; answer with
    /// [`LibraryIntent::ProfilesLoaded`].
    LoadProfiles,
    /// Persist a favorite flag; answer with [`LibraryIntent::FavoriteSaved`].
    SetFavorite {
        /// Profile name.
        name: String,
        /// New favorite flag.
        favorite: bool,
    },
    /// Read a running-sessions snapshot; answer with
    /// [`LibraryIntent::RunningSnapshot`].
    RequestRunningSessions,
}

/// Library screen state.
///
/// The visible list is cached and recomputed inside
/// [`reduce`](ViewModel::reduce) when it can change, so selectors are cheap
/// enough to call every frame.
#[derive(Debug, Clone, Default)]
pub struct LibraryViewModel {
    cards: Vec<ProfileCard>,
    /// Lowercase search keys from [`select::search_keys`], built once per load.
    keys: Vec<String>,
    query: String,
    filter: LibraryFilter,
    sort: LibrarySort,
    load_error: Option<ServiceError>,
    running: RunningProfiles,
    /// Favorite values requested per profile, oldest first, awaiting
    /// [`LibraryIntent::FavoriteSaved`].
    pending_favorites: BTreeMap<String, VecDeque<bool>>,
    visible: Vec<usize>,
}

impl LibraryViewModel {
    /// Visible cards, in display order.
    pub fn visible(&self) -> impl Iterator<Item = &ProfileCard> + '_ {
        self.visible.iter().map(|&index| &self.cards[index])
    }

    /// Names of the visible cards, in display order.
    pub fn visible_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.visible().map(|card| card.name.as_str())
    }

    /// Current search text, as typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Current filter.
    #[must_use]
    pub fn filter(&self) -> LibraryFilter {
        self.filter
    }

    /// Current sort.
    #[must_use]
    pub fn sort(&self) -> LibrarySort {
        self.sort
    }

    /// Last load or favorite-save error, shown as a banner.
    #[must_use]
    pub fn load_error(&self) -> Option<&ServiceError> {
        self.load_error.as_ref()
    }

    /// Currently Playing badge count; see
    /// [`RunningProfiles::currently_playing_count`]. The UI hides the badge at 0.
    #[must_use]
    pub fn currently_playing_count(&self) -> usize {
        self.running.currently_playing_count()
    }

    /// Whether `profile_name` has a live game session.
    #[must_use]
    pub fn is_running(&self, profile_name: &str) -> bool {
        self.running.contains(profile_name)
    }

    fn recompute(&mut self) {
        self.visible = select::visible_indices(
            &self.cards,
            &self.keys,
            &self.query,
            self.filter,
            self.sort,
            &self.running,
        );
    }

    fn recompute_if_running_filter(&mut self) {
        if self.filter == LibraryFilter::CurrentlyRunning {
            self.recompute();
        }
    }

    fn card_mut(&mut self, name: &str) -> Option<&mut ProfileCard> {
        self.cards.iter_mut().find(|card| card.name == name)
    }

    /// Pops the oldest pending favorite value requested for `name`.
    fn take_pending_favorite(&mut self, name: &str) -> Option<bool> {
        let queue = self.pending_favorites.get_mut(name)?;
        let requested = queue.pop_front();
        if queue.is_empty() {
            self.pending_favorites.remove(name);
        }
        requested
    }
}

impl ViewModel for LibraryViewModel {
    type Intent = LibraryIntent;
    type Effect = LibraryEffect;

    fn reduce(&mut self, intent: LibraryIntent) -> Vec<LibraryEffect> {
        match intent {
            LibraryIntent::ProfilesLoaded(Ok(cards)) => {
                self.keys = select::search_keys(&cards);
                self.cards = cards;
                self.load_error = None;
                self.recompute();
            }
            LibraryIntent::ProfilesLoaded(Err(error)) => self.load_error = Some(error),
            LibraryIntent::Refresh => return vec![LibraryEffect::LoadProfiles],
            LibraryIntent::SetQuery(query) => {
                self.query = query;
                self.recompute();
            }
            LibraryIntent::SetFilter(filter) => {
                self.filter = filter;
                self.recompute();
            }
            LibraryIntent::SetSort(sort) => {
                self.sort = sort;
                self.recompute();
            }
            LibraryIntent::ToggleFavorite(name) => {
                let Some(card) = self.card_mut(&name) else {
                    return Vec::new();
                };
                card.is_favorite = !card.is_favorite;
                let favorite = card.is_favorite;
                self.pending_favorites
                    .entry(name.clone())
                    .or_default()
                    .push_back(favorite);
                self.recompute();
                return vec![LibraryEffect::SetFavorite { name, favorite }];
            }
            LibraryIntent::FavoriteSaved { name, result } => {
                // Saves for one profile complete in request order. A failure
                // restores the value from before that request (React sets the
                // pre-toggle value back), so a later in-flight save still wins.
                let requested = self.take_pending_favorite(&name);
                if let Err(error) = result {
                    if let (Some(requested), Some(card)) = (requested, self.card_mut(&name)) {
                        card.is_favorite = !requested;
                    }
                    self.load_error = Some(error);
                    self.recompute();
                }
            }
            LibraryIntent::RunningSnapshot { sessions, seq } => {
                self.running.apply_snapshot(sessions, seq);
                self.recompute_if_running_filter();
            }
            LibraryIntent::Session(event) => {
                let resync = self.running.apply(event);
                self.recompute_if_running_filter();
                if resync {
                    return vec![LibraryEffect::RequestRunningSessions];
                }
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
