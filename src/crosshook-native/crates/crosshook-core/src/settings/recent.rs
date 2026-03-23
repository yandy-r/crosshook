use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_DIR: &str = "crosshook";
const RECENT_FILE_NAME: &str = "recent.toml";
const MAX_RECENT_FILES: usize = 10;

#[derive(Debug, Clone)]
pub struct RecentFilesStore {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentFilesData {
    pub game_paths: Vec<String>,
    pub trainer_paths: Vec<String>,
    pub dll_paths: Vec<String>,
}

#[derive(Debug)]
pub enum RecentFilesStoreError {
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl fmt::Display for RecentFilesStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::TomlDe(error) => write!(f, "{error}"),
            Self::TomlSer(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RecentFilesStoreError {}

impl From<std::io::Error> for RecentFilesStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for RecentFilesStoreError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDe(value)
    }
}

impl From<toml::ser::Error> for RecentFilesStoreError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlSer(value)
    }
}

impl Default for RecentFilesStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RecentFilesStore {
    pub fn new() -> Self {
        let path = BaseDirs::new()
            .expect("home directory is required for CrossHook recent files storage")
            .data_local_dir()
            .join(SETTINGS_DIR)
            .join(RECENT_FILE_NAME);

        Self { path }
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<RecentFilesData, RecentFilesStoreError> {
        if !self.path.exists() {
            return Ok(RecentFilesData::default());
        }

        let content = fs::read_to_string(&self.path)?;
        let mut recent_files: RecentFilesData = toml::from_str(&content)?;
        normalize_existing_paths(&mut recent_files.game_paths);
        normalize_existing_paths(&mut recent_files.trainer_paths);
        normalize_existing_paths(&mut recent_files.dll_paths);
        Ok(recent_files)
    }

    pub fn save(&self, recent_files: &RecentFilesData) -> Result<(), RecentFilesStoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut recent_files = recent_files.clone();
        cap_recent_paths(&mut recent_files.game_paths);
        cap_recent_paths(&mut recent_files.trainer_paths);
        cap_recent_paths(&mut recent_files.dll_paths);

        fs::write(&self.path, toml::to_string_pretty(&recent_files)?)?;
        Ok(())
    }
}

fn normalize_existing_paths(paths: &mut Vec<String>) {
    paths.retain(|path| Path::new(path).exists());
    cap_recent_paths(paths);
}

fn cap_recent_paths(paths: &mut Vec<String>) {
    if paths.len() > MAX_RECENT_FILES {
        paths.truncate(MAX_RECENT_FILES);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_file(path: &Path) {
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn save_and_load_round_trip_preserves_lists() {
        let temp_dir = tempdir().unwrap();
        let store = RecentFilesStore::with_path(temp_dir.path().join("recent.toml"));

        let game_a = temp_dir.path().join("game-a.exe");
        let trainer_a = temp_dir.path().join("trainer-a.exe");
        let dll_a = temp_dir.path().join("dll-a.dll");
        create_file(&game_a);
        create_file(&trainer_a);
        create_file(&dll_a);

        let recent_files = RecentFilesData {
            game_paths: vec![game_a.display().to_string()],
            trainer_paths: vec![trainer_a.display().to_string()],
            dll_paths: vec![dll_a.display().to_string()],
        };

        store.save(&recent_files).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded, recent_files);
    }

    #[test]
    fn load_filters_missing_paths_and_caps_each_list() {
        let temp_dir = tempdir().unwrap();
        let store = RecentFilesStore::with_path(temp_dir.path().join("recent.toml"));

        let game_paths: Vec<String> = std::iter::once(
            temp_dir
                .path()
                .join("missing-game.exe")
                .display()
                .to_string(),
        )
        .chain((0..12).map(|index| {
            let path = temp_dir.path().join(format!("game-{index}.exe"));
            create_file(&path);
            path.display().to_string()
        }))
        .collect();

        let trainer_paths: Vec<String> = (0..11)
            .map(|index| {
                let path = temp_dir.path().join(format!("trainer-{index}.exe"));
                create_file(&path);
                path.display().to_string()
            })
            .chain(std::iter::once(
                temp_dir
                    .path()
                    .join("missing-trainer.exe")
                    .display()
                    .to_string(),
            ))
            .collect();

        let dll_paths: Vec<String> = std::iter::once(
            temp_dir
                .path()
                .join("missing-dll.dll")
                .display()
                .to_string(),
        )
        .chain((0..8).map(|index| {
            let path = temp_dir.path().join(format!("dll-{index}.dll"));
            create_file(&path);
            path.display().to_string()
        }))
        .collect();

        let recent_files = RecentFilesData {
            game_paths,
            trainer_paths,
            dll_paths,
        };

        fs::write(&store.path, toml::to_string_pretty(&recent_files).unwrap()).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded.game_paths.len(), MAX_RECENT_FILES);
        assert_eq!(loaded.trainer_paths.len(), MAX_RECENT_FILES);
        assert_eq!(loaded.dll_paths.len(), 8);
        assert!(!loaded
            .game_paths
            .iter()
            .any(|path| path.contains("missing-game.exe")));
        assert!(!loaded
            .trainer_paths
            .iter()
            .any(|path| path.contains("missing-trainer.exe")));
        assert!(!loaded
            .dll_paths
            .iter()
            .any(|path| path.contains("missing-dll.dll")));
        assert!(loaded
            .game_paths
            .iter()
            .all(|path| Path::new(path).exists()));
        assert!(loaded
            .trainer_paths
            .iter()
            .all(|path| Path::new(path).exists()));
        assert!(loaded.dll_paths.iter().all(|path| Path::new(path).exists()));
    }

    #[test]
    fn load_returns_empty_when_file_is_missing() {
        let temp_dir = tempdir().unwrap();
        let store = RecentFilesStore::with_path(temp_dir.path().join("recent.toml"));

        let loaded = store.load().unwrap();
        assert_eq!(loaded, RecentFilesData::default());
    }
}
