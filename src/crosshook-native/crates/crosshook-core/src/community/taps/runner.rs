use std::collections::BTreeMap;
use std::ffi::OsString;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

/// One git process to run. Core owns argv and env; the runner only executes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInvocation {
    /// Human-readable step name, e.g. `"clone community tap"`.
    pub action: &'static str,
    /// Arguments after the program name (`git`).
    pub args: Vec<OsString>,
    /// Environment pairs the process must run with.
    pub env: BTreeMap<String, String>,
    /// Working directory (`None` for clone).
    pub directory: Option<PathBuf>,
}

/// Captured result of a finished git process. A non-zero exit is `success == false`, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Why a git process produced no [`GitOutput`].
#[derive(Debug)]
pub enum GitRunError {
    /// The run was cancelled; the driver aborts the sync and never falls back to the cache.
    Cancelled,
    /// Spawn, wait or I/O failure.
    Failed(std::io::Error),
}

/// Executes git invocations on behalf of the tap-sync driver.
pub trait TapGitRunner: Send + Sync {
    fn run<'a>(
        &'a self,
        invocation: GitInvocation,
    ) -> Pin<Box<dyn Future<Output = Result<GitOutput, GitRunError>> + Send + 'a>>;

    /// Polled by the driver before every step and before indexing.
    fn is_cancelled(&self) -> bool {
        false
    }
}
