use crosshook_app::bootstrap::{EnvPolicy, PreRuntimeEnv};

/// Classic's renderer environment policy, applied before GTK or Tokio starts.
pub(crate) struct WebKitEnvPolicy;

#[derive(Debug, PartialEq, Eq)]
struct Actions {
    disable_dmabuf: bool,
    remove_gdk_backend: bool,
}

fn decide(wayland: bool, gdk: Option<&str>, webkit_set: bool) -> Actions {
    Actions {
        disable_dmabuf: !webkit_set,
        remove_gdk_backend: wayland && gdk == Some("x11"),
    }
}

impl EnvPolicy for WebKitEnvPolicy {
    fn apply(&self, env: &mut PreRuntimeEnv<'_>) {
        let gdk = env.var_os("GDK_BACKEND");
        let actions = decide(
            env.var_os("WAYLAND_DISPLAY").is_some(),
            gdk.as_deref().and_then(|value| value.to_str()),
            env.var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_some(),
        );
        // Prevent GBM EGL display creation failures on multi-GPU systems (e.g. eGPU
        // setups) by disabling WebKitGTK's DMA-BUF renderer before WebKit initializes.
        if actions.disable_dmabuf {
            env.set("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // With WebKitGTK 2.44+, native Wayland is more reliable and avoids EGL
        // rendering failures on Intel+NVIDIA hybrid GPU systems where XWayland
        // compositing can fail silently. Allow Wayland sessions to use the native
        // backend when a stale environment forced X11.
        if actions.remove_gdk_backend {
            env.remove("GDK_BACKEND");
        }
    }
}

#[cfg(test)]
mod tests;
