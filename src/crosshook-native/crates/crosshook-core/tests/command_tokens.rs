//! Unit, Classic parity, and property-based tests for `crosshook_core::launch::command_tokens`.

use crosshook_core::launch::preview::{
    EnvVarSource, GamescopeDecisionPreview, LaunchPreview, PreviewEnvVar, PreviewValidation,
    ProtonSetup, ResolvedLaunchMethod,
};
use crosshook_core::launch::{
    highlight_launch_preview, highlight_role, tokenize_command, CommandHighlightRole,
    CommandTokenKind,
};
use proptest::prelude::*;

fn kinds_and_slices(input: &str) -> Vec<(CommandTokenKind, &str)> {
    tokenize_command(input)
        .into_iter()
        .map(|tok| (tok.kind, tok.slice(input)))
        .collect()
}

fn make_preview() -> LaunchPreview {
    LaunchPreview {
        resolved_method: ResolvedLaunchMethod::ProtonRun,
        validation: PreviewValidation { issues: Vec::new() },
        environment: Some(vec![
            PreviewEnvVar {
                key: "DXVK_HUD".to_string(),
                value: "fps".to_string(),
                source: EnvVarSource::ProfileCustom,
            },
            PreviewEnvVar {
                key: "MALICIOUS".to_string(),
                value: "<script>alert(\"x\")</script> $()\nnext".to_string(),
                source: EnvVarSource::ProfileCustom,
            },
            PreviewEnvVar {
                key: "HOME".to_string(),
                value: "/home/devuser".to_string(),
                source: EnvVarSource::Host,
            },
        ]),
        cleared_variables: Vec::new(),
        wrappers: Some(vec!["gamescope".to_string(), "mangohud".to_string()]),
        wrapper_details: None,
        effective_command: Some(
            "gamescope -- mangohud /usr/bin/proton run /games/synthetic quest/game.exe --flag"
                .to_string(),
        ),
        directives_error: None,
        steam_launch_options: None,
        proton_setup: Some(ProtonSetup {
            wine_prefix_path: "/prefixes/synthetic".to_string(),
            compat_data_path: "/steam/compatdata/9999001".to_string(),
            steam_client_install_path: "/steam/root".to_string(),
            proton_executable: "/usr/bin/proton".to_string(),
            umu_run_path: None,
        }),
        working_directory: "/games/synthetic quest".to_string(),
        game_executable: "/games/synthetic quest/game.exe".to_string(),
        game_executable_name: "game.exe".to_string(),
        trainer: None,
        generated_at: "2026-04-23T12:00:00.000Z".to_string(),
        display_text: String::new(),
        gamescope_active: false,
        gamescope_decision: GamescopeDecisionPreview {
            enabled: false,
            allow_nested: false,
            inside_gamescope_session: false,
            active: false,
            reason: String::new(),
            mangohud_folded_into_gamescope: false,
        },
        umu_decision: None,
    }
}

// ── tokenize_command ────────────────────────────────────────────────────────

#[test]
fn tokenizes_steam_launch_options_with_env_wrappers_and_placeholder() {
    let input = "DXVK_HUD=fps PROTON_LOG=\"1\" gamescope -f --mangoapp -- mangohud %command% --skip-launcher \"extra flag\"";
    assert_eq!(
        kinds_and_slices(input),
        vec![
            (CommandTokenKind::EnvAssignment, "DXVK_HUD=fps"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::EnvAssignment, "PROTON_LOG=\"1\""),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Wrapper, "gamescope"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Flag, "-f"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Flag, "--mangoapp"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Argument, "--"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Wrapper, "mangohud"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::CommandPlaceholder, "%command%"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Flag, "--skip-launcher"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::QuotedString, "\"extra flag\""),
        ]
    );
}

#[test]
fn tokenizes_direct_proton_command_with_wrappers_and_executable() {
    let input = "unshare --net /usr/bin/proton run /games/quest/game.exe --flag -=bad";
    assert_eq!(
        kinds_and_slices(input),
        vec![
            (CommandTokenKind::Wrapper, "unshare"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Flag, "--net"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Executable, "/usr/bin/proton"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Argument, "run"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Argument, "/games/quest/game.exe"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Flag, "--flag"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Argument, "-=bad"),
        ]
    );
}

