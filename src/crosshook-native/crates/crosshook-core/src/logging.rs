use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use directories::BaseDirs;
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

pub const DEFAULT_LOG_DIRECTORY_NAME: &str = "crosshook/logs";
pub const DEFAULT_LOG_FILE_NAME: &str = "crosshook.log";
pub const DEFAULT_LOG_ROTATED_FILES: usize = 3;
pub const DEFAULT_LOG_ROTATION_BYTES: u64 = 1_048_576;

pub type LoggingResult<T> = std::result::Result<T, LoggingError>;

#[derive(Debug)]
pub enum LoggingError {
    HomeDirectoryUnavailable,
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    SubscriberAlreadyInitialized(String),
}

impl Display for LoggingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::HomeDirectoryUnavailable => {
                write!(f, "unable to resolve the user data directory for logging")
            }
            Self::Io {
                action,
                path,
                source,
            } => {
                write!(f, "failed to {action} '{}': {source}", path.display())
            }
            Self::SubscriberAlreadyInitialized(error) => {
                write!(f, "failed to initialize tracing subscriber: {error}")
            }
        }
    }
}

impl Error for LoggingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::HomeDirectoryUnavailable | Self::SubscriberAlreadyInitialized(_) => None,
        }
    }
}

pub fn log_file_path() -> LoggingResult<PathBuf> {
    resolve_log_file_path(None)
}

pub fn init_logging(mirror_stdout: bool) -> LoggingResult<PathBuf> {
    let log_file_path = resolve_log_file_path(None)?;
    let subscriber = build_subscriber(&log_file_path, mirror_stdout)?;

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|error| LoggingError::SubscriberAlreadyInitialized(error.to_string()))?;

    tracing::info!(
        log_file_path = %log_file_path.display(),
        mirror_stdout,
        "structured logging initialized"
    );

    Ok(log_file_path)
}

fn build_subscriber(
    log_file_path: &Path,
    mirror_stdout: bool,
) -> LoggingResult<impl tracing::Subscriber + Send + Sync> {
    let writer = RotatingLogWriter::open(
        log_file_path,
        mirror_stdout,
        DEFAULT_LOG_ROTATION_BYTES,
        DEFAULT_LOG_ROTATED_FILES,
    )?;

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("the fallback logging filter must be valid");

    Ok(tracing_subscriber::registry().with(env_filter).with(
        tracing_subscriber::fmt::layer()
            .compact()
            .with_ansi(false)
            .with_timer(UtcTime::rfc_3339())
            .with_writer(writer),
    ))
}

fn resolve_log_file_path(base_dir: Option<&Path>) -> LoggingResult<PathBuf> {
    let base_directory = match base_dir {
        Some(path) => path.to_path_buf(),
        None => {
            let base_dirs = BaseDirs::new().ok_or(LoggingError::HomeDirectoryUnavailable)?;
            base_dirs.data_local_dir().to_path_buf()
        }
    };

    Ok(base_directory
        .join(DEFAULT_LOG_DIRECTORY_NAME)
        .join(DEFAULT_LOG_FILE_NAME))
}

#[derive(Clone)]
struct RotatingLogWriter {
    state: Arc<Mutex<RotatingLogState>>,
}

impl RotatingLogWriter {
    fn open(
        log_file_path: &Path,
        mirror_stdout: bool,
        max_bytes: u64,
        retained_rotations: usize,
    ) -> LoggingResult<Self> {
        if let Some(parent) = log_file_path.parent() {
            fs::create_dir_all(parent).map_err(|source| LoggingError::Io {
                action: "create the log directory",
                path: parent.to_path_buf(),
                source,
            })?;
        }

        if max_bytes > 0 && log_file_path.exists() {
            let metadata = fs::metadata(log_file_path).map_err(|source| LoggingError::Io {
                action: "inspect the existing log file",
                path: log_file_path.to_path_buf(),
                source,
            })?;

            if metadata.len() >= max_bytes {
                rotate_log_files(log_file_path, retained_rotations)?;
            }
        }

        let file = open_log_file(log_file_path).map_err(|source| LoggingError::Io {
            action: "open the log file",
            path: log_file_path.to_path_buf(),
            source,
        })?;
        let current_size = file
            .metadata()
            .map_err(|source| LoggingError::Io {
                action: "inspect the log file",
                path: log_file_path.to_path_buf(),
                source,
            })?
            .len();

        Ok(Self {
            state: Arc::new(Mutex::new(RotatingLogState {
                file,
                current_size,
                log_file_path: log_file_path.to_path_buf(),
                mirror_stdout,
                max_bytes,
                retained_rotations,
            })),
        })
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for RotatingLogWriter {
    type Writer = RotatingLogHandle;

    fn make_writer(&'a self) -> Self::Writer {
        RotatingLogHandle {
            state: Arc::clone(&self.state),
        }
    }
}

struct RotatingLogHandle {
    state: Arc<Mutex<RotatingLogState>>,
}

impl Write for RotatingLogHandle {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("log writer mutex was poisoned"))?;

        state.write(buffer)?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("log writer mutex was poisoned"))?;

