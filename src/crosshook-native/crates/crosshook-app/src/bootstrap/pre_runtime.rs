//! Phase A: synchronous pre-runtime bootstrap, before CrossHook starts any thread.
//!
//! [`pre_runtime`] runs before any Tokio runtime, Tauri builder, GTK
//! initialization, or thread spawned by CrossHook itself, in this fixed
//! order (externally preloaded libraries may already own threads; see
//! [`pre_runtime`]):
//!
//! 1. Flatpak gate (+ XDG override in host-XDG shared mode)
//! 2. First-run Flatpak migration (isolated mode only)
//! 3. Legacy Tauri app-id migration
//! 4. UI [`EnvPolicy`] hook
//!
//! Every step is recorded in the returned token's [`StepLog`], including
//! steps that are skipped in the current mode, so one expected order holds
//! everywhere. The returned [`PreRuntime`] token is consumed by
//! `App::bootstrap`, which makes skipping Phase A a compile error.

use serde::Serialize;

use super::env_policy::{EnvPolicy, PreRuntimeEnv};
use super::error::BootstrapError;
use super::host::{PreRuntimeHost, SystemHost};
use super::report::FlatpakMigrationReport;
use super::steps::{BootstrapStep, StepLog, StepOutcome};
use crosshook_core::platform::trace;

/// Options for Phase A. `#[non_exhaustive]` so later options can be added
/// without a breaking change.
#[non_exhaustive]
pub struct PreRuntimeOptions {
    /// Optional shell environment policy, applied after both migrations.
    pub env_policy: Option<Box<dyn EnvPolicy>>,
}

impl PreRuntimeOptions {
    /// Creates options with no environment policy.
    pub fn new() -> Self {
        Self { env_policy: None }
    }

    /// Attaches the shell's environment policy.
    pub fn with_env_policy(mut self, policy: impl EnvPolicy + 'static) -> Self {
        self.env_policy = Some(Box::new(policy));
        self
    }
}

impl Default for PreRuntimeOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// How the Flatpak startup gate resolved this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlatpakMode {
    /// Not running under Flatpak.
    Native,
    /// Flatpak per-app isolation (default); first-run migration applies.
    Isolated,
    /// Flatpak host-XDG shared mode (explicit opt-in).
    HostXdgShared,
}

/// Token proving Phase A ran. Only constructible by [`pre_runtime`] (and
/// its test-seams host variant); consumed by `App::bootstrap`.
pub struct PreRuntime {
    pub(crate) steps: StepLog,
    pub(crate) mode: FlatpakMode,
    pub(crate) migration: Option<FlatpakMigrationReport>,
}

impl PreRuntime {
    /// Ordered record of every bootstrap step so far.
    pub fn steps(&self) -> &StepLog {
        &self.steps
    }

    /// Flatpak mode decided by the startup gate.
    pub fn flatpak_mode(&self) -> FlatpakMode {
        self.mode
    }

    /// First-run Flatpak migration outcome, when migration ran.
    pub fn migration(&self) -> Option<&FlatpakMigrationReport> {
        self.migration.as_ref()
    }

    /// Splits the token into the parts Phase B continues with.
    pub(crate) fn into_parts(self) -> (StepLog, FlatpakMode, Option<FlatpakMigrationReport>) {
        (self.steps, self.mode, self.migration)
    }
}

/// Runs Phase A with the production system host.
///
/// Phase A never refuses to start. When the process already has more than one
/// thread at entry (for example threads created by `LD_PRELOAD` libraries such
/// as the Steam overlay), the condition is logged and the environment step is
/// recorded as [`StepOutcome::Degraded`]; every step still runs, matching the
/// behavior before the ordering contract existed. Migration failures are
/// best-effort: logged, recorded as degraded, and never returned as `Err`.
/// The `Result` return type is kept so future fatal pre-runtime failures do
/// not change the signature.
pub fn pre_runtime(opts: PreRuntimeOptions) -> Result<PreRuntime, BootstrapError> {
    let mut host = SystemHost::new();
    run_pre_runtime(opts, &mut host)
}

#[cfg(feature = "test-seams")]
#[doc(hidden)]
/// Test seam: runs Phase A against an injected host so integration tests
/// can observe step order without a real sandbox.
pub fn pre_runtime_with_host(
    opts: PreRuntimeOptions,
    host: &mut impl PreRuntimeHost,
) -> Result<PreRuntime, BootstrapError> {
    run_pre_runtime(opts, host)
}

#[cfg(feature = "test-seams")]
#[doc(hidden)]
/// Test seam: deliberately runs Phase A steps out of order
/// (legacy app-id → gate → Flatpak migration → env policy). The shared
/// order checker and the trace assertions must report this as a violation;
/// used by the `bootstrap_ordering` harness to prove the checks are real.
pub fn pre_runtime_misordered_with_host(
    opts: PreRuntimeOptions,
    host: &mut impl PreRuntimeHost,
) -> Result<PreRuntime, BootstrapError> {
    run_pre_runtime_misordered(opts, host)
}

fn run_pre_runtime(
    opts: PreRuntimeOptions,
    host: &mut dyn PreRuntimeHost,
) -> Result<PreRuntime, BootstrapError> {
    let multi_threaded = note_entry_threads(host);

    // SEAM(M1-12): instance lock acquires here (step 0)

    let mut log = StepLog::new();
    // Production order: 1 -> 2 -> 3 -> 4. Swapping these calls must
    // fail the unit order tests and the ordering harness.
    let mode = step_flatpak_gate(host, &mut log);
    let migration = step_flatpak_migration(host, mode, &mut log);
    step_legacy_app_id(host, &mut log);
    step_env_policy(opts.env_policy.as_deref(), host, multi_threaded, &mut log);
    finish_pre_runtime(log, mode, migration)
}

