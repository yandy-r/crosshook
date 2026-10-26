//! Generic supervisor for long-running work.
//!
//! [`JobRegistry::start`] runs a job closure with a [`JobContext`] and returns
//! a [`JobHandle`] already subscribed to the job's bounded event stream
//! ([`JobEvent`]: logs, progress, then exactly one `Finished`). Jobs are
//! addressed by unguessable [`JobId`]s, may hold an exclusive [`JobKey`], and
//! are cancelled cooperatively through a token derived from the registry (and,
//! for [`App::jobs`](crate::App::jobs), the app shutdown token).
//! [`supervise_child`] ties a process group to that token.

mod process;
mod registry;
mod types;

pub(crate) use process::LOG_DIR;
pub use process::{
    create_log_path, create_log_path_in, kill_processes_using_prefix, slugify_target,
    supervise_child, ChildExit, DEFAULT_KILL_GRACE,
};
pub use registry::{
    JobContext, JobEvents, JobHandle, JobRegistry, JobRegistryConfig, DEFAULT_JOB_RETENTION,
};
pub use types::{JobEvent, JobEventKind, JobId, JobKey, JobProgress, JobSpec, Outcome};

#[cfg(test)]
mod tests;
