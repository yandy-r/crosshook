use super::*;
use crate::services::onboarding::dismissals::require_readiness_metadata;
use crate::services::onboarding::LegacyNag;

const DISABLED_ERROR: &str = "readiness nag dismissals require the SQLite metadata store; dismiss_umu_install_nag and dismiss_steam_deck_caveats remain available via settings.toml";

#[test]
fn require_readiness_metadata_accepts_available_store() {
    require_readiness_metadata(&MetadataStore::open_in_memory().unwrap()).unwrap();
}

#[tokio::test]
async fn legacy_dismissals_dual_write_settings_and_sqlite() {
    for (nag, id) in [
        (LegacyNag::UmuInstall, "umu_run"),
        (LegacyNag::SteamDeckCaveats, "steam_deck_caveats"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let settings = SettingsStore::with_base_path(tmp.path().to_path_buf());
        let metadata = MetadataStore::open_in_memory().unwrap();
        let service = service(settings.clone(), metadata.clone());
        service.dismiss_legacy_nag(nag).await.unwrap();
        let saved = settings.load().unwrap();
        match nag {
            LegacyNag::UmuInstall => assert!(saved.install_nag_dismissed_at.is_some()),
            LegacyNag::SteamDeckCaveats => assert!(saved.steam_deck_caveats_dismissed_at.is_some()),
        }
        assert!(!saved.onboarding_completed);
        assert!(metadata
            .get_dismissed_readiness_nags()
            .unwrap()
            .contains(id));
    }
}

#[tokio::test]
async fn legacy_dismissals_succeed_with_settings_only_when_metadata_disabled() {
    for nag in [LegacyNag::UmuInstall, LegacyNag::SteamDeckCaveats] {
        let tmp = tempfile::tempdir().unwrap();
        let settings = SettingsStore::with_base_path(tmp.path().to_path_buf());
        service(settings.clone(), MetadataStore::disabled())
            .dismiss_legacy_nag(nag)
            .await
            .unwrap();
        let saved = settings.load().unwrap();
        match nag {
            LegacyNag::UmuInstall => assert!(saved.install_nag_dismissed_at.is_some()),
            LegacyNag::SteamDeckCaveats => assert!(saved.steam_deck_caveats_dismissed_at.is_some()),
        }
        assert!(!saved.onboarding_completed);
    }
}

#[tokio::test]
async fn legacy_dismissals_keep_settings_when_sqlite_write_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let settings = SettingsStore::with_base_path(tmp.path().to_path_buf());
    let metadata = MetadataStore::open_in_memory().unwrap();
    metadata
        .with_sqlite_conn("test dropping dismissals", |conn| {
            conn.execute_batch("DROP TABLE readiness_nag_dismissals")?;
            Ok(())
        })
        .unwrap();
    let service = service(settings.clone(), metadata);
    service
        .dismiss_legacy_nag(LegacyNag::UmuInstall)
        .await
        .unwrap();
    service
        .dismiss_legacy_nag(LegacyNag::SteamDeckCaveats)
        .await
        .unwrap();
    let saved = settings.load().unwrap();
    assert!(saved.install_nag_dismissed_at.is_some());
    assert!(saved.steam_deck_caveats_dismissed_at.is_some());
}

#[tokio::test]
async fn legacy_dismissal_settings_failure_does_not_write_sqlite() {
    let tmp = tempfile::tempdir().unwrap();
    let blocked = tmp.path().join("not-a-directory");
    std::fs::write(&blocked, "blocked").unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();
    let service = service(SettingsStore::with_base_path(blocked), metadata.clone());
    assert!(service
        .dismiss_legacy_nag(LegacyNag::UmuInstall)
        .await
        .is_err());
    assert!(metadata.get_dismissed_readiness_nags().unwrap().is_empty());
}

#[tokio::test]
async fn dismiss_readiness_nag_disabled_store_preserves_legacy_text_before_validation() {
    let tmp = tempfile::tempdir().unwrap();
    let service = service(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        MetadataStore::disabled(),
    );
    let error = service
        .dismiss_readiness_nag("not-a-tool".into(), None)
        .await
        .unwrap_err();
    assert_eq!(error.message(), DISABLED_ERROR);
}

#[tokio::test]
async fn dismiss_readiness_nag_unknown_tool_preserves_legacy_text() {
    let tmp = tempfile::tempdir().unwrap();
    let service = service(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        MetadataStore::open_in_memory().unwrap(),
    );
    let error = service
        .dismiss_readiness_nag("not-a-tool".into(), None)
        .await
        .unwrap_err();
    assert_eq!(
        error.message(),
        "unknown readiness tool_id: not-a-tool. Use a tool id from the host readiness catalog."
    );
}
