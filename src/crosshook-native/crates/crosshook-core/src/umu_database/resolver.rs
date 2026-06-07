use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{Duration as ChronoDuration, Utc};
use tokio::sync::Mutex as AsyncMutex;

use super::api_client::{lookup_umu_game_id, validate_umu_id, UmuGameIdApiLookup};
use crate::launch::request::{
    LaunchRequest, UmuGameIdLookupKey, UmuGameIdResolution, UmuGameIdResolutionSource,
    METHOD_PROTON_RUN,
};
use crate::launch::script_runner::{force_no_umu_for_launch_request, should_use_umu};
use crate::metadata::{
    normalize_umu_gameid_codename, normalize_umu_gameid_store, MetadataStore, MetadataStoreError,
    UmuGameIdCacheRow,
};
use crate::settings::UmuDatabaseLookupPreference;

const CACHE_TTL_DAYS: i64 = 7;
const MAX_CACHE_PAYLOAD_JSON_BYTES: usize = 256 * 1024;

static IN_FLIGHT_LOOKUPS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();

fn cache_expiry() -> String {
    (Utc::now() + ChronoDuration::days(CACHE_TTL_DAYS)).to_rfc3339()
}

fn lookup_lock(key: &UmuGameIdLookupKey) -> Arc<AsyncMutex<()>> {
    let map = IN_FLIGHT_LOOKUPS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map
        .lock()
        .expect("umu GAMEID lookup lock map should not be poisoned");
    guard
        .entry(format!("{}:{}", key.store, key.codename))
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn remove_lookup_lock(key: &UmuGameIdLookupKey, lock: &Arc<AsyncMutex<()>>) {
    let Some(map) = IN_FLIGHT_LOOKUPS.get() else {
        return;
    };
    let mut guard = map
        .lock()
        .expect("umu GAMEID lookup lock map should not be poisoned");
    let map_key = format!("{}:{}", key.store, key.codename);
    if guard
        .get(&map_key)
        .is_some_and(|stored| Arc::ptr_eq(stored, lock))
    {
        guard.remove(&map_key);
    }
}

fn explicit_or_steam_resolution(request: &LaunchRequest) -> Option<UmuGameIdResolution> {
    let explicit = request.runtime.umu_game_id.trim();
    if !explicit.is_empty() {
        if let Err(error) = validate_umu_id(explicit) {
            tracing::warn!(%error, "invalid explicit umu GAMEID override");
            return Some(fallback(
                UmuGameIdResolutionSource::Fallback,
                None,
                Some("invalid_explicit_umu_game_id".to_string()),
            ));
        }
        return Some(UmuGameIdResolution {
            game_id: explicit.to_string(),
            store: None,
            source: UmuGameIdResolutionSource::ExplicitOverride,
            lookup_key: None,
            fetched_at: None,
            expires_at: None,
            error_category: None,
        });
    }

    let steam_id = if request.steam.app_id.trim().is_empty() {
        request.runtime.steam_app_id.trim()
    } else {
        request.steam.app_id.trim()
    };
    if !steam_id.is_empty() {
        return Some(UmuGameIdResolution {
            game_id: steam_id.to_string(),
            store: Some("steam".to_string()),
            source: UmuGameIdResolutionSource::SteamAppId,
            lookup_key: None,
            fetched_at: None,
            expires_at: None,
            error_category: None,
        });
    }

    None
}

fn fallback(
    source: UmuGameIdResolutionSource,
    key: Option<UmuGameIdLookupKey>,
    error_category: Option<String>,
) -> UmuGameIdResolution {
    UmuGameIdResolution {
        game_id: "umu-0".to_string(),
        store: key.as_ref().map(|key| key.store.clone()),
        source,
        lookup_key: key,
        fetched_at: None,
        expires_at: None,
        error_category,
    }
}

fn from_cache(row: UmuGameIdCacheRow, source: UmuGameIdResolutionSource) -> UmuGameIdResolution {
    let key = UmuGameIdLookupKey {
        store: row.store.clone(),
        codename: row.codename.clone(),
    };
    match (row.status.as_str(), row.umu_id.as_deref()) {
        ("found", Some(umu_id)) => {
            if let Err(error) = validate_umu_id(umu_id) {
                tracing::warn!(%error, "invalid cached umu GAMEID");
                return fallback(
                    UmuGameIdResolutionSource::ApiUnavailable,
                    Some(key),
                    Some("invalid_cached_umu_game_id".to_string()),
                );
            }
            UmuGameIdResolution {
                game_id: umu_id.to_string(),
                store: Some(row.store),
                source,
                lookup_key: Some(key),
                fetched_at: Some(row.fetched_at),
                expires_at: row.expires_at,
                error_category: row.last_error,
            }
        }
        ("missing", _) => UmuGameIdResolution {
            game_id: "umu-0".to_string(),
            store: Some(row.store),
            source: UmuGameIdResolutionSource::CachedNotFound,
            lookup_key: Some(key),
            fetched_at: Some(row.fetched_at),
            expires_at: row.expires_at,
            error_category: None,
        },
        _ => fallback(
            UmuGameIdResolutionSource::ApiUnavailable,
            Some(key),
            row.last_error,
        ),
    }
}

fn lookup_key_from_request(request: &LaunchRequest) -> Option<UmuGameIdLookupKey> {
    let store = match normalize_umu_gameid_store(&request.runtime.umu_store) {
        Ok(store) => store,
        Err(error) => {
            tracing::warn!(%error, "invalid umu GAMEID lookup store hint");
            return None;
        }
    };
    let codename = match normalize_umu_gameid_codename(&request.runtime.umu_codename) {
        Ok(codename) => codename,
        Err(error) => {
            tracing::warn!(%error, "invalid umu GAMEID lookup codename hint");
            return None;
        }
    };
    Some(UmuGameIdLookupKey { store, codename })
}

fn read_fresh_cache(
    metadata_store: &MetadataStore,
    key: &UmuGameIdLookupKey,
) -> Result<Option<UmuGameIdResolution>, MetadataStoreError> {
    metadata_store
        .get_umu_gameid_cache_entry(&key.store, &key.codename)
        .map(|row| row.map(|row| from_cache(row, UmuGameIdResolutionSource::FreshCache)))
}

fn stale_or_unavailable(
    metadata_store: &MetadataStore,
    key: UmuGameIdLookupKey,
    error_category: String,
) -> UmuGameIdResolution {
    if let Ok(Some(row)) =
        metadata_store.get_stale_umu_gameid_cache_entry(&key.store, &key.codename)
    {
        return from_cache(row, UmuGameIdResolutionSource::StaleCache);
    }
    fallback(
        UmuGameIdResolutionSource::ApiUnavailable,
        Some(key),
        Some(error_category),
    )
}

fn limited_payload_json(payload_json: String) -> Option<String> {
    if payload_json.len() > MAX_CACHE_PAYLOAD_JSON_BYTES {
        tracing::warn!(
            bytes = payload_json.len(),
            limit = MAX_CACHE_PAYLOAD_JSON_BYTES,
            "umu GAMEID API payload too large to cache"
        );
        None
    } else {
        Some(payload_json)
    }
}

async fn live_lookup(
    metadata_store: &MetadataStore,
    key: UmuGameIdLookupKey,
) -> UmuGameIdResolution {
    match lookup_umu_game_id(&key.store, &key.codename).await {
        Ok(UmuGameIdApiLookup::Found(entry, payload_json)) => {
            let row = UmuGameIdCacheRow::found(
                &key.store,
                &key.codename,
                &entry.umu_id,
                limited_payload_json(payload_json),
                Utc::now().to_rfc3339(),
                Some(cache_expiry()),
            );
            if let Ok(row) = row {
                if let Err(error) = metadata_store.put_umu_gameid_cache_entry(&row) {
                    tracing::warn!(%error, "failed to persist umu GAMEID cache hit");
                }
                return from_cache(row, UmuGameIdResolutionSource::FreshLookup);
            }
            fallback(
                UmuGameIdResolutionSource::ApiUnavailable,
                Some(key),
                Some("invalid_cache_row".to_string()),
            )
        }
        Ok(UmuGameIdApiLookup::NotFound(payload_json)) => {
            let row = UmuGameIdCacheRow::missing(
                &key.store,
                &key.codename,
                limited_payload_json(payload_json),
                Utc::now().to_rfc3339(),
                Some(cache_expiry()),
            );
            if let Ok(row) = row {
                if let Err(error) = metadata_store.put_umu_gameid_cache_entry(&row) {
                    tracing::warn!(%error, "failed to persist umu GAMEID cached miss");
                }
                return from_cache(row, UmuGameIdResolutionSource::CachedNotFound);
            }
            fallback(
                UmuGameIdResolutionSource::ApiUnavailable,
                Some(key),
                Some("invalid_cache_row".to_string()),
            )
        }
        Err(error) => {
            tracing::warn!(%error, "umu GAMEID lookup failed");
            stale_or_unavailable(metadata_store, key, "api_unavailable".to_string())
        }
    }
}

pub async fn resolve_umu_game_id(
    request: &LaunchRequest,
    lookup_preference: UmuDatabaseLookupPreference,
    metadata_store: &MetadataStore,
) -> UmuGameIdResolution {
    if let Some(resolution) = explicit_or_steam_resolution(request) {
        return resolution;
    }

    if request.resolved_method() != METHOD_PROTON_RUN {
        return fallback(UmuGameIdResolutionSource::Fallback, None, None);
    }

    let (will_use_umu, _) = should_use_umu(request, force_no_umu_for_launch_request(request));
    if !will_use_umu {
        return fallback(UmuGameIdResolutionSource::Fallback, None, None);
    }

    let key = lookup_key_from_request(request);

    if !lookup_preference.is_enabled() {
        return fallback(UmuGameIdResolutionSource::LookupDisabled, key, None);
    }

    let Some(key) = key else {
        return fallback(UmuGameIdResolutionSource::MissingHints, None, None);
    };

    match read_fresh_cache(metadata_store, &key) {
        Ok(Some(resolution)) => return resolution,
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to read fresh umu GAMEID cache entry");
            return stale_or_unavailable(metadata_store, key, "metadata_unavailable".to_string());
        }
    }

    let lock = lookup_lock(&key);
    let _lookup_guard = lock.lock().await;
    let resolution = match read_fresh_cache(metadata_store, &key) {
        Ok(Some(resolution)) => resolution,
        Ok(None) => live_lookup(metadata_store, key.clone()).await,
        Err(error) => {
            tracing::warn!(%error, "failed to read fresh umu GAMEID cache entry after lock");
            stale_or_unavailable(
                metadata_store,
                key.clone(),
                "metadata_unavailable".to_string(),
            )
        }
    };
    remove_lookup_lock(&key, &lock);
    resolution
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::OnceLock;

    use super::*;
    use crate::launch::request::{RuntimeLaunchConfig, SteamLaunchConfig};
    use crate::profile::TrainerLoadingMode;
    use crate::settings::{UmuDatabaseLookupPreference, UmuPreference};
    use tokio::sync::Mutex;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
            let previous = std::env::var_os(key);
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    async fn lock_env() -> tokio::sync::MutexGuard<'static, ()> {
        ENV_LOCK.get_or_init(|| Mutex::new(())).lock().await
    }

    fn request() -> (tempfile::TempDir, LaunchRequest) {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let game_path = temp_dir.path().join("game.exe");
        let prefix_path = temp_dir.path().join("prefix");
        let proton_path = temp_dir.path().join("proton");
        std::fs::write(&game_path, b"game").expect("game file");
        std::fs::create_dir_all(&prefix_path).expect("prefix dir");
        std::fs::write(&proton_path, b"proton").expect("proton file");

        (
            temp_dir,
            LaunchRequest {
                method: METHOD_PROTON_RUN.to_string(),
                game_path: game_path.to_string_lossy().into_owned(),
                trainer_loading_mode: TrainerLoadingMode::SourceDirectory,
                steam: SteamLaunchConfig::default(),
                runtime: RuntimeLaunchConfig {
                    prefix_path: prefix_path.to_string_lossy().into_owned(),
                    proton_path: proton_path.to_string_lossy().into_owned(),
                    umu_store: "gog".to_string(),
                    umu_codename: "cyberpunk_2077".to_string(),
                    ..Default::default()
                },
                umu_preference: UmuPreference::Umu,
                launch_game_only: true,
                ..Default::default()
            },
        )
    }

    fn install_umu_run(path: &Path) {
        std::fs::write(path, b"#!/bin/sh\nexit 0\n").expect("umu-run file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions).expect("chmod");
        }
    }

    fn make_umu_available() -> (tempfile::TempDir, EnvGuard) {
        let temp_dir = tempfile::tempdir().expect("umu path temp dir");
        let umu_run = temp_dir.path().join("umu-run");
        install_umu_run(&umu_run);
        let path = match std::env::var_os("PATH") {
            Some(existing) => {
                let mut paths = std::env::split_paths(&existing).collect::<Vec<_>>();
                paths.insert(0, temp_dir.path().to_path_buf());
                std::env::join_paths(paths).expect("join PATH")
            }
            None => temp_dir.path().as_os_str().to_os_string(),
        };
        (temp_dir, EnvGuard::set("PATH", path))
    }

    #[tokio::test]
    async fn resolver_prefers_explicit_override_without_lookup() {
        let (_temp, mut request) = request();
        request.runtime.umu_game_id = "UMU-OVERRIDE".to_string();
        let metadata_store = MetadataStore::open_in_memory().unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "UMU-OVERRIDE");
        assert_eq!(
            resolution.source,
            UmuGameIdResolutionSource::ExplicitOverride
        );
    }

    #[tokio::test]
    async fn resolver_rejects_invalid_explicit_override() {
        let (_temp, mut request) = request();
        request.runtime.umu_game_id = "bad gameid".to_string();
        let metadata_store = MetadataStore::open_in_memory().unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "umu-0");
        assert_eq!(resolution.source, UmuGameIdResolutionSource::Fallback);
        assert_eq!(
            resolution.error_category.as_deref(),
            Some("invalid_explicit_umu_game_id")
        );
    }

    #[tokio::test]
    async fn resolver_uses_steam_app_id_before_lookup() {
        let (_temp, mut request) = request();
        request.steam.app_id = "12345".to_string();
        let metadata_store = MetadataStore::open_in_memory().unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "12345");
        assert_eq!(resolution.store.as_deref(), Some("steam"));
        assert_eq!(resolution.source, UmuGameIdResolutionSource::SteamAppId);
    }

    #[tokio::test]
    async fn resolver_reports_lookup_disabled_with_hints() {
        let _env_lock = lock_env().await;
        let (_umu_temp, _path) = make_umu_available();
        let (_temp, request) = request();
        let metadata_store = MetadataStore::open_in_memory().unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Disabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "umu-0");
        assert_eq!(resolution.source, UmuGameIdResolutionSource::LookupDisabled);
        assert_eq!(
            resolution.lookup_key,
            Some(UmuGameIdLookupKey {
                store: "gog".to_string(),
                codename: "cyberpunk_2077".to_string()
            })
        );
    }

    #[tokio::test]
    async fn resolver_reports_missing_hints_when_lookup_enabled() {
        let _env_lock = lock_env().await;
        let (_umu_temp, _path) = make_umu_available();
        let (_temp, mut request) = request();
        request.runtime.umu_store.clear();
        request.runtime.umu_codename.clear();
        let metadata_store = MetadataStore::open_in_memory().unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "umu-0");
        assert_eq!(resolution.source, UmuGameIdResolutionSource::MissingHints);
        assert_eq!(resolution.lookup_key, None);
    }

    #[tokio::test]
    async fn resolver_returns_fresh_cache_hit_without_http_lookup() {
        let _env_lock = lock_env().await;
        let (_umu_temp, _path) = make_umu_available();
        let (_temp, request) = request();
        let metadata_store = MetadataStore::open_in_memory().unwrap();
        metadata_store
            .put_umu_gameid_cache_entry(
                &UmuGameIdCacheRow::found(
                    "gog",
                    "cyberpunk_2077",
                    "UMU-CACHED",
                    None,
                    "2026-06-07T00:00:00Z",
                    Some("2099-01-01T00:00:00Z".to_string()),
                )
                .unwrap(),
            )
            .unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "UMU-CACHED");
        assert_eq!(resolution.source, UmuGameIdResolutionSource::FreshCache);
    }

    #[tokio::test]
    async fn resolver_rejects_invalid_cached_umu_id() {
        let _env_lock = lock_env().await;
        let (_umu_temp, _path) = make_umu_available();
        let (_temp, request) = request();
        let metadata_store = MetadataStore::open_in_memory().unwrap();
        metadata_store
            .with_sqlite_conn("seed invalid umu GAMEID cache row", |conn| {
                conn.execute(
                    "INSERT INTO umu_gameid_lookup_cache (
                        store,
                        codename,
                        umu_id,
                        status,
                        payload_json,
                        fetched_at,
                        expires_at,
                        last_error,
                        updated_at
                    ) VALUES (?1, ?2, ?3, 'found', NULL, ?4, ?5, NULL, ?4)",
                    rusqlite::params![
                        "gog",
                        "cyberpunk_2077",
                        "bad gameid",
                        "2026-06-07T00:00:00Z",
                        "2099-01-01T00:00:00Z"
                    ],
                )
                .map_err(|source| MetadataStoreError::Database {
                    action: "seed invalid umu GAMEID cache row",
                    source,
                })?;
                Ok(())
            })
            .unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "umu-0");
        assert_eq!(resolution.source, UmuGameIdResolutionSource::ApiUnavailable);
        assert_eq!(
            resolution.error_category.as_deref(),
            Some("invalid_cached_umu_game_id")
        );
    }

    #[tokio::test]
    async fn resolver_live_lookup_found_persists_fresh_cache() {
        let _env_lock = lock_env().await;
        let (_umu_temp, _path) = make_umu_available();
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .and(query_param("store", "gog"))
            .and(query_param("codename", "cyberpunk_2077"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "title": "Cyberpunk 2077",
                    "umu_id": "UMU-LIVE"
                }
            ])))
            .mount(&mock_server)
            .await;
        let _api_url = EnvGuard::set(
            "CROSSHOOK_TEST_UMU_GAMEID_API_URL",
            format!("{}/umu_api.php", mock_server.uri()),
        );
        let (_temp, request) = request();
        let metadata_store = MetadataStore::open_in_memory().unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "UMU-LIVE");
        assert_eq!(resolution.source, UmuGameIdResolutionSource::FreshLookup);
        let cached = metadata_store
            .get_umu_gameid_cache_entry("gog", "cyberpunk_2077")
            .unwrap()
            .unwrap();
        assert_eq!(cached.status, "found");
        assert_eq!(cached.umu_id.as_deref(), Some("UMU-LIVE"));
    }

    #[tokio::test]
    async fn resolver_live_lookup_not_found_persists_cached_miss() {
        let _env_lock = lock_env().await;
        let (_umu_temp, _path) = make_umu_available();
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .and(query_param("store", "gog"))
            .and(query_param("codename", "cyberpunk_2077"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&mock_server)
            .await;
        let _api_url = EnvGuard::set(
            "CROSSHOOK_TEST_UMU_GAMEID_API_URL",
            format!("{}/umu_api.php", mock_server.uri()),
        );
        let (_temp, request) = request();
        let metadata_store = MetadataStore::open_in_memory().unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "umu-0");
        assert_eq!(resolution.source, UmuGameIdResolutionSource::CachedNotFound);
        assert!(resolution.fetched_at.is_some());
        assert!(resolution.expires_at.is_some());
        let cached = metadata_store
            .get_umu_gameid_cache_entry("gog", "cyberpunk_2077")
            .unwrap()
            .unwrap();
        assert_eq!(cached.status, "missing");
    }

    #[tokio::test]
    async fn resolver_retries_after_cached_error_and_uses_stale_found_fallback() {
        let _env_lock = lock_env().await;
        let (_umu_temp, _path) = make_umu_available();
        let (_temp, request) = request();
        let _api_url = EnvGuard::set(
            "CROSSHOOK_TEST_UMU_GAMEID_API_URL",
            "http://127.0.0.1:9/umu_api.php",
        );

        let metadata_store = MetadataStore::open_in_memory().unwrap();
        metadata_store
            .put_umu_gameid_cache_entry(
                &UmuGameIdCacheRow::error(
                    "gog",
                    "cyberpunk_2077",
                    "timeout",
                    "2026-06-01T00:00:00Z",
                    Some("2099-01-01T00:00:00Z".to_string()),
                )
                .unwrap(),
            )
            .unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;
        assert_eq!(resolution.source, UmuGameIdResolutionSource::ApiUnavailable);
        assert_eq!(
            resolution.error_category.as_deref(),
            Some("api_unavailable")
        );

        metadata_store
            .put_umu_gameid_cache_entry(
                &UmuGameIdCacheRow::found(
                    "gog",
                    "cyberpunk_2077",
                    "UMU-STALE",
                    None,
                    "2026-06-01T00:00:00Z",
                    Some("2026-06-02T00:00:00Z".to_string()),
                )
                .unwrap(),
            )
            .unwrap();

        let resolution = resolve_umu_game_id(
            &request,
            UmuDatabaseLookupPreference::Enabled,
            &metadata_store,
        )
        .await;

        assert_eq!(resolution.game_id, "UMU-STALE");
        assert_eq!(resolution.source, UmuGameIdResolutionSource::StaleCache);
    }
}