        state.flush()
    }
}

struct RotatingLogState {
    file: File,
    current_size: u64,
    log_file_path: PathBuf,
    mirror_stdout: bool,
    max_bytes: u64,
    retained_rotations: usize,
}

impl RotatingLogState {
    fn write(&mut self, buffer: &[u8]) -> io::Result<()> {
        if self.max_bytes > 0 && self.current_size > 0 {
            let projected_size = self.current_size.saturating_add(buffer.len() as u64);
            if projected_size > self.max_bytes {
                self.rotate()?;
            }
        }

        self.file.write_all(buffer)?;
        self.current_size = self.current_size.saturating_add(buffer.len() as u64);

        if self.mirror_stdout {
            let mut stdout = io::stdout().lock();
            stdout.write_all(buffer)?;
            stdout.flush()?;
        }

        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()?;

        if self.mirror_stdout {
            io::stdout().lock().flush()?;
        }

        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        self.file.flush()?;
        rotate_log_files(&self.log_file_path, self.retained_rotations).map_err(io::Error::other)?;
        self.file = open_log_file(&self.log_file_path).map_err(|source| {
            io::Error::other(LoggingError::Io {
                action: "reopen the log file",
                path: self.log_file_path.clone(),
                source,
            })
        })?;
        self.current_size = 0;
        Ok(())
    }
}

fn rotate_log_files(log_file_path: &Path, retained_rotations: usize) -> LoggingResult<()> {
    if retained_rotations == 0 {
        return Ok(());
    }

    let oldest = rotated_log_file_path(log_file_path, retained_rotations);
    if oldest.exists() {
        fs::remove_file(&oldest).map_err(|source| LoggingError::Io {
            action: "remove the oldest rotated log file",
            path: oldest,
            source,
        })?;
    }

    for index in (1..retained_rotations).rev() {
        let source = rotated_log_file_path(log_file_path, index);
        if source.exists() {
            let destination = rotated_log_file_path(log_file_path, index + 1);
            fs::rename(&source, &destination).map_err(|error| LoggingError::Io {
                action: "rotate the log file",
                path: source,
                source: error,
            })?;
        }
    }

    if log_file_path.exists() {
        let rotated_path = rotated_log_file_path(log_file_path, 1);
        fs::rename(log_file_path, &rotated_path).map_err(|source| LoggingError::Io {
            action: "rotate the active log file",
            path: log_file_path.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}

fn rotated_log_file_path(log_file_path: &Path, rotation_index: usize) -> PathBuf {
    let mut path = log_file_path.to_path_buf();
    let file_name = format!(
        "{}.{rotation_index}",
        log_file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(DEFAULT_LOG_FILE_NAME)
    );
    path.set_file_name(file_name);
    path
}

fn open_log_file(log_file_path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;
    use tracing::Level;

    #[test]
    fn resolves_log_file_path_under_local_share_crosshook() {
        let tempdir = tempdir().expect("temp dir");
        let log_path = resolve_log_file_path(Some(tempdir.path())).expect("log path");

        assert_eq!(
            log_path,
            tempdir
                .path()
                .join(DEFAULT_LOG_DIRECTORY_NAME)
                .join(DEFAULT_LOG_FILE_NAME)
        );
    }

    #[test]
    fn write_to_log_file_uses_rotation_and_stdout_configuration() {
        let tempdir = tempdir().expect("temp dir");
        let log_path = resolve_log_file_path(Some(tempdir.path())).expect("log path");
        let subscriber = build_subscriber_for_test(&log_path, false, 64, 2).expect("subscriber");

        tracing::subscriber::with_default(subscriber, || {
            tracing::event!(Level::INFO, "first message");
            tracing::event!(Level::INFO, "second message");
        });

        let content = fs::read_to_string(&log_path).expect("log file contents");
        assert!(content.contains("first message") || content.contains("second message"));
    }

    #[test]
    fn rotates_existing_logs_before_writing_new_entries() {
        let tempdir = tempdir().expect("temp dir");
        let log_path = resolve_log_file_path(Some(tempdir.path())).expect("log path");

        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent).expect("log directory");
        }

        fs::write(&log_path, "x".repeat(256)).expect("seed log file");

        let subscriber = build_subscriber_for_test(&log_path, false, 64, 2).expect("subscriber");
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("rotated entry");
        });

        assert!(rotated_log_file_path(&log_path, 1).exists());
        let current_log = fs::read_to_string(&log_path).expect("current log");
        assert!(current_log.contains("rotated entry"));
    }

    fn build_subscriber_for_test(
        log_path: &Path,
        mirror_stdout: bool,
        max_bytes: u64,
        retained_rotations: usize,
    ) -> LoggingResult<impl tracing::Subscriber + Send + Sync> {
        let writer =
            RotatingLogWriter::open(log_path, mirror_stdout, max_bytes, retained_rotations)?;
        let filter = EnvFilter::new("trace");

        Ok(tracing_subscriber::registry().with(filter).with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_ansi(false)
                .with_timer(UtcTime::rfc_3339())
                .with_writer(writer),
        ))
    }
}
