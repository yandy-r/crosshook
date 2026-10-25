//! UI environment policy hook and its guarded environment accessor.
//!
//! Shells supply an [`EnvPolicy`] to adjust renderer-related process
//! environment variables (for example WebKitGTK variables in the Classic
//! shell). The policy runs exactly once inside
//! [`pre_runtime`](super::pre_runtime::pre_runtime), after both migrations
//! before CrossHook starts any thread of its own (externally preloaded
//! libraries may already own threads; `pre_runtime` records that as a
//! degraded step and still runs the policy). [`PreRuntimeEnv`] is the
//! only handle a policy receives; it cannot be constructed outside
//! `pre_runtime`, which is what keeps every process-environment mutation on
//! the audited startup path.

use std::ffi::{OsStr, OsString};

use super::host::{PreRuntimeHost, SystemHost};
use super::report::FlatpakMigrationReport;

/// Shell-supplied hook that adjusts process environment variables pre-runtime.
///
/// Implementations are called exactly once, after both migrations, before
/// CrossHook starts any thread of its own. The process may still have
/// threads from externally preloaded libraries; that is recorded by
/// [`pre_runtime`](super::pre_runtime::pre_runtime) as a degraded step and
/// does not prevent the policy from running.
pub trait EnvPolicy: Send + Sync {
    /// Applies the policy through the guarded environment accessor.
    fn apply(&self, env: &mut PreRuntimeEnv<'_>);
}

/// Guarded accessor for process environment variables during Phase A.
///
/// Reads and writes are forwarded to the active
/// `PreRuntimeHost`: production uses
/// `SystemHost`, which performs the real `std::env` mutation at the single
/// documented `unsafe` site, while tests use a recording fake. Every write
/// records a `crosshook_core::platform::trace::env_mutation` event at the
/// moment of mutation.
///
/// This type deliberately has no public constructor: only
/// [`pre_runtime`](super::pre_runtime::pre_runtime) (and its host seam) can
/// create one, so no other code can obtain a sanctioned mutation handle.
pub struct PreRuntimeEnv<'a> {
    host: &'a mut dyn PreRuntimeHost,
}

impl<'a> PreRuntimeEnv<'a> {
    pub(crate) fn new(host: &'a mut dyn PreRuntimeHost) -> Self {
        Self { host }
    }

    /// Reads an environment variable without mutating it.
    pub fn var_os(&self, key: impl AsRef<OsStr>) -> Option<OsString> {
        self.host.env_var(key.as_ref())
    }

    /// Sets an environment variable, recording the mutation in the trace.
    pub fn set(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) {
        let key = key.as_ref();
        crosshook_core::platform::trace::env_mutation(key.to_string_lossy().as_ref());
        self.host.set_env_var(key, value.as_ref());
    }

    /// Removes an environment variable, recording the mutation in the trace.
    pub fn remove(&mut self, key: impl AsRef<OsStr>) {
        let key = key.as_ref();
        crosshook_core::platform::trace::env_mutation(key.to_string_lossy().as_ref());
        self.host.remove_env_var(key);
    }
}
impl PreRuntimeHost for SystemHost {
    fn thread_count(&self) -> Option<u32> {
        crosshook_core::platform::trace::thread_count()
    }

    fn is_flatpak(&self) -> bool {
        crosshook_core::platform::is_flatpak()
    }

    fn host_xdg_opt_in(&self) -> bool {
        crosshook_core::flatpak_migration::is_host_xdg_opt_in()
    }

    fn override_xdg(&mut self) {
        // SAFETY: `pre_runtime` runs before CrossHook creates any runtime,
        // builder, or thread, and before any `BaseDirs` lookup, so CrossHook
        // has no concurrent env readers. Externally preloaded libraries may
        // own threads at entry (reported as a degraded step); this preserves
        // the pre-contract behavior and cannot be fully excluded.
        unsafe { crosshook_core::platform::override_xdg_for_flatpak_host_access() }
    }

    fn run_flatpak_migration(&mut self) -> Result<FlatpakMigrationReport, String> {
        match crosshook_core::flatpak_migration::run() {
            Ok(outcome) => Ok(FlatpakMigrationReport {
                imported_config: outcome.imported_config,
                imported_subtrees: outcome
                    .imported_subtrees
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
                skipped_subtrees: outcome
                    .skipped_subtrees
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
                error: None,
            }),
            Err(error) => Err(error.to_string()),
        }
    }

    fn migrate_legacy_app_id(&mut self) {
        crosshook_core::app_id_migration::migrate_legacy_tauri_app_id_xdg_directories();
    }

    fn env_var(&self, key: &OsStr) -> Option<OsString> {
        std::env::var_os(key)
    }

    fn set_env_var(&mut self, key: &OsStr, value: &OsStr) {
        // SAFETY: see `SystemHost` docs — pre-runtime only, before CrossHook
        // spawns any thread, reached exclusively through `PreRuntimeEnv`
        // inside `pre_runtime`. Preloaded external threads are not excluded.
        unsafe { std::env::set_var(key, value) }
    }

    fn remove_env_var(&mut self, key: &OsStr) {
        // SAFETY: see `SystemHost` docs — pre-runtime only, before CrossHook
        // spawns any thread, reached exclusively through `PreRuntimeEnv`
        // inside `pre_runtime`. Preloaded external threads are not excluded.
        unsafe { std::env::remove_var(key) }
    }
}
