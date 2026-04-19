use directories::BaseDirs;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::AppSettingsData;

#[derive(Debug, Clone)]
pub struct SettingsStore {
    pub base_path: PathBuf,
    io_lock: Arc<Mutex<()>>,
}

#[derive(Debug)]
pub enum SettingsStoreError {
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl fmt::Display for SettingsStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::TomlDe(error) => write!(f, "{error}"),
            Self::TomlSer(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SettingsStoreError {}

impl From<std::io::Error> for SettingsStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for SettingsStoreError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDe(value)
    }
}

impl From<toml::ser::Error> for SettingsStoreError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlSer(value)
    }
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsStore {
    pub fn try_new() -> Result<Self, String> {
        let base_path = BaseDirs::new()
            .ok_or("home directory not found — CrossHook requires a user home directory")?
            .config_dir()
            .join("crosshook");
        Ok(Self {
            base_path,
            io_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn new() -> Self {
        Self::try_new().expect("home directory is required for CrossHook settings storage")
    }

    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self {
            base_path,
            io_lock: Arc::new(Mutex::new(())),
        }
    }

    fn load_unlocked(&self) -> Result<AppSettingsData, SettingsStoreError> {
        let path = self.settings_path();
        if !path.exists() {
            return Ok(AppSettingsData::default());
        }

        let content = fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(Into::into)
    }

    fn save_unlocked(&self, settings: &AppSettingsData) -> Result<(), SettingsStoreError> {
        fs::write(self.settings_path(), toml::to_string_pretty(settings)?)?;
        Ok(())
    }

    pub fn load(&self) -> Result<AppSettingsData, SettingsStoreError> {
        let _guard = self.io_lock.lock().expect("settings mutex poisoned");
        fs::create_dir_all(&self.base_path)?;
        self.load_unlocked()
    }

    pub fn save(&self, settings: &AppSettingsData) -> Result<(), SettingsStoreError> {
        let _guard = self.io_lock.lock().expect("settings mutex poisoned");
        fs::create_dir_all(&self.base_path)?;
        self.save_unlocked(settings)
    }

    /// Explicitly writes normalized settings to disk for callers that opt-in
    /// to backfilling newly added fields. Returns true if the file changed.
    pub fn migrate_or_save_settings(
        &self,
        settings: &AppSettingsData,
    ) -> Result<bool, SettingsStoreError> {
        let _guard = self.io_lock.lock().expect("settings mutex poisoned");
        fs::create_dir_all(&self.base_path)?;

        let path = self.settings_path();
        let serialized = toml::to_string_pretty(settings)?;
        let should_write = match fs::read_to_string(&path) {
            Ok(content) => content != serialized,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => return Err(SettingsStoreError::Io(error)),
        };

        if should_write {
            fs::write(path, serialized)?;
        }

        Ok(should_write)
    }

    /// Atomically load-mutate-save settings under a single process-local lock.
    /// The file is only written if `mutator` returns `Ok(_)`.
    pub fn update<F, T, E>(&self, mutator: F) -> Result<Result<T, E>, SettingsStoreError>
    where
        F: FnOnce(&mut AppSettingsData) -> Result<T, E>,
    {
        let _guard = self.io_lock.lock().expect("settings mutex poisoned");
        fs::create_dir_all(&self.base_path)?;

        let mut settings = self.load_unlocked()?;
        let result = mutator(&mut settings);
        if result.is_ok() {
            self.save_unlocked(&settings)?;
        }
        Ok(result)
    }

    pub fn settings_path(&self) -> PathBuf {
        self.base_path.join("settings.toml")
    }
}
