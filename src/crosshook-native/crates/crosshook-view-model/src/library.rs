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

use std::collections::BTreeMap;

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
    /// Result of the [`LibraryEffect::SetFavorite`] for `name` and
    /// `favorite`, echoed from the effect. Replies may arrive in any order.
    /// `Ok` confirms `favorite`; `Err` keeps the confirmed value and sets
    /// [`LibraryViewModel::load_error`]. Once no save for the profile is in
    /// flight, its card shows the confirmed value.
    FavoriteSaved {
        /// Profile name from the effect.
        name: String,
        /// Requested flag from the effect.
        favorite: bool,
        /// Save outcome.
        result: Result<(), ServiceError>,
    },
    /// The bridge has subscribed to session events: requests the first
    /// running-sessions snapshot. The bridge never requests a snapshot on its
    /// own, so every reply carries a token the view-model issued.
    Start,
    /// Result of [`LibraryEffect::RequestRunningSessions`] with `token`.
    /// Only the reply to the latest request applies; others are ignored.
    RunningSnapshot {
        /// Token from the effect.
        token: u64,
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
    /// [`LibraryIntent::RunningSnapshot`] carrying `token`.
    RequestRunningSessions {
        /// Request token to echo back.
        token: u64,
    },
}

/// Favorite saves in flight for one profile.
#[derive(Debug, Clone, Copy)]
struct PendingFavorite {
    /// Last value the backend confirmed (or loaded, before any save).
    confirmed: bool,
    /// Value of the latest request; shown while saves are in flight.
    latest: bool,
    /// Saves without a reply yet; at least 1 while the entry exists.
    in_flight: usize,
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
    /// Profiles with favorite saves in flight; removed when the last reply
    /// arrives.
    pending_favorites: BTreeMap<String, PendingFavorite>,
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

    /// Card value for `name` after a save reply: the latest request while
    /// saves are in flight, the confirmed value once none are.
    fn settle_favorite(&mut self, name: &str, favorite: bool, saved: bool) {
        let Some(pending) = self.pending_favorites.get_mut(name) else {
            return;
        };
        if saved {
            pending.confirmed = favorite;
        }
        pending.in_flight -= 1;
        let shown = if pending.in_flight == 0 {
            let confirmed = pending.confirmed;
            self.pending_favorites.remove(name);
            confirmed
        } else {
            pending.latest
        };
        if let Some(card) = self.card_mut(name) {
            card.is_favorite = shown;
        }
    }

    fn request_running_sessions(&mut self) -> LibraryEffect {
        LibraryEffect::RequestRunningSessions {
            token: self.running.request_snapshot(),
        }
    }
}

impl ViewModel for LibraryViewModel {
    type Intent = LibraryIntent;
    type Effect = LibraryEffect;

    fn reduce(&mut self, intent: LibraryIntent) -> Vec<LibraryEffect> {
        match intent {
            LibraryIntent::ProfilesLoaded(Ok(mut cards)) => {
                // A load may predate saves still in flight: show their latest
                // request. Profiles with nothing pending take the loaded value.
                for card in &mut cards {
                    if let Some(pending) = self.pending_favorites.get(&card.name) {
                        card.is_favorite = pending.latest;
                    }
                }
                self.keys = select::search_keys(&cards);
                self.cards = cards;
                self.load_error = None;
                self.recompute();
            }
            LibraryIntent::ProfilesLoaded(Err(error)) => self.load_error = Some(error),
            LibraryIntent::Refresh => return vec![LibraryEffect::LoadProfiles],
            LibraryIntent::Start => return vec![self.request_running_sessions()],
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
                let confirmed = card.is_favorite;
                card.is_favorite = !card.is_favorite;
                let favorite = card.is_favorite;
                let pending =
                    self.pending_favorites
                        .entry(name.clone())
                        .or_insert(PendingFavorite {
                            confirmed,
                            latest: favorite,
                            in_flight: 0,
                        });
                pending.latest = favorite;
                pending.in_flight += 1;
                self.recompute();
                return vec![LibraryEffect::SetFavorite { name, favorite }];
            }
            LibraryIntent::FavoriteSaved {
                name,
                favorite,
                result,
            } => {
                let saved = result.is_ok();
                self.settle_favorite(&name, favorite, saved);
                if let Err(error) = result {
                    self.load_error = Some(error);
                }
                self.recompute();
            }
            LibraryIntent::RunningSnapshot {
                token,
                sessions,
                seq,
            } => {
                if self.running.apply_snapshot(token, sessions, seq) {
                    self.recompute_if_running_filter();
                }
            }
            LibraryIntent::Session(event) => {
                let request = self.running.apply(event);
                self.recompute_if_running_filter();
                if let Some(token) = request {
                    return vec![LibraryEffect::RequestRunningSessions { token }];
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
