//! Phase-B store ownership and ordered initialization.

use std::ops::Deref;
use std::sync::Arc;

use super::{
    catalogs, reconcile, scans, stores, ui_import, BootstrapError, BootstrapOptions, BootstrapStep,
    DegradedStores, FlatpakStartup, JobState, LoggingMode, PreRuntime, RuntimeChoice, Services,
    StartupHandle, StartupJobs, StartupReport, StepLog, StepOutcome, StoreRoots, Stores,
};
use crate::App;
use crosshook_core::logging::{self, LoggingError};
use crosshook_core::metadata::MetadataStatus;
use crosshook_core::settings::{
    webkit_import::webkit_local_storage_paths, AppSettingsData, SettingsStore,
};

const METADATA_UNAVAILABLE: &str = "metadata unavailable or read-only";

struct BootstrapData {
    stores: Stores,
    services: Services,
    startup: StartupHandle,
    steps: StepLog,
}

/// An [`App`] whose stores, services and startup state were initialized by
/// [`App::bootstrap`].
///
/// Cheap to [`Clone`]; clones share one [`App`] and one set of stores.
/// Dereferences to [`App`] for the runtime, event bus and job registry. The
/// bootstrapped accessors are infallible because the only constructor is
/// [`App::bootstrap`]; runtime-only owners ([`App::new`], [`App::with_handle`])
/// have no such accessors.
#[derive(Clone)]
pub struct BootstrappedApp {
    app: App,
    data: Arc<BootstrapData>,
}

impl BootstrappedApp {
    /// The underlying runtime owner.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// Persistent stores initialized by [`App::bootstrap`].
    pub fn stores(&self) -> &Stores {
        &self.data.stores
    }

    /// Toolkit-independent services initialized by [`App::bootstrap`].
    pub fn services(&self) -> &Services {
        &self.data.services
    }

    /// Cloneable snapshot handle that does not retain an application owner.
    pub fn startup(&self) -> StartupHandle {
        self.data.startup.clone()
    }

    /// Reads the live, idempotent startup snapshot.
    pub fn startup_report(&self) -> StartupReport {
        self.data.startup.snapshot()
    }

    /// Ordered initialization outcomes from both bootstrap phases.
    pub fn bootstrap_steps(&self) -> &StepLog {
        &self.data.steps
    }
}

impl Deref for BootstrappedApp {
    type Target = App;

    fn deref(&self) -> &App {
        &self.app
    }
}

static_assertions::assert_impl_all!(BootstrappedApp: Send, Sync);

impl App {
    /// Consumes the pre-runtime token, initializes stores and schedules startup jobs.
    ///
    /// Required store and runtime failures are typed errors; optional facilities
    /// degrade in the report. This function never exits the process.
    pub fn bootstrap(
        pre: PreRuntime,
        opts: BootstrapOptions,
    ) -> Result<(BootstrappedApp, StartupReport), BootstrapError> {
        let (mut steps, mode, migration) = pre.into_parts();
        let app = match opts.runtime {
            RuntimeChoice::Owned => App::new().map_err(BootstrapError::Runtime)?,
            RuntimeChoice::Handle(handle) => App::with_handle(handle),
        };
        steps.push(BootstrapStep::Runtime, StepOutcome::Ran);
        let settings_store = stores::init_settings(&opts.store_roots)?;
        let (settings, settings_unreadable) = load_settings_step(&settings_store, &mut steps);
        let logging_unavailable = init_logging_step(opts.logging, &settings, &mut steps);
        relog_degraded_phase_a(&steps);
        let local_storage = match &opts.store_roots {
            StoreRoots::Xdg => webkit_local_storage_paths(),
            StoreRoots::Explicit { .. } => Vec::new(),
        };
        ui_import::import_step(
            &settings_store,
            &settings,
            settings_unreadable,
            &local_storage,
        );
        development_scripts_step(&mut steps);
        let (stores, metadata_error) =
            init_stores_step(&opts.store_roots, &settings_store, &settings, &mut steps)?;
        init_metadata_steps(&stores, &settings, &mut steps);
        let services = Services::new(&stores, app.events().clone(), app.runtime_handle());
        let startup = StartupHandle::new(
            stores.clone(),
            FlatpakStartup { mode, migration },
            DegradedStores {
                metadata: stores.metadata.status(),
                metadata_error,
                settings_unreadable,
                logging_unavailable,
            },
            StartupJobs {
                health: JobState::Pending,
                offline_readiness: JobState::Pending,
                version_scan: JobState::Pending,
                umu_refresh: JobState::Pending,
                background_portal: JobState::Pending,
            },
        );
        steps.push(BootstrapStep::StartupJobs, StepOutcome::Ran);
        let boot = BootstrappedApp {
            app,
            data: Arc::new(BootstrapData {
                stores,
                services,
                startup: startup.clone(),
                steps,
            }),
        };
        scans::schedule(
            boot.app(),
            startup,
            boot.stores(),
            boot.services(),
            opts.umu_refresh,
        );
        let report = boot.startup_report();
        Ok((boot, report))
    }
}