#[test]
fn tokenizes_escaped_quotes_and_unterminated_quotes_losslessly() {
    let escaped = r#"%command% "--cfg=\"C:\\My Games\\config.ini\"" 'literal value'"#;
    assert_eq!(
        kinds_and_slices(escaped),
        vec![
            (CommandTokenKind::CommandPlaceholder, "%command%"),
            (CommandTokenKind::Whitespace, " "),
            (
                CommandTokenKind::QuotedString,
                r#""--cfg=\"C:\\My Games\\config.ini\"""#
            ),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::QuotedString, "'literal value'"),
        ]
    );

    let unterminated_double = r#"%command% "unterminated \"quote"#;
    assert_eq!(
        kinds_and_slices(unterminated_double),
        vec![
            (CommandTokenKind::CommandPlaceholder, "%command%"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::QuotedString, r#""unterminated \"quote"#),
        ]
    );

    let unterminated_single = "%command% 'unterminated single";
    assert_eq!(
        kinds_and_slices(unterminated_single),
        vec![
            (CommandTokenKind::CommandPlaceholder, "%command%"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::QuotedString, "'unterminated single"),
        ]
    );

    let trailing_backslash = "%command% foo\\";
    assert_eq!(
        kinds_and_slices(trailing_backslash),
        vec![
            (CommandTokenKind::CommandPlaceholder, "%command%"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Argument, "foo\\"),
        ]
    );
}

#[test]
fn tokenizes_env_wrapper_and_quoted_env_values() {
    let input = r#"env FOO="a \"b\" c" BAR='x y' gamemoderun %command%"#;
    assert_eq!(
        kinds_and_slices(input),
        vec![
            (CommandTokenKind::Wrapper, "env"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::EnvAssignment, r#"FOO="a \"b\" c""#),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::EnvAssignment, "BAR='x y'"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::Wrapper, "gamemoderun"),
            (CommandTokenKind::Whitespace, " "),
            (CommandTokenKind::CommandPlaceholder, "%command%"),
        ]
    );
}

#[test]
fn maps_every_token_kind_to_theme_role() {
    assert_eq!(
        highlight_role(CommandTokenKind::EnvAssignment),
        CommandHighlightRole::EnvKey
    );
    assert_eq!(
        highlight_role(CommandTokenKind::Wrapper),
        CommandHighlightRole::Binary
    );
    assert_eq!(
        highlight_role(CommandTokenKind::Executable),
        CommandHighlightRole::Binary
    );
    assert_eq!(
        highlight_role(CommandTokenKind::Flag),
        CommandHighlightRole::Flag
    );
    assert_eq!(
        highlight_role(CommandTokenKind::CommandPlaceholder),
        CommandHighlightRole::Placeholder
    );
    assert_eq!(
        highlight_role(CommandTokenKind::QuotedString),
        CommandHighlightRole::Value
    );
    assert_eq!(
        highlight_role(CommandTokenKind::Whitespace),
        CommandHighlightRole::Plain
    );
    assert_eq!(
        highlight_role(CommandTokenKind::Argument),
        CommandHighlightRole::Plain
    );
}

// ── highlight_launch_preview (mirrors HighlightedCommandBlock.test.tsx) ────

#[test]
fn preview_highlighter_emits_expected_roles_and_excludes_host_env() {
    let spans = highlight_launch_preview(&make_preview());

    assert!(spans.iter().any(|s| {
        s.role == CommandHighlightRole::Comment && s.text.contains("proton_run launch preview")
    }));
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::EnvKey && s.text == "DXVK_HUD"));
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Value && s.text == "\"fps\""));
    assert!(!spans.iter().any(|s| s.text == "HOME"));
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Binary && s.text == "gamescope"));
    assert!(spans.iter().any(|s| {
        s.role == CommandHighlightRole::Binary && s.text == "/games/synthetic quest/game.exe"
    }));
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Flag && s.text == "--flag"));
}

#[test]
fn preview_highlighter_json_encodes_unsafe_env_values_as_plain_text() {
    let spans = highlight_launch_preview(&make_preview());
    let expected = serde_json::to_string("<script>alert(\"x\")</script> $()\nnext").unwrap();
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Value && s.text == expected));
}

