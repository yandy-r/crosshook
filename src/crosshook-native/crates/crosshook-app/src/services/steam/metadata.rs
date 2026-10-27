use std::future::Future;

use crosshook_core::game_images::{download_and_cache_image, import_custom_art, GameImageType};
use crosshook_core::metadata::MetadataStore;
use crosshook_core::settings::SettingsStore;
use crosshook_core::steam_metadata::{lookup_steam_metadata, SteamMetadataLookupResult};

use super::SteamService;
use crate::services::run_blocking;
use crate::{AppError, AppErrorKind};

impl SteamService {
    /// Looks up Steam details, retaining core's fresh/stale/unavailable states.
    #[tracing::instrument(skip_all)]
    pub async fn fetch_game_metadata(
        &self,
        app_id: String,
        force_refresh: Option<bool>,
    ) -> Result<SteamMetadataLookupResult, AppError> {
        let store = self.metadata.clone();
        let runtime = tokio::runtime::Handle::current();
        // Core mixes async HTTP with synchronous SQLite. Poll its future on a
        // blocking worker, using the caller's runtime for the HTTP reactor.
        run_blocking(move || {
            Ok(runtime.block_on(lookup_steam_metadata(
                &store,
                &app_id,
                force_refresh.unwrap_or(false),
            )))
        })
        .await
    }

    /// Returns cached/downloaded art, or Unavailable when core has no usable art.
    #[tracing::instrument(skip_all)]
    pub async fn fetch_cover_art(
        &self,
        app_id: String,
        image_type: Option<String>,
    ) -> Result<Option<String>, AppError> {
        let image_type = parse_image_type(image_type.as_deref(), false)?;
        fetch_cover_art_with(
            self.metadata.clone(),
            self.settings.clone(),
            app_id,
            image_type,
            |store, app_id, image_type, api_key| async move {
                download_and_cache_image(&store, &app_id, image_type, api_key.as_deref()).await
            },
        )
        .await
    }

    /// Imports a content-addressed cover with bounded source reads.
    #[tracing::instrument(skip_all)]
    pub async fn import_custom_cover_art(&self, source_path: String) -> Result<String, AppError> {
        self.import_custom_art(source_path, None).await
    }

    /// Imports cover, portrait or background art with core's format validation.
    #[tracing::instrument(skip_all)]
    pub async fn import_custom_art(
        &self,
        source_path: String,
        art_type: Option<String>,
    ) -> Result<String, AppError> {
        let image_type = parse_image_type(art_type.as_deref(), true)?;
        run_blocking(move || {
            import_custom_art(&source_path, image_type)
                .map_err(|message| AppError::new(AppErrorKind::Io, message))
        })
        .await
    }
}

pub(super) fn parse_image_type(
    value: Option<&str>,
    custom: bool,
) -> Result<GameImageType, AppError> {
    match value.unwrap_or("cover") {
        "cover" => Ok(GameImageType::Cover),
        "hero" if !custom => Ok(GameImageType::Hero),
        "capsule" if !custom => Ok(GameImageType::Capsule),
        "portrait" => Ok(GameImageType::Portrait),
        "background" => Ok(GameImageType::Background),
        other => Err(AppError::validation(
            if custom { "art_type" } else { "image_type" },
            format!(
                "Unknown {} type: {other}",
                if custom { "art" } else { "image" }
            ),
        )),
    }
}

// The injected operation lets offline and key-handling tests remain deterministic
// without changing URLs, environment variables or the public service contract.
pub(super) async fn fetch_cover_art_with<F, Fut>(
    store: MetadataStore,
    settings: SettingsStore,
    app_id: String,
    image_type: GameImageType,
    fetch: F,
) -> Result<Option<String>, AppError>
where
    F: FnOnce(MetadataStore, String, GameImageType, Option<String>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Option<String>, String>>,
{
    let runtime = tokio::runtime::Handle::current();
    run_blocking(move || {
        // This value is deliberately never a tracing field or error message.
        let api_key = settings
            .load()
            .ok()
            .and_then(|settings| settings.steamgriddb_api_key)
            .filter(|key| !key.trim().is_empty());
        match runtime.block_on(fetch(store, app_id, image_type, api_key)) {
            Ok(Some(path)) => Ok(Some(path)),
            // Core has already attempted its stale-cache fallback before None.
            Ok(None) => Err(AppError::new(
                AppErrorKind::Unavailable,
                "game image is unavailable",
            )),
            Err(message) => Err(AppError::new(AppErrorKind::Io, message)),
        }
    })
    .await
}
