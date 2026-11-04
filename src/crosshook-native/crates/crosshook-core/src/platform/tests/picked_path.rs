use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use super::super::picked_path::{resolve_picked_host_path, PrefetchedXattr, XattrReader};
use super::super::portals::documents::DocumentsPortal;
use super::super::{
    resolve_picked_host_path_default, resolve_picked_sandbox_path, HostPathError, PickedPathUse,
};

const DOC_FILE: &str = "/run/user/1000/doc/abc123/game.exe";
const DOC_NESTED: &str = "/run/user/1000/doc/abc123/Games/sub/game.exe";
const FLATPAK_DOC_FILE: &str = "/run/flatpak/doc/abc123/game.exe";
const FLATPAK_DOC_NESTED: &str = "/run/flatpak/doc/abc123/Games/sub/game.exe";

/// Xattr fake returning a fixed value and recording every path it is asked for.
struct FakeXattr {
    value: Option<Vec<u8>>,
    calls: Mutex<Vec<String>>,
}

impl FakeXattr {
    fn returning(value: Option<&[u8]>) -> Self {
        Self {
            value: value.map(<[u8]>::to_vec),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl XattrReader for FakeXattr {
    fn document_portal_host_path(&self, path: &str) -> Option<Vec<u8>> {
        self.calls.lock().unwrap().push(path.to_string());
        self.value.clone()
    }
}

/// Portal fake returning a fixed reply and recording every id list it gets.
struct FakePortal {
    reply: Result<HashMap<String, Vec<u8>>, String>,
    calls: Mutex<Vec<Vec<String>>>,
}

impl FakePortal {
    fn replying(entries: &[(&str, &[u8])]) -> Self {
        let map = entries
            .iter()
            .map(|(id, path)| ((*id).to_string(), path.to_vec()))
            .collect();
        Self {
            reply: Ok(map),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn failing() -> Self {
        Self {
            reply: Err("org.freedesktop.DBus.Error.ServiceUnknown".to_string()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl DocumentsPortal for FakePortal {
    async fn get_host_paths(&self, doc_ids: &[String]) -> Result<HashMap<String, Vec<u8>>, String> {
        self.calls.lock().unwrap().push(doc_ids.to_vec());
        self.reply.clone()
    }
}

/// Resolves `path` with fakes that must never be consulted (non-portal input).
async fn resolve_without_portal(path: &str) -> Result<PathBuf, HostPathError> {
    let xattr = FakeXattr::returning(Some(b"/must/not/be/used"));
    let portal = FakePortal::replying(&[("abc123", b"/must/not/be/used")]);
    let result = resolve_picked_host_path(path, &xattr, &portal).await;
    assert!(xattr.calls().is_empty(), "xattr read for `{path}`");
    assert!(portal.calls().is_empty(), "portal called for `{path}`");
    result
}

/// Resolves a portal path whose xattr is missing, so the portal fake answers.
async fn resolve_via_portal(path: &str, portal: &FakePortal) -> Result<PathBuf, HostPathError> {
    resolve_picked_host_path(path, &FakeXattr::returning(None), portal).await
}

fn unresolved(file_name: &str) -> HostPathError {
    HostPathError::UnresolvedDocumentPortalPath {
        file_name: file_name.to_string(),
    }
}

fn unavailable(file_name: &str) -> HostPathError {
    HostPathError::DocumentPortalUnavailable {
        file_name: file_name.to_string(),
    }
}

fn invalid_reply(file_name: &str) -> HostPathError {
    HostPathError::InvalidPortalReply {
        file_name: file_name.to_string(),
    }
}

#[tokio::test]
async fn empty_and_whitespace_input_is_empty_error() {
    assert_eq!(resolve_without_portal("").await, Err(HostPathError::Empty));
    assert_eq!(
        resolve_without_portal(" \t\n ").await,
        Err(HostPathError::Empty)
    );
}

#[tokio::test]
async fn run_host_paths_are_stripped_and_cleaned() {
    for (input, expected) in [
        ("/run/host", "/"),
        ("  /run/host/  ", "/"),
        (
            "/run/host/usr/share/steam/compatibilitytools.d/GE/proton",
            "/usr/share/steam/compatibilitytools.d/GE/proton",
        ),
        ("/run/host//a//b", "/a/b"),
        ("/run/hostile/game.exe", "/run/hostile/game.exe"),
    ] {
        assert_eq!(
            resolve_without_portal(input).await,
            Ok(PathBuf::from(expected)),
            "for `{input}`"
        );
    }
}

#[tokio::test]
async fn plain_absolute_path_is_lexically_cleaned_but_keeps_parent_components() {
    assert_eq!(
        resolve_without_portal("/home//user/./Games/game.exe").await,
        Ok(PathBuf::from("/home/user/Games/game.exe"))
    );
    assert_eq!(
        resolve_without_portal("/home/user/../other/game.exe").await,
        Ok(PathBuf::from("/home/user/../other/game.exe"))
    );
}

/// A backslash is an ordinary file-name byte in a POSIX path, so a `/` path
/// containing one is still cleaned and the backslash is kept verbatim.
#[tokio::test]
async fn absolute_path_with_backslash_is_cleaned_like_any_posix_path() {
    assert_eq!(
        resolve_without_portal("/home/user//Wine\\Games/./game.exe").await,
        Ok(PathBuf::from("/home/user/Wine\\Games/game.exe"))
    );
}

#[tokio::test]
async fn lookalike_portal_mounts_are_plain_paths() {
    for path in [
        "/run/user/me/doc/abc123/game.exe",
        "/run/user/1000/docs/abc123/game.exe",
        "/run/flatpak/docs/abc123/game.exe",
        "/run/flatpak/document/abc123/game.exe",
    ] {
        assert_eq!(
            resolve_without_portal(path).await,
            Ok(PathBuf::from(path)),
            "for `{path}`"
        );
    }
}

#[tokio::test]
async fn windows_style_and_relative_paths_pass_through_trimmed() {
    for (input, expected) in [
        ("  C:\\Games\\game.exe ", "C:\\Games\\game.exe"),
        ("Z:/games//game.exe", "Z:/games//game.exe"),
        (" games/./game.exe ", "games/./game.exe"),
    ] {
        assert_eq!(
            resolve_without_portal(input).await,
            Ok(PathBuf::from(expected))
        );
    }
}

#[tokio::test]
async fn document_portal_path_resolves_via_xattr_without_dbus() {
    for path in [DOC_NESTED, FLATPAK_DOC_NESTED] {
        let xattr = FakeXattr::returning(Some(b"/home/user/Games//game.exe\0"));
        let portal = FakePortal::replying(&[("abc123", b"/wrong/path")]);

        let resolved = resolve_picked_host_path(&format!(" {path} "), &xattr, &portal).await;

        assert_eq!(resolved, Ok(PathBuf::from("/home/user/Games/game.exe")));
        assert_eq!(xattr.calls(), vec![path.to_string()]);
        assert!(portal.calls().is_empty(), "portal called for `{path}`");
    }
}

#[tokio::test]
async fn xattr_value_is_not_trimmed() {
    let xattr = FakeXattr::returning(Some(b"/home/user/Games/game.exe \n"));
    let portal = FakePortal::replying(&[("abc123", b"/wrong/path")]);

    let resolved = resolve_picked_host_path(DOC_FILE, &xattr, &portal).await;

    assert_eq!(resolved, Ok(PathBuf::from("/home/user/Games/game.exe \n")));
    assert!(portal.calls().is_empty());
}

#[tokio::test]
async fn invalid_xattr_values_fall_back_to_dbus() {
    let invalid: [&[u8]; 4] = [
        b"home/user/game.exe",
        b"/home/user\0/game.exe",
        b"/home/user/\xffgame.exe",
        b"\0",
    ];
    for value in invalid {
        let xattr = FakeXattr::returning(Some(value));
        let portal = FakePortal::replying(&[("abc123", b"/home/user/game.exe")]);

        let resolved = resolve_picked_host_path(DOC_FILE, &xattr, &portal).await;

        assert_eq!(
            resolved,
            Ok(PathBuf::from("/home/user/game.exe")),
            "for {value:?}"
        );
        assert_eq!(portal.calls(), vec![vec!["abc123".to_string()]]);
    }
}

#[tokio::test]
async fn document_portal_file_resolves_via_dbus_fallback() {
    for path in [DOC_FILE, FLATPAK_DOC_FILE] {
        let xattr = FakeXattr::returning(None);
        let portal = FakePortal::replying(&[("abc123", b"/home/user/Games/game.exe\0")]);

        let resolved = resolve_picked_host_path(path, &xattr, &portal).await;

        assert_eq!(resolved, Ok(PathBuf::from("/home/user/Games/game.exe")));
        assert_eq!(xattr.calls(), vec![path.to_string()]);
        assert_eq!(portal.calls(), vec![vec!["abc123".to_string()]]);
    }
}

#[tokio::test]
async fn document_portal_directory_export_appends_remainder() {
    for path in [
        "/run/user/1000/doc/abc123/Games//sub/./game.exe",
        "/run/flatpak/doc/abc123/Games//sub/./game.exe",
    ] {
        let portal = FakePortal::replying(&[("abc123", b"/home/user/Games\0")]);

        let resolved = resolve_via_portal(path, &portal).await;

        assert_eq!(
            resolved,
            Ok(PathBuf::from("/home/user/Games/sub/game.exe")),
            "for `{path}`"
        );
    }
}

#[tokio::test]
async fn document_portal_unavailable_when_xattr_missing_and_dbus_fails() {
    let portal = FakePortal::failing();

    let resolved = resolve_via_portal(DOC_FILE, &portal).await;

    assert_eq!(resolved, Err(unavailable("game.exe")));
    assert_eq!(portal.calls().len(), 1);
}

#[tokio::test]
async fn document_portal_unresolved_when_reply_lacks_doc_id() {
    let portal = FakePortal::replying(&[("other-id", b"/home/user/game.exe")]);

    let resolved = resolve_via_portal(DOC_FILE, &portal).await;

    assert_eq!(resolved, Err(unresolved("game.exe")));
}

#[tokio::test]
async fn document_portal_rejects_invalid_replies() {
    let replies: [&[u8]; 3] = [
        b"home/user/game.exe\0",
        b"/home/user\0/game.exe\0",
        b"/home/user/\xffgame.exe",
    ];
    for reply in replies {
        let portal = FakePortal::replying(&[("abc123", reply)]);

        let resolved = resolve_via_portal(DOC_FILE, &portal).await;

        assert_eq!(resolved, Err(invalid_reply("game.exe")), "for {reply:?}");
    }
}

#[tokio::test]
async fn malformed_document_portal_paths_are_rejected_before_lookup() {
    for (path, file_name) in [
        (
            "/run/user/1000/doc/abc123/Games/../../../etc/passwd",
            "passwd",
        ),
        ("/run/flatpak/doc/abc123/Games/../../etc/passwd", "passwd"),
        ("/run/user/1000/doc", "doc"),
        ("/run/user/1000/doc/", "doc"),
        ("/run/user/1000/doc/abc123", "abc123"),
        ("/run/user/1000/doc/abc123/", "abc123"),
        ("/run/user/1000/doc//game.exe", "game.exe"),
        ("/run/user/1000/doc/../game.exe", "game.exe"),
        ("/run/flatpak/doc", "doc"),
        ("/run/flatpak/doc/abc123", "abc123"),
    ] {
        let xattr = FakeXattr::returning(Some(b"/home/user/game.exe"));
        let portal = FakePortal::replying(&[("abc123", b"/home/user/game.exe")]);

        let resolved = resolve_picked_host_path(path, &xattr, &portal).await;

        assert_eq!(resolved, Err(unresolved(file_name)), "for `{path}`");
        assert!(xattr.calls().is_empty(), "xattr read for `{path}`");
        assert!(portal.calls().is_empty(), "portal called for `{path}`");
    }
}

/// The default resolver must decide non-portal and malformed portal input
/// lexically: neither reaches the xattr read nor the session bus, so the
/// results do not depend on the test machine.
#[tokio::test]
async fn default_resolver_classifies_before_any_io() {
    assert_eq!(
        resolve_picked_host_path_default(" /run/host/usr/bin/protontricks ").await,
        Ok(PathBuf::from("/usr/bin/protontricks"))
    );
    assert_eq!(
        resolve_picked_host_path_default("/home//user/game.exe").await,
        Ok(PathBuf::from("/home/user/game.exe"))
    );
    assert_eq!(
        resolve_picked_host_path_default("/run/flatpak/doc/abc123/a/../../x").await,
        Err(unresolved("x"))
    );
    assert_eq!(
        resolve_picked_host_path_default("  ").await,
        Err(HostPathError::Empty)
    );
}

#[test]
fn prefetched_xattr_answers_only_for_its_path() {
    let prefetched = PrefetchedXattr {
        path: DOC_FILE.to_string(),
        value: Some(b"/home/user/game.exe".to_vec()),
    };

    assert_eq!(
        prefetched.document_portal_host_path(DOC_FILE),
        Some(b"/home/user/game.exe".to_vec())
    );
    assert_eq!(prefetched.document_portal_host_path(DOC_NESTED), None);
}

#[test]
fn host_path_error_messages_name_the_file_and_the_remedy() {
    assert_eq!(
        unresolved("game.exe").to_string(),
        "CrossHook could not find the real location of \"game.exe\". Grant CrossHook access to \
         its folder (for example with Flatseal, or `flatpak override --user \
         --filesystem=<folder> dev.crosshook.CrossHook`) and pick it again."
    );
    assert_eq!(
        unavailable("game.exe").to_string(),
        "CrossHook could not find the real location of \"game.exe\" because the desktop \
         document portal did not answer (it needs xdg-desktop-portal 1.17 or newer). Update \
         xdg-desktop-portal, or grant CrossHook access to the file's folder (for example with \
         Flatseal) and pick it again."
    );
    assert_eq!(
        invalid_reply("game.exe").to_string(),
        "the desktop document portal returned an invalid location for \"game.exe\""
    );
    assert_eq!(
        HostPathError::Empty.to_string(),
        "the selected path is empty"
    );
}

#[tokio::test]
async fn host_path_errors_never_show_the_sandbox_path() {
    let failing = FakePortal::failing();
    let unknown = FakePortal::replying(&[]);
    let invalid = FakePortal::replying(&[("abc123", b"relative")]);
    for portal in [&failing, &unknown, &invalid] {
        let message = resolve_via_portal(DOC_NESTED, portal)
            .await
            .unwrap_err()
            .to_string();
        assert!(!message.contains("/run/"), "message leaks path: {message}");
        assert!(message.contains("\"game.exe\""), "message: {message}");
    }
}

#[test]
fn sandbox_resolution_keeps_document_portal_path() {
    assert_eq!(
        resolve_picked_sandbox_path(&format!("  {DOC_NESTED}\n")),
        DOC_NESTED
    );
    assert_eq!(
        resolve_picked_sandbox_path(&format!(" {FLATPAK_DOC_NESTED} ")),
        FLATPAK_DOC_NESTED
    );
}

#[test]
fn sandbox_resolution_strips_run_host() {
    assert_eq!(
        resolve_picked_sandbox_path(" /run/host/home/user/art.png "),
        "/home/user/art.png"
    );
    assert_eq!(resolve_picked_sandbox_path("/run/host"), "/");
    assert_eq!(
        resolve_picked_sandbox_path("/home/user/art.png"),
        "/home/user/art.png"
    );
}

#[test]
fn picked_path_use_uses_lowercase_wire_names() {
    assert_eq!(
        serde_json::from_str::<PickedPathUse>("\"host\"").unwrap(),
        PickedPathUse::Host
    );
    assert_eq!(
        serde_json::from_str::<PickedPathUse>("\"sandbox\"").unwrap(),
        PickedPathUse::Sandbox
    );
    assert_eq!(
        serde_json::to_string(&PickedPathUse::Sandbox).unwrap(),
        "\"sandbox\""
    );
    assert!(serde_json::from_str::<PickedPathUse>("\"Host\"").is_err());
}