/// Loads settings, falling back to defaults. Returns `(settings, unreadable)`.
fn load_settings_step(store: &SettingsStore, steps: &mut StepLog) -> (AppSettingsData, bool) {
    let loaded = store.load();
    let unreadable = loaded.is_err();
    if let Err(error) = &loaded {
        tracing::warn!(%error, "startup settings unreadable; using defaults");
    }
    steps.push(
        BootstrapStep::SettingsStore,
        if unreadable {
            StepOutcome::Degraded("settings unreadable")
        } else {
            StepOutcome::Ran
        },
    );
    (loaded.unwrap_or_default(), unreadable)
}

/// Initializes logging per `mode`. Returns whether logging is unavailable.
fn init_logging_step(mode: LoggingMode, settings: &AppSettingsData, steps: &mut StepLog) -> bool {
    let LoggingMode::Init { mirror_stdout } = mode else {
        steps.push(
            BootstrapStep::Logging,
            StepOutcome::Skipped("logging disabled"),
        );
        return false;
    };
    let unavailable = match logging::init_logging(mirror_stdout, Some(settings.log_filter.trim())) {
        Ok(path) => {
            tracing::info!(log_path = %path.display(), "starting CrossHook Native");
            false
        }
        Err(LoggingError::SubscriberAlreadyInitialized(_)) => false,
        Err(error) => {
            eprintln!("CrossHook: logging unavailable: {error}");
            true
        }
    };
    steps.push(
        BootstrapStep::Logging,
        if unavailable {
            StepOutcome::Degraded("logging unavailable")
        } else {
            StepOutcome::Ran
        },
    );
    unavailable
}

fn development_scripts_step(steps: &mut StepLog) {
    let scripts = crosshook_core::runtime_helpers::ensure_development_scripts_executable();
    if let Err(error) = &scripts {
        tracing::warn!(%error, "development scripts could not be made executable");
    }
    steps.push(
        BootstrapStep::DevelopmentScripts,
        if scripts.is_ok() {
            StepOutcome::Ran
        } else {
            StepOutcome::Degraded("development scripts unavailable")
        },
    );
}

/// Opens the remaining stores and records their steps. Returns the bundle and
/// the sanitized metadata-unavailable detail.
fn init_stores_step(
    roots: &StoreRoots,
    settings_store: &SettingsStore,
    settings: &AppSettingsData,
    steps: &mut StepLog,
) -> Result<(Stores, Option<String>), BootstrapError> {
    let (stores, metadata_error) = stores::init_remaining(roots, settings_store, settings)?;
    for step in [
        BootstrapStep::ProfileStore,
        BootstrapStep::RecentFilesStore,
        BootstrapStep::CommunityTapStore,
    ] {
        steps.push(step, StepOutcome::Ran);
    }
    steps.push(
        BootstrapStep::MetadataStore,
        if stores.metadata.status() != MetadataStatus::Ok {
            StepOutcome::Degraded(METADATA_UNAVAILABLE)
        } else {
            StepOutcome::Ran
        },
    );
    Ok((stores, metadata_error))
}

/// Catalog initialization, reconciliation and nag sync. The metadata-backed
/// steps are skipped when the metadata store is unavailable or read-only.
fn init_metadata_steps(stores: &Stores, settings: &AppSettingsData, steps: &mut StepLog) {
    catalogs::initialize(&stores.settings);
    steps.push(BootstrapStep::Catalogs, StepOutcome::Ran);
    if stores.metadata.status() != MetadataStatus::Ok {
        tracing::warn!(
            status = ?stores.metadata.status(),
            "metadata read-only or unavailable; optional startup writes skipped"
        );
        steps.push(
            BootstrapStep::MetadataReconciliation,
            StepOutcome::Skipped(METADATA_UNAVAILABLE),
        );
        steps.push(
            BootstrapStep::NagSync,
            StepOutcome::Skipped(METADATA_UNAVAILABLE),
        );
        return;
    }
    let reconciled = reconcile::run_metadata_reconciliation(&stores.metadata, &stores.profiles);
    if let Err(error) = &reconciled {
        tracing::warn!(%error, "startup metadata reconciliation failed");
    }
    catalogs::persist(&stores.metadata);
    steps.push(
        BootstrapStep::MetadataReconciliation,
        if reconciled.is_ok() {
            StepOutcome::Ran
        } else {
            StepOutcome::Degraded("metadata reconciliation failed")
        },
    );
    reconcile::sync_nags(settings, &stores.metadata);
    steps.push(BootstrapStep::NagSync, StepOutcome::Ran);
}

/// Re-logs every degraded Phase A step once logging is installed.
///
/// Phase A records those outcomes before the subscriber exists, so the
/// original `tracing` warnings never reach the log. The step log is the
/// source.
fn relog_degraded_phase_a(steps: &StepLog) {
    for record in steps.records() {
        if !BootstrapStep::PRE_RUNTIME_ORDER.contains(&record.step) {
            continue;
        }
        let StepOutcome::Degraded(reason) = record.outcome else {
            continue;
        };
        tracing::warn!(
            step = record.step.name(),
            reason,
            "pre-runtime step degraded"
        );
    }
}
