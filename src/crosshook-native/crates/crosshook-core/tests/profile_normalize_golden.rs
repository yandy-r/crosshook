//! Byte-identical TOML golden tests for `crosshook_core::profile::normalize_profile`.
//!
//! Each `*.input.json` fixture under `tests/fixtures/profile_normalize/` has:
//! - `*.classic.normalized.json`: output of Classic TypeScript `normalizeProfileForSave`
//!   (verified by `src/utils/__tests__/exportProfileNormalizeFixtures.test.ts`).
//! - `*.classic.toml`: the exact bytes written by `ProfileStore::save` for that Classic
//!   normalized profile.
//!
//! This test verifies that:
//! 1. Saving `*.classic.normalized.json` via `ProfileStore::save` reproduces `*.classic.toml`
//!    byte-for-byte (or regenerates `*.classic.toml` when `CROSSHOOK_WRITE_CLASSIC_TOML_FIXTURES=1`
//!    outside CI).
//! 2. Running Rust `normalize_profile` on `*.input.json` and saving via `ProfileStore::save`
//!    into a tempdir produces bytes **byte-identical** to `*.classic.toml`.

use std::fs;
use std::path::{Path, PathBuf};

use crosshook_core::profile::{normalize_profile, GameProfile, ProfileStore};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profile_normalize")
}

fn input_fixture_paths() -> Vec<PathBuf> {
    let dir = fixture_dir();
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", dir.display()))
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".input.json"))
        })
        .collect();
    paths.sort();
    paths
}

fn save_profile_to_bytes(profile: &GameProfile) -> Vec<u8> {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = ProfileStore::with_base_path(dir.path().to_path_buf());
    store.save("golden", profile).expect("save golden profile");
    fs::read(dir.path().join("golden.toml")).expect("read saved golden TOML")
}

fn stem_of(input_path: &Path) -> String {
    let name = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .expect("utf-8 fixture filename");
    name.strip_suffix(".input.json")
        .expect("input fixture suffix")
        .to_string()
}

#[test]
fn profile_normalize_matches_classic_toml_byte_for_byte() {
    let write_mode = std::env::var("CROSSHOOK_WRITE_CLASSIC_TOML_FIXTURES").as_deref() == Ok("1");
    if write_mode {
        assert!(
            std::env::var_os("CI").is_none(),
            "CROSSHOOK_WRITE_CLASSIC_TOML_FIXTURES must not be set in CI"
        );
    }

    let inputs = input_fixture_paths();
    assert!(
        inputs.len() >= 8,
        "expected at least 8 profile_normalize fixtures, found {}",
        inputs.len()
    );

    let dir = fixture_dir();
    for input_path in inputs {
        let stem = stem_of(&input_path);
        let classic_json_path = dir.join(format!("{stem}.classic.normalized.json"));
        let classic_toml_path = dir.join(format!("{stem}.classic.toml"));

        let classic_json = fs::read_to_string(&classic_json_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", classic_json_path.display()));
        let classic_profile: GameProfile = serde_json::from_str(&classic_json)
            .unwrap_or_else(|err| panic!("parse {}: {err}", classic_json_path.display()));
        let classic_saved_bytes = save_profile_to_bytes(&classic_profile);

        if write_mode {
            fs::write(&classic_toml_path, &classic_saved_bytes)
                .unwrap_or_else(|err| panic!("write {}: {err}", classic_toml_path.display()));
        } else {
            let committed_toml_bytes = fs::read(&classic_toml_path)
                .unwrap_or_else(|err| panic!("read {}: {err}", classic_toml_path.display()));
            assert_eq!(
                classic_saved_bytes,
                committed_toml_bytes,
                "{stem}: Classic normalized JSON saved bytes drifted from committed *.classic.toml; rerun ./scripts/export-ts-parity-fixtures.sh"
            );
        }

        let input_json = fs::read_to_string(&input_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", input_path.display()));
        let draft_profile: GameProfile = serde_json::from_str(&input_json)
            .unwrap_or_else(|err| panic!("parse {}: {err}", input_path.display()));
        let rust_normalized = normalize_profile(draft_profile);
        let rust_saved_bytes = save_profile_to_bytes(&rust_normalized);

        let expected_bytes = fs::read(&classic_toml_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", classic_toml_path.display()));

        if rust_saved_bytes != expected_bytes {
            let actual_str = String::from_utf8_lossy(&rust_saved_bytes);
            let expected_str = String::from_utf8_lossy(&expected_bytes);
            panic!(
                "{stem}: Rust normalize_profile TOML differs from Classic *.classic.toml\n\
                 --- expected (classic) ---\n{expected_str}\n\
                 --- actual (rust) ---\n{actual_str}"
            );
        }
    }
}
