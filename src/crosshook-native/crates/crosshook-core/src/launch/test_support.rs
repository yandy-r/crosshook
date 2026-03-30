use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn path_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) struct ScopedCommandSearchPath {
    previous: Option<std::path::PathBuf>,
    _guard: MutexGuard<'static, ()>,
}

impl ScopedCommandSearchPath {
    pub(crate) fn new(value: &Path) -> Self {
        let guard = path_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous =
            crate::launch::optimizations::swap_test_command_search_path(Some(value.to_path_buf()));

        Self {
            previous,
            _guard: guard,
        }
    }
}

impl Drop for ScopedCommandSearchPath {
    fn drop(&mut self) {
        crate::launch::optimizations::swap_test_command_search_path(self.previous.take());
    }
}
