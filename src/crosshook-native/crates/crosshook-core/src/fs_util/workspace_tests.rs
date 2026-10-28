//! Containment matrix for [`ensure_within_workspace`] using real temp dirs.
//!
//! Fixture assumption: the temp filesystem is a plain case-sensitive Linux fs
//! (ext4/tmpfs). Casefold-enabled ext4 or normalization-insensitive mounts do
//! not share these name semantics.

use super::{ensure_within_workspace, WorkspacePathError};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn variant(error: &WorkspacePathError) -> &'static str {
    match error {
        WorkspacePathError::InvalidRoot => "InvalidRoot",
        WorkspacePathError::Traversal => "Traversal",
        WorkspacePathError::OutsideWorkspace => "OutsideWorkspace",
        WorkspacePathError::NotFound => "NotFound",
        WorkspacePathError::Io { .. } => "Io",
    }
}

fn write_json(path: &Path) {
    fs::write(path, br#"{"ok":true}"#).expect("write json fixture");
}

#[test]
fn accepts_nested_relative_candidate() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join("profiles/game")).unwrap();
    let target = root.join("profiles/game/community-profile.json");
    write_json(&target);

    let resolved =
        ensure_within_workspace(&root, Path::new("profiles/game/community-profile.json")).unwrap();
    assert_eq!(resolved, fs::canonicalize(&target).unwrap());
}

#[test]
fn accepts_nested_absolute_candidate() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join("profiles")).unwrap();
    let target = root.join("profiles/community-profile.json");
    write_json(&target);

    let resolved = ensure_within_workspace(&root, &target).unwrap();
    assert_eq!(resolved, fs::canonicalize(&target).unwrap());
}

#[test]
fn candidate_equal_to_root_returns_canonical_root() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(&root).unwrap();

    let resolved = ensure_within_workspace(&root, &root).unwrap();
    assert_eq!(resolved, fs::canonicalize(&root).unwrap());
}

#[test]
fn rejects_relative_parent_dir_that_ends_inside() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join("profiles")).unwrap();
    let target = root.join("profiles/game.json");
    write_json(&target);

    let error = ensure_within_workspace(&root, Path::new("other/../profiles/game.json"))
        .expect_err("relative .. must be refused");
    assert_eq!(variant(&error), "Traversal");
}

#[test]
fn rejects_absolute_parent_dir_that_ends_inside() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(root.join("profiles")).unwrap();
    let target = root.join("profiles/game.json");
    write_json(&target);

    let candidate = root.join("profiles/../profiles/game.json");
    let error =
        ensure_within_workspace(&root, &candidate).expect_err("absolute .. must be refused");
    assert_eq!(variant(&error), "Traversal");
}

#[test]
fn rejects_absolute_candidate_outside_root() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    let outside = temp.path().join("other");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let target = outside.join("game.json");
    write_json(&target);

    let error = ensure_within_workspace(&root, &target).expect_err("outside path");
    assert_eq!(variant(&error), "OutsideWorkspace");
}

#[test]
fn rejects_sibling_sharing_root_name_prefix() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    let evil = temp.path().join("root-evil");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&evil).unwrap();
    let target = evil.join("game.json");
    write_json(&target);

    let error = ensure_within_workspace(&root, &target).expect_err("sibling prefix escape");
    assert_eq!(variant(&error), "OutsideWorkspace");
}

#[test]
fn rejects_missing_root_and_file_root() {
    let temp = tempdir().unwrap();
    let missing = temp.path().join("missing");
    let error =
        ensure_within_workspace(&missing, Path::new("game.json")).expect_err("missing root");
    assert_eq!(variant(&error), "InvalidRoot");

    let file_root = temp.path().join("root.json");
    write_json(&file_root);
    let error = ensure_within_workspace(&file_root, Path::new("game.json")).expect_err("file root");
    assert_eq!(variant(&error), "InvalidRoot");
}

#[test]
fn rejects_missing_candidate() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(&root).unwrap();

    let relative = ensure_within_workspace(&root, Path::new("missing.json"))
        .expect_err("missing relative candidate");
    assert_eq!(variant(&relative), "NotFound");

    let absolute = ensure_within_workspace(&root, &root.join("nested/missing.json"))
        .expect_err("missing nested candidate");
    assert_eq!(variant(&absolute), "NotFound");
}

#[test]
fn rejects_empty_candidate() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("root");
    fs::create_dir_all(&root).unwrap();

    let error = ensure_within_workspace(&root, Path::new("")).expect_err("empty candidate");
    assert_eq!(variant(&error), "OutsideWorkspace");
}

