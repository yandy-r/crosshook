use super::MetadataStoreError;
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub struct GameImageCacheRow {
    pub cache_id: String,
    pub steam_app_id: String,
    pub image_type: String,
    pub source: String,
    pub file_path: String,
    pub file_size: i64,
    pub content_hash: String,
    pub mime_type: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub source_url: String,
    pub preferred_source: String,
    pub expires_at: Option<String>,
    pub fetched_at: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn upsert_game_image(
    conn: &Connection,
    steam_app_id: &str,
    image_type: &str,
    source: &str,
    file_path: &str,
    file_size: Option<i64>,
    content_hash: Option<&str>,
    mime_type: Option<&str>,
    source_url: Option<&str>,
    expires_at: Option<&str>,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "INSERT INTO game_image_cache
             (cache_id, steam_app_id, image_type, source, file_path, file_size,
              content_hash, mime_type, source_url, fetched_at, created_at, updated_at, expires_at)
         VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 datetime('now'), datetime('now'), datetime('now'), ?9)
         ON CONFLICT(steam_app_id, image_type, source) DO UPDATE SET
             file_path     = excluded.file_path,
             file_size     = excluded.file_size,
             content_hash  = excluded.content_hash,
             mime_type     = excluded.mime_type,
             source_url    = excluded.source_url,
             updated_at    = datetime('now'),
             expires_at    = excluded.expires_at",
        params![
            steam_app_id,
            image_type,
            source,
            file_path,
            file_size.unwrap_or(0),
            content_hash.unwrap_or(""),
            mime_type.unwrap_or("image/jpeg"),
            source_url.unwrap_or(""),
            expires_at,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a game image cache row",
        source,
    })?;

    Ok(())
}

pub fn get_game_image(
    conn: &Connection,
    steam_app_id: &str,
    image_type: &str,
) -> Result<Option<GameImageCacheRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT cache_id, steam_app_id, image_type, source, file_path, file_size,
                    content_hash, mime_type, width, height, source_url, preferred_source,
                    expires_at, fetched_at, created_at, updated_at
             FROM game_image_cache
             WHERE steam_app_id = ?1 AND image_type = ?2
             LIMIT 1",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare get game image query",
            source,
        })?;

    stmt.query_row(params![steam_app_id, image_type], |row| {
        Ok(GameImageCacheRow {
            cache_id: row.get(0)?,
            steam_app_id: row.get(1)?,
            image_type: row.get(2)?,
            source: row.get(3)?,
            file_path: row.get(4)?,
            file_size: row.get(5)?,
            content_hash: row.get(6)?,
            mime_type: row.get(7)?,
            width: row.get(8)?,
            height: row.get(9)?,
            source_url: row.get(10)?,
            preferred_source: row.get(11)?,
            expires_at: row.get(12)?,
            fetched_at: row.get(13)?,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
        })
    })
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "get game image by steam_app_id and image_type",
        source,
    })
}

