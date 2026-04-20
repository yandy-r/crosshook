//! Host-side prefix-root resolver: under Flatpak + isolation mode, wine prefixes
//! stay on the host filesystem even though sandbox XDG paths point into
//! ~/.var/app/.../. This module centralizes the decision + path derivation.

use std::path::PathBuf;

use crate::platform::{is_flatpak, EnvSink, SystemEnv};

const PREFIX_SEGMENT: &str = "crosshook/prefixes";
const ISOLATION_OPT_OUT_VAR: &str = "CROSSHOOK_FLATPAK_HOST_XDG";

/// Returns the host-side wine prefix root when running under Flatpak in
/// isolation mode.
///
/// Returns `Some(<HOME>/.local/share/crosshook/prefixes)` when:
/// - the process is running inside a Flatpak sandbox, **and**
/// - isolation mode is active (i.e. `CROSSHOOK_FLATPAK_HOST_XDG` is **not**
///   set to `"1"` or `"true"`).
///
/// Returns `None` in all other cases (not Flatpak, or isolation opted out).
pub fn host_prefix_root() -> Option<PathBuf> {
    let env = SystemEnv;
    host_prefix_root_with(&env, is_flatpak())
}

/// Returns `true` when isolation mode is active.
///
/// Isolation mode is active unless `CROSSHOOK_FLATPAK_HOST_XDG` is set to
/// `"1"` or `"true"` (case-insensitive). Any other value — including `"0"`,
/// `"false"`, or the variable being unset — leaves isolation active.
pub(crate) fn is_isolation_mode_active(env: &dyn EnvSink) -> bool {
    match env.get(ISOLATION_OPT_OUT_VAR) {
        Some(value) => {
            let s = value.to_string_lossy();
            let trimmed = s.trim();
            // Opt-in values ("1" / "true") disable isolation (return false here).
            // Anything else (including "0", "false", "", unset) → isolation active.
            !(trimmed == "1" || trimmed.eq_ignore_ascii_case("true"))
        }
        None => true,
    }
}

/// Test-injectable variant of [`host_prefix_root`].
///
/// Accepts a caller-supplied [`EnvSink`] and a pre-determined `is_flatpak`
/// boolean so unit tests never touch the real process environment or the
/// `/.flatpak-info` file.
pub(crate) fn host_prefix_root_with(env: &dyn EnvSink, is_flatpak: bool) -> Option<PathBuf> {
    if !is_flatpak {
        return None;
    }
    if !is_isolation_mode_active(env) {
        return None;
    }
    let home = env.get("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".local/share")
            .join(PREFIX_SEGMENT),
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;
    use crate::platform::tests::common::FakeEnv;

    fn fake_with_home(home: &str) -> FakeEnv {
        let mut env = FakeEnv::default();
        env.reads.insert("HOME".to_string(), OsString::from(home));
        env
    }

    #[test]
    fn flatpak_and_no_env_returns_host_path() {
        let env = fake_with_home("/h");
        let got = host_prefix_root_with(&env, true).unwrap();
        assert_eq!(got, PathBuf::from("/h/.local/share/crosshook/prefixes"));
    }

    #[test]
    fn env_var_opt_in_one_returns_none() {
        let mut env = fake_with_home("/h");
        env.reads.insert(
            "CROSSHOOK_FLATPAK_HOST_XDG".to_string(),
            OsString::from("1"),
        );
        assert!(host_prefix_root_with(&env, true).is_none());
    }

    #[test]
    fn env_var_true_returns_none() {
        let mut env = fake_with_home("/h");
        env.reads.insert(
            "CROSSHOOK_FLATPAK_HOST_XDG".to_string(),
            OsString::from("TRUE"),
        );
        assert!(host_prefix_root_with(&env, true).is_none());
    }

    #[test]
    fn env_var_true_lowercase_returns_none() {
        let mut env = fake_with_home("/h");
        env.reads.insert(
            "CROSSHOOK_FLATPAK_HOST_XDG".to_string(),
            OsString::from("true"),
        );
        assert!(host_prefix_root_with(&env, true).is_none());
    }

    #[test]
    fn env_var_zero_is_isolation_active() {
        let mut env = fake_with_home("/h");
        env.reads.insert(
            "CROSSHOOK_FLATPAK_HOST_XDG".to_string(),
            OsString::from("0"),
        );
        assert!(host_prefix_root_with(&env, true).is_some());
    }

    #[test]
    fn env_var_false_is_isolation_active() {
        let mut env = fake_with_home("/h");
        env.reads.insert(
            "CROSSHOOK_FLATPAK_HOST_XDG".to_string(),
            OsString::from("false"),
        );
        assert!(host_prefix_root_with(&env, true).is_some());
    }

    #[test]
    fn not_flatpak_returns_none() {
        let env = fake_with_home("/h");
        assert!(host_prefix_root_with(&env, false).is_none());
    }

    #[test]
    fn no_home_returns_none() {
        let env = FakeEnv::default();
        assert!(host_prefix_root_with(&env, true).is_none());
    }

    #[test]
    fn isolation_mode_active_when_var_unset() {
        let env = FakeEnv::default();
        assert!(is_isolation_mode_active(&env));
    }

    #[test]
    fn isolation_mode_inactive_when_var_is_one() {
        let mut env = FakeEnv::default();
        env.reads.insert(
            "CROSSHOOK_FLATPAK_HOST_XDG".to_string(),
            OsString::from("1"),
        );
        assert!(!is_isolation_mode_active(&env));
    }

    #[test]
    fn isolation_mode_inactive_when_var_is_true() {
        let mut env = FakeEnv::default();
        env.reads.insert(
            "CROSSHOOK_FLATPAK_HOST_XDG".to_string(),
            OsString::from("True"),
        );
        assert!(!is_isolation_mode_active(&env));
    }
}
