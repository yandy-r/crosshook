use crosshook_core::game_images::import::import_custom_art_into;
use crosshook_core::game_images::GameImageType;
use crosshook_core::metadata::{sha256_hex, MetadataStore};
use crosshook_core::settings::{AppSettingsData, SettingsStore};
use crosshook_core::steam_metadata::{SteamMetadataLookupResult, SteamMetadataLookupState};

use super::super::metadata::{fetch_cover_art_with, parse_image_type};
use super::helpers::{service, TraceCapture, PNG};
use crate::AppErrorKind;

#[test]
fn image_and_custom_art_parse_matrix_preserves_frozen_errors() {
    assert_eq!(parse_image_type(None, false).unwrap(), GameImageType::Cover);
    assert_eq!(parse_image_type(None, true).unwrap(), GameImageType::Cover);
    for (name, kind) in [
        ("cover", GameImageType::Cover),
        ("hero", GameImageType::Hero),
        ("capsule", GameImageType::Capsule),
        ("portrait", GameImageType::Portrait),
        ("background", GameImageType::Background),
    ] {
        assert_eq!(parse_image_type(Some(name), false).unwrap(), kind);
        if matches!(kind, GameImageType::Hero | GameImageType::Capsule) {
            assert_eq!(
                parse_image_type(Some(name), true).unwrap_err().message(),
                format!("Unknown art type: {name}")
            );
        } else {
            assert_eq!(parse_image_type(Some(name), true).unwrap(), kind);
        }
    }
    for custom in [false, true] {
        for name in ["", "Cover", " cover", "svg"] {
            let error = parse_image_type(Some(name), custom).unwrap_err();
            assert!(matches!(error.kind(), AppErrorKind::Validation { .. }));
            assert_eq!(
                error.message(),
                format!(
                    "Unknown {} type: {name}",
                    if custom { "art" } else { "image" }
                )
            );
        }
    }
}

#[tokio::test]
async fn metadata_invalid_id_is_idle_and_cached_details_return_without_network() {
    let temp = tempfile::tempdir().unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();
    let service = service(temp.path(), metadata.clone());
    assert_eq!(
        service
            .fetch_game_metadata("not numeric".into(), None)
            .await
            .unwrap(),
        SteamMetadataLookupResult::default()
    );
    let payload = serde_json::to_string(&SteamMetadataLookupResult {
        app_id: "220".into(),
        state: SteamMetadataLookupState::Ready,
        ..Default::default()
    })
    .unwrap();
    metadata
        .put_cache_entry(
            "https://store.steampowered.com/api/appdetails",
            "steam:appdetails:v1:220",
            &payload,
            Some("9999-12-31T00:00:00+00:00"),
        )
        .unwrap();
    let result = service
        .fetch_game_metadata(" 220 ".into(), Some(false))
        .await
        .unwrap();
    assert_eq!(result.app_id, "220");
    assert_eq!(result.state, SteamMetadataLookupState::Ready);
    assert!(result.from_cache);
    assert!(!result.is_stale);
}

#[tokio::test]
async fn cover_art_returns_real_cached_file_before_network() {
    let temp = tempfile::tempdir().unwrap();
    let metadata = MetadataStore::open_in_memory().unwrap();
    let path = temp.path().join("cached.png");
    std::fs::write(&path, PNG).unwrap();
    metadata
        .upsert_game_image(
            "220",
            "cover",
            "steam_cdn",
            path.to_str().unwrap(),
            None,
            None,
            Some("image/png"),
            None,
            None,
        )
        .unwrap();
    let result = service(temp.path(), metadata)
        .fetch_cover_art("220".into(), None)
        .await
        .unwrap();
    assert_eq!(result.as_deref(), path.to_str());
}

#[tokio::test]
async fn unavailable_and_cached_fallback_results_preserve_service_contract() {
    let temp = tempfile::tempdir().unwrap();
    let settings = SettingsStore::with_base_path(temp.path().join("settings"));
    let result = fetch_cover_art_with(
        MetadataStore::disabled(),
        settings.clone(),
        "220".into(),
        GameImageType::Cover,
        |_, _, _, _| async { Ok(None) },
    )
    .await
    .unwrap_err();
    assert_eq!(result.kind(), AppErrorKind::Unavailable);
    let cached = fetch_cover_art_with(
        MetadataStore::disabled(),
        settings,
        "220".into(),
        GameImageType::Cover,
        |_, _, _, _| async { Ok(Some("cached-art.png".into())) },
    )
    .await
    .unwrap();
    assert_eq!(cached.as_deref(), Some("cached-art.png"));
}

