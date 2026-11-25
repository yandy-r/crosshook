use std::fmt;
use std::sync::{Arc, OnceLock};

use crosshook_app::bootstrap::{EnvPolicy, PreRuntimeEnv};

/// WebKitGTK's DMA-BUF renderer switch; `1` turns the renderer off.
const WEBKIT_DISABLE_DMABUF_RENDERER: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// How the WebKitGTK DMA-BUF renderer was resolved for this process.
///
/// Deliberately carries no environment value: `WEBKIT_DISABLE_DMABUF_RENDERER`
/// is user-supplied, so the decision reports what was done, never what was set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DmabufDecision {
    /// `WEBKIT_DISABLE_DMABUF_RENDERER` was already set; the value is left as is.
    UserOverride,
    /// An NVIDIA GPU was detected and nothing was set, so the policy set `=1`.
    DisabledForNvidia,
    /// No NVIDIA GPU was detected and nothing was set; WebKitGTK's default stays on.
    EnabledDefault,
}

impl fmt::Display for DmabufDecision {
    /// Snake-case label, the same in the stderr and structured log lines.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UserOverride => "user_override",
            Self::DisabledForNvidia => "disabled_for_nvidia",
            Self::EnabledDefault => "enabled_default",
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Actions {
    dmabuf: DmabufDecision,
    remove_gdk_backend: bool,
}

impl Actions {
    /// Whether the policy writes `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
    fn disable_dmabuf(&self) -> bool {
        self.dmabuf == DmabufDecision::DisabledForNvidia
    }
}

/// Pure decision table: `webkit_set` means `WEBKIT_DISABLE_DMABUF_RENDERER`
/// already has a value, `has_nvidia` that an NVIDIA GPU was detected.
fn decide(wayland: bool, gdk: Option<&str>, webkit_set: bool, has_nvidia: bool) -> Actions {
    Actions {
        dmabuf: if webkit_set {
            DmabufDecision::UserOverride
        } else if has_nvidia {
            DmabufDecision::DisabledForNvidia
        } else {
            DmabufDecision::EnabledDefault
        },
        remove_gdk_backend: wayland && gdk == Some("x11"),
    }
}

/// Recorded outcome of [`WebKitEnvPolicy`] for structured startup logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DmabufReport {
    pub(crate) decision: DmabufDecision,
    pub(crate) nvidia_gpu: bool,
}

/// Classic's renderer environment policy, applied before GTK or Tokio starts.
pub(crate) struct WebKitEnvPolicy {
    has_nvidia: bool,
    /// The outcome of the one `apply` call, for `lib.rs` to log once tracing exists.
    report: Arc<OnceLock<DmabufReport>>,
}

impl WebKitEnvPolicy {
    /// Probes the DRM devices for an NVIDIA GPU.
    pub(crate) fn new(report: Arc<OnceLock<DmabufReport>>) -> Self {
        Self::with_nvidia_and_slot(crosshook_core::platform::has_nvidia_gpu(), report)
    }

    /// Same policy with the GPU probe replaced by `has_nvidia`.
    #[cfg(test)]
    pub(crate) fn with_nvidia(has_nvidia: bool) -> Self {
        Self::with_nvidia_and_slot(has_nvidia, Arc::default())
    }

    fn with_nvidia_and_slot(has_nvidia: bool, report: Arc<OnceLock<DmabufReport>>) -> Self {
        Self { has_nvidia, report }
    }
}

impl EnvPolicy for WebKitEnvPolicy {
    fn apply(&self, env: &mut PreRuntimeEnv<'_>) {
        let gdk = env.var_os("GDK_BACKEND");
        let webkit_raw = env.var_os(WEBKIT_DISABLE_DMABUF_RENDERER);
        let webkit_non_empty = webkit_raw.as_deref().is_some_and(|value| !value.is_empty());
        let actions = decide(
            env.var_os("WAYLAND_DISPLAY").is_some(),
            gdk.as_deref().and_then(|value| value.to_str()),
            webkit_non_empty,
            self.has_nvidia,
        );
        // WebKitGTK's DMA-BUF renderer draws nothing on NVIDIA (blank screen), so
        // it is disabled there before WebKit initializes. Non-NVIDIA systems keep
        // DMA-BUF enabled by default; users on affected multi-GPU/eGPU setups can
        // still set `WEBKIT_DISABLE_DMABUF_RENDERER=1` explicitly, and any
        // non-empty user value always wins. WebKitGTK 2.44+ disables DMA-BUF for
        // any present value other than "0" (including ""), so clear an inherited
        // empty value when selecting the default-enabled path.
        if actions.disable_dmabuf() {
            env.set(WEBKIT_DISABLE_DMABUF_RENDERER, "1");
        } else if actions.dmabuf == DmabufDecision::EnabledDefault && webkit_raw.is_some() {
            env.remove(WEBKIT_DISABLE_DMABUF_RENDERER);
        }
        // With WebKitGTK 2.44+, native Wayland is more reliable and avoids EGL
        // rendering failures on Intel+NVIDIA hybrid GPU systems where XWayland
        // compositing can fail silently. Allow Wayland sessions to use the native
        // backend when a stale environment forced X11.
        if actions.remove_gdk_backend {
            env.remove("GDK_BACKEND");
        }
        let _ = self.report.set(DmabufReport {
            decision: actions.dmabuf,
            nvidia_gpu: self.has_nvidia,
        });
        // Tracing has no subscriber yet (`App::bootstrap` installs one, and then
        // logs this decision again), so report it to stderr for this step.
        eprintln!(
            "CrossHook: WebKit DMA-BUF renderer policy: {} (nvidia_gpu={})",
            actions.dmabuf, self.has_nvidia
        );
    }
}

#[cfg(test)]
mod tests;
