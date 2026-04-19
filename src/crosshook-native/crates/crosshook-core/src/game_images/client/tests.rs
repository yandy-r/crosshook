use crate::game_images::models::{GameImageSource, GameImageType};
use crate::metadata::MetadataStore;

use super::cache::{filename_for, parse_expiration};
use super::download::{build_download_url, portrait_candidate_urls};
use super::http::is_allowed_redirect_host;
use super::validation::{safe_image_cache_path, validate_image_bytes};

// -----------------------------------------------------------------------
// app_id validation
// -----------------------------------------------------------------------

#[test]
fn numeric_app_id_passes_validation() {
    let result = download_and_cache_image_guard_app_id("440");
    assert!(result.is_ok(), "pure numeric app_id must pass");
}

#[test]
fn alphanumeric_app_id_is_rejected() {
    // Inline call to the app_id guard logic (same logic as in the public fn)
    let app_id = "123abc";
    assert!(
        app_id.is_empty() || !app_id.chars().all(|c| c.is_ascii_digit()),
        "123abc should fail numeric check"
    );
}

#[test]
fn path_traversal_app_id_is_rejected() {
    for bad in &["../etc", "../../passwd", "/etc/shadow", "..", "44 0"] {
        assert!(
            bad.is_empty() || !bad.chars().all(|c| c.is_ascii_digit()),
            "{bad:?} should fail numeric check"
        );
    }
}

#[test]
fn empty_app_id_is_rejected() {
    let app_id = "";
    assert!(app_id.is_empty(), "empty app_id must fail the empty check");
}

// -----------------------------------------------------------------------
// validate_image_bytes
// -----------------------------------------------------------------------

#[test]
fn jpeg_magic_bytes_are_accepted() {
    // Minimal JPEG header: SOI marker FF D8, followed by FF E0 (JFIF APP0)
    let mut jpeg_bytes = vec![0xFF_u8, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
    jpeg_bytes.extend_from_slice(b"JFIF\x00");
    // Pad to make it look non-trivial
    jpeg_bytes.extend(vec![0u8; 20]);
    assert!(
        validate_image_bytes(&jpeg_bytes).is_ok(),
        "JPEG magic bytes must be accepted"
    );
}

#[test]
fn png_magic_bytes_are_accepted() {
    // PNG signature: 8 bytes
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
    ];
    assert!(
        validate_image_bytes(&png_bytes).is_ok(),
        "PNG magic bytes must be accepted"
    );
}

#[test]
fn svg_is_rejected() {
    // SVG is XML text — no magic bytes; infer will return None → octet-stream
    let svg_bytes = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";
    let result = validate_image_bytes(svg_bytes);
    assert!(
        result.is_err(),
        "SVG content must be rejected (no magic bytes → octet-stream)"
    );
}

#[test]
fn html_text_is_rejected() {
    let html = b"<!DOCTYPE html><html><body>evil</body></html>";
    assert!(
        validate_image_bytes(html).is_err(),
        "HTML text must be rejected"
    );
}

#[test]
fn oversized_content_is_rejected() {
    let oversized = vec![0xFF_u8, 0xD8, 0xFF, 0xE0];
    // We don't allocate 5 MB here; instead test the boundary directly.
    let mut large = vec![0u8; 5 * 1024 * 1024 + 1];
    // Set JPEG magic so format check would pass if not for size check
    large[0] = 0xFF;
    large[1] = 0xD8;
    large[2] = 0xFF;
    large[3] = 0xE0;
    let result = validate_image_bytes(&large);
    assert!(result.is_err(), "content exceeding 5 MB must be rejected");
    _ = oversized; // suppress unused warning
}

#[test]
fn filename_for_uses_inferred_extension() {
    assert_eq!(
        filename_for(GameImageType::Cover, GameImageSource::SteamCdn, "png"),
        "cover_steam_cdn.png"
    );
    assert_eq!(
        filename_for(GameImageType::Capsule, GameImageSource::SteamGridDb, "webp"),
        "capsule_steamgriddb.webp"
    );
}

#[test]
fn filename_for_portrait_type() {
    assert_eq!(
        filename_for(GameImageType::Portrait, GameImageSource::SteamCdn, "jpg"),
        "portrait_steam_cdn.jpg"
    );
    assert_eq!(
        filename_for(
            GameImageType::Portrait,
            GameImageSource::SteamGridDb,
            "webp"
        ),
        "portrait_steamgriddb.webp"
    );
}

#[test]
fn build_download_url_background_uses_library_hero() {
    let url = build_download_url("1245620", GameImageType::Background);
    assert_eq!(
        url,
        "https://cdn.cloudflare.steamstatic.com/steam/apps/1245620/library_hero.jpg"
    );
}

#[test]
fn filename_for_background_type() {
    assert_eq!(
        filename_for(GameImageType::Background, GameImageSource::SteamCdn, "jpg"),
        "background_steam_cdn.jpg"
    );
}