#[tokio::test]
async fn api_key_is_fail_soft_and_absent_from_success_auth_network_traces() {
    let temp = tempfile::tempdir().unwrap();
    let settings = SettingsStore::with_base_path(temp.path().join("settings"));
    let sentinel = format!("test-only-{}", uuid::Uuid::new_v4());
    settings
        .save(&AppSettingsData {
            steamgriddb_api_key: Some(sentinel.clone()),
            ..Default::default()
        })
        .unwrap();
    let capture = TraceCapture::default();
    let _guard = tracing::subscriber::set_default(capture.clone());
    for outcome in ["success", "auth", "network"] {
        let expected = sentinel.clone();
        let dispatch = tracing::Dispatch::new(capture.clone());
        let result = fetch_cover_art_with(
            MetadataStore::disabled(),
            settings.clone(),
            "220".into(),
            GameImageType::Cover,
            move |_, _, _, key| async move {
                assert_eq!(key.as_deref(), Some(expected.as_str()));
                tracing::dispatcher::with_default(&dispatch, || {
                    tracing::info!(outcome, "image fetch outcome");
                });
                if outcome == "success" {
                    Ok(Some("image.png".into()))
                } else {
                    Ok(None)
                }
            },
        )
        .await;
        assert_eq!(result.is_ok(), outcome == "success");
    }
    {
        let output = capture.0.lock().unwrap();
        assert!(output.contains("image fetch outcome"));
        assert!(!output.contains(&sentinel));
    }

    // Malformed settings drops the key rather than preventing art lookup.
    std::fs::write(settings.base_path.join("settings.toml"), "invalid = [").unwrap();
    fetch_cover_art_with(
        MetadataStore::disabled(),
        settings,
        "220".into(),
        GameImageType::Cover,
        |_, _, _, key| async move {
            assert!(key.is_none());
            Ok(Some("cached.png".into()))
        },
    )
    .await
    .unwrap();
}

#[test]
fn imports_are_content_addressed_idempotent_and_infer_extension() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("misleading.svg");
    std::fs::write(&source, PNG).unwrap();
    let media = temp.path().join("media");
    for (kind, directory) in [
        (GameImageType::Cover, "covers"),
        (GameImageType::Portrait, "portraits"),
        (GameImageType::Background, "backgrounds"),
    ] {
        let first = import_custom_art_into(source.to_str().unwrap(), kind, &media).unwrap();
        let expected = media
            .join(directory)
            .join(format!("{}.png", &sha256_hex(PNG)[..16]));
        assert_eq!(first, expected.to_str().unwrap());
        let before = std::fs::metadata(&first).unwrap().modified().unwrap();
        assert_eq!(
            import_custom_art_into(source.to_str().unwrap(), kind, &media).unwrap(),
            first
        );
        assert_eq!(
            std::fs::metadata(&first).unwrap().modified().unwrap(),
            before
        );
        assert_eq!(std::fs::read(first).unwrap(), PNG);
    }
}

#[test]
fn imports_reject_forbidden_and_oversized_content_without_output() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("bad.png");
    let media = temp.path().join("media");
    for bytes in [
        b"<svg></svg>".as_slice(),
        b"<html>not art</html>",
        b"random",
    ] {
        std::fs::write(&source, bytes).unwrap();
        assert!(
            import_custom_art_into(source.to_str().unwrap(), GameImageType::Cover, &media)
                .unwrap_err()
                .contains("image validation failed")
        );
        assert!(!media.exists());
    }
    let mut bytes = vec![0; 5 * 1024 * 1024 + 1];
    bytes[..PNG.len()].copy_from_slice(PNG);
    std::fs::write(&source, bytes).unwrap();
    assert!(
        import_custom_art_into(source.to_str().unwrap(), GameImageType::Cover, &media)
            .unwrap_err()
            .contains("size limit")
    );
    assert!(!media.exists());
}

#[tokio::test]
async fn public_imports_preserve_validation_and_missing_source_errors() {
    let temp = tempfile::tempdir().unwrap();
    let service = service(temp.path(), MetadataStore::disabled());
    assert_eq!(
        service
            .import_custom_art("missing.png".into(), Some("hero".into()))
            .await
            .unwrap_err()
            .message(),
        "Unknown art type: hero"
    );
    assert!(service
        .import_custom_cover_art(
            temp.path()
                .join("missing.png")
                .to_string_lossy()
                .into_owned()
        )
        .await
        .unwrap_err()
        .message()
        .contains("does not exist"));
    let missing = "/nonexistent-crosshook-test/x.png";
    let error = service
        .import_custom_cover_art(missing.into())
        .await
        .unwrap_err();
    assert!(error.message().contains("does not exist"));
    assert!(!error.message().contains("nonexistent-crosshook-test"));
    assert!(service
        .fetch_cover_art("../bad".into(), None)
        .await
        .unwrap_err()
        .message()
        .contains("invalid app_id"));
}
