use super::super::steam_deck::is_steam_deck_from_sources;

#[test]
fn steam_deck_detected_via_steam_deck_env_only() {
    let result = is_steam_deck_from_sources(
        |key| {
            if key == "SteamDeck" {
                Some("1".to_string())
            } else {
                None
            }
        },
        None,
    );
    assert!(result, "SteamDeck=1 env should be sufficient");
}

#[test]
fn steam_deck_detected_via_steamos_env_only() {
    let result = is_steam_deck_from_sources(
        |key| {
            if key == "SteamOS" {
                Some("1".to_string())
            } else {
                None
            }
        },
        None,
    );
    assert!(result, "SteamOS=1 env should be sufficient");
}

#[test]
fn steam_deck_detected_via_variant_id_in_os_release() {
    let os_release = "ID=steamos\nVARIANT_ID=steamdeck\nVERSION_ID=3.5\n";
    let result = is_steam_deck_from_sources(|_| None, Some(os_release));
    assert!(
        result,
        "VARIANT_ID=steamdeck in os-release should be detected"
    );
}

#[test]
fn steam_deck_detected_via_id_steamos_in_os_release() {
    let os_release = "ID=steamos\nVERSION_ID=3.5\n";
    let result = is_steam_deck_from_sources(|_| None, Some(os_release));
    assert!(result, "ID=steamos in os-release should be detected");
}

#[test]
fn steam_deck_detected_via_uppercase_id_steamos_in_os_release() {
    let os_release = "ID=STEAMOS\nVERSION_ID=3.5\n";
    let result = is_steam_deck_from_sources(|_| None, Some(os_release));
    assert!(result, "ID=STEAMOS should be detected case-insensitively");
}

#[test]
fn steam_deck_not_detected_for_arch_id() {
    let os_release = "ID=arch\nID_LIKE=\nVERSION_ID=\n";
    let result = is_steam_deck_from_sources(|_| None, Some(os_release));
    assert!(!result, "ID=arch should not trigger Steam Deck detection");
}

#[test]
fn steam_deck_not_detected_when_no_env_and_no_os_release() {
    let result = is_steam_deck_from_sources(|_| None, None);
    assert!(!result, "empty env + None os_release should return false");
}

#[test]
fn steam_deck_detected_via_quoted_variant_id() {
    let os_release = "ID=steamos\nVARIANT_ID=\"steamdeck\"\nVERSION_ID=3.5\n";
    let result = is_steam_deck_from_sources(|_| None, Some(os_release));
    assert!(
        result,
        "VARIANT_ID=\"steamdeck\" (double-quoted) should be detected"
    );
}

#[test]
fn steam_deck_detected_via_single_quoted_variant_id() {
    let os_release = "VARIANT_ID='steamdeck'\n";
    let result = is_steam_deck_from_sources(|_| None, Some(os_release));
    assert!(
        result,
        "VARIANT_ID='steamdeck' (single-quoted) should be detected"
    );
}
