use super::models::{TrainerSearchResponse, TrainerSearchResult};
use crate::metadata::MetadataStoreError;
use rusqlite::{params, Connection};

/// Search `trainer_sources` rows using LIKE matching against `game_name`, `source_name`,
/// and `notes`.
///
/// Returns a paginated [`TrainerSearchResponse`] with a `total_count` for the full match set.
///
/// # Errors
///
/// - [`MetadataStoreError::Validation`] if `query` is empty after trimming.
/// - [`MetadataStoreError::Database`] on any SQLite error.
pub fn search_trainer_sources(
    conn: &Connection,
    query: &str,
    limit: i64,
    offset: i64,
) -> Result<TrainerSearchResponse, MetadataStoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Err(MetadataStoreError::Validation(
            "search query cannot be empty".into(),
        ));
    }

    // Cap at 512 bytes, respecting UTF-8 character boundaries.
    let cutoff = q
        .char_indices()
        .take_while(|(i, _)| *i < 512)
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    let q = &q[..cutoff];

    let limit = std::cmp::min(limit, 50);

    let total_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM trainer_sources ts
             JOIN community_taps ct ON ts.tap_id = ct.tap_id
             WHERE (ts.game_name LIKE '%' || ?1 || '%'
                 OR ts.source_name LIKE '%' || ?1 || '%'
                 OR ts.notes LIKE '%' || ?1 || '%')",
            params![q],
            |row| row.get(0),
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "count trainer sources matching query",
            source,
        })?;

    let mut stmt = conn
        .prepare(
            "SELECT ts.id, ts.game_name, ts.steam_app_id, ts.source_name,
                    ts.source_url, ts.trainer_version, ts.game_version,
                    ts.notes, ts.sha256, ts.relative_path,
                    ct.tap_url, ct.local_path, 0.0 AS relevance_score
             FROM trainer_sources ts
             JOIN community_taps ct ON ts.tap_id = ct.tap_id
             WHERE (ts.game_name LIKE '%' || ?1 || '%'
                 OR ts.source_name LIKE '%' || ?1 || '%'
                 OR ts.notes LIKE '%' || ?1 || '%')
             ORDER BY ts.game_name, ts.source_name
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare search trainer sources query",
            source,
        })?;

    let results = stmt
        .query_map(params![q, limit, offset], |row| {
            Ok(TrainerSearchResult {
                id: row.get(0)?,
                game_name: row.get(1)?,
                steam_app_id: row.get(2)?,
                source_name: row.get(3)?,
                source_url: row.get(4)?,
                trainer_version: row.get(5)?,
                game_version: row.get(6)?,
                notes: row.get(7)?,
                sha256: row.get(8)?,
                relative_path: row.get(9)?,
                tap_url: row.get(10)?,
                tap_local_path: row.get(11)?,
                relevance_score: row.get(12)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "execute search trainer sources query",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "read search trainer sources rows",
            source,
        })?;

    Ok(TrainerSearchResponse {
        results,
        total_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MetadataStore;

    fn insert_test_tap(conn: &Connection, tap_id: &str, tap_url: &str) {
        conn.execute(
            "INSERT INTO community_taps (tap_id, tap_url, tap_branch, local_path, created_at, updated_at)
             VALUES (?1, ?2, '', '/tmp/test', datetime('now'), datetime('now'))",
            params![tap_id, tap_url],
        )
        .unwrap();
    }

    fn insert_test_source(
        conn: &Connection,
        tap_id: &str,
        game_name: &str,
        source_name: &str,
        source_url: &str,
        notes: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO trainer_sources (tap_id, game_name, source_name, source_url, relative_path, created_at, notes)
             VALUES (?1, ?2, ?3, ?4, 'test/path', datetime('now'), ?5)",
            params![tap_id, game_name, source_name, source_url, notes],
        )
        .unwrap();
    }

    #[test]
    fn search_returns_error_for_empty_query() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test empty query", |conn| {
                let result = search_trainer_sources(conn, "", 20, 0);
                assert!(result.is_err());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn search_returns_empty_for_no_matches() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test no matches", |conn| {
                let result = search_trainer_sources(conn, "nonexistent", 20, 0).unwrap();
                assert_eq!(result.results.len(), 0);
                assert_eq!(result.total_count, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn search_matches_game_name_substring() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test game_name match", |conn| {
                insert_test_tap(conn, "tap-001", "https://example.com/tap.git");
                insert_test_source(
                    conn,
                    "tap-001",
                    "Elden Ring",
                    "FLiNG Trainer",
                    "https://example.com/elden.exe",
                    None,
                );
                let result = search_trainer_sources(conn, "Elden", 20, 0).unwrap();
                assert_eq!(result.results.len(), 1, "expected 1 result for 'Elden'");
                assert_eq!(result.results[0].game_name, "Elden Ring");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn search_matches_source_name_substring() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test source_name match", |conn| {
                insert_test_tap(conn, "tap-002", "https://example.com/tap2.git");
                insert_test_source(
                    conn,
                    "tap-002",
                    "Cyberpunk 2077",
                    "WeMod Trainer",
                    "https://example.com/cyberpunk.exe",
                    None,
                );
                let result = search_trainer_sources(conn, "WeMod", 20, 0).unwrap();
                assert_eq!(result.results.len(), 1, "expected 1 result for 'WeMod'");
                assert_eq!(result.results[0].source_name, "WeMod Trainer");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn search_matches_notes_substring() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test notes match", |conn| {
                insert_test_tap(conn, "tap-003", "https://example.com/tap3.git");
                insert_test_source(
                    conn,
                    "tap-003",
                    "Dark Souls III",
                    "Some Trainer",
                    "https://example.com/darksouls.exe",
                    Some("Requires DirectX 12"),
                );
                let result = search_trainer_sources(conn, "DirectX", 20, 0).unwrap();
                assert_eq!(result.results.len(), 1, "expected 1 result for 'DirectX'");
                assert_eq!(result.results[0].game_name, "Dark Souls III");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn search_respects_limit_cap_at_50() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test limit cap", |conn| {
                insert_test_tap(conn, "tap-004", "https://example.com/tap4.git");
                // Insert 60 rows so that a cap of 50 is observable.
                for i in 0..60 {
                    conn.execute(
                        "INSERT INTO trainer_sources (tap_id, game_name, source_name, source_url, relative_path, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
                        params![
                            "tap-004",
                            format!("Game {i:02}"),
                            format!("Source {i:02}"),
                            format!("https://example.com/trainer{i}.exe"),
                            format!("path/to/{i}"),
                        ],
                    )
                    .unwrap();
                }
                // Request more than 50 rows.
                let result = search_trainer_sources(conn, "Game", 100, 0).unwrap();
                assert!(
                    result.results.len() <= 50,
                    "expected at most 50 results, got {}",
                    result.results.len()
                );
                assert_eq!(result.results.len(), 50);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn search_respects_offset_pagination() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test offset pagination", |conn| {
                insert_test_tap(conn, "tap-005", "https://example.com/tap5.git");
                // Insert 3 rows with names that sort alphabetically.
                insert_test_source(
                    conn,
                    "tap-005",
                    "Alpha Game",
                    "Trainer A",
                    "https://example.com/alpha.exe",
                    None,
                );
                insert_test_source(
                    conn,
                    "tap-005",
                    "Beta Game",
                    "Trainer B",
                    "https://example.com/beta.exe",
                    None,
                );
                insert_test_source(
                    conn,
                    "tap-005",
                    "Gamma Game",
                    "Trainer C",
                    "https://example.com/gamma.exe",
                    None,
                );
                // limit=1, offset=1 should return the second row in alphabetical order.
                let result = search_trainer_sources(conn, "Game", 1, 1).unwrap();
                assert_eq!(result.results.len(), 1, "expected exactly 1 result");
                assert_eq!(result.results[0].game_name, "Beta Game");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn search_returns_tap_url_from_join() {
        let store = MetadataStore::open_in_memory().unwrap();
        store
            .with_sqlite_conn("test tap_url join", |conn| {
                let expected_tap_url = "https://mygittap.example.com/trainers.git";
                insert_test_tap(conn, "tap-006", expected_tap_url);
                insert_test_source(
                    conn,
                    "tap-006",
                    "Witcher 3",
                    "Nexus Trainer",
                    "https://example.com/witcher.exe",
                    None,
                );
                let result = search_trainer_sources(conn, "Witcher", 20, 0).unwrap();
                assert_eq!(result.results.len(), 1, "expected 1 result");
                assert_eq!(result.results[0].tap_url, expected_tap_url);
                Ok(())
            })
            .unwrap();
    }
}
