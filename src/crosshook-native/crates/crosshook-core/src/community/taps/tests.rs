use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

use crate::community::{CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating};
use crate::profile::GameProfile;

use super::store::CommunityTapStore;
use super::types::{
    CommunityTapError, CommunityTapSubscription, CommunityTapSyncStatus, CommunityTapWorkspace,
};
use super::validation::{is_valid_git_sha, validate_branch_name, validate_tap_url};

fn init_repo(path: &Path) {
    let output = Command::new("git")
        .args(["init", "-b", "main"])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["config", "user.email", "crosshook@example.invalid"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["config", "user.name", "CrossHook"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

fn commit_file(path: &Path, relative: &str, content: &str, message: &str) {
    let full_path = path.join(relative);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&full_path, content).unwrap();

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["add", relative])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["commit", "-m", message])
        .env("GIT_AUTHOR_NAME", "CrossHook")
        .env("GIT_AUTHOR_EMAIL", "crosshook@example.invalid")
        .env("GIT_COMMITTER_NAME", "CrossHook")
        .env("GIT_COMMITTER_EMAIL", "crosshook@example.invalid")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn rev_parse_head(path: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn syncs_and_indexes_local_tap_repo() {
    let temp_dir = tempdir().unwrap();
    let source_repo = temp_dir.path().join("source");
    let store_root = temp_dir.path().join("store");
    fs::create_dir_all(&source_repo).unwrap();
    init_repo(&source_repo);

    let manifest = CommunityProfileManifest::new(
        CommunityProfileMetadata {
            game_name: "Elden Ring".to_string(),
            game_version: "1.0".to_string(),
            trainer_name: "Trainer".to_string(),
            trainer_version: "1".to_string(),
            proton_version: "9".to_string(),
            platform_tags: vec!["linux".to_string()],
            compatibility_rating: CompatibilityRating::Working,
            author: "CrossHook".to_string(),
            description: "Test profile".to_string(),
            trainer_sha256: None,
        },
        GameProfile::default(),
    );
    commit_file(
        &source_repo,
        "profiles/elden-ring/community-profile.json",
        &serde_json::to_string_pretty(&manifest).unwrap(),
        "add community profile",
    );

    let store = CommunityTapStore::with_base_path(store_root);
    let subscription = CommunityTapSubscription {
        url: source_repo.display().to_string(),
        branch: Some("main".to_string()),
        pinned_commit: None,
    };
    // Use sync_workspace directly to bypass URL scheme validation (test uses local path).
    let workspace = CommunityTapWorkspace {
        local_path: store.workspace_path(&subscription),
        subscription,
    };

    let result = store.sync_workspace(&workspace).unwrap();
    assert_eq!(result.status, CommunityTapSyncStatus::Cloned);
    assert_eq!(result.index.entries.len(), 1);
    assert_eq!(
        result.index.entries[0].manifest.metadata.game_name,
        "Elden Ring"
    );

    let second = store.sync_workspace(&workspace).unwrap();
    assert_eq!(second.status, CommunityTapSyncStatus::Updated);
    assert_eq!(second.index.entries.len(), 1);
}

#[test]
fn rejects_blank_tap_urls() {
    use std::path::PathBuf;
    let store = CommunityTapStore::with_base_path(PathBuf::from("/tmp/crosshook-taps-test"));
    let error = store
        .sync_tap(&CommunityTapSubscription {
            url: "   ".to_string(),
            branch: None,
            pinned_commit: None,
        })
        .unwrap_err();

    assert!(matches!(error, CommunityTapError::EmptyTapUrl));
}

#[test]
fn is_valid_git_sha_rejects_injection_attempts() {
    assert!(!is_valid_git_sha("'; rm -rf /"));
    assert!(!is_valid_git_sha("--force"));
    assert!(!is_valid_git_sha("-q"));
    assert!(!is_valid_git_sha("$(reboot)"));
}

#[test]
fn is_valid_git_sha_rejects_invalid_lengths() {
    assert!(!is_valid_git_sha("")); // empty
    assert!(!is_valid_git_sha("abc123")); // 6 chars — one short of minimum
    assert!(!is_valid_git_sha(&"a".repeat(65))); // 65 chars — one over maximum
}

#[test]
fn git_security_env_pairs_include_config_isolation() {
    use super::git::git_security_env_pairs;
    let keys: Vec<_> = git_security_env_pairs().iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&"GIT_CONFIG_NOSYSTEM"));
    assert!(keys.contains(&"GIT_CONFIG_GLOBAL"));
    assert!(keys.contains(&"GIT_TERMINAL_PROMPT"));
}