#[cfg(unix)]
mod symlink_cases {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn accepts_final_symlink_resolving_inside() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("profiles")).unwrap();
        let real = root.join("profiles/real.json");
        write_json(&real);
        let link = root.join("link.json");
        symlink("profiles/real.json", &link).unwrap();

        let resolved = ensure_within_workspace(&root, &link).unwrap();
        assert_eq!(resolved, fs::canonicalize(&real).unwrap());
    }

    #[test]
    fn accepts_intermediate_symlink_resolving_inside() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("profiles")).unwrap();
        let real = root.join("profiles/game.json");
        write_json(&real);
        let link_dir = root.join("linked");
        symlink("profiles", &link_dir).unwrap();

        let resolved = ensure_within_workspace(&root, Path::new("linked/game.json")).unwrap();
        assert_eq!(resolved, fs::canonicalize(&real).unwrap());
    }

    #[test]
    fn rejects_final_symlink_resolving_outside() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let real = outside.join("game.json");
        write_json(&real);
        let link = root.join("link.json");
        symlink(&real, &link).unwrap();

        let error = ensure_within_workspace(&root, &link).expect_err("outward final link");
        assert_eq!(variant(&error), "OutsideWorkspace");
    }

    #[test]
    fn rejects_intermediate_symlink_resolving_outside_even_if_later_link_returns() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(root.join("profiles")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let real = root.join("profiles/game.json");
        write_json(&real);
        // root/hole -> outside, outside/back -> root: even though a second link
        // leads back in, the outward intermediate must fail at its own prefix.
        symlink(&outside, root.join("hole")).unwrap();
        symlink(&root, outside.join("back")).unwrap();

        let error = ensure_within_workspace(&root, Path::new("hole/back/profiles/game.json"))
            .expect_err("outward intermediate link");
        assert_eq!(variant(&error), "OutsideWorkspace");
    }

    #[test]
    fn rejects_dangling_final_symlink() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(&root).unwrap();
        let link = root.join("dangling.json");
        symlink("missing-target.json", &link).unwrap();

        let error = ensure_within_workspace(&root, &link).expect_err("dangling final link");
        assert_eq!(variant(&error), "NotFound");
    }

    #[test]
    fn rejects_dangling_intermediate_symlink() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(&root).unwrap();
        symlink("missing-dir", root.join("hole")).unwrap();

        let error = ensure_within_workspace(&root, Path::new("hole/game.json"))
            .expect_err("dangling intermediate link");
        assert_eq!(variant(&error), "NotFound");
    }

    #[test]
    fn rejects_looping_symlink_as_io_error() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(&root).unwrap();
        symlink("loop", root.join("loop")).unwrap();

        let error = ensure_within_workspace(&root, &root.join("loop")).expect_err("symlink loop");
        assert_eq!(variant(&error), "Io");
    }

    #[test]
    fn rejects_absolute_candidate_under_symlinked_sibling_alias_of_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("profiles")).unwrap();
        let real = root.join("profiles/game.json");
        write_json(&real);
        // An alias that is NOT the supplied or canonical root spelling must not
        // gain authority, even though it points at the same directory.
        let alias = temp.path().join("alias");
        symlink(&root, &alias).unwrap();

        let error = ensure_within_workspace(&root, &alias.join("profiles/game.json"))
            .expect_err("sibling alias must not gain authority");
        assert_eq!(variant(&error), "OutsideWorkspace");
    }

    #[test]
    fn accepts_absolute_candidate_spelled_through_trusted_root_symlink() {
        let temp = tempdir().unwrap();
        let actual = temp.path().join("actual");
        fs::create_dir_all(actual.join("profiles")).unwrap();
        let real = actual.join("profiles/game.json");
        write_json(&real);
        let root = temp.path().join("root");
        symlink(&actual, &root).unwrap();

        // The supplied root spelling keeps authority after canonicalization.
        let resolved = ensure_within_workspace(&root, &root.join("profiles/game.json")).unwrap();
        assert_eq!(resolved, fs::canonicalize(&real).unwrap());
    }

    #[test]
    fn rejects_intermediate_file_component() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir_all(&root).unwrap();
        write_json(&root.join("game.json"));

        let error = ensure_within_workspace(&root, Path::new("game.json/deeper.json"))
            .expect_err("intermediate file component");
        assert_eq!(variant(&error), "Io");
    }
}

mod name_spelling_cases {
    use super::*;

    #[test]
    fn rejects_case_sibling_escape() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("Taps");
        let impostor = temp.path().join("taps");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&impostor).unwrap();
        let outside = impostor.join("game.json");
        write_json(&outside);

        // Guard: fixture filesystem must treat the two spellings as distinct.
        assert!(
            !root.join("game.json").exists(),
            "fixture requires a case-sensitive filesystem"
        );
        let error = ensure_within_workspace(&root, &outside).expect_err("case sibling");
        assert_eq!(variant(&error), "OutsideWorkspace");
    }

    #[test]
    fn rejects_nfc_nfd_sibling_escape() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("caf\u{e9}"); // NFC spelling, the authority
        let impostor = temp.path().join("cafe\u{301}"); // NFD spelling
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&impostor).unwrap();
        let outside = impostor.join("game.json");
        write_json(&outside);

        // Guard: fixture filesystem must treat NFC/NFD spellings as distinct.
        assert!(
            !root.join("game.json").exists(),
            "fixture requires distinct NFC/NFD names"
        );
        let error = ensure_within_workspace(&root, &outside).expect_err("NFD sibling");
        assert_eq!(variant(&error), "OutsideWorkspace");
    }

    #[test]
    fn accepts_valid_unicode_path_inside_root() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("root");
        let nested = root.join("cafe\u{301}");
        fs::create_dir_all(&nested).unwrap();
        let target = nested.join("game.json");
        write_json(&target);

        let resolved = ensure_within_workspace(&root, Path::new("cafe\u{301}/game.json")).unwrap();
        assert_eq!(resolved, fs::canonicalize(&target).unwrap());
    }
}

#[test]
fn display_and_source_never_leak_paths() {
    let io_error = WorkspacePathError::Io {
        operation: "resolve workspace path",
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "detail"),
    };
    let fixtures = [
        WorkspacePathError::InvalidRoot,
        WorkspacePathError::Traversal,
        WorkspacePathError::OutsideWorkspace,
        WorkspacePathError::NotFound,
        io_error,
    ];
    for error in &fixtures {
        let text = error.to_string();
        for needle in ["/", "tmp", "game.json"] {
            assert!(
                !text.contains(needle),
                "display must not leak path text: {text:?} contains {needle:?}"
            );
        }
    }
    let io_error = &fixtures[4];
    assert!(
        std::error::Error::source(io_error).is_some(),
        "Io must expose its source"
    );
    assert!(std::error::Error::source(&fixtures[0]).is_none());
}
