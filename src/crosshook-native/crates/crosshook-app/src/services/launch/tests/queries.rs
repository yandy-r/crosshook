//! Read-only query tests (Task 2.6): history guards (I-61), key consistency
//! between `reset_sessions` and `running_profiles`, preview's mod-coexistence
//! scope (I-16) and UMU enrichment.

use crosshook_core::launch::{
    session_profile_key_for_name, SessionKind, TeardownReason, METHOD_STEAM_APPLAUNCH,
};
use crosshook_core::mods::{ModCategory, ModProvenance, ProfileModInput};
use crosshook_core::settings::{AppSettingsData, UmuDatabaseLookupPreference};

use super::super::queries::enrich_umu_gameid;
use super::helpers::{Harness, Metadata};
use crate::AppErrorKind;

#[test]
fn validate_reports_the_first_issue_for_an_invalid_request() {
    let harness = Harness::new(Metadata::Enabled);
    let mut req = harness.proton_request("validate");
    req.method = "bogus-method".to_string();

    let issue = harness.service.validate(&req).unwrap_err();
    assert!(!issue.message.is_empty(), "issue carries a message");
}

#[test]
fn validate_accepts_a_valid_request() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .service
        .validate(&harness.proton_request("valid"))
        .expect("valid request passes");
}

#[tokio::test]
async fn launch_history_rejects_a_blank_profile_name() {
    let harness = Harness::new(Metadata::Enabled);

    let error = harness
        .service
        .launch_history("   ".to_string(), None)
        .await
        .unwrap_err();
    assert_eq!(
        error.kind(),
        AppErrorKind::Validation {
            field: "profile_name"
        }
    );
    assert_eq!(error.message(), "profile_name is required");
}

#[tokio::test]
async fn launch_history_returns_empty_when_the_store_is_unavailable() {
    let harness = Harness::new(Metadata::Disabled);

    let rows = harness
        .service
        .launch_history("anything".to_string(), None)
        .await
        .expect("unavailable store is not an error");
    assert!(rows.is_empty());
}

#[tokio::test]
async fn launch_history_defaults_to_twenty_rows() {
    let harness = Harness::new(Metadata::Enabled);
    for _ in 0..25 {
        harness
            .metadata
            .record_launch_started(Some("history-default"), "native", None)
            .unwrap();
    }

    let rows = harness
        .service
        .launch_history(" history-default ".to_string(), None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 20, "default limit is 20 (trimmed query)");
    assert!(
        rows.windows(2).all(|w| w[0].started_at >= w[1].started_at),
        "newest first"
    );
}

#[tokio::test]
async fn launch_history_caps_the_limit_at_fifty() {
    let harness = Harness::new(Metadata::Enabled);
    for _ in 0..55 {
        harness
            .metadata
            .record_launch_started(Some("history-cap"), "native", None)
            .unwrap();
    }

    let rows = harness
        .service
        .launch_history("history-cap".to_string(), Some(1000))
        .await
        .unwrap();
    assert_eq!(rows.len(), 50, "MAX_HISTORY_LIST_LIMIT caps the query");
}

#[tokio::test]
async fn launch_history_scopes_rows_to_the_profile() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .metadata
        .record_launch_started(Some("history-scoped"), "native", None)
        .unwrap();
    harness
        .metadata
        .record_launch_started(Some("history-other"), "native", None)
        .unwrap();

    let rows = harness
        .service
        .launch_history("history-scoped".to_string(), Some(10))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].launch_method, "native");
}

#[test]
fn running_profiles_lists_only_game_sessions_by_key() {
    let harness = Harness::new(Metadata::Enabled);
    let registry = harness.service.registry();
    let key_a = session_profile_key_for_name(Some("running-a"));
    let key_b = session_profile_key_for_name(Some("running-b"));
    let (game_id, _rx_a) = registry.register(SessionKind::Game, key_a.clone());
    let (_trainer_id, _rx_b) = registry.register(SessionKind::Trainer, key_b.clone());

    assert_eq!(
        harness.service.running_profiles(),
        vec![key_a],
        "trainer sessions are not listed"
    );

    registry.deregister(game_id);
    assert!(harness.service.running_profiles().is_empty());
}