#[test]
fn is_tap_available_offline_false_when_workspace_missing() {
    let temp_dir = tempdir().unwrap();
    let store = CommunityTapStore::with_base_path(temp_dir.path().to_path_buf());
    let subscription = CommunityTapSubscription {
        url: "https://example.invalid/tap.git".to_string(),
        branch: None,
        pinned_commit: None,
    };
    assert!(!store.is_tap_available_offline(&subscription));
}

#[test]
fn is_valid_git_sha_accepts_valid_hashes() {
    assert!(is_valid_git_sha("abc1234")); // 7-char short hash
    assert!(is_valid_git_sha("deadbeef01234567890abcdef0123456789abcde")); // 40-char SHA1
    assert!(is_valid_git_sha(&"a".repeat(64))); // 64-char SHA256
}

#[test]
fn rejects_injection_attempt_as_pinned_commit() {
    let temp_dir = tempdir().unwrap();
    let source_repo = temp_dir.path().join("source");
    let store_root = temp_dir.path().join("store");
    fs::create_dir_all(&source_repo).unwrap();
    init_repo(&source_repo);

    let manifest = CommunityProfileManifest::new(
        CommunityProfileMetadata {
            game_name: "Test".to_string(),
            game_version: "1.0".to_string(),
            trainer_name: "Trainer".to_string(),
            trainer_version: "1".to_string(),
            proton_version: "9".to_string(),
            platform_tags: vec![],
            compatibility_rating: CompatibilityRating::Unknown,
            author: String::new(),
            description: String::new(),
            trainer_sha256: None,
        },
        GameProfile::default(),
    );
    commit_file(
        &source_repo,
        "profiles/test/community-profile.json",
        &serde_json::to_string_pretty(&manifest).unwrap(),
        "add test profile",
    );

    let store = CommunityTapStore::with_base_path(store_root);
    let subscription = CommunityTapSubscription {
        url: source_repo.display().to_string(),
        branch: Some("main".to_string()),
        pinned_commit: Some("'; rm -rf /".to_string()),
    };
    // Use sync_workspace directly to bypass URL scheme validation (test uses local path).
    let workspace = CommunityTapWorkspace {
        local_path: store.workspace_path(&subscription),
        subscription,
    };

    let err = store.sync_workspace(&workspace).unwrap_err();
    assert!(
        matches!(err, CommunityTapError::InvalidPinnedCommit(_)),
        "expected InvalidPinnedCommit, got: {err}"
    );
}

