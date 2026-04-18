use chrono::Utc;

use super::{preset_store, MetadataStore, MetadataStoreError};
use crate::metadata::models::{BundledOptimizationPresetRow, ProfileLaunchPresetOrigin};

impl MetadataStore {
    pub fn list_bundled_optimization_presets(
        &self,
    ) -> Result<Vec<BundledOptimizationPresetRow>, MetadataStoreError> {
        self.with_conn("list bundled optimization presets", |conn| {
            preset_store::list_bundled_optimization_presets(conn)
        })
    }

    pub fn get_bundled_optimization_preset(
        &self,
        preset_id: &str,
    ) -> Result<Option<BundledOptimizationPresetRow>, MetadataStoreError> {
        self.with_conn("get bundled optimization preset", |conn| {
            preset_store::get_bundled_optimization_preset(conn, preset_id)
        })
    }

    pub fn upsert_profile_launch_preset_metadata(
        &self,
        profile_id: &str,
        preset_name: &str,
        origin: ProfileLaunchPresetOrigin,
        source_bundled_preset_id: Option<&str>,
    ) -> Result<(), MetadataStoreError> {
        let now = Utc::now().to_rfc3339();
        self.with_conn("upsert profile launch preset metadata", |conn| {
            preset_store::upsert_profile_launch_preset_metadata(
                conn,
                profile_id,
                preset_name,
                origin,
                source_bundled_preset_id,
                &now,
            )
        })
    }
}
