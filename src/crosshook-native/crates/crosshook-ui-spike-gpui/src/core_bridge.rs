//! The only module that calls `crosshook-core` (acceptance criterion 5).
//!
//! Every core call runs on the app-owned tokio runtime's blocking pool, reached through the
//! vendored `gpui_tokio` (`Tokio::spawn`), never on gpui's foreground executor. Enforcement:
//!
//! 1. Structural: `scripts/spike-gpui-core-boundary.sh` (and the `core_calls_only_here` test)
//!    fail if `crosshook_core` appears in any other source file.
//! 2. Runtime: each blocking function starts with `assert_off_ui_thread()`, which panics if it
//!    runs on the thread that started gpui (recorded in `main` before `Application::run`).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::thread::ThreadId;

use crosshook_core::profile::{GamescopeConfig, ProfileStore};
use gpui::{App, AppContext, Task};

#[path = "../vendor/gpui_tokio/src/gpui_tokio.rs"]
pub mod gpui_tokio;

pub use gpui_tokio::Tokio;

static UI_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// Call on the thread that will run gpui's foreground executor, before `Application::run`.
pub fn mark_ui_thread() {
    let _ = UI_THREAD.set(std::thread::current().id());
}

/// Panics if called on the gpui foreground thread.
pub fn assert_off_ui_thread() {
    if let Some(ui) = UI_THREAD.get() {
        assert_ne!(
            *ui,
            std::thread::current().id(),
            "crosshook-core called on the gpui foreground thread"
        );
    }
}

/// The app-owned runtime crosshook-core needs (built in `main`, before gpui starts).
pub fn build_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("spike-tokio")
        .enable_all()
        .build()
        .expect("tokio runtime")
}

/// Hands gpui_tokio a handle to the app-owned runtime (`Tokio::spawn` then targets it).
pub fn init_from_handle(cx: &mut App, handle: tokio::runtime::Handle) {
    gpui_tokio::init_from_handle(cx, handle);
}

/// Runs blocking core work on the tokio blocking pool; resolves on gpui's side as a `Task`.
pub fn blocking<C, T>(
    cx: &C,
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Task<Result<T, String>>
where
    C: AppContext,
    T: Send + 'static,
{
    let task = Tokio::spawn(cx, async move {
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|e| e.to_string())?
    });
    cx.background_spawn(async move { task.await.map_err(|e| e.to_string())? })
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub profiles: PathBuf,
    pub data_home: PathBuf,
}

/// XDG roots via the core gateway `platform::base_dirs()` (never `BaseDirs::new()`).
pub fn paths() -> Paths {
    assert_off_ui_thread();
    let dirs = crosshook_core::platform::base_dirs();
    let config = dirs.as_ref().map_or_else(
        || PathBuf::from(".config"),
        |d| d.config_dir().to_path_buf(),
    );
    let data = dirs.as_ref().map_or_else(
        || PathBuf::from(".local/share"),
        |d| d.data_dir().to_path_buf(),
    );
    Paths {
        profiles: config.join("crosshook/profiles"),
        data_home: data,
    }
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub name: String,
    pub title: String,
    pub app_id: String,
}

fn store(paths: &Paths) -> ProfileStore {
    ProfileStore::with_base_path(paths.profiles.clone())
}

/// Blocking: every profile summary sorted by title, plus the resolved paths.
pub fn load_summaries() -> Result<(Paths, Vec<Summary>), String> {
    assert_off_ui_thread();
    let paths = paths();
    let store = store(&paths);
    let names = store.list().map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        match store.load(&name) {
            Ok(profile) => {
                let title = if profile.game.name.is_empty() {
                    name.clone()
                } else {
                    profile.game.name.clone()
                };
                let app_id = if profile.steam.app_id.is_empty() {
                    profile.runtime.steam_app_id.clone()
                } else {
                    profile.steam.app_id.clone()
                };
                out.push(Summary {
                    name,
                    title,
                    app_id,
                });
            }
            Err(error) => eprintln!("spike: skip profile {name}: {error}"),
        }
    }
    out.sort_by(|a, b| a.title.cmp(&b.title));
    Ok((paths, out))
}

/// Blocking: one profile's gamescope config.
pub fn load_gamescope(paths: &Paths, name: &str) -> Result<GamescopeConfig, String> {
    assert_off_ui_thread();
    store(paths)
        .load(name)
        .map(|p| p.launch.gamescope)
        .map_err(|e| e.to_string())
}

/// Blocking: read-modify-write one profile's gamescope config (fixture copy only).
pub fn save_gamescope(paths: &Paths, name: &str, config: GamescopeConfig) -> Result<(), String> {
    assert_off_ui_thread();
    let store = store(paths);
    let mut profile = store.load(name).map_err(|e| e.to_string())?;
    profile.launch.gamescope = config;
    store.save(name, &profile).map_err(|e| e.to_string())
}

pub use crosshook_core::profile::{GamescopeConfig as CoreGamescope, GamescopeFilter};

#[cfg(test)]
mod tests {
    #[test]
    fn core_calls_only_here() {
        let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
        let mut offenders = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(src)];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.file_name().unwrap() != "core_bridge.rs"
                    && std::fs::read_to_string(&path)
                        .unwrap()
                        .contains(concat!("crosshook", "_core"))
                {
                    offenders.push(path);
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "core used outside core_bridge: {offenders:?}"
        );
    }

    #[test]
    fn guard_panics_on_ui_thread() {
        super::mark_ui_thread();
        let caught = std::panic::catch_unwind(super::assert_off_ui_thread);
        // Another test may have marked a different thread first; only assert when it is ours.
        if super::UI_THREAD.get() == Some(&std::thread::current().id()) {
            assert!(caught.is_err());
        }
        std::thread::spawn(super::assert_off_ui_thread)
            .join()
            .unwrap();
    }
}
