//! UI-agnostic launch-preview command highlighter.
//!
//! Direct Rust equivalent of Classic's `HighlightedCommandBlock.tsx::commandTokens`,
//! producing [`HighlightedSpan`]s tagged with [`CommandHighlightRole`] for native
//! UI widgets.

use crate::launch::preview::{EnvVarSource, LaunchPreview};

use super::CommandHighlightRole;

/// Maximum number of flag tokens extracted from `effective_command` in
/// [`highlight_launch_preview`] (matches Classic `MAX_FLAG_TOKENS`).
pub const MAX_PREVIEW_FLAG_TOKENS: usize = 8;

/// Maximum printable length of comment display fields in
/// [`highlight_launch_preview`] (matches Classic `MAX_DISPLAY_FIELD_LEN`).
pub const MAX_PREVIEW_DISPLAY_FIELD_LEN: usize = 64;

/// One highlighted display span produced by [`highlight_launch_preview`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightedSpan {
    /// Display text for this span.
    pub text: String,
    /// UI-agnostic theme role for styling.
    pub role: CommandHighlightRole,
}

/// Strictly matches Classic's `HighlightedCommandBlock.tsx::isFlagToken`
/// (`/^-{1,2}[a-zA-Z0-9][\w\-=.]*$/`, excluding `--`).
fn is_strict_preview_flag_token(token: &str) -> bool {
    if token == "--" {
        return false;
    }
    let rest = if let Some(after) = token.strip_prefix("--") {
        after
    } else if let Some(after) = token.strip_prefix('-') {
        after
    } else {
        return false;
    };
    let mut bytes = rest.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'=' | b'.'))
}

/// Strips characters outside printable ASCII (`0x20..=0x7E`) and caps to
/// [`MAX_PREVIEW_DISPLAY_FIELD_LEN`] characters with an ellipsis suffix
/// (matching Classic `HighlightedCommandBlock.tsx::sanitizeDisplayField`).
fn sanitize_display_field(value: &str) -> String {
    let printable: String = value
        .chars()
        .filter(|ch| ('\x20'..='\x7E').contains(ch))
        .collect();
    if printable.len() > MAX_PREVIEW_DISPLAY_FIELD_LEN {
        format!("{}…", &printable[..MAX_PREVIEW_DISPLAY_FIELD_LEN])
    } else {
        printable
    }
}

fn push_span(
    spans: &mut Vec<HighlightedSpan>,
    text: impl Into<String>,
    role: CommandHighlightRole,
) {
    spans.push(HighlightedSpan {
        text: text.into(),
        role,
    });
}

fn push_word_spans(spans: &mut Vec<HighlightedSpan>, words: &[String], role: CommandHighlightRole) {
    for word in words {
        let trimmed = word.trim();
        if trimmed.is_empty() {
            continue;
        }
        push_span(spans, trimmed, role);
        push_span(spans, " ", CommandHighlightRole::Comment);
    }
}

/// Produces UI-agnostic highlighted display spans for a [`LaunchPreview`],
/// matching Classic's `HighlightedCommandBlock.tsx::commandTokens`.
pub fn highlight_launch_preview(preview: &LaunchPreview) -> Vec<HighlightedSpan> {
    let mut spans = Vec::new();

    push_span(
        &mut spans,
        format!(
            "# {} launch preview",
            sanitize_display_field(preview.resolved_method.as_str())
        ),
        CommandHighlightRole::Comment,
    );

    let game_label_source = if !preview.game_executable_name.is_empty() {
        preview.game_executable_name.as_str()
    } else {
        preview.game_executable.as_str()
    };
    if !game_label_source.is_empty() {
        push_span(
            &mut spans,
            format!(" for {}", sanitize_display_field(game_label_source)),
            CommandHighlightRole::Comment,
        );
    }
    push_span(&mut spans, "\n", CommandHighlightRole::Comment);

    if let Some(environment) = &preview.environment {
        for env_var in environment
            .iter()
            .filter(|var| var.source == EnvVarSource::ProfileCustom)
        {
            push_span(&mut spans, &env_var.key, CommandHighlightRole::EnvKey);
            push_span(&mut spans, "=", CommandHighlightRole::Comment);
            let encoded = serde_json::to_string(&env_var.value)
                .unwrap_or_else(|_| format!("\"{}\"", env_var.value));
            push_span(&mut spans, encoded, CommandHighlightRole::Value);
            push_span(&mut spans, " ", CommandHighlightRole::Comment);
        }
    }

    if let Some(wrappers) = &preview.wrappers {
        push_word_spans(&mut spans, wrappers, CommandHighlightRole::Binary);
    }

    if let Some(proton) = preview
        .proton_setup
        .as_ref()
        .filter(|setup| !setup.proton_executable.is_empty())
    {
        push_span(
            &mut spans,
            &proton.proton_executable,
            CommandHighlightRole::Binary,
        );
        push_span(&mut spans, " run ", CommandHighlightRole::Flag);
    }

    push_span(
        &mut spans,
        &preview.game_executable,
        CommandHighlightRole::Binary,
    );

    if let Some(effective_command) = preview
        .effective_command
        .as_deref()
        .filter(|cmd| !cmd.is_empty())
    {
        let all_flags: Vec<String> = effective_command
            .split_whitespace()
            .filter(|part| is_strict_preview_flag_token(part))
            .map(ToOwned::to_owned)
            .collect();
        let truncated = all_flags.len() > MAX_PREVIEW_FLAG_TOKENS;
        let shown = if truncated {
            &all_flags[..MAX_PREVIEW_FLAG_TOKENS]
        } else {
            &all_flags[..]
        };
        if !shown.is_empty() {
            push_span(&mut spans, " ", CommandHighlightRole::Comment);
            push_word_spans(&mut spans, shown, CommandHighlightRole::Flag);
            if truncated {
                push_span(&mut spans, "…", CommandHighlightRole::Comment);
            }
        }
    }

    spans
}
