use super::MetadataStoreError;
use rusqlite::Connection;

/// Host readiness catalog, nag dismissals, and last snapshot (issue #269).
pub(super) fn migrate_20_to_21(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS host_readiness_catalog (
            tool_id TEXT PRIMARY KEY NOT NULL,
            binary_name TEXT NOT NULL,
            display_name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            docs_url TEXT NOT NULL DEFAULT '',
            required INTEGER NOT NULL DEFAULT 0,
            category TEXT NOT NULL DEFAULT '',
            install_commands_json TEXT NOT NULL DEFAULT '[]',
            source TEXT NOT NULL DEFAULT 'default',
            catalog_version INTEGER NOT NULL DEFAULT 1,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS readiness_nag_dismissals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_id TEXT NOT NULL,
            dismissed_at TEXT NOT NULL,
            expires_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_readiness_nag_dismissals_tool_id
            ON readiness_nag_dismissals(tool_id);

        CREATE TABLE IF NOT EXISTS host_readiness_snapshots (
            id INTEGER PRIMARY KEY NOT NULL,
            detected_distro_family TEXT NOT NULL,
            tool_results_json TEXT NOT NULL,
            all_passed INTEGER NOT NULL,
            critical_failures INTEGER NOT NULL,
            warnings INTEGER NOT NULL,
            checked_at TEXT NOT NULL
        );
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 20 to 21",
        source,
    })?;

    Ok(())
}

/// Dedicated Proton release catalog table (issue #274).
///
/// `external_cache_entries` caps payloads at 512 KiB and blends all provider data into a single
/// opaque JSON blob; the new typed table allows partial catalog refreshes and per-provider
/// queries. The DELETE evicts legacy `protonup:catalog:*` entries; they are regenerated on next
/// fetch, so removing them here is safe.
pub(super) fn migrate_21_to_22(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS proton_release_catalog (
            provider_id     TEXT NOT NULL,
            version_tag     TEXT NOT NULL,
            payload_json    TEXT NOT NULL,
            release_url     TEXT,
            download_url    TEXT,
            checksum_url    TEXT,
            checksum_kind   TEXT,
            asset_size      INTEGER,
            fetched_at      TEXT NOT NULL,
            expires_at      TEXT,
            PRIMARY KEY (provider_id, version_tag)
        );
        CREATE INDEX IF NOT EXISTS idx_proton_catalog_provider_fetched
            ON proton_release_catalog(provider_id, fetched_at);

        DELETE FROM external_cache_entries WHERE cache_key LIKE 'protonup:catalog:%';
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 21 to 22",
        source,
    })?;

    Ok(())
}

/// v23 — evict cached Proton release payloads so newly-added DTO fields
/// (notably `published_at`) repopulate on next fetch. The cache is runtime
/// metadata, not user data; the next catalog call rebuilds it from GitHub.
pub(super) fn migrate_22_to_23(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch("DELETE FROM proton_release_catalog;")
        .map_err(|source| MetadataStoreError::Database {
            action: "run metadata migration 22 to 23",
            source,
        })?;

    Ok(())
}
