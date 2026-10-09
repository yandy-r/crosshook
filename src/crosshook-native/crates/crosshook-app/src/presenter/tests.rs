//! Tests for home-path redaction (deterministic; no environment mutation).

use std::path::Path;

use super::{
    sanitize_display_path, sanitize_display_path_with_home, sanitize_message_with_home,
    sanitize_with_home, scrub_generic_home,
};

#[test]
fn redacts_home_at_component_boundaries() {
    assert_eq!(sanitize_with_home("/home/user/x", "/home/user"), "$HOME/x");
    assert_eq!(
        sanitize_with_home("failed reading /home/user/x: nope", "/home/user"),
        "failed reading $HOME/x: nope"
    );
    assert_eq!(
        sanitize_with_home("path=\"/home/user/x\"", "/home/user"),
        "path=\"$HOME/x\""
    );
}

#[test]
fn preserves_neighbour_home_suffix() {
    // Longer directory starting with the home string must stay intact.
    assert_eq!(
        sanitize_with_home("/home/user2/x", "/home/user"),
        "/home/user2/x"
    );
    assert_eq!(
        sanitize_with_home("/home/user-name", "/home/user"),
        "/home/user-name"
    );
    // `/home/user` is a suffix of a *different* path component chain.
    assert_eq!(
        sanitize_with_home("word/home/user", "/home/user"),
        "word/home/user"
    );
}

#[test]
fn home_trailing_separator_and_no_op() {
    assert_eq!(
        sanitize_with_home("cfg=/home/user/", "/home/user/"),
        "cfg=$HOME/"
    );
    assert_eq!(sanitize_with_home("plain text", "/home/user"), "plain text");
    // Empty or trivial home values are ignored rather than corrupting output.
    assert_eq!(sanitize_with_home("/home/user/x", ""), "/home/user/x");
    assert_eq!(sanitize_with_home("/home/user/x", "/"), "/home/user/x");
}

#[test]
fn generic_fallback_scrubs_home_paths() {
    assert_eq!(scrub_generic_home("/home/other/cache"), "$HOME/cache");
    assert_eq!(
        scrub_generic_home("log at /home/a and /home/b"),
        "log at $HOME and $HOME"
    );
    // `/home/` alone (no user component) is left untouched.
    assert_eq!(scrub_generic_home("see /home/ docs"), "see /home/ docs");
    assert_eq!(scrub_generic_home("nothing here"), "nothing here");
}

#[test]
fn sanitize_message_redacts_current_home() {
    // Uses the real environment read-only; any current home with at least one
    // separator is redacted when embedded in a message.
    if let Ok(home) = std::env::var("HOME") {
        if home.len() > 1 && !home.ends_with('/') {
            let message = format!("failed to read {home}/library");
            let sanitized = super::sanitize_message(&message);
            assert!(sanitized.contains("$HOME"), "unredacted: {sanitized}");
        }
    }
}

#[test]
fn display_path_exact_and_embedded_home() {
    let home = Some(Path::new("/home/user"));
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/home/user"), home),
        "$HOME"
    );
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/home/user/.local/share/x"), home),
        "$HOME/.local/share/x"
    );
    // Trailing separator on `home` still matches at the component boundary.
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/home/user/x"), Some(Path::new("/home/user/"))),
        "$HOME/x"
    );
}

#[test]
fn display_path_keeps_neighbour_home() {
    let home = Some(Path::new("/home/user"));
    for neighbour in ["/home/user2/x", "/home/user-name", "/home/user.bak/x"] {
        assert_eq!(
            sanitize_display_path_with_home(Path::new(neighbour), home),
            neighbour
        );
    }
}

#[test]
fn display_path_unicode_boundary() {
    let home = Some(Path::new("/home/user"));
    // A non-ASCII letter extends the component: neighbour, not home.
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/home/userñ/x"), home),
        "/home/userñ/x"
    );
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/home/user/ñ"), home),
        "$HOME/ñ"
    );
    // Unicode home is redacted whole and its neighbours are preserved.
    let uhome = Some(Path::new("/home/josé"));
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/home/josé/x"), uhome),
        "$HOME/x"
    );
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/home/josé2/x"), uhome),
        "/home/josé2/x"
    );
    // Multibyte char before the match must not panic on slicing.
    assert_eq!(
        sanitize_display_path_with_home(Path::new("é/home/user"), home),
        "é/home/user"
    );
}

#[test]
fn display_path_falls_back_without_home() {
    for home in [None, Some(Path::new(""))] {
        assert_eq!(
            sanitize_display_path_with_home(Path::new("/home/other/cache"), home),
            "$HOME/cache"
        );
    }
}

#[test]
fn display_path_env_wrapper_redacts_current_home() {
    if let Some(home) = std::env::var_os("HOME") {
        let home = Path::new(&home);
        if home.as_os_str().len() > 1 && !home.to_string_lossy().ends_with('/') {
            let sanitized = sanitize_display_path(&home.join("library"));
            assert!(sanitized.starts_with("$HOME"), "unredacted: {sanitized}");
        }
    }
}

#[test]
fn display_path_known_var_home_alias() {
    // Canonical Fedora alias of a `/home/<user>` home is redacted lexically.
    let home = Some(Path::new("/home/user"));
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/var/home/user/x"), home),
        "$HOME/x"
    );
    // Neighbor semantics hold for the alias too.
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/var/home/user2/x"), home),
        "/var/home/user2/x"
    );
    // Non-/home homes get no alias.
    assert_eq!(
        sanitize_display_path_with_home(Path::new("/var/home/root/x"), Some(Path::new("/root"))),
        "/var/home/root/x"
    );
}

#[test]
fn message_redacts_known_home_with_trailing_period() {
    assert_eq!(
        sanitize_message_with_home("cannot open /home/user.", "/home/user"),
        "cannot open $HOME."
    );
    assert_eq!(
        sanitize_message_with_home("cannot open /var/home/user/x", "/home/user"),
        "cannot open $HOME/x"
    );
}

#[test]
fn message_scrubs_other_users_with_home_known() {
    assert_eq!(
        sanitize_message_with_home(
            "a /home/user/x b /home/other/y c /var/home/third.",
            "/home/user"
        ),
        "a $HOME/x b $HOME/y c $HOME."
    );
    // Neighbor of the known home is another user's home: scrubbed in messages.
    assert_eq!(
        sanitize_message_with_home("/home/user2/x", "/home/user"),
        "$HOME/x"
    );
}

#[test]
fn message_invalid_home_falls_back_to_generic() {
    for home in ["", "/"] {
        assert_eq!(
            sanitize_message_with_home("see /home/other/x and /var/home/o2.", home),
            "see $HOME/x and $HOME."
        );
    }
    // Unicode and mid-word cases do not panic or over-redact.
    assert_eq!(
        sanitize_message_with_home("é /home/user ñ /var/home/josé/x", "/"),
        "é $HOME ñ $HOME/x"
    );
    assert_eq!(
        sanitize_message_with_home("é/home/user", "/"),
        "é/home/user"
    );
}

#[test]
fn generic_scrub_handles_var_home() {
    assert_eq!(scrub_generic_home("/var/home/user/x"), "$HOME/x");
    assert_eq!(scrub_generic_home("at /var/home/user."), "at $HOME.");
    // `/var` not at a boundary is a different word.
    assert_eq!(scrub_generic_home("x/var/home/u"), "x/var/home/u");
}