pub fn evict_expired_images(conn: &Connection) -> Result<Vec<String>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT file_path FROM game_image_cache
             WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare evict expired game images select",
            source,
        })?;

    let file_paths = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "query expired game image file paths",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect expired game image file paths",
            source,
        })?;

    conn.execute(
        "DELETE FROM game_image_cache
         WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
        [],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "delete expired game image cache rows",
        source,
    })?;

    Ok(file_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{db, migrations};
    use rusqlite::Connection;

    fn open_test_db() -> Connection {
        let conn = db::open_in_memory().expect("open in-memory db");
        migrations::run_migrations(&conn).expect("run migrations");
        conn
    }

    #[test]
    fn upsert_then_get_returns_correct_data() {
        let conn = open_test_db();

        upsert_game_image(
            &conn,
            "1245620",
            "cover",
            "steam_cdn",
            "/cache/covers/1245620.jpg",
            Some(102_400),
            Some("abc123"),
            Some("image/jpeg"),
            Some("https://cdn.steam.example/1245620/cover.jpg"),
            None,
        )
        .unwrap();

        let row = get_game_image(&conn, "1245620", "cover")
            .unwrap()
            .expect("row should exist after upsert");

        assert_eq!(row.steam_app_id, "1245620");
        assert_eq!(row.image_type, "cover");
        assert_eq!(row.source, "steam_cdn");
        assert_eq!(row.file_path, "/cache/covers/1245620.jpg");
        assert_eq!(row.file_size, 102_400);
        assert_eq!(row.content_hash, "abc123");
        assert_eq!(row.mime_type, "image/jpeg");
        assert_eq!(
            row.source_url,
            "https://cdn.steam.example/1245620/cover.jpg"
        );
        assert!(row.expires_at.is_none());
    }

    #[test]
    fn upsert_with_same_key_updates_existing_row() {
        let conn = open_test_db();

        upsert_game_image(
            &conn,
            "570",
            "cover",
            "steam_cdn",
            "/cache/covers/570_v1.jpg",
            Some(50_000),
            Some("hash_v1"),
            Some("image/jpeg"),
            None,
            None,
        )
        .unwrap();

        // Upsert again with updated data for the same (steam_app_id, image_type, source) key.
        upsert_game_image(
            &conn,
            "570",
            "cover",
            "steam_cdn",
            "/cache/covers/570_v2.jpg",
            Some(60_000),
            Some("hash_v2"),
            Some("image/png"),
            Some("https://cdn.example/570/cover.png"),
            None,
        )
        .unwrap();

        let row = get_game_image(&conn, "570", "cover")
            .unwrap()
            .expect("row should exist after second upsert");

        assert_eq!(row.file_path, "/cache/covers/570_v2.jpg");
        assert_eq!(row.file_size, 60_000);
        assert_eq!(row.content_hash, "hash_v2");
        assert_eq!(row.mime_type, "image/png");
        assert_eq!(row.source_url, "https://cdn.example/570/cover.png");

        // Verify only one row exists for this key.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM game_image_cache WHERE steam_app_id = '570' AND image_type = 'cover'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "upsert must not create duplicate rows");
    }

    #[test]
    fn get_returns_none_for_missing_entry() {
        let conn = open_test_db();
        let result = get_game_image(&conn, "999999", "cover").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn evict_expired_images_removes_expired_rows_and_returns_paths() {
        let conn = open_test_db();

        // Insert an already-expired row (expires_at in the past).
        conn.execute(
            "INSERT INTO game_image_cache
                 (cache_id, steam_app_id, image_type, source, file_path, file_size,
                  content_hash, mime_type, source_url, preferred_source,
                  fetched_at, created_at, updated_at, expires_at)
             VALUES ('id-expired', '440', 'cover', 'steam_cdn', '/cache/expired.jpg', 0,
                     '', 'image/jpeg', '', 'auto',
                     datetime('now'), datetime('now'), datetime('now'),
                     datetime('now', '-1 day'))",
            [],
        )
        .unwrap();

        // Insert a row that is NOT expired.
        conn.execute(
            "INSERT INTO game_image_cache
                 (cache_id, steam_app_id, image_type, source, file_path, file_size,
                  content_hash, mime_type, source_url, preferred_source,
                  fetched_at, created_at, updated_at, expires_at)
             VALUES ('id-valid', '730', 'cover', 'steam_cdn', '/cache/valid.jpg', 0,
                     '', 'image/jpeg', '', 'auto',
                     datetime('now'), datetime('now'), datetime('now'),
                     datetime('now', '+7 days'))",
            [],
        )
        .unwrap();

        // Insert a row with NULL expires_at (never expires).
        conn.execute(
            "INSERT INTO game_image_cache
                 (cache_id, steam_app_id, image_type, source, file_path, file_size,
                  content_hash, mime_type, source_url, preferred_source,
                  fetched_at, created_at, updated_at, expires_at)
             VALUES ('id-never', '271590', 'cover', 'steam_cdn', '/cache/never.jpg', 0,
                     '', 'image/jpeg', '', 'auto',
                     datetime('now'), datetime('now'), datetime('now'),
                     NULL)",
            [],
        )
        .unwrap();

        let evicted = evict_expired_images(&conn).unwrap();

        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0], "/cache/expired.jpg");

        // Confirm the expired row is gone and the others remain.
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM game_image_cache", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 2, "only the expired row should be deleted");

        let still_there = get_game_image(&conn, "730", "cover").unwrap();
        assert!(still_there.is_some(), "valid row must survive eviction");

        let never_there = get_game_image(&conn, "271590", "cover").unwrap();
        assert!(
            never_there.is_some(),
            "never-expiring row must survive eviction"
        );
    }
}
