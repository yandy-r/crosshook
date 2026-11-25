use super::*;

#[test]
fn dmabuf_follows_the_user_value_then_nvidia_presence() {
    for (webkit_set, has_nvidia, expected) in [
        (false, true, DmabufDecision::DisabledForNvidia),
        (false, false, DmabufDecision::EnabledDefault),
        (true, true, DmabufDecision::UserOverride),
        (true, false, DmabufDecision::UserOverride),
    ] {
        // The session type never changes the DMA-BUF answer.
        for (wayland, gdk) in [
            (false, None),
            (true, None),
            (false, Some("x11")),
            (true, Some("x11")),
        ] {
            let actions = decide(wayland, gdk, webkit_set, has_nvidia);
            assert_eq!(
                actions.dmabuf, expected,
                "wayland={wayland} gdk={gdk:?} webkit_set={webkit_set} has_nvidia={has_nvidia}"
            );
            assert_eq!(
                actions.disable_dmabuf(),
                expected == DmabufDecision::DisabledForNvidia,
                "wayland={wayland} gdk={gdk:?} webkit_set={webkit_set} has_nvidia={has_nvidia}"
            );
        }
    }
}

#[test]
fn dmabuf_decision_labels_are_stable() {
    assert_eq!(DmabufDecision::UserOverride.to_string(), "user_override");
    assert_eq!(
        DmabufDecision::DisabledForNvidia.to_string(),
        "disabled_for_nvidia"
    );
    assert_eq!(
        DmabufDecision::EnabledDefault.to_string(),
        "enabled_default"
    );
}

#[test]
fn clears_only_stale_x11_override_in_wayland_session() {
    for has_nvidia in [false, true] {
        for (wayland, gdk, expected) in [
            (true, Some("x11"), true),
            (false, Some("x11"), false),
            (true, Some("wayland"), false),
            (true, Some("x11,wayland"), false),
            (true, Some(""), false),
            (true, None, false),
        ] {
            assert_eq!(
                decide(wayland, gdk, false, has_nvidia).remove_gdk_backend,
                expected,
                "wayland={wayland} gdk={gdk:?} has_nvidia={has_nvidia}"
            );
        }
    }
}

