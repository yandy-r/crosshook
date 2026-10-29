//! Child-process supervision and process-related helpers moved from the Tauri
//! shell: log-file creation, target slugs, and the Wine-prefix `/proc` scan.

mod logs;
mod supervise;
mod sweep;

pub(crate) use logs::LOG_DIR;
pub use logs::{create_log_file_in, create_log_path, create_log_path_in, slugify_target};
pub use supervise::{supervise_child, supervise_child_with_kill, ChildExit, DEFAULT_KILL_GRACE};
pub use sweep::{kill_processes_using_prefix, processes_using_prefix};

#[cfg(test)]
mod tests;
