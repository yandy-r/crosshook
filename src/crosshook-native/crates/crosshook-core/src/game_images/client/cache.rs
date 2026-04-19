use std::path::PathBuf;

use chrono::{DateTime, NaiveDateTime, Utc};

use crate::game_images::models::{GameImageSource, GameImageType};
use crate::metadata::MetadataStore;

// ---------------------------------------------------------------------------
// Cache helpers
// ---------------------------------------------------------------------------

pub(super) fn parse_expiration(expires_at: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(expires_at)
        .map(|value| value.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(expires_at, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|value| value.and_utc())
        })
}

pub(super) fn filename_for(
    image_type: GameImageType,
    source: GameImageSource,
    extension: &str,
) -> String {
    let source_suffix = match source {
        GameImageSource::SteamCdn => "steam_cdn",
        GameImageSource::SteamGridDb => "steamgriddb",
    };
    let type_prefix = match image_type {
        GameImageType::Cover => "cover",
        GameImageType::Hero => "hero",
        GameImageType::Capsule => "capsule",
        GameImageType::Portrait => "portrait",
        GameImageType::Background => "background",
    };
    format!("{type_prefix}_{source_suffix}.{extension}")
}

pub(super) fn image_cache_base_dir() -> Result<PathBuf, String> {
    directories::BaseDirs::new()
        .ok_or_else(|| "home directory not found".to_string())
        .map(|dirs| {
            dirs.data_local_dir()
                .join("crosshook")
                .join("cache")
                .join("images")
        })
}

/// Return the file path from a stale (possibly expired) cached entry if the
/// file still exists on disk.
pub(super) fn stale_fallback_path(
    store: &MetadataStore,
    app_id: &str,
    image_type_str: &str,
) -> Option<String> {
    let row = store
        .get_game_image(app_id, image_type_str)
        .ok()
        .flatten()?;
    let cached_path = PathBuf::from(&row.file_path);
    if cached_path.exists() {
        tracing::debug!(
            app_id,
            image_type = image_type_str,
            "serving stale cached game image as fallback"
        );
        Some(row.file_path)
    } else {
        delete_game_image_row(store, app_id, image_type_str);
        None
    }
}

/// Best-effort deletion of a DB row for a missing cache file.
pub(super) fn delete_game_image_row(store: &MetadataStore, app_id: &str, image_type_str: &str) {
    if let Err(error) = store.with_sqlite_conn("delete a stale game image cache row", |conn| {
        conn.execute(
            "DELETE FROM game_image_cache WHERE steam_app_id = ?1 AND image_type = ?2",
            rusqlite::params![app_id, image_type_str],
        )
        .map_err(|source| crate::metadata::MetadataStoreError::Database {
            action: "delete stale game image row",
            source,
        })?;
        Ok(())
    }) {
        tracing::warn!(
            app_id,
            image_type = image_type_str,
            %error,
            "failed to delete stale game image cache row"
        );
    }
}