#[test]
fn preview_highlighter_does_not_promote_bare_double_dash_or_non_flag_hyphen_tokens() {
    let mut preview = make_preview();
    preview.effective_command = Some(
        "gamescope -- mangohud /usr/bin/proton run /games/synthetic quest/game.exe --flag -=bad"
            .to_string(),
    );

    let spans = highlight_launch_preview(&preview);
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Flag && s.text == "--flag"));
    assert!(!spans.iter().any(|s| s.text == "--"));
    assert!(!spans.iter().any(|s| s.text == "-=bad"));
}

#[test]
fn preview_highlighter_shows_ellipsis_when_flags_exceed_cap() {
    let mut preview = make_preview();
    preview.wrappers = Some(Vec::new());
    let many_flags: Vec<String> = (0..10).map(|i| format!("--opt{i}")).collect();
    preview.effective_command = Some(format!("/games/game.exe {}", many_flags.join(" ")));

    let spans = highlight_launch_preview(&preview);
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Flag && s.text == "--opt0"));
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Flag && s.text == "--opt7"));
    assert!(!spans.iter().any(|s| s.text == "--opt8"));
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Comment && s.text == "…"));
}

#[test]
fn preview_highlighter_keeps_wrapper_binaries_separate_from_effective_command() {
    let spans = highlight_launch_preview(&make_preview());

    let gamescope_count = spans
        .iter()
        .filter(|s| s.role == CommandHighlightRole::Binary && s.text == "gamescope")
        .count();
    let mangohud_count = spans
        .iter()
        .filter(|s| s.role == CommandHighlightRole::Binary && s.text == "mangohud")
        .count();
    assert_eq!(gamescope_count, 1);
    assert_eq!(mangohud_count, 1);
    assert!(!spans.iter().any(|s| s.text == "--"));
}

#[test]
fn preview_highlighter_sanitizes_and_truncates_long_non_ascii_comment_fields() {
    let mut preview = make_preview();
    preview.game_executable_name = format!("{}日本語\x07\n", "a".repeat(80));

    let spans = highlight_launch_preview(&preview);
    let expected = format!(" for {}…", "a".repeat(64));
    assert!(spans
        .iter()
        .any(|s| s.role == CommandHighlightRole::Comment && s.text == expected));
}

// ── proptest: UTF-8 boundary and lossless partition invariants ──────────────

proptest! {
    #[test]
    fn tokenize_command_preserves_utf8_boundaries_and_partitions_input(input in ".*") {
        let tokens = tokenize_command(&input);

        if input.is_empty() {
            prop_assert!(tokens.is_empty());
        } else {
            prop_assert!(!tokens.is_empty());
            prop_assert_eq!(tokens[0].range.start, 0);
            prop_assert_eq!(tokens.last().unwrap().range.end, input.len());

            let mut reconstructed = String::with_capacity(input.len());
            let mut prev_end = 0usize;
            for token in &tokens {
                prop_assert_eq!(token.range.start, prev_end);
                prop_assert!(token.range.start < token.range.end);
                prop_assert!(token.range.end <= input.len());
                prop_assert!(input.is_char_boundary(token.range.start));
                prop_assert!(input.is_char_boundary(token.range.end));
                reconstructed.push_str(token.slice(&input));
                prev_end = token.range.end;
            }
            prop_assert_eq!(reconstructed, input);
        }
    }

    #[test]
    fn tokenize_command_handles_arbitrary_quotes_escapes_and_placeholders(
        parts in prop::collection::vec(
            prop_oneof![
                Just("%command%".to_string()),
                Just("\"".to_string()),
                Just("'".to_string()),
                Just("\\".to_string()),
                Just(" ".to_string()),
                Just("\t\n".to_string()),
                Just("--".to_string()),
                Just("gamescope".to_string()),
                Just("FOO=bar".to_string()),
                Just("日本語".to_string()),
                "[-a-zA-Z0-9_=./\"'\\\\ ]{0,16}",
            ],
            0..12
        )
    ) {
        let input = parts.concat();
        let tokens = tokenize_command(&input);
        let mut rebuilt = String::new();
        for tok in &tokens {
            prop_assert!(input.is_char_boundary(tok.range.start));
            prop_assert!(input.is_char_boundary(tok.range.end));
            rebuilt.push_str(tok.slice(&input));
        }
        prop_assert_eq!(rebuilt, input);
    }
}
