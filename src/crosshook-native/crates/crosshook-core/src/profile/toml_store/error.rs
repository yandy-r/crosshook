use std::fmt;

#[derive(Debug)]
pub enum ProfileStoreError {
    InvalidName(String),
    NotFound(std::path::PathBuf),
    AlreadyExists(String),
    InvalidLaunchOptimizationId(String),
    LaunchPresetNotFound(String),
    ReservedLaunchPresetName(String),
    InvalidLaunchPresetName(String),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl fmt::Display for ProfileStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(f, "invalid profile name: {name}"),
            Self::NotFound(path) => write!(f, "profile file not found: {}", path.display()),
            Self::AlreadyExists(name) => {
                write!(f, "a profile named '{name}' already exists")
            }
            Self::InvalidLaunchOptimizationId(id) => {
                write!(f, "unknown launch optimization id: {id}")
            }
            Self::LaunchPresetNotFound(name) => {
                write!(f, "launch optimization preset not found: {name}")
            }
            Self::ReservedLaunchPresetName(name) => {
                write!(
                    f,
                    "preset name is reserved for bundled presets (must not start with 'bundled/'): {name}"
                )
            }
            Self::InvalidLaunchPresetName(msg) => write!(f, "{msg}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::TomlDe(error) => write!(f, "{error}"),
            Self::TomlSer(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProfileStoreError {}

impl From<std::io::Error> for ProfileStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ProfileStoreError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDe(value)
    }
}

impl From<toml::ser::Error> for ProfileStoreError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlSer(value)
    }
}
