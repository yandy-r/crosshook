//! Schema migrations. Current: v23. Tier files: [`v1_v10`], [`v11_v20`], [`v21_v23`].
//! See [`super`] for the metadata facade.

mod v11_v20;
mod v1_v10;
mod v21_v23;

#[cfg(test)]
mod tests;

use super::MetadataStoreError;
use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<(), MetadataStoreError> {
    let version = conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "read metadata schema version",
            source,
        })?;

    if version < 1 {
        v1_v10::migrate_0_to_1(conn)?;
        conn.pragma_update(None, "user_version", 1_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 2 {
        v1_v10::migrate_1_to_2(conn)?;
        conn.pragma_update(None, "user_version", 2_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 3 {
        v1_v10::migrate_2_to_3(conn)?;
        conn.pragma_update(None, "user_version", 3_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 4 {
        v1_v10::migrate_3_to_4(conn)?;
        conn.pragma_update(None, "user_version", 4_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 5 {
        v1_v10::migrate_4_to_5(conn)?;
        conn.pragma_update(None, "user_version", 5_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 6 {
        v1_v10::migrate_5_to_6(conn)?;
        conn.pragma_update(None, "user_version", 6_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 7 {
        v1_v10::migrate_6_to_7(conn)?;
        conn.pragma_update(None, "user_version", 7_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 8 {
        v1_v10::migrate_7_to_8(conn)?;
        conn.pragma_update(None, "user_version", 8_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 9 {
        v1_v10::migrate_8_to_9(conn)?;
        conn.pragma_update(None, "user_version", 9_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 10 {
        v1_v10::migrate_9_to_10(conn)?;
        conn.pragma_update(None, "user_version", 10_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 11 {
        v11_v20::migrate_10_to_11(conn)?;
        conn.pragma_update(None, "user_version", 11_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 12 {
        v11_v20::migrate_11_to_12(conn)?;
        conn.pragma_update(None, "user_version", 12_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 12",
                source,
            })?;
    }

    if version < 13 {
        v11_v20::migrate_12_to_13(conn)?;
        conn.pragma_update(None, "user_version", 13_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 13",
                source,
            })?;
    }

    if version < 14 {
        v11_v20::migrate_13_to_14(conn)?;
        conn.pragma_update(None, "user_version", 14_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 14",
                source,
            })?;
    }

    if version < 15 {
        v11_v20::migrate_14_to_15(conn)?;
        conn.pragma_update(None, "user_version", 15_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 15",
                source,
            })?;
    }

    if version < 16 {
        v11_v20::migrate_15_to_16(conn)?;
        conn.pragma_update(None, "user_version", 16_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 16",
                source,
            })?;
    }

    if version < 17 {
        v11_v20::migrate_16_to_17(conn)?;
        conn.pragma_update(None, "user_version", 17_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 17",
                source,
            })?;
    }

    if version < 18 {
        v11_v20::migrate_17_to_18(conn)?;
        conn.pragma_update(None, "user_version", 18_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 18",
                source,
            })?;
    }

    if version < 19 {
        v11_v20::migrate_18_to_19(conn)?;
        conn.pragma_update(None, "user_version", 19_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 19",
                source,
            })?;
    }

    if version < 20 {
        v11_v20::migrate_19_to_20(conn)?;
        conn.pragma_update(None, "user_version", 20_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 20",
                source,
            })?;
    }

    if version < 21 {
        v21_v23::migrate_20_to_21(conn)?;
        conn.pragma_update(None, "user_version", 21_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 21",
                source,
            })?;
    }

    if version < 22 {
        v21_v23::migrate_21_to_22(conn)?;
        conn.pragma_update(None, "user_version", 22_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 22",
                source,
            })?;
    }

    if version < 23 {
        v21_v23::migrate_22_to_23(conn)?;
        conn.pragma_update(None, "user_version", 23_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "set user_version to 23",
                source,
            })?;
    }

    Ok(())
}
