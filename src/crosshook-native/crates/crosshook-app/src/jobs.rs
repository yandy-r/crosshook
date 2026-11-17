//! Generic supervisor for long-running work.
//!
//! [`JobRegistry::start`] runs a job closure with a [`JobContext`] and returns
//! a [`JobHandle`] already subscribed to the job's bounded event stream
//! ([`JobEvent`]: logs, progress, then exactly one `Finished`). Jobs are
//! addressed by unguessable [`JobId`]s, may hold an exclusive [`JobKey`], and
//! are cancelled cooperatively through a token derived from the registry (and,
//! for [`App::jobs`](crate::App::jobs), the app shutdown token).
//! [`supervise_child`] ties a process group to that token;
//! [`supervise_child_with_kill`] adds an immediate-kill token.

mod process;
mod registry;
mod types;

pub(crate) use process::require;
pub(crate) use process::{
    create_log_file_in, kill_processes_using_prefix, processes_using_prefix,
    supervise_child_with_kill, sweep_prefix_until_clear, ChildExit, LogDir, PrefixSweep,
};
pub use process::{create_log_path_in, slugify_target, supervise_child, DEFAULT_KILL_GRACE};
#[cfg(test)]
pub(crate) use process::{test_log_dir, unusable_log_dir};
#[cfg(test)]
pub(crate) use registry::JobRegistryConfig;
pub use registry::{JobContext, JobEvents, JobHandle, JobRegistry};
pub use types::{JobEvent, JobEventKind, JobId, JobKey, JobProgress, JobSpec, Outcome};

#[cfg(test)]
mod tests;