mod apply {
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};
    use std::sync::{Arc, OnceLock};
    use std::time::Duration;

    use crosshook_app::bootstrap::{pre_runtime_with_host, PreRuntimeHost, PreRuntimeOptions};
    use crosshook_app::instance_lock::InstanceLockAttempt;

    use super::super::{DmabufDecision, WebKitEnvPolicy};

    const DMABUF: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

    #[derive(Default)]
    struct FakeHost {
        env: HashMap<OsString, OsString>,
        calls: Vec<String>,
    }

    impl FakeHost {
        fn with(vars: &[(&str, &str)]) -> Self {
            Self {
                env: vars
                    .iter()
                    .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                    .collect(),
                calls: Vec::new(),
            }
        }

        fn dmabuf(&self) -> Option<&OsString> {
            self.env.get(OsStr::new(DMABUF))
        }
    }

    impl PreRuntimeHost for FakeHost {
        fn thread_count(&self) -> Option<u32> {
            Some(1)
        }
        fn acquire_instance_lock(&mut self, _wait: Option<Duration>) -> InstanceLockAttempt {
            InstanceLockAttempt::Unavailable("test host")
        }
        fn is_flatpak(&self) -> bool {
            false
        }
        fn host_xdg_opt_in(&self) -> bool {
            false
        }
        fn override_xdg(&mut self) {}
        fn run_flatpak_migration(
            &mut self,
        ) -> Result<crosshook_app::bootstrap::FlatpakMigrationReport, String> {
            unreachable!("native fake host never migrates")
        }
        fn migrate_legacy_app_id(&mut self) {}
        fn env_var(&self, key: &OsStr) -> Option<OsString> {
            self.env.get(key).cloned()
        }
        fn set_env_var(&mut self, key: &OsStr, value: &OsStr) {
            self.calls.push(format!("set:{}", key.to_string_lossy()));
            self.env.insert(key.to_owned(), value.to_owned());
        }
        fn remove_env_var(&mut self, key: &OsStr) {
            self.calls.push(format!("remove:{}", key.to_string_lossy()));
            self.env.remove(key);
        }
    }

    fn run_with(policy: WebKitEnvPolicy, host: &mut FakeHost) {
        pre_runtime_with_host(PreRuntimeOptions::new().with_env_policy(policy), host)
            .expect("native run with WebKit policy");
    }

    /// Runs Phase A as if `has_nvidia` had been detected and returns the
    /// decision the policy recorded.
    fn run(has_nvidia: bool, host: &mut FakeHost) -> DmabufDecision {
        let report = Arc::new(OnceLock::new());
        run_with(
            WebKitEnvPolicy::with_nvidia_and_slot(has_nvidia, Arc::clone(&report)),
            host,
        );
        let recorded = report
            .get()
            .copied()
            .expect("the policy records its decision");
        assert_eq!(recorded.nvidia_gpu, has_nvidia);
        recorded.decision
    }

    #[test]
    fn nvidia_without_a_value_disables_the_dmabuf_renderer() {
        let mut host = FakeHost::with(&[]);
        assert_eq!(run(true, &mut host), DmabufDecision::DisabledForNvidia);
        assert_eq!(host.dmabuf(), Some(&OsString::from("1")));
        assert_eq!(host.calls, vec!["set:WEBKIT_DISABLE_DMABUF_RENDERER"]);
    }

    #[test]
    fn non_nvidia_without_a_value_leaves_the_dmabuf_renderer_alone() {
        let mut host = FakeHost::with(&[]);
        assert_eq!(run(false, &mut host), DmabufDecision::EnabledDefault);
        assert_eq!(host.dmabuf(), None);
        assert!(host.calls.is_empty());
    }

    #[test]
    fn nvidia_with_an_explicit_opt_out_keeps_the_user_value() {
        let mut host = FakeHost::with(&[(DMABUF, "0")]);
        assert_eq!(run(true, &mut host), DmabufDecision::UserOverride);
        assert_eq!(host.dmabuf(), Some(&OsString::from("0")));
        assert!(host.calls.is_empty());
    }

    #[test]
    fn non_nvidia_with_an_explicit_opt_in_keeps_the_user_value() {
        let mut host = FakeHost::with(&[(DMABUF, "1")]);
        assert_eq!(run(false, &mut host), DmabufDecision::UserOverride);
        assert_eq!(host.dmabuf(), Some(&OsString::from("1")));
        assert!(host.calls.is_empty());
    }

    #[test]
    fn empty_user_value_is_treated_as_unset() {
        let mut host = FakeHost::with(&[(DMABUF, "")]);
        assert_eq!(run(true, &mut host), DmabufDecision::DisabledForNvidia);
        assert_eq!(host.dmabuf(), Some(&OsString::from("1")));
        assert_eq!(host.calls, vec!["set:WEBKIT_DISABLE_DMABUF_RENDERER"]);
    }

    #[test]
    fn empty_user_value_on_non_nvidia_is_removed_for_default_renderer() {
        let mut host = FakeHost::with(&[(DMABUF, "")]);
        assert_eq!(run(false, &mut host), DmabufDecision::EnabledDefault);
        assert_eq!(host.dmabuf(), None);
        assert_eq!(host.calls, vec!["remove:WEBKIT_DISABLE_DMABUF_RENDERER"]);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_user_value_is_still_an_override() {
        use std::os::unix::ffi::OsStringExt;

        let value = OsString::from_vec(vec![0xff, b'1']);
        let mut host = FakeHost::with(&[]);
        host.env.insert(OsString::from(DMABUF), value.clone());
        assert_eq!(run(true, &mut host), DmabufDecision::UserOverride);
        assert!(host.calls.is_empty());
        assert_eq!(host.dmabuf(), Some(&value));
    }

    #[test]
    fn wayland_with_stale_x11_sets_dmabuf_flag_and_removes_gdk_backend() {
        let mut host = FakeHost::with(&[("WAYLAND_DISPLAY", "wayland-0"), ("GDK_BACKEND", "x11")]);
        assert_eq!(run(true, &mut host), DmabufDecision::DisabledForNvidia);
        assert_eq!(host.dmabuf(), Some(&OsString::from("1")));
        assert!(!host.env.contains_key(OsStr::new("GDK_BACKEND")));
        assert_eq!(
            host.calls,
            vec!["set:WEBKIT_DISABLE_DMABUF_RENDERER", "remove:GDK_BACKEND"]
        );
    }

    #[test]
    fn wayland_with_stale_x11_on_non_nvidia_only_removes_gdk_backend() {
        let mut host = FakeHost::with(&[("WAYLAND_DISPLAY", "wayland-0"), ("GDK_BACKEND", "x11")]);
        run_with(WebKitEnvPolicy::with_nvidia(false), &mut host);
        assert_eq!(host.dmabuf(), None);
        assert!(!host.env.contains_key(OsStr::new("GDK_BACKEND")));
        assert_eq!(host.calls, vec!["remove:GDK_BACKEND"]);
    }

    #[test]
    fn preexisting_webkit_flag_and_non_wayland_session_leave_env_untouched() {
        let mut host = FakeHost::with(&[(DMABUF, "0"), ("GDK_BACKEND", "x11")]);
        assert_eq!(run(true, &mut host), DmabufDecision::UserOverride);
        assert!(host.calls.is_empty());
        assert_eq!(host.dmabuf(), Some(&OsString::from("0")));
        assert_eq!(
            host.env.get(OsStr::new("GDK_BACKEND")),
            Some(&OsString::from("x11"))
        );
    }
}
