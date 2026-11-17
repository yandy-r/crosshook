//! Tauri command handlers. Every handler is `async`, takes the one managed
//! `State<'_, BootstrappedApp>` and delegates to `app.services()`; payload
//! shapes, relays and event names live in `crosshook_app::ipc` and
//! `crate::event_bridge`.

pub mod bench;
pub mod catalog;
pub mod collections;
pub mod community;
pub mod diagnostics;
pub mod discovery;
pub mod export;
pub mod game_metadata;
pub mod health;
pub mod install;
pub mod launch;
pub mod lutris;
pub mod metadata;
pub mod migration;
pub mod mods;
pub mod offline;
pub mod onboarding;
pub mod prefix_deps;
pub mod profile;
pub mod protondb;
pub mod protonup;
pub mod run_executable;
pub mod settings;
pub mod shared;
pub mod startup;
pub mod steam;
pub mod storage;
pub mod umu_database;
pub mod update;
pub mod version;

use crosshook_app::BootstrappedApp;
use tauri::State;

/// The one managed state every command borrows.
pub(crate) type AppState<'a> = State<'a, BootstrappedApp>;

/// Plain IPC result: `Ok` serializes exactly like the bare value, `Err` is the
/// sanitized message string.
pub(crate) type IpcResult<T> = Result<T, String>;
