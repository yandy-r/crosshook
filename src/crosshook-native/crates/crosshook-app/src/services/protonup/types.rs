//! Public DTOs of the protonup service. The serde shapes are frozen: they are
//! the IPC contract of the Tauri commands.

use crosshook_core::protonup::progress::ProtonInstallProgress;
use tokio::sync::broadcast;

/// Opaque identity of the caller that started an install.
///
/// The Tauri adapter derives it from the invoking webview label; it is never a
/// frontend-supplied argument. Only the owner of an install can cancel it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstallOwner(String);

impl InstallOwner {
    /// Wraps an adapter-derived owner label (for example a webview label).
    pub fn new(label: impl Into<String>) -> Self {
        Self(label.into())
    }
}

/// Returned from `protonup_install_version_async` so the frontend can track
/// the operation and subscribe to `protonup:install:progress` events.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonInstallHandle {
    /// Job id of the install; also the `op_id` of its progress events.
    pub op_id: String,
}

/// Returned from `protonup_uninstall_version`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonUninstallResponse {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Steam app ids whose compat-tool mapping still uses this tool.
    pub conflicting_app_ids: Vec<String>,
    /// Fixed, path-free refusal or failure text.
    pub error_message: Option<String>,
}

/// Returned from `protonup_plan_uninstall_version` so the UI can surface
/// `conflicting_app_ids` before any deletion happens.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonUninstallPlanResponse {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Steam app ids whose compat-tool mapping still uses this tool.
    pub conflicting_app_ids: Vec<String>,
    /// Fixed, path-free refusal or failure text.
    pub error_message: Option<String>,
}

/// A started install: the job id (which is the `op_id`) and a receiver of its
/// progress snapshots. Drain it with [`next_progress`](super::next_progress).
pub struct ProtonInstallStart {
    /// Job id of the install (the `op_id`).
    pub op_id: String,
    /// Progress snapshots of the install.
    pub progress: broadcast::Receiver<ProtonInstallProgress>,
}

impl ProtonInstallStart {
    /// The IPC handle for this install.
    pub fn handle(&self) -> ProtonInstallHandle {
        ProtonInstallHandle {
            op_id: self.op_id.clone(),
        }
    }
}