#[test]
fn reset_sessions_cancels_every_session_of_the_profile_and_returns_the_count() {
    let harness = Harness::new(Metadata::Enabled);
    let registry = harness.service.registry();
    let key = session_profile_key_for_name(Some("reset-target"));
    let (_game_id, mut game_rx) = registry.register(SessionKind::Game, key.clone());
    let (_trainer_id, mut trainer_rx) = registry.register(SessionKind::Trainer, key.clone());
    let (_other_id, mut other_rx) = registry.register(
        SessionKind::Game,
        session_profile_key_for_name(Some("reset-other")),
    );

    assert_eq!(harness.service.reset_sessions("reset-target"), 2);
    assert_eq!(
        game_rx.try_recv().ok(),
        Some(TeardownReason::UserRequest),
        "game session cancelled as a user request"
    );
    assert_eq!(
        trainer_rx.try_recv().ok(),
        Some(TeardownReason::UserRequest),
        "trainer sessions of the profile are cancelled too"
    );
    assert!(
        other_rx.try_recv().is_err(),
        "other profiles are untouched (no cascade)"
    );
    // The reset signals but does not remove sessions; they stay listed until
    // their stream tasks deregister (deregistration is a finalizer concern).
    let running = harness.service.running_profiles();
    assert_eq!(
        running.len(),
        2,
        "signalled sessions remain registered until they exit: {running:?}"
    );
}

#[test]
fn reset_and_running_share_the_anonymous_key_for_blank_names() {
    let harness = Harness::new(Metadata::Enabled);
    let registry = harness.service.registry();
    let anonymous = session_profile_key_for_name(None);
    assert_eq!(anonymous, session_profile_key_for_name(Some("")));
    assert_eq!(anonymous, session_profile_key_for_name(Some("   ")));

    let (_game_id, mut rx) = registry.register(SessionKind::Game, anonymous.clone());
    assert_eq!(
        harness.service.running_profiles(),
        vec![anonymous],
        "a blank-named session lists under the anonymous key"
    );

    assert_eq!(
        harness.service.reset_sessions(""),
        1,
        "a blank name resolves to the same anonymous key"
    );
    assert_eq!(rx.try_recv().ok(), Some(TeardownReason::UserRequest));
}

#[tokio::test]
async fn preview_appends_only_mod_coexistence_warnings() {
    let harness = Harness::new(Metadata::Enabled);
    let name = "preview-mods";

    // Register the profile and one enabled overlay mod; the request enables
    // MangoHud, which triggers the `mod_coexistence_overlay_conflict` rule.
    harness
        .profiles
        .save(name, &crosshook_core::profile::GameProfile::default())
        .unwrap();
    harness
        .metadata
        .sync_profiles_from_store(&harness.profiles)
        .unwrap();
    let profile_id = harness.metadata.lookup_profile_id(name).unwrap().unwrap();
    harness
        .metadata
        .add_profile_mod(
            &profile_id,
            &ProfileModInput {
                name: "Reshade".to_string(),
                category: ModCategory::OverlayInjection,
                paths: Vec::new(),
                enabled: true,
                source_url: None,
                provenance: ModProvenance::Manual,
            },
        )
        .unwrap();

    let mut req = harness.proton_request(name);
    req.mangohud.enabled = true;
    let preview = harness.service.preview(req).await.unwrap();

    let codes: Vec<_> = preview
        .validation
        .issues
        .iter()
        .filter_map(|issue| issue.code.as_deref())
        .collect();
    assert!(
        codes.contains(&"mod_coexistence_overlay_conflict"),
        "coexistence warning is appended, got {codes:?}"
    );
    assert!(
        preview.validation.issues.iter().all(|issue| issue
            .code
            .as_deref()
            .is_none_or(|code| code.starts_with("mod_coexistence_"))),
        "no low-disk, offline or trainer-hash warnings, got {codes:?}"
    );
}

