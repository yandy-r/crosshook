// ── apply_collection_defaults (M3 + M4 regression tests) ─────────────────
//
// These unit tests cover the private helper extracted from `profile_load`:
//   - Fail-open on missing collection_id (M3 cleanup — no layer 3 no-op).
//   - Fail-open on transient / not-found errors (preserves launch path).
//   - Bubble `Corrupt` errors so the frontend can surface them via the
//     existing `useProfile.loadProfile` error channel (M4).

mod apply_collection_defaults_tests {
    use crosshook_core::metadata::{MetadataStore, MetadataStoreError};
    use crosshook_core::profile::{CollectionDefaultsSection, GameProfile};

    use crate::commands::profile::shared::apply_collection_defaults;

    fn profile_with_custom_env(name: &str, value: &str) -> GameProfile {
        let mut profile = GameProfile::default();
        profile
            .launch
            .custom_env_vars
            .insert(name.to_string(), value.to_string());
        profile
    }

    #[test]
    fn none_collection_id_returns_profile_unchanged() {
        let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
        let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");

        let result = apply_collection_defaults(profile.clone(), &store, None)
            .expect("None collection id must succeed");

        assert_eq!(result, profile);
    }

    #[test]
    fn empty_collection_id_returns_profile_unchanged() {
        let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
        let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");

        // Whitespace-only ids are treated as "no collection context" — this
        // mirrors the normalization in `useProfile.loadProfile` which drops
        // empty trimmed ids before calling the command.
        let result = apply_collection_defaults(profile.clone(), &store, Some("   "))
            .expect("empty collection id must succeed");

        assert_eq!(result, profile);
    }

    #[test]
    fn valid_defaults_merge_into_profile() {
        let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
        let collection_id = store
            .create_collection("Speedrun Tools")
            .expect("create collection");

        let mut defaults = CollectionDefaultsSection::default();
        defaults.method = Some("proton_run".to_string());
        defaults
            .custom_env_vars
            .insert("CROSSHOOK_PROBE".to_string(), "1".to_string());
        store
            .set_collection_defaults(&collection_id, Some(&defaults))
            .expect("seed collection defaults");

        let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");
        let result = apply_collection_defaults(profile, &store, Some(collection_id.as_str()))
            .expect("valid defaults must merge");

        assert_eq!(result.launch.method, "proton_run");
        assert_eq!(
            result
                .launch
                .custom_env_vars
                .get("CROSSHOOK_PROBE")
                .cloned(),
            Some("1".to_string()),
            "collection env vars must merge on top of profile env vars"
        );
        assert_eq!(
            result.launch.custom_env_vars.get("PROFILE_ONLY").cloned(),
            Some("keep-me".to_string()),
            "profile env vars without a collision must be preserved"
        );
    }

    #[test]
    fn unknown_collection_id_fails_open_with_unmodified_profile() {
        let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
        let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");

        // With the M1 fix, `get_collection_defaults` on a nonexistent
        // collection returns `Validation(...)`. The helper must treat that
        // as fail-open and return the raw profile rather than hard-block
        // the launch.
        let result = apply_collection_defaults(profile.clone(), &store, Some("no-such-id"))
            .expect("unknown collection id must fail open, not propagate Validation");

        assert_eq!(result, profile);
    }

    #[test]
    fn corrupt_defaults_bubble_error_to_caller() {
        let store = MetadataStore::open_in_memory().expect("open in-memory metadata store");
        let collection_id = store
            .create_collection("Broken")
            .expect("create collection");

        // Force a corrupt JSON payload via raw SQL. `set_collection_defaults`
        // would refuse to write invalid JSON, so we go under it via
        // `with_sqlite_conn`. We avoid the `rusqlite::params!` macro so
        // src-tauri doesn't need a direct rusqlite dev-dep: tuple params
        // implement `rusqlite::Params` directly.
        store
            .with_sqlite_conn("seed corrupt defaults", |conn| {
                conn.execute(
                    "UPDATE collections SET defaults_json = ?1 WHERE collection_id = ?2",
                    ("{not-valid-json", collection_id.as_str()),
                )
                .expect("raw update");
                Ok(())
            })
            .expect("with_sqlite_conn");

        let profile = profile_with_custom_env("PROFILE_ONLY", "keep-me");
        let err = apply_collection_defaults(profile, &store, Some(collection_id.as_str()))
            .expect_err("corrupt defaults must bubble up");

        assert!(
            matches!(err, MetadataStoreError::Corrupt(_)),
            "corrupt JSON must surface as Corrupt so the launch entrypoint can \
             show the error, got {err:?}"
        );
    }
}
