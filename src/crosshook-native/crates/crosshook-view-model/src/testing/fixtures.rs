//! Fixture builders for view-model tests.
//!
//! ```
//! use crosshook_view_model::library::SessionKind;
//! use crosshook_view_model::testing::fixtures::{card, started};
//!
//! let alpha = card("alpha").game_name("Alpha Game").steam_app_id("123").favorite().build();
//! assert!(alpha.is_favorite);
//! let _event = started("s1", Some("alpha"), SessionKind::Game, 1);
//! ```

use crate::library::{ProfileCard, RunningSession, SessionEvent, SessionId, SessionKind};

/// Starts a [`ProfileCard`] named `name` with an empty game name, no Steam
/// App ID, and not a favorite.
pub fn card(name: &str) -> CardBuilder {
    CardBuilder(ProfileCard {
        name: name.to_owned(),
        game_name: String::new(),
        steam_app_id: String::new(),
        is_favorite: false,
    })
}

/// Builder returned by [`card`].
#[derive(Debug, Clone)]
#[must_use]
pub struct CardBuilder(ProfileCard);

impl CardBuilder {
    /// Sets the game display name.
    pub fn game_name(mut self, game_name: &str) -> Self {
        game_name.clone_into(&mut self.0.game_name);
        self
    }

    /// Sets the Steam App ID.
    pub fn steam_app_id(mut self, steam_app_id: &str) -> Self {
        steam_app_id.clone_into(&mut self.0.steam_app_id);
        self
    }

    /// Marks the card as a favorite.
    pub fn favorite(mut self) -> Self {
        self.0.is_favorite = true;
        self
    }

    /// Returns the card.
    #[must_use]
    pub fn build(self) -> ProfileCard {
        self.0
    }
}

/// A [`SessionEvent::Started`] for session `id`.
#[must_use]
pub fn started(id: &str, profile_name: Option<&str>, kind: SessionKind, seq: u64) -> SessionEvent {
    SessionEvent::Started {
        session_id: SessionId::new(id),
        profile_name: profile_name.map(str::to_owned),
        kind,
        seq,
    }
}

/// A [`SessionEvent::Exited`] for session `id`.
#[must_use]
pub fn exited(id: &str, seq: u64) -> SessionEvent {
    SessionEvent::Exited {
        session_id: SessionId::new(id),
        seq,
    }
}

/// A live [`RunningSession`] for a snapshot.
#[must_use]
pub fn running(id: &str, profile_name: Option<&str>, kind: SessionKind) -> RunningSession {
    RunningSession {
        session_id: SessionId::new(id),
        profile_name: profile_name.map(str::to_owned),
        kind,
    }
}
