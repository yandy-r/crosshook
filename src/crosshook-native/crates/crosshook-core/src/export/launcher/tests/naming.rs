use super::super::content::build_desktop_entry_content;
use super::super::names::resolve_display_name;
use super::super::paths::{
    escape_desktop_exec_argument, normalize_host_unix_path, shell_single_quoted,
};
use super::super::{resolve_target_home_path, sanitize_launcher_slug};

static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct ScopedHome {
    original: Option<String>,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl ScopedHome {
    fn unset() -> Self {
        let guard = HOME_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let original = std::env::var("HOME").ok();
        // SAFETY: tests serialize HOME mutation through HOME_LOCK.
        unsafe { std::env::remove_var("HOME") };
        Self {
            original,
            _guard: guard,
        }
    }
}

impl Drop for ScopedHome {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => {
                // SAFETY: tests serialize HOME mutation through HOME_LOCK.
                unsafe { std::env::set_var("HOME", value) };
            }
            None => {
                // SAFETY: tests serialize HOME mutation through HOME_LOCK.
                unsafe { std::env::remove_var("HOME") };
            }
        }
    }
}

#[test]
fn slug_generation_collapses_non_alphanumeric_runs() {
    assert_eq!(
        sanitize_launcher_slug("  CrossHook: Trainer 2026!!  "),
        "crosshook-trainer-2026"
    );
    assert_eq!(sanitize_launcher_slug(""), "crosshook-trainer");
    assert_eq!(sanitize_launcher_slug("---"), "crosshook-trainer");
}

#[test]
fn shell_single_quote_escaping_matches_posix_pattern() {
    assert_eq!(shell_single_quoted("abc"), "'abc'");
    assert_eq!(shell_single_quoted("a'b"), "'a'\"'\"'b'");
}

#[test]
fn desktop_exec_escaping_follows_freedesktop_spec() {
    assert_eq!(
        escape_desktop_exec_argument("/tmp/Cross Hook/runner\".sh"),
        "/tmp/Cross\\ Hook/runner\\\".sh"
    );
}

#[test]
fn desktop_exec_escaping_doubles_percent_signs() {
    assert_eq!(
        escape_desktop_exec_argument("/opt/trainers/%u/trainer.exe"),
        "/opt/trainers/%%u/trainer.exe"
    );
}

#[test]
fn normalize_host_unix_path_rewrites_flatpak_host_mount_paths() {
    assert_eq!(
        normalize_host_unix_path("/run/host/home/alice/.local/share/icons/cover.png"),
        "/home/alice/.local/share/icons/cover.png"
    );
}

#[test]
fn normalize_host_unix_path_leaves_regular_host_paths_unchanged() {
    let path = "/home/alice/.local/share/icons/cover.png";
    assert_eq!(normalize_host_unix_path(path), path);
}

#[test]
fn resolve_display_name_strips_trainer_suffix_from_trainer_stem() {
    assert_eq!(
        resolve_display_name("", "1245620", "/opt/trainers/Game Name - Trainer.exe"),
        "Game Name"
    );
}

#[test]
fn desktop_icon_falls_back_to_applications_games() {
    let content = build_desktop_entry_content("Test", "test", "/tmp/launcher.sh", "");
    assert!(content.contains("Icon=applications-games"));
}

#[test]
fn desktop_entry_contains_crosshook_metadata_lines() {
    let content = build_desktop_entry_content(
        "Elden Ring Deluxe",
        "elden-ring-deluxe",
        "/tmp/launcher.sh",
        "",
    );
    assert!(content.contains("X-CrossHook-Profile=Elden Ring Deluxe\n"));
    assert!(content.contains("X-CrossHook-Slug=elden-ring-deluxe\n"));
}

#[test]
fn resolves_home_from_steam_client_suffix() {
    assert_eq!(
        resolve_target_home_path(
            "/tmp/wrong/compatdata/steam",
            "/home/user/.local/share/Steam"
        ),
        "/home/user"
    );
}

#[test]
fn resolves_home_from_flatpak_steam_client_suffix() {
    assert_eq!(
        resolve_target_home_path(
            "/tmp/wrong/compatdata/steam",
            "/home/user/.var/app/com.valvesoftware.Steam/data/Steam"
        ),
        "/home/user"
    );
}

#[test]
fn rejects_invalid_preferred_home_when_no_better_fallback_exists() {
    let _home = ScopedHome::unset();
    assert_eq!(resolve_target_home_path("relative/path", ""), "");
    assert_eq!(resolve_target_home_path("/tmp/compatdata/steam", ""), "");
}
