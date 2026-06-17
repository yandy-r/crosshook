use super::catalog::{
    parse_catalog_toml, CommandArgumentCatalog, CommandArgumentEntry, DEFAULT_CATALOG_TOML,
};
use super::resolver::{
    resolve_command_arguments_with_catalog, CommandArgumentResolveError, ResolvedCommandArguments,
};
use crate::launch::request::{METHOD_NATIVE, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};

fn make_test_catalog() -> CommandArgumentCatalog {
    let (entries, warnings) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "test");
    assert!(
        warnings.is_empty(),
        "default command argument catalog must parse cleanly: {warnings:?}"
    );
    CommandArgumentCatalog::from_entries(entries)
}

fn make_toml_entry(id: &str, label: &str, category: &str, tokens: &[&str]) -> String {
    let token_list = tokens
        .iter()
        .map(|token| format!("\"{token}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "[[command_argument]]\nid = \"{id}\"\nlabel = \"{label}\"\ncategory = \"{category}\"\ntokens = [{token_list}]\napplicable_methods = [\"proton_run\", \"steam_applaunch\"]\n"
    )
}

#[test]
fn default_catalog_toml_parses_with_no_warnings() {
    let (entries, warnings) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
    assert!(
        warnings.is_empty(),
        "default catalog had warnings: {warnings:?}"
    );
    assert!(!entries.is_empty(), "default catalog should have entries");
}

#[test]
fn default_catalog_has_six_entries() {
    let (entries, _) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
    assert_eq!(
        entries.len(),
        6,
        "expected exactly 6 entries in the default catalog"
    );
}

#[test]
fn skip_launcher_catalog_token_uses_double_dash_hyphenated_form() {
    let (entries, _) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
    let skip_launcher = entries
        .iter()
        .find(|entry| entry.id == "skip_launcher")
        .expect("skip_launcher catalog entry");
    assert_eq!(skip_launcher.tokens, vec!["--skip-launcher".to_string()]);
}

#[test]
fn nolauncher_catalog_token_uses_double_dash_form() {
    let (entries, _) = parse_catalog_toml(DEFAULT_CATALOG_TOML, "embedded default");
    let nolauncher = entries
        .iter()
        .find(|entry| entry.id == "nolauncher")
        .expect("nolauncher catalog entry");
    assert_eq!(nolauncher.tokens, vec!["--nolauncher".to_string()]);
}

#[test]
fn skip_launcher_and_nolauncher_conflict() {
    let catalog = make_test_catalog();
    let ids = vec!["skip_launcher".to_string(), "nolauncher".to_string()];
    let error = resolve_command_arguments_with_catalog(&ids, &[], METHOD_PROTON_RUN, &catalog)
        .expect_err("conflicting launcher skip flags should fail");

    assert_eq!(
        error,
        CommandArgumentResolveError::Incompatible {
            first: "skip_launcher".to_string(),
            second: "nolauncher".to_string(),
        }
    );
}

#[test]
fn parse_skips_entry_with_empty_id() {
    let toml = "[[command_argument]]\nid = \"\"\nlabel = \"Something\"\ncategory = \"graphics\"\ntokens = [\"-foo\"]\napplicable_methods = [\"proton_run\"]\n";
    let (entries, warnings) = parse_catalog_toml(toml, "test");
    assert!(entries.is_empty());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("empty id"));
}

#[test]
fn parse_skips_entry_with_unrecognized_category() {
    let toml = make_toml_entry("my_arg", "My Arg", "bogus", &["-foo"]);
    let (entries, warnings) = parse_catalog_toml(&toml, "test");
    assert!(entries.is_empty());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unrecognized category"));
}

#[test]
fn parse_skips_entry_with_empty_tokens() {
    let toml = "[[command_argument]]\nid = \"no_tokens\"\nlabel = \"No Tokens\"\ncategory = \"graphics\"\ntokens = []\napplicable_methods = [\"proton_run\"]\n";
    let (entries, warnings) = parse_catalog_toml(toml, "test");
    assert!(entries.is_empty());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("empty tokens"));
}

#[test]
fn parse_skips_duplicate_ids_keeping_first() {
    let toml = format!(
        "{}{}",
        make_toml_entry("dup_id", "First", "graphics", &["-a"]),
        make_toml_entry("dup_id", "Second", "graphics", &["-b"]),
    );
    let (entries, warnings) = parse_catalog_toml(&toml, "test");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].tokens, vec!["-a".to_string()]);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("duplicate"));
}

#[test]
fn parse_returns_empty_on_invalid_toml() {
    let (entries, warnings) = parse_catalog_toml("not toml !!!", "test");
    assert!(entries.is_empty());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("failed to parse"));
}

