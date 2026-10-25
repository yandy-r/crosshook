//! ProtonDB accept tests.
//!
//! Owned by YAN-811 Task 1.5.

use std::fs;

use crosshook_core::protondb::AcceptSuggestionRequest;

use super::helpers::{steam_profile, Harness, Metadata};
use crate::{AppErrorKind, AppEvent, ProfileChangeReason};

fn seed(harness: &Harness) -> Vec<u8> {
    harness
        .profiles
        .save("Alpha", &steam_profile("/home/tester"))
        .unwrap();
    fs::read(harness.profiles.base_path.join("Alpha.toml")).unwrap()
}

fn saved_alpha() -> AppEvent {
    AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Saved,
        names: vec!["Alpha".to_string()],
    }
}

#[tokio::test]
async fn catalog_accept_toggles_once_and_records_revision() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&harness);
    let accept = || AcceptSuggestionRequest::Catalog {
        profile_name: "  Alpha ".to_string(),
        catalog_entry_id: "disable_steam_input".to_string(),
    };

    let first = harness
        .writer
        .accept_protondb_suggestion(accept())
        .await
        .unwrap();
    let second = harness
        .writer
        .accept_protondb_suggestion(accept())
        .await
        .unwrap();

    assert_eq!(first.output.toggled_option_ids, vec!["disable_steam_input"]);
    assert_eq!(first.output.applied_keys, vec!["PROTON_NO_STEAMINPUT"]);
    assert!(first.new_revision_id.is_some());
    assert!(second.output.toggled_option_ids.is_empty());
    let enabled = harness
        .profiles
        .load("Alpha")
        .unwrap()
        .launch
        .optimizations
        .enabled_option_ids;
    assert_eq!(enabled, vec!["disable_steam_input"]);
    assert_eq!(harness.next_event().await, Some(saved_alpha()));
    assert_eq!(harness.next_event().await, Some(saved_alpha()));
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn env_var_accept_inserts_last_write_wins() {
    let mut harness = Harness::new(Metadata::Enabled);
    seed(&harness);
    for value in ["1", "2"] {
        harness
            .writer
            .accept_protondb_suggestion(AcceptSuggestionRequest::EnvVar {
                profile_name: "Alpha".to_string(),
                env_key: "DXVK_ASYNC".to_string(),
                env_value: value.to_string(),
            })
            .await
            .unwrap();
    }

    let profile = harness.profiles.load("Alpha").unwrap();
    assert_eq!(
        profile
            .launch
            .custom_env_vars
            .get("DXVK_ASYNC")
            .map(String::as_str),
        Some("2")
    );
    assert_eq!(harness.next_event().await, Some(saved_alpha()));
    assert_eq!(harness.next_event().await, Some(saved_alpha()));
}

#[tokio::test]
async fn unsafe_env_suggestions_are_rejected_without_disk_or_event_effects() {
    let mut harness = Harness::new(Metadata::Enabled);
    let before = seed(&harness);

    for (key, value) in [
        ("LD_PRELOAD", "/tmp/evil.so"),
        ("STEAM_COMPAT_DATA_PATH", "/tmp/x"),
        ("DXVK_ASYNC", "1; rm -rf ~"),
        ("bad key", "1"),
    ] {
        let error = harness
            .writer
            .accept_protondb_suggestion(AcceptSuggestionRequest::EnvVar {
                profile_name: "Alpha".to_string(),
                env_key: key.to_string(),
                env_value: value.to_string(),
            })
            .await
            .unwrap_err();
        assert_eq!(
            error.kind(),
            AppErrorKind::Validation { field: "env_key" },
            "{key}={value}"
        );
    }

    assert_eq!(
        fs::read(harness.profiles.base_path.join("Alpha.toml")).unwrap(),
        before
    );
    assert!(harness
        .metadata
        .lookup_profile_id("Alpha")
        .unwrap()
        .is_none());
    assert_eq!(harness.next_event().await, None);
}

#[tokio::test]
async fn invalid_catalog_requests_are_rejected_without_effects() {
    let mut harness = Harness::new(Metadata::Enabled);
    let before = seed(&harness);

    let blank_name = harness
        .writer
        .accept_protondb_suggestion(AcceptSuggestionRequest::Catalog {
            profile_name: "   ".to_string(),
            catalog_entry_id: "disable_steam_input".to_string(),
        })
        .await
        .unwrap_err();
    let unknown = harness
        .writer
        .accept_protondb_suggestion(AcceptSuggestionRequest::Catalog {
            profile_name: "Alpha".to_string(),
            catalog_entry_id: "no_such_entry".to_string(),
        })
        .await
        .unwrap_err();

    assert_eq!(
        blank_name.kind(),
        AppErrorKind::Validation {
            field: "profile_name"
        }
    );
    assert_eq!(
        unknown.kind(),
        AppErrorKind::Validation {
            field: "catalog_entry_id"
        }
    );
    assert_eq!(
        fs::read(harness.profiles.base_path.join("Alpha.toml")).unwrap(),
        before
    );
    assert_eq!(harness.next_event().await, None);
}
