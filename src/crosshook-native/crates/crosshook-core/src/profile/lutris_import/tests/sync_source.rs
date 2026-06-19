use crate::metadata::SyncSource;

#[test]
fn sync_source_lutris_import_as_str() {
    assert_eq!(SyncSource::LutrisImport.as_str(), "lutris_import");
}
