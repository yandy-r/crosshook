//! Library data from the real `crosshook-core` `ProfileStore` on the isolated XDG tree.
//! All functions here block; call them through `Task::perform` + `spawn_blocking`.

use std::path::PathBuf;

use crosshook_core::profile::{GamescopeConfig, ProfileStore};

#[derive(Debug, Clone)]
pub struct Paths {
    pub profiles: PathBuf,
    pub data_home: PathBuf,
}

impl Paths {
    /// XDG roots from the environment (the bench runner isolates them). Uses the core
    /// gateway `platform::base_dirs()` rather than reading `directories` directly.
    pub fn from_env() -> Self {
        let dirs = crosshook_core::platform::base_dirs();
        let config = dirs.as_ref().map_or_else(
            || PathBuf::from(".config"),
            |d| d.config_dir().to_path_buf(),
        );
        let data = dirs.as_ref().map_or_else(
            || PathBuf::from(".local/share"),
            |d| d.data_dir().to_path_buf(),
        );
        Self {
            profiles: config.join("crosshook/profiles"),
            data_home: data,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub name: String,
    pub title: String,
    pub app_id: String,
}

pub fn store(paths: &Paths) -> ProfileStore {
    ProfileStore::with_base_path(paths.profiles.clone())
}

/// Blocking: list and load every profile summary, sorted by title.
pub fn load_summaries(paths: &Paths) -> Result<Vec<Summary>, String> {
    let store = store(paths);
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
    Ok(out)
}

/// Blocking: read one profile's gamescope config.
pub fn load_gamescope(paths: &Paths, name: &str) -> Result<GamescopeConfig, String> {
    store(paths)
        .load(name)
        .map(|p| p.launch.gamescope)
        .map_err(|e| e.to_string())
}

/// Blocking: read-modify-write one profile's gamescope config.
pub fn save_gamescope(paths: &Paths, name: &str, config: GamescopeConfig) -> Result<(), String> {
    let store = store(paths);
    let mut profile = store.load(name).map_err(|e| e.to_string())?;
    profile.launch.gamescope = config;
    store.save(name, &profile).map_err(|e| e.to_string())
}
