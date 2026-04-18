//! Tests for `list_launchers` and `find_orphaned_launchers`.

use tempfile::tempdir;

use crate::export::launcher::combine_host_unix_path;

use super::super::queries::{find_orphaned_launchers, list_launchers};
use super::create_file_with_content;

#[test]
fn list_with_no_launchers() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();

    let result = list_launchers(&home, "");
    assert!(result.is_empty(), "should return empty vec");
}

#[test]
fn list_with_launchers() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();

    // Create two launcher scripts
    let script_a = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        "alpha-game-trainer.sh",
    );
    let desktop_a = combine_host_unix_path(
        &home,
        ".local/share/applications",
        "crosshook-alpha-game-trainer.desktop",
    );
    let script_b = combine_host_unix_path(
        &home,
        ".local/share/crosshook/launchers",
        "beta-game-trainer.sh",
    );

    create_file_with_content(&script_a, "#!/usr/bin/env bash\n# script a");
    create_file_with_content(
        &desktop_a,
        "[Desktop Entry]\nName=Alpha Game - Trainer\nExec=/bin/bash alpha.sh\n",
    );
    create_file_with_content(&script_b, "#!/usr/bin/env bash\n# script b");

    let result = list_launchers(&home, "");

    assert_eq!(result.len(), 2, "should find two launchers");

    // Results should be sorted by slug
    assert_eq!(result[0].launcher_slug, "alpha-game");
    assert_eq!(result[0].display_name, "Alpha Game");
    assert!(result[0].script_exists);
    assert!(result[0].desktop_entry_exists);

    assert_eq!(result[1].launcher_slug, "beta-game");
    // No .desktop file, so display_name falls back to slug
    assert_eq!(result[1].display_name, "beta-game");
    assert!(result[1].script_exists);
    assert!(!result[1].desktop_entry_exists);
}

#[test]
fn find_orphaned_launchers_returns_only_unknown_slugs() {
    let temp = tempdir().expect("temp dir");
    let home = temp.path().to_string_lossy().into_owned();

    create_file_with_content(
        &combine_host_unix_path(
            &home,
            ".local/share/crosshook/launchers",
            "known-game-trainer.sh",
        ),
        "#!/usr/bin/env bash\n# script known",
    );
    create_file_with_content(
        &combine_host_unix_path(
            &home,
            ".local/share/crosshook/launchers",
            "orphan-game-trainer.sh",
        ),
        "#!/usr/bin/env bash\n# script orphan",
    );

    let result = find_orphaned_launchers(&["known-game".to_string()], &home, "");

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].launcher_slug, "orphan-game");
}
