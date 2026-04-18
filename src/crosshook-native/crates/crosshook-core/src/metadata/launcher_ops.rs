use super::{launch_history, launcher_sync, MetadataStore, MetadataStoreError};
use crate::launch::diagnostics::models::DiagnosticReport;

impl MetadataStore {
    pub fn observe_launcher_exported(
        &self,
        profile_name: Option<&str>,
        slug: &str,
        display_name: &str,
        script_path: &str,
        desktop_entry_path: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a launcher export", |conn| {
            launcher_sync::observe_launcher_exported(
                conn,
                profile_name,
                slug,
                display_name,
                script_path,
                desktop_entry_path,
            )
        })
    }

    pub fn observe_launcher_deleted(&self, launcher_slug: &str) -> Result<(), MetadataStoreError> {
        self.with_conn("observe a launcher deletion", |conn| {
            launcher_sync::observe_launcher_deleted(conn, launcher_slug)
        })
    }

    pub fn observe_launcher_renamed(
        &self,
        old_slug: &str,
        new_slug: &str,
        new_display_name: &str,
        new_script_path: &str,
        new_desktop_entry_path: &str,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn_mut("observe a launcher rename", |conn| {
            launcher_sync::observe_launcher_renamed(
                conn,
                old_slug,
                new_slug,
                new_display_name,
                new_script_path,
                new_desktop_entry_path,
            )
        })
    }

    pub fn record_launch_started(
        &self,
        profile_name: Option<&str>,
        method: &str,
        log_path: Option<&str>,
    ) -> Result<String, MetadataStoreError> {
        self.with_conn("record a launch start", |conn| {
            launch_history::record_launch_started(conn, profile_name, method, log_path)
        })
    }

    pub fn record_launch_finished(
        &self,
        operation_id: &str,
        exit_code: Option<i32>,
        signal: Option<i32>,
        report: &DiagnosticReport,
    ) -> Result<(), MetadataStoreError> {
        self.with_conn("record a launch finish", |conn| {
            launch_history::record_launch_finished(conn, operation_id, exit_code, signal, report)
        })
    }

    pub fn sweep_abandoned_operations(&self) -> Result<usize, MetadataStoreError> {
        self.with_conn("sweep abandoned operations", |conn| {
            launch_history::sweep_abandoned_operations(conn)
        })
    }
}
