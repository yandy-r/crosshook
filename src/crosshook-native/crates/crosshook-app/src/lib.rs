//! Application layer: an owner handle ([`App`]) over an explicit Tokio
//! runtime, a bounded event bus, sanitized presenter errors, and
//! toolkit-independent [`services`] over `crosshook-core` stores.
//!
//! No UI toolkit dependency lives here; shells (Tauri, native UI) delegate to
//! the services. See the crate `README.md`.

#![deny(missing_docs)]

mod app;
pub mod bootstrap;
mod error;
mod events;
pub mod jobs;
pub mod log_tail;
pub mod presenter;
mod runtime;
pub mod services;

pub use app::App;
pub use bootstrap::BootstrappedApp;
pub use error::{AppError, AppErrorKind};
pub use events::{
    AppEvent, EventBus, EventReceiver, ProfileChangeReason, StartupJobKind, DEFAULT_EVENT_CAPACITY,
    MAX_EVENT_CAPACITY,
};
pub use services::launch::{
    InjectionLogData, LaunchEvent, LaunchEvents, LaunchSession, LaunchStartError,
};
pub use services::{
    CollectionsService, CommunityService, DiscoveryService, InstallService, LaunchService,
    LauncherExportService, LutrisService, ModsService, PrefixDepsService, ProfileWriter,
    ProtonDbService, ProtonMigrationService, ProtonUpService, RunExecutableService, SteamService,
    StorageService, UiSettingsService, UmuService, UpdateService, VersionService,
};