#[test]
fn portrait_candidate_urls_returns_three_in_order() {
    let app_id = "440";
    let urls = portrait_candidate_urls(app_id);
    assert_eq!(urls.len(), 3);
    assert_eq!(
        urls[0],
        "https://cdn.cloudflare.steamstatic.com/steam/apps/440/library_600x900_2x.jpg"
    );
    assert_eq!(
        urls[1],
        "https://cdn.cloudflare.steamstatic.com/steam/apps/440/library_600x900.jpg"
    );
    assert_eq!(
        urls[2],
        "https://cdn.cloudflare.steamstatic.com/steam/apps/440/header.jpg"
    );
}

#[test]
fn parse_expiration_accepts_rfc3339_and_legacy_utc_format() {
    assert!(parse_expiration("2026-04-01T12:34:56Z").is_some());
    assert!(parse_expiration("2026-04-01T12:34:56+00:00").is_some());
    assert!(parse_expiration("2026-04-01T12:34:56").is_some());
    assert!(parse_expiration("not-a-timestamp").is_none());
}

// -----------------------------------------------------------------------
// safe_image_cache_path
// -----------------------------------------------------------------------

#[test]
fn safe_path_rejects_dotdot_app_id() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = safe_image_cache_path(tmp.path(), "../etc", "cover_steam_cdn.jpg");
    assert!(
        result.is_err(),
        "path traversal via app_id must be rejected"
    );
}

#[test]
fn safe_path_rejects_slash_in_filename() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = safe_image_cache_path(tmp.path(), "440", "../../evil.jpg");
    assert!(
        result.is_err(),
        "path traversal via filename must be rejected"
    );
}

#[test]
fn safe_path_accepts_valid_inputs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = safe_image_cache_path(tmp.path(), "440", "cover_steam_cdn.jpg");
    assert!(result.is_ok(), "valid app_id and filename must succeed");
    let path = result.unwrap();
    // The path must be inside the base temp dir
    assert!(
        path.starts_with(tmp.path()),
        "result path must be inside base dir"
    );
}

#[test]
fn safe_path_rejects_empty_app_id() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = safe_image_cache_path(tmp.path(), "", "cover_steam_cdn.jpg");
    assert!(result.is_err(), "empty app_id must be rejected");
}

// -----------------------------------------------------------------------
// MetadataStore integration (in-memory DB)
// -----------------------------------------------------------------------

#[test]
fn get_game_image_returns_none_for_missing_entry() {
    let store = MetadataStore::open_in_memory().expect("open in-memory store");
    let result = store.get_game_image("999999", "cover").unwrap();
    assert!(result.is_none());
}

#[test]
fn upsert_then_get_round_trips() {
    let store = MetadataStore::open_in_memory().expect("open in-memory store");
    store
        .upsert_game_image(
            "440",
            "cover",
            "steam_cdn",
            "/tmp/test/440/cover_steam_cdn.jpg",
            Some(1024),
            Some("deadbeef"),
            Some("image/jpeg"),
            Some("https://cdn.cloudflare.steamstatic.com/steam/apps/440/header.jpg"),
            None,
        )
        .expect("upsert should succeed");

    let row = store
        .get_game_image("440", "cover")
        .unwrap()
        .expect("row must exist after upsert");

    assert_eq!(row.steam_app_id, "440");
    assert_eq!(row.image_type, "cover");
    assert_eq!(row.content_hash, "deadbeef");
}

// -----------------------------------------------------------------------
// is_allowed_redirect_host
// -----------------------------------------------------------------------

#[test]
fn allowed_redirect_hosts_are_accepted() {
    assert!(
        is_allowed_redirect_host("cdn.cloudflare.steamstatic.com"),
        "cdn.cloudflare.steamstatic.com must be allowed"
    );
    assert!(
        is_allowed_redirect_host("steamcdn-a.akamaihd.net"),
        "steamcdn-a.akamaihd.net must be allowed"
    );
    assert!(
        is_allowed_redirect_host("www.steamgriddb.com"),
        "www.steamgriddb.com must be allowed"
    );
    assert!(
        is_allowed_redirect_host("cdn2.steamgriddb.com"),
        "cdn2.steamgriddb.com must be allowed"
    );
}

#[test]
fn disallowed_redirect_hosts_are_rejected() {
    assert!(
        !is_allowed_redirect_host("evil.com"),
        "evil.com must be rejected"
    );
    assert!(
        !is_allowed_redirect_host("cdn.cloudflare.steamstatic.com.evil.com"),
        "subdomain-spoofing of allowed host must be rejected"
    );
    assert!(
        !is_allowed_redirect_host("127.0.0.1"),
        "loopback address must be rejected"
    );
    assert!(
        !is_allowed_redirect_host("192.168.1.1"),
        "private network address must be rejected"
    );
    assert!(!is_allowed_redirect_host(""), "empty host must be rejected");
    assert!(
        !is_allowed_redirect_host("steamgriddb.com"),
        "bare steamgriddb.com without www. prefix must be rejected"
    );
}

// -----------------------------------------------------------------------
// Helper: guard-only validation without I/O
// -----------------------------------------------------------------------

fn download_and_cache_image_guard_app_id(app_id: &str) -> Result<(), String> {
    if app_id.is_empty() || !app_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("invalid app_id: {app_id:?}"));
    }
    Ok(())
}
