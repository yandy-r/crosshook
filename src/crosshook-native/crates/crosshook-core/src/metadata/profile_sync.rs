use super::{db, MetadataStoreError, SyncReport, SyncSource};
use crate::profile::{validate_name, GameProfile, ProfileStore};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub fn observe_profile_write(
    conn: &Connection,
    name: &str,
    profile: &GameProfile,
    path: &Path,
    source: SyncSource,
    source_profile_id: Option<&str>,
) -> Result<(), MetadataStoreError> {
    validate_profile_name(name)?;

    let now = Utc::now().to_rfc3339();
    let created_at = created_at_for_insert(path, source).unwrap_or_else(|| now.clone());
    let current_path = path.to_string_lossy().into_owned();
    let game_name = nullable_text(&profile.game.name);
    let launch_method = nullable_text(&profile.launch.method.to_string());
    let content_hash = compute_content_hash(profile);

    conn.execute(
        "INSERT INTO profiles (
            profile_id,
            current_filename,
            current_path,
            game_name,
            launch_method,
            source,
            source_profile_id,
            content_hash,
            deleted_at,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10)
        ON CONFLICT(current_filename) DO UPDATE SET
            current_path = excluded.current_path,
            game_name = excluded.game_name,
            launch_method = excluded.launch_method,
            source = COALESCE(excluded.source, profiles.source),
            source_profile_id = COALESCE(excluded.source_profile_id, profiles.source_profile_id),
            content_hash = excluded.content_hash,
            deleted_at = NULL,
            updated_at = excluded.updated_at",
        params![
            db::new_id(),
            name,
            current_path,
            game_name,
            launch_method,
            source.as_str(),
            source_profile_id,
            content_hash,
            created_at,
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a profile metadata row",
        source,
    })?;

    Ok(())
}

pub fn lookup_profile_id(
    conn: &Connection,
    name: &str,
) -> Result<Option<String>, MetadataStoreError> {
    conn.query_row(
        "SELECT profile_id FROM profiles WHERE current_filename = ?1 AND deleted_at IS NULL",
        params![name],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "look up a profile id by name",
        source,
    })
}

pub fn observe_profile_rename(
    conn: &Connection,
    old_name: &str,
    new_name: &str,
    old_path: &Path,
    new_path: &Path,
) -> Result<(), MetadataStoreError> {
    validate_profile_name(old_name)?;
    validate_profile_name(new_name)?;

    let tx =
        Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|source| {
            MetadataStoreError::Database {
                action: "start a profile rename transaction",
                source,
            }
        })?;

    let profile_id = tx
        .query_row(
            "SELECT profile_id FROM profiles WHERE current_filename = ?1",
            params![old_name],
            |row: &Row<'_>| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|source| MetadataStoreError::Database {
            action: "look up the profile being renamed",
            source,
        })?
        .ok_or_else(|| {
            MetadataStoreError::Corrupt(format!(
                "profile metadata row missing for rename from '{old_name}' to '{new_name}'"
            ))
        })?;

    let now = Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE profiles
         SET current_filename = ?1,
             current_path = ?2,
             deleted_at = NULL,
             updated_at = ?3
         WHERE current_filename = ?4",
        params![new_name, new_path.to_string_lossy(), now, old_name],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "update the renamed profile metadata row",
        source,
    })?;

    tx.execute(
        "INSERT INTO profile_name_history (
            profile_id,
            old_name,
            new_name,
            old_path,
            new_path,
            source,
            created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            profile_id,
            old_name,
            new_name,
            old_path.to_string_lossy(),
            new_path.to_string_lossy(),
            "app_rename",
            now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "record a profile rename history row",
        source,
    })?;

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the profile rename transaction",
        source,
    })?;

    Ok(())
}

pub fn observe_profile_delete(conn: &Connection, name: &str) -> Result<(), MetadataStoreError> {
    validate_profile_name(name)?;

    conn.execute(
        "UPDATE profiles SET deleted_at = ?1 WHERE current_filename = ?2",
        params![Utc::now().to_rfc3339(), name],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "soft-delete a profile metadata row",
        source,
    })?;

    Ok(())
}

