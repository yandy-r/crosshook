mod v11_v20;
mod v21_v23;
mod v24_v25;
mod v25_v26;
mod v26_v27;

#[test]
fn migration_ladder_reaches_supported_max_and_rejects_newer_before_writes() {
    use super::{run_migrations, MetadataStoreError, SUPPORTED_MAX_VERSION};
    use rusqlite::Connection;

    let conn = Connection::open_in_memory().unwrap();
    run_migrations(&conn).unwrap();
    let version: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, SUPPORTED_MAX_VERSION);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("metadata.db");
    let newer = SUPPORTED_MAX_VERSION + 1;
    let writer = Connection::open(&path).unwrap();
    writer.pragma_update(None, "user_version", newer).unwrap();
    drop(writer);
    let before = std::fs::read(&path).unwrap();
    let conn = Connection::open(&path).unwrap();
    assert!(matches!(
        run_migrations(&conn),
        Err(MetadataStoreError::NewerSchema { found, supported })
            if found == newer && supported == SUPPORTED_MAX_VERSION
    ));
    drop(conn);
    assert_eq!(before, std::fs::read(&path).unwrap());
}