#[tokio::test]
async fn preview_adds_no_coexistence_warnings_without_enabled_mods() {
    let harness = Harness::new(Metadata::Enabled);
    let preview = harness
        .service
        .preview(harness.proton_request("preview-plain"))
        .await
        .unwrap();
    assert!(
        preview.validation.issues.iter().all(|issue| issue
            .code
            .as_deref()
            .is_none_or(|code| code.starts_with("mod_coexistence_"))),
        "unknown profiles yield no coexistence warnings"
    );
}

#[test]
fn steam_launch_options_command_joins_env_wrappers_and_tokens() {
    let harness = Harness::new(Metadata::Enabled);
    let mut env = std::collections::BTreeMap::new();
    env.insert("MY_VAR".to_string(), "my value".to_string());

    let command = harness
        .service
        .steam_launch_options_command(&[], &env, None, &[], &["--fullscreen".to_string()])
        .unwrap();

    assert!(
        command.contains("MY_VAR=\"my value\""),
        "custom env: {command}"
    );
    assert!(command.contains("%command%"), "command marker: {command}");
    assert!(
        command.ends_with("--fullscreen"),
        "custom argument tokens come last: {command}"
    );
}

#[test]
fn steam_launch_options_command_keeps_debug_text_for_unknown_arguments() {
    let harness = Harness::new(Metadata::Enabled);

    let error = harness
        .service
        .steam_launch_options_command(
            &[],
            &std::collections::BTreeMap::new(),
            None,
            &["no-such-argument-id".to_string()],
            &[],
        )
        .unwrap_err();
    // `CommandArgumentResolveError` has no `Display`; the text is its Debug
    // form (flagged follow-up in the plan).
    assert!(
        error.message().contains("Unknown"),
        "debug text, got: {}",
        error.message()
    );
}

#[test]
fn steam_launch_options_command_maps_unknown_option_ids_to_validation() {
    let harness = Harness::new(Metadata::Enabled);

    let error = harness
        .service
        .steam_launch_options_command(
            &["no-such-option-id".to_string()],
            &std::collections::BTreeMap::new(),
            None,
            &[],
            &[],
        )
        .unwrap_err();
    assert!(matches!(error.kind(), AppErrorKind::Validation { .. }));
}

#[tokio::test]
async fn enrich_umu_gameid_uses_the_steam_app_id_for_steam_requests() {
    let harness = Harness::new(Metadata::Enabled);
    let mut req = harness.steam_request("umu-steam");

    enrich_umu_gameid(&harness.settings, &harness.metadata, &mut req).await;

    let resolution = req.resolved_umu_game_id.expect("resolution is set");
    assert_eq!(resolution.game_id, "12345");
    assert_eq!(req.method, METHOD_STEAM_APPLAUNCH);
}

#[tokio::test]
async fn enrich_umu_gameid_defaults_when_settings_cannot_load() {
    let harness = Harness::new(Metadata::Enabled);
    // Corrupt the settings file so `load` fails and the default (lookup
    // disabled) applies; the explicit override below still wins.
    let config_dir = harness.root.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("settings.toml"), "[ not toml").unwrap();
    let mut req = harness.proton_request("umu-explicit");
    req.runtime.umu_game_id = "umu-123".to_string();

    enrich_umu_gameid(&harness.settings, &harness.metadata, &mut req).await;

    let resolution = req.resolved_umu_game_id.expect("resolution is set");
    assert_eq!(resolution.game_id, "umu-123");
}

#[tokio::test]
async fn enrich_umu_gameid_is_fail_soft_for_a_proton_request_without_hints() {
    let harness = Harness::new(Metadata::Enabled);
    harness
        .settings
        .save(&AppSettingsData {
            umu_database_lookup: UmuDatabaseLookupPreference::Enabled,
            ..AppSettingsData::default()
        })
        .unwrap();
    let mut req = harness.proton_request("umu-fallback");

    enrich_umu_gameid(&harness.settings, &harness.metadata, &mut req).await;

    // No store/codename hints and no network: fail-soft fallback, never Err.
    let resolution = req.resolved_umu_game_id.expect("resolution is set");
    assert!(!resolution.game_id.is_empty(), "a fallback GAMEID resolves");
}