pub fn sync_profiles_from_store(
    conn: &Connection,
    store: &ProfileStore,
) -> Result<SyncReport, MetadataStoreError> {
    let mut report = SyncReport::default();
    let profile_names = store.list().map_err(|source| {
        MetadataStoreError::Corrupt(format!(
            "failed to list profile store contents during metadata sync: {source}"
        ))
    })?;
    let seen_names: HashSet<String> = profile_names.iter().cloned().collect();

    for name in profile_names {
        report.profiles_seen += 1;

        let existed = conn
            .query_row(
                "SELECT profile_id FROM profiles WHERE current_filename = ?1",
                params![name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| MetadataStoreError::Database {
                action: "check whether a profile metadata row already exists",
                source,
            })?
            .is_some();

        let path = store.base_path.join(format!("{name}.toml"));
        let profile = match store.load(&name) {
            Ok(profile) => profile,
            Err(error) => {
                report.errors.push(format!(
                    "failed to load profile '{name}' during metadata sync: {error}"
                ));
                continue;
            }
        };

        observe_profile_write(
            conn,
            &name,
            &profile,
            &path,
            SyncSource::InitialCensus,
            None,
        )?;
        if existed {
            report.updated += 1;
        } else {
            report.created += 1;
        }
    }

    let mut stmt = conn
        .prepare("SELECT current_filename FROM profiles WHERE deleted_at IS NULL")
        .map_err(|source| MetadataStoreError::Database {
            action: "query active profile metadata rows",
            source,
        })?;
    let active_names = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "iterate active profile metadata rows",
            source,
        })?;

    for active_name in active_names {
        let active_name = active_name.map_err(|source| MetadataStoreError::Database {
            action: "read an active profile metadata row",
            source,
        })?;
        if !seen_names.contains(&active_name) {
            observe_profile_delete(conn, &active_name)?;
            report.deleted += 1;
        }
    }

    Ok(report)
}

fn validate_profile_name(name: &str) -> Result<(), MetadataStoreError> {
    validate_name(name).map_err(|error| {
        MetadataStoreError::Corrupt(format!("invalid profile name observed: {error}"))
    })
}

fn created_at_for_insert(path: &Path, source: SyncSource) -> Option<String> {
    match source {
        SyncSource::InitialCensus => fs::metadata(path)
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .map(system_time_to_rfc3339),
        SyncSource::AppWrite
        | SyncSource::AppRename
        | SyncSource::AppDuplicate
        | SyncSource::AppDelete
        | SyncSource::FilesystemScan
        | SyncSource::Import
        | SyncSource::AppMigration => None,
    }
}

fn system_time_to_rfc3339(time: std::time::SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}

fn nullable_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Compute a hex-encoded SHA-256 digest of arbitrary byte content.
///
/// Used by profile sync and config revision capture to produce deterministic
/// content hashes. Exposed publicly so the Tauri command layer can reuse it
/// instead of duplicating the hashing logic.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn compute_content_hash(profile: &GameProfile) -> Option<String> {
    let serialized = toml::to_string_pretty(profile).ok()?;
    Some(sha256_hex(serialized.as_bytes()))
}

#[cfg(test)]
mod launch_preset_metadata_tests {
    use super::observe_profile_write;
    use crate::metadata::{db, migrations, SyncSource};
    use crate::profile::{GameProfile, LaunchOptimizationsSection, LaunchSection};
    use rusqlite::{params, OptionalExtension};
    use std::collections::BTreeMap;
    use std::path::Path;

    #[test]
    fn observe_profile_write_content_hash_changes_when_launch_presets_change() {
        let conn = db::open_in_memory().expect("open memory db");
        migrations::run_migrations(&conn).expect("migrations");
        let path = Path::new("/fake/profiles/demo.toml");

        let mut launch = LaunchSection::default();
        launch.method = "proton_run".to_string();
        launch.optimizations.enabled_option_ids = vec!["use_gamemode".to_string()];
        let mut presets = BTreeMap::new();
        presets.insert(
            "p".to_string(),
            LaunchOptimizationsSection {
                enabled_option_ids: vec![],
            },
        );
        launch.presets = presets;

        let profile_v1 = GameProfile {
            launch: launch.clone(),
            ..GameProfile::default()
        };

        observe_profile_write(&conn, "demo", &profile_v1, path, SyncSource::AppWrite, None)
            .expect("observe v1");

        let h1: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM profiles WHERE current_filename = ?1",
                params!["demo"],
                |row| row.get(0),
            )
            .optional()
            .expect("query")
            .flatten();

        launch
            .presets
            .get_mut("p")
            .expect("preset")
            .enabled_option_ids = vec!["enable_hdr".to_string()];

        let profile_v2 = GameProfile {
            launch,
            ..GameProfile::default()
        };

        observe_profile_write(&conn, "demo", &profile_v2, path, SyncSource::AppWrite, None)
            .expect("observe v2");

        let h2: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM profiles WHERE current_filename = ?1",
                params!["demo"],
                |row| row.get(0),
            )
            .optional()
            .expect("query")
            .flatten();

        assert!(h1.is_some() && h2.is_some());
        assert_ne!(h1, h2);
    }
}
