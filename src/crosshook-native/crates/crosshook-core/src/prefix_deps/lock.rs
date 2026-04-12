use super::PrefixDepsError;
use std::sync::Arc;
use std::sync::Mutex;

/// Global lock preventing concurrent prefix dependency installs.
///
/// Only one install can run at a time per CrossHook instance.
/// The lock holds the active prefix path for UI status queries.
pub struct PrefixDepsInstallLock {
    active: Arc<Mutex<Option<String>>>,
}

impl Default for PrefixDepsInstallLock {
    fn default() -> Self {
        Self::new()
    }
}

impl PrefixDepsInstallLock {
    pub fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(None)),
        }
    }

    /// Try to acquire the install lock for the given prefix path.
    ///
    /// Returns a guard that releases the lock on drop.
    /// Returns `PrefixDepsError::AlreadyInstalling` if another install is in progress.
    pub async fn try_acquire(
        &self,
        prefix_path: String,
    ) -> Result<PrefixDepsLockGuard, PrefixDepsError> {
        let mut guard = self
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(ref existing) = *guard {
            return Err(PrefixDepsError::AlreadyInstalling {
                prefix_path: existing.clone(),
            });
        }
        *guard = Some(prefix_path);
        Ok(PrefixDepsLockGuard {
            active: Arc::clone(&self.active),
        })
    }

    /// Check if any install is currently active.
    pub async fn is_locked(&self) -> bool {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    /// Return the prefix path being installed, if any.
    pub async fn active_prefix(&self) -> Option<String> {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Guard that releases the install lock when dropped.
#[derive(Debug)]
pub struct PrefixDepsLockGuard {
    active: Arc<Mutex<Option<String>>>,
}

impl Drop for PrefixDepsLockGuard {
    fn drop(&mut self) {
        let mut guard = self
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lock_acquire_succeeds_when_free() {
        let lock = PrefixDepsInstallLock::new();
        let guard = lock.try_acquire("/tmp/pfx".to_string()).await;
        assert!(guard.is_ok());
        assert!(lock.is_locked().await);
    }

    #[tokio::test]
    async fn lock_rejects_concurrent_install() {
        let lock = PrefixDepsInstallLock::new();
        let _guard = lock.try_acquire("/tmp/pfx-a".to_string()).await.unwrap();
        let result = lock.try_acquire("/tmp/pfx-b".to_string()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            PrefixDepsError::AlreadyInstalling { prefix_path } => {
                assert_eq!(prefix_path, "/tmp/pfx-a");
            }
            other => panic!("expected AlreadyInstalling, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn lock_releases_on_guard_drop() {
        let lock = PrefixDepsInstallLock::new();
        {
            let _guard = lock.try_acquire("/tmp/pfx".to_string()).await.unwrap();
            assert!(lock.is_locked().await);
        }
        // Guard dropped
        assert!(!lock.is_locked().await);
        // Can acquire again
        let guard2 = lock.try_acquire("/tmp/pfx-2".to_string()).await;
        assert!(guard2.is_ok());
    }

    #[tokio::test]
    async fn active_prefix_returns_correct_path() {
        let lock = PrefixDepsInstallLock::new();
        assert!(lock.active_prefix().await.is_none());
        let _guard = lock.try_acquire("/tmp/pfx-a".to_string()).await.unwrap();
        assert_eq!(lock.active_prefix().await, Some("/tmp/pfx-a".to_string()));
    }
}
