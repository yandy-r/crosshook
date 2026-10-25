use super::*;

#[test]
fn disables_dmabuf_only_when_unset() {
    assert!(decide(false, None, false).disable_dmabuf);
    assert!(!decide(false, None, true).disable_dmabuf);
}

#[test]
fn clears_only_stale_x11_override_in_wayland_session() {
    for (wayland, gdk, expected) in [
        (true, Some("x11"), true),
        (false, Some("x11"), false),
        (true, Some("wayland"), false),
        (true, Some("x11,wayland"), false),
        (true, Some(""), false),
        (true, None, false),
    ] {
        assert_eq!(decide(wayland, gdk, true).remove_gdk_backend, expected);
    }
}

mod apply {
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};

    use crosshook_app::bootstrap::{pre_runtime_with_host, PreRuntimeHost, PreRuntimeOptions};

    use super::super::WebKitEnvPolicy;

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
    }

    impl PreRuntimeHost for FakeHost {
        fn thread_count(&self) -> Option<u32> {
            Some(1)
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

    fn run(host: &mut FakeHost) {
        pre_runtime_with_host(
            PreRuntimeOptions::new().with_env_policy(WebKitEnvPolicy),
            host,
        )
        .expect("native run with WebKit policy");
    }

    #[test]
    fn wayland_with_stale_x11_sets_dmabuf_flag_and_removes_gdk_backend() {
        let mut host = FakeHost::with(&[("WAYLAND_DISPLAY", "wayland-0"), ("GDK_BACKEND", "x11")]);
        run(&mut host);
        assert_eq!(
            host.env.get(OsStr::new("WEBKIT_DISABLE_DMABUF_RENDERER")),
            Some(&OsString::from("1"))
        );
        assert!(!host.env.contains_key(OsStr::new("GDK_BACKEND")));
        assert_eq!(
            host.calls,
            vec!["set:WEBKIT_DISABLE_DMABUF_RENDERER", "remove:GDK_BACKEND"]
        );
    }

    #[test]
    fn preexisting_webkit_flag_and_non_wayland_session_leave_env_untouched() {
        let mut host = FakeHost::with(&[
            ("WEBKIT_DISABLE_DMABUF_RENDERER", "0"),
            ("GDK_BACKEND", "x11"),
        ]);
        run(&mut host);
        assert!(host.calls.is_empty());
        assert_eq!(
            host.env.get(OsStr::new("WEBKIT_DISABLE_DMABUF_RENDERER")),
            Some(&OsString::from("0"))
        );
        assert_eq!(
            host.env.get(OsStr::new("GDK_BACKEND")),
            Some(&OsString::from("x11"))
        );
    }
}