#[test]
fn rejects_unknown_argument_id() {
    let catalog = make_test_catalog();
    let ids = vec!["not_a_real_id".to_string()];
    let error = resolve_command_arguments_with_catalog(&ids, &[], METHOD_PROTON_RUN, &catalog)
        .expect_err("unknown id should fail");

    assert_eq!(
        error,
        CommandArgumentResolveError::Unknown("not_a_real_id".to_string())
    );
}

#[test]
fn rejects_duplicate_selected_ids() {
    let catalog = make_test_catalog();
    let ids = vec!["force_vulkan".to_string(), "force_vulkan".to_string()];
    let error = resolve_command_arguments_with_catalog(&ids, &[], METHOD_PROTON_RUN, &catalog)
        .expect_err("duplicate id should fail");

    assert_eq!(
        error,
        CommandArgumentResolveError::Duplicate("force_vulkan".to_string())
    );
}

#[test]
fn rejects_incompatible_argument_pair() {
    let catalog = make_test_catalog();
    let ids = vec!["force_vulkan".to_string(), "force_dx11".to_string()];
    let error = resolve_command_arguments_with_catalog(&ids, &[], METHOD_PROTON_RUN, &catalog)
        .expect_err("conflicting ids should fail");

    assert_eq!(
        error,
        CommandArgumentResolveError::Incompatible {
            first: "force_vulkan".to_string(),
            second: "force_dx11".to_string(),
        }
    );
}

#[test]
fn rejects_method_gated_argument_on_native() {
    let catalog = make_test_catalog();
    let ids = vec!["force_vulkan".to_string()];
    let error = resolve_command_arguments_with_catalog(&ids, &[], METHOD_NATIVE, &catalog)
        .expect_err("native method should fail for proton-only entry");

    assert_eq!(
        error,
        CommandArgumentResolveError::NotSupportedForMethod {
            argument_id: "force_vulkan".to_string(),
            method: METHOD_NATIVE.to_string(),
        }
    );
}

#[test]
fn resolves_curated_tokens_in_catalog_order() {
    let catalog = make_test_catalog();
    let ids = vec!["skip_launcher".to_string(), "force_vulkan".to_string()];
    let resolved = resolve_command_arguments_with_catalog(&ids, &[], METHOD_PROTON_RUN, &catalog)
        .expect("resolve arguments");

    assert_eq!(
        resolved,
        ResolvedCommandArguments {
            tokens: vec!["-force_vulkan".to_string(), "--skip-launcher".to_string()],
        }
    );
}

#[test]
fn appends_custom_args_after_curated_tokens_in_user_order() {
    let catalog = make_test_catalog();
    let ids = vec!["force_vulkan".to_string()];
    let custom = vec!["-dx11".to_string(), "+set cl_showfps 1".to_string()];
    let resolved =
        resolve_command_arguments_with_catalog(&ids, &custom, METHOD_PROTON_RUN, &catalog)
            .expect("resolve arguments");

    assert_eq!(
        resolved.tokens,
        vec![
            "-force_vulkan".to_string(),
            "-dx11".to_string(),
            "+set cl_showfps 1".to_string(),
        ]
    );
}

#[test]
fn custom_args_only_preserves_user_order() {
    let catalog = make_test_catalog();
    let custom = vec![
        "--flag=value".to_string(),
        "/path/with spaces/game.cfg".to_string(),
    ];
    let resolved =
        resolve_command_arguments_with_catalog(&[], &custom, METHOD_STEAM_APPLAUNCH, &catalog)
            .expect("resolve custom-only arguments");

    assert_eq!(resolved.tokens, custom);
}

#[test]
fn steam_applaunch_method_resolves_supported_entries() {
    let catalog = make_test_catalog();
    let ids = vec!["skip_launcher".to_string()];
    let resolved =
        resolve_command_arguments_with_catalog(&ids, &[], METHOD_STEAM_APPLAUNCH, &catalog)
            .expect("steam applaunch should resolve");

    assert_eq!(resolved.tokens, vec!["--skip-launcher".to_string()]);
}

#[test]
fn multi_token_entry_emits_all_catalog_tokens() {
    let catalog = CommandArgumentCatalog::from_entries(vec![CommandArgumentEntry {
        id: "combo".to_string(),
        tokens: vec!["-a".to_string(), "-b".to_string()],
        label: "Combo".to_string(),
        description: String::new(),
        help_text: String::new(),
        category: "graphics".to_string(),
        advanced: false,
        community: false,
        applicable_methods: vec![METHOD_PROTON_RUN.to_string()],
        conflicts_with: Vec::new(),
    }]);

    let resolved = resolve_command_arguments_with_catalog(
        &["combo".to_string()],
        &[],
        METHOD_PROTON_RUN,
        &catalog,
    )
    .expect("resolve multi-token entry");

    assert_eq!(resolved.tokens, vec!["-a".to_string(), "-b".to_string()]);
}