#[test]
fn pinned_tap_stays_on_selected_commit() {
    let temp_dir = tempdir().unwrap();
    let source_repo = temp_dir.path().join("source");
    let store_root = temp_dir.path().join("store");
    fs::create_dir_all(&source_repo).unwrap();
    init_repo(&source_repo);

    let manifest_v1 = CommunityProfileManifest::new(
        CommunityProfileMetadata {
            game_name: "Elden Ring".to_string(),
            game_version: "1.0".to_string(),
            trainer_name: "Trainer".to_string(),
            trainer_version: "1".to_string(),
            proton_version: "9".to_string(),
            platform_tags: vec!["linux".to_string()],
            compatibility_rating: CompatibilityRating::Working,
            author: "CrossHook".to_string(),
            description: "Pinned v1".to_string(),
            trainer_sha256: None,
        },
        GameProfile::default(),
    );
    commit_file(
        &source_repo,
        "profiles/elden-ring/community-profile.json",
        &serde_json::to_string_pretty(&manifest_v1).unwrap(),
        "add v1 profile",
    );
    let pinned_commit = rev_parse_head(&source_repo);

    let store = CommunityTapStore::with_base_path(store_root);
    let subscription = CommunityTapSubscription {
        url: source_repo.display().to_string(),
        branch: Some("main".to_string()),
        pinned_commit: Some(pinned_commit.clone()),
    };
    // Use sync_workspace directly to bypass URL scheme validation (test uses local path).
    let workspace = CommunityTapWorkspace {
        local_path: store.workspace_path(&subscription),
        subscription,
    };

    let first_sync = store.sync_workspace(&workspace).unwrap();
    assert_eq!(first_sync.head_commit, pinned_commit);

    let manifest_v2 = CommunityProfileManifest::new(
        CommunityProfileMetadata {
            game_name: "Elden Ring".to_string(),
            game_version: "1.1".to_string(),
            trainer_name: "Trainer".to_string(),
            trainer_version: "2".to_string(),
            proton_version: "9".to_string(),
            platform_tags: vec!["linux".to_string()],
            compatibility_rating: CompatibilityRating::Working,
            author: "CrossHook".to_string(),
            description: "Pinned v2".to_string(),
            trainer_sha256: None,
        },
        GameProfile::default(),
    );
    commit_file(
        &source_repo,
        "profiles/elden-ring/community-profile.json",
        &serde_json::to_string_pretty(&manifest_v2).unwrap(),
        "update profile",
    );

    let second_sync = store.sync_workspace(&workspace).unwrap();
    assert_eq!(second_sync.status, CommunityTapSyncStatus::Updated);
    assert_eq!(second_sync.head_commit, pinned_commit);
    assert_eq!(second_sync.index.entries.len(), 1);
    assert_eq!(
        second_sync.index.entries[0]
            .manifest
            .metadata
            .trainer_version,
        "1"
    );
}

#[test]
fn validate_branch_name_accepts_valid_names() {
    assert!(validate_branch_name("main").is_ok());
    assert!(validate_branch_name("feature/my-branch").is_ok());
    assert!(validate_branch_name("release_1.0").is_ok());
    assert!(validate_branch_name("v2.3-stable").is_ok());
    assert!(validate_branch_name("a/b/c.d_e-f").is_ok());
}

#[test]
fn validate_branch_name_rejects_leading_dash() {
    assert!(matches!(
        validate_branch_name("--upload-pack=/evil"),
        Err(CommunityTapError::InvalidBranch(_))
    ));
    assert!(matches!(
        validate_branch_name("-q"),
        Err(CommunityTapError::InvalidBranch(_))
    ));
}

#[test]
fn validate_branch_name_rejects_special_chars() {
    assert!(matches!(
        validate_branch_name("branch;rm -rf /"),
        Err(CommunityTapError::InvalidBranch(_))
    ));
    assert!(matches!(
        validate_branch_name("branch$(evil)"),
        Err(CommunityTapError::InvalidBranch(_))
    ));
    assert!(matches!(
        validate_branch_name("branch with spaces"),
        Err(CommunityTapError::InvalidBranch(_))
    ));
}

#[test]
fn validate_tap_url_accepts_https() {
    assert!(validate_tap_url("https://github.com/user/repo").is_ok());
    assert!(validate_tap_url("https://gitlab.com/org/crosshook-taps").is_ok());
}

#[test]
fn validate_tap_url_accepts_ssh_git() {
    assert!(validate_tap_url("ssh://git@github.com/user/repo").is_ok());
    assert!(validate_tap_url("ssh://git@gitlab.com/user/repo").is_ok());
}

#[test]
fn validate_tap_url_rejects_file_scheme() {
    assert!(matches!(
        validate_tap_url("file:///home/user/.ssh/"),
        Err(CommunityTapError::InvalidTapUrl(_))
    ));
    assert!(matches!(
        validate_tap_url("file:///etc/passwd"),
        Err(CommunityTapError::InvalidTapUrl(_))
    ));
}

#[test]
fn validate_tap_url_rejects_git_scheme() {
    assert!(matches!(
        validate_tap_url("git://github.com/user/repo"),
        Err(CommunityTapError::InvalidTapUrl(_))
    ));
}

#[test]
fn validate_tap_url_accepts_absolute_paths() {
    assert!(validate_tap_url("/tmp/local-repo").is_ok());
    assert!(validate_tap_url("/home/user/crosshook-test-tap").is_ok());
}

#[test]
fn validate_tap_url_rejects_relative_paths() {
    assert!(matches!(
        validate_tap_url("../relative-repo"),
        Err(CommunityTapError::InvalidTapUrl(_))
    ));
}