/// Deliberate harness order: 3 -> 1 -> 2 -> 4. Compiled only for the
/// ordering seam; production startup has no misordered path.
#[cfg(feature = "test-seams")]
fn run_pre_runtime_misordered(
    opts: PreRuntimeOptions,
    host: &mut dyn PreRuntimeHost,
) -> Result<PreRuntime, BootstrapError> {
    let multi_threaded = note_entry_threads(host);
    let mut log = StepLog::new();
    step_legacy_app_id(host, &mut log);
    let mode = step_flatpak_gate(host, &mut log);
    let migration = step_flatpak_migration(host, mode, &mut log);
    step_env_policy(opts.env_policy.as_deref(), host, multi_threaded, &mut log);
    finish_pre_runtime(log, mode, migration)
}

fn note_entry_threads(host: &mut dyn PreRuntimeHost) -> bool {
    // Environment mutation is only guaranteed sound before any other thread
    // exists. Threads can already exist at entry when external libraries are
    // preloaded (for example the Steam overlay), and the pre-contract startup
    // mutated the environment regardless, so a multi-threaded entry degrades
    // instead of aborting. An unreadable `/proc` cannot prove the precondition
    // either way, so warn and proceed rather than blocking startup.
    match host.thread_count() {
        Some(threads) if threads > 1 => {
            tracing::warn!(
                threads,
                "pre-runtime entered with multiple threads; environment mutation proceeds in degraded mode"
            );
            eprintln!(
                "CrossHook: pre-runtime entered with {threads} threads; \
                 environment mutation proceeds in degraded mode"
            );
            true
        }
        Some(_) => false,
        None => {
            tracing::warn!("pre-runtime thread count unavailable; proceeding without verification");
            false
        }
    }
}

fn finish_pre_runtime(
    steps: StepLog,
    mode: FlatpakMode,
    migration: Option<FlatpakMigrationReport>,
) -> Result<PreRuntime, BootstrapError> {
    trace::mark("pre:done");
    Ok(PreRuntime {
        steps,
        mode,
        migration,
    })
}

fn step_flatpak_gate(host: &mut dyn PreRuntimeHost, log: &mut StepLog) -> FlatpakMode {
    trace::mark("gate:begin");
    let (mode, outcome) = if !host.is_flatpak() {
        (
            FlatpakMode::Native,
            StepOutcome::Skipped("not running under Flatpak"),
        )
    } else if host.host_xdg_opt_in() {
        tracing::info!(mode = "host-xdg-shared", "flatpak startup mode");
        eprintln!("CrossHook: flatpak host-XDG shared mode (opt-in)");
        host.override_xdg();
        (FlatpakMode::HostXdgShared, StepOutcome::Ran)
    } else {
        tracing::info!(mode = "per-app-isolation", "flatpak startup mode");
        eprintln!("CrossHook: flatpak per-app isolation mode");
        (FlatpakMode::Isolated, StepOutcome::Ran)
    };
    log.push(BootstrapStep::FlatpakGate, outcome);
    trace::mark("gate:end");
    mode
}

fn step_flatpak_migration(
    host: &mut dyn PreRuntimeHost,
    mode: FlatpakMode,
    log: &mut StepLog,
) -> Option<FlatpakMigrationReport> {
    trace::mark("flatpak_migration:begin");
    let (report, outcome) = if mode != FlatpakMode::Isolated {
        (None, StepOutcome::Skipped("not in Flatpak isolated mode"))
    } else {
        match host.run_flatpak_migration() {
            Ok(report) => (Some(report), StepOutcome::Ran),
            Err(detail) => {
                let sanitized = crate::presenter::sanitize_message(&detail);
                tracing::warn!(error = %sanitized, "flatpak first-run migration failed");
                eprintln!("CrossHook: flatpak first-run migration failed: {sanitized}");
                (
                    Some(FlatpakMigrationReport {
                        imported_config: false,
                        imported_subtrees: Vec::new(),
                        skipped_subtrees: Vec::new(),
                        error: Some(sanitized),
                    }),
                    StepOutcome::Degraded("flatpak migration failed"),
                )
            }
        }
    };
    log.push(BootstrapStep::FlatpakMigration, outcome);
    trace::mark("flatpak_migration:end");
    report
}

fn step_legacy_app_id(host: &mut dyn PreRuntimeHost, log: &mut StepLog) {
    trace::mark("legacy_app_id:begin");
    host.migrate_legacy_app_id();
    log.push(BootstrapStep::LegacyAppIdMigration, StepOutcome::Ran);
    trace::mark("legacy_app_id:end");
}

fn step_env_policy(
    policy: Option<&dyn EnvPolicy>,
    host: &mut dyn PreRuntimeHost,
    multi_threaded: bool,
    log: &mut StepLog,
) {
    trace::mark("env_policy:begin");
    let applied = match policy {
        Some(policy) => {
            let mut env = PreRuntimeEnv::new(host);
            policy.apply(&mut env);
            true
        }
        None => false,
    };
    let outcome = if multi_threaded {
        StepOutcome::Degraded("multi-threaded at entry")
    } else if applied {
        StepOutcome::Ran
    } else {
        StepOutcome::Skipped("no env policy")
    };
    log.push(BootstrapStep::EnvPolicy, outcome);
    trace::mark("env_policy:end");
}

#[cfg(test)]
mod tests;
