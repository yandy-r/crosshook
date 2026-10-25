//! List, load, summaries, favorites list and favorite toggle facades.
//!
//! Reads never take the writer lock. The favorite toggle is metadata-only:
//! it goes through `commit` for serialization and the `FavoriteToggled`
//! event, but writes no TOML, captures no revision and leaves cached health
//! untouched (D5).

use crosshook_core::profile::{
    resolve_art_app_id, CollectionDefaultsSection, GameProfile, ProfileStore,
};

use super::run_blocking;
use super::types::{Persisted, ProfileCommit, ProfileMutation, ProfileSummary};
use super::writer::CommitContext;
use super::ProfileWriter;
use crate::{AppError, AppErrorKind, ProfileChangeReason};

impl ProfileWriter {
    /// Lists profile names (file stems), sorted.
    ///
    /// # Errors
    /// Filesystem failure reading the profiles directory.
    pub async fn list(&self) -> Result<Vec<String>, AppError> {
        let profiles = self.profiles.clone();
        run_blocking(move || Ok(profiles.list()?)).await
    }

    /// Loads `name` as stored, keeping unknown TOML keys. Callers apply any
    /// collection layer and clear extras before crossing the IPC boundary.
    ///
    /// # Errors
    /// Invalid name, missing profile, unreadable or invalid TOML.
    pub async fn load(&self, name: String) -> Result<GameProfile, AppError> {
        let profiles = self.profiles.clone();
        run_blocking(move || Ok(profiles.load(&name)?)).await
    }

    /// Builds library card summaries for every loadable profile; profiles
    /// that fail to load are skipped with a warning.
    ///
    /// `resolve_collection_defaults` supplies the collection launch-defaults
    /// layer (collection composition stays with the caller until M1-35). It
    /// runs at most once, and only when at least one profile loaded, so an
    /// empty library never surfaces a collection error.
    ///
    /// # Errors
    /// Listing failure, or the error returned by `resolve_collection_defaults`.
    pub async fn list_summaries<F>(
        &self,
        resolve_collection_defaults: F,
    ) -> Result<Vec<ProfileSummary>, AppError>
    where
        F: FnOnce() -> Result<Option<CollectionDefaultsSection>, AppError> + Send + 'static,
    {
        let profiles = self.profiles.clone();
        run_blocking(move || build_summaries(&profiles, resolve_collection_defaults)).await
    }

    /// Lists favorite profile names. A disabled metadata store has no
    /// favorites and returns an empty list.
    ///
    /// # Errors
    /// Metadata database failure.
    pub async fn list_favorites(&self) -> Result<Vec<String>, AppError> {
        let metadata = self.metadata.clone();
        run_blocking(move || Ok(metadata.list_favorite_profiles()?)).await
    }

    /// Sets or clears the favorite flag of `name` in metadata and publishes
    /// `FavoriteToggled`. Never touches the profile TOML, config revisions or
    /// cached health.
    ///
    /// # Errors
    /// `Validation` for an invalid name; `Unavailable` when the metadata
    /// store is disabled (D6); metadata database failure.
    pub async fn set_favorite(
        &self,
        name: String,
        favorite: bool,
    ) -> Result<ProfileCommit<()>, AppError> {
        let name = ProfileStore::validate_profile_name(&name)?;
        self.commit(
            SetFavorite { name, favorite },
            ProfileChangeReason::FavoriteToggled,
        )
        .await
    }
}

fn build_summaries<F>(
    profiles: &ProfileStore,
    resolve_collection_defaults: F,
) -> Result<Vec<ProfileSummary>, AppError>
where
    F: FnOnce() -> Result<Option<CollectionDefaultsSection>, AppError>,
{
    let names = profiles.list()?;
    let mut resolve = Some(resolve_collection_defaults);
    let mut defaults = None;
    let mut summaries = Vec::with_capacity(names.len());
    for name in names {
        let profile = match profiles.load(&name) {
            Ok(profile) => profile,
            Err(error) => {
                tracing::warn!(profile_name = %name, %error, "skipping profile in summaries");
                continue;
            }
        };
        if let Some(resolve) = resolve.take() {
            defaults = resolve()?;
        }
        let layered = match defaults.as_ref() {
            Some(defaults) => profile.effective_profile_with(Some(defaults)),
            None => profile,
        };
        summaries.push(summarize(name, &layered.effective_profile()));
    }
    Ok(summaries)
}

fn summarize(name: String, effective: &GameProfile) -> ProfileSummary {
    let non_empty = |path: &str| {
        let path = path.trim();
        (!path.is_empty()).then(|| path.to_string())
    };
    ProfileSummary {
        name,
        game_name: effective.game.name.clone(),
        steam_app_id: resolve_art_app_id(effective).to_string(),
        custom_cover_art_path: non_empty(&effective.game.custom_cover_art_path),
        custom_portrait_art_path: non_empty(&effective.game.custom_portrait_art_path),
        network_isolation: effective.launch.network_isolation,
    }
}

/// Holds a name already validated by [`ProfileWriter::set_favorite`].
struct SetFavorite {
    name: String,
    favorite: bool,
}

impl ProfileMutation for SetFavorite {
    type Output = ();

    fn accepts(&self, reason: &ProfileChangeReason) -> bool {
        matches!(reason, ProfileChangeReason::FavoriteToggled)
    }

    fn persist(self, ctx: &CommitContext<'_>) -> Result<Persisted<()>, AppError> {
        if !ctx.metadata.is_available() {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "favorites require the metadata store, which is unavailable",
            ));
        }
        ctx.metadata
            .set_profile_favorite(&self.name, self.favorite)?;
        Ok(Persisted {
            output: (),
            effects: Vec::new(),
            names: Some(vec![self.name]),
        })
    }
}
