//! UI-agnostic command tokenizer and launch-preview command highlighter.
//!
//! Provides two complementary primitives shared by native UI widgets:
//!
//! 1. [`tokenize_command`]: a lossless, range-based lexer over arbitrary shell /
//!    Steam launch option command lines (`KEY=VAL`, wrappers, executables, flags,
//!    `%command%`, quoted strings with escapes / unterminated quotes, and
//!    whitespace). Every returned [`CommandToken::range`] lies on valid UTF-8
//!    char boundaries and partitions the input completely.
//! 2. [`highlight_launch_preview`]: direct Rust equivalent of Classic's
//!    `HighlightedCommandBlock.tsx::commandTokens`, producing UI-agnostic
//!    [`HighlightedSpan`]s with theme roles for a [`LaunchPreview`].

mod preview;

use std::ops::Range;

pub use preview::{
    highlight_launch_preview, HighlightedSpan, MAX_PREVIEW_DISPLAY_FIELD_LEN,
    MAX_PREVIEW_FLAG_TOKENS,
};

/// Semantic kind of one token produced by [`tokenize_command`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandTokenKind {
    /// Leading shell environment variable assignment (`KEY=VALUE`).
    EnvAssignment,
    /// Known wrapper command (`gamescope`, `mangohud`, `gamemoderun`, `umu-run`, `env`, `unshare`, …).
    Wrapper,
    /// Primary executable token (first non-env, non-wrapper command token).
    Executable,
    /// Option flag (`-f`, `--flag`, `--opt=value`), excluding bare `--` and `-=bad`.
    Flag,
    /// Steam launch options `%command%` placeholder.
    CommandPlaceholder,
    /// Single- or double-quoted string (including escaped or unterminated quotes).
    QuotedString,
    /// Contiguous whitespace run.
    Whitespace,
    /// Any other unquoted argument or separator (`--`, positional values, paths).
    Argument,
}

/// UI-agnostic theme role used by native widgets to color command tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandHighlightRole {
    /// Comment lines, punctuation, and truncation ellipsis.
    Comment,
    /// Environment variable key or assignment.
    EnvKey,
    /// Quoted string or environment value.
    Value,
    /// Wrapper or executable binary.
    Binary,
    /// Command-line flag.
    Flag,
    /// Steam `%command%` placeholder.
    Placeholder,
    /// Plain argument or whitespace.
    Plain,
}

impl CommandTokenKind {
    /// Maps a token kind to its UI-agnostic theme highlight role.
    pub const fn highlight_role(self) -> CommandHighlightRole {
        match self {
            Self::EnvAssignment => CommandHighlightRole::EnvKey,
            Self::Wrapper | Self::Executable => CommandHighlightRole::Binary,
            Self::Flag => CommandHighlightRole::Flag,
            Self::CommandPlaceholder => CommandHighlightRole::Placeholder,
            Self::QuotedString => CommandHighlightRole::Value,
            Self::Whitespace | Self::Argument => CommandHighlightRole::Plain,
        }
    }
}

/// Maps a [`CommandTokenKind`] to its UI-agnostic [`CommandHighlightRole`].
pub const fn highlight_role(kind: CommandTokenKind) -> CommandHighlightRole {
    kind.highlight_role()
}

/// A single token slice inside the source command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandToken {
    /// Semantic token classification.
    pub kind: CommandTokenKind,
    /// Byte range within the input string (guaranteed valid UTF-8 char boundaries).
    pub range: Range<usize>,
}

impl CommandToken {
    /// Returns the substring of `source` referenced by this token's range.
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        &source[self.range.clone()]
    }
}

/// Wrapper command parsing behavior while walking command-prefix tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WrapperBehavior {
    /// Immediately followed by the wrapped command (no wrapper arguments).
    Immediate,
    /// Accepts flags/args until `--`, after which command position resumes.
    UntilDoubleDash,
    /// Accepts flags (`-…`) before the wrapped command (e.g. `unshare --net`).
    FlagsBeforeCommand,
    /// Accepts `KEY=VAL` env assignments and flags before the wrapped command (`env`).
    EnvPrefix,
}

/// Position in the shell command prefix used to classify env assignments,
/// wrappers, and the primary executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandStage {
    /// Expecting leading `KEY=VAL`, wrappers, `%command%`, or the primary executable.
    CommandPrefix,
    /// Inside wrapper options until `--` resets back to [`Self::CommandPrefix`].
    WrapperUntilDoubleDash,
    /// Inside wrapper flags (`-…`); the first non-flag token is the wrapped command.
    WrapperFlagsBeforeCommand,
    /// Inside `env` arguments: `KEY=VAL` stays `EnvAssignment`, flags stay `Flag`,
    /// first other token is the wrapped command.
    EnvCommandPrefix,
    /// Past the primary executable (or `%command%`); remaining tokens are flags/args/strings.
    Arguments,
}

fn wrapper_behavior(token: &str) -> Option<WrapperBehavior> {
    let basename = token.rsplit('/').next().unwrap_or(token);
    match basename {
        "mangohud" | "gamemoderun" | "prime-run" | "obs-gamecapture" | "umu-run" => {
            Some(WrapperBehavior::Immediate)
        }
        "gamescope" => Some(WrapperBehavior::UntilDoubleDash),
        "unshare" => Some(WrapperBehavior::FlagsBeforeCommand),
        "env" => Some(WrapperBehavior::EnvPrefix),
        _ => None,
    }
}

/// Returns true when `name` is a valid POSIX shell variable name
/// (`[A-Za-z_][A-Za-z0-9_]*`).
fn is_valid_env_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Returns true when `token` begins with `VALID_NAME=`.
fn is_env_assignment(token: &str) -> bool {
    token
        .find('=')
        .is_some_and(|eq| is_valid_env_name(&token[..eq]))
}

/// Matches Classic's `isFlagToken`: `/^-{1,2}[a-zA-Z0-9][\w\-=.]*$/` excluding `--`.
///
/// Also accepts flags with a quoted value after `=` (e.g. `--cfg="my path"`).
pub(crate) fn is_flag_token(token: &str) -> bool {
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

    let mut chars = rest.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }

    if let Some((name, _value)) = rest.split_once('=') {
        return name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
    }

    rest.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'=' | b'.'))
}

/// Advances `idx` past a double-quoted segment starting at the opening `"` byte
/// offset. Handles `\"` and `\\` escapes and stops cleanly at EOF if
/// unterminated.
fn scan_double_quoted(input: &str, open_idx: usize) -> usize {
    let mut chars = input[open_idx..].char_indices();
    let _ = chars.next(); // consume opening '"'
    let mut escaped = false;
    for (rel, ch) in chars {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return open_idx + rel + ch.len_utf8();
        }
    }
    input.len()
}

/// Advances `idx` past a single-quoted segment starting at the opening `'` byte
/// offset. Stops cleanly at EOF if unterminated.
fn scan_single_quoted(input: &str, open_idx: usize) -> usize {
    let mut chars = input[open_idx..].char_indices();
    let _ = chars.next(); // consume opening '\''
    for (rel, ch) in chars {
        if ch == '\'' {
            return open_idx + rel + ch.len_utf8();
        }
    }
    input.len()
}

/// Scans one non-whitespace token starting at `start` (which must be a valid
/// UTF-8 char boundary on a non-whitespace character).
///
/// Returns `(end_byte_index, is_pure_quoted_string)`.
fn scan_word(input: &str, start: usize) -> (usize, bool) {
    let first = input[start..].chars().next().expect("non-empty slice");
    if first == '"' {
        let end = scan_double_quoted(input, start);
        // If immediately followed by non-whitespace, continue scanning the
        // compound shell word; otherwise it is a standalone quoted string.
        if end == input.len() || input[end..].chars().next().is_some_and(char::is_whitespace) {
            return (end, true);
        }
        return (scan_unquoted_tail(input, end), false);
    }
    if first == '\'' {
        let end = scan_single_quoted(input, start);
        if end == input.len() || input[end..].chars().next().is_some_and(char::is_whitespace) {
            return (end, true);
        }
        return (scan_unquoted_tail(input, end), false);
    }

    (scan_unquoted_tail(input, start), false)
}

/// Scans an unquoted word (or tail after a quote), honoring backslash escapes
/// and embedded single/double-quoted segments (e.g. `FOO="bar baz"` or
/// `--cfg="my file"`).
fn scan_unquoted_tail(input: &str, start: usize) -> usize {
    let mut idx = start;
    while idx < input.len() {
        let ch = input[idx..].chars().next().expect("valid char boundary");
        if ch.is_whitespace() {
            break;
        }
        match ch {
            '\\' => {
                idx += ch.len_utf8();
                if idx < input.len() {
                    let next = input[idx..].chars().next().expect("valid char boundary");
                    idx += next.len_utf8();
                }
            }
            '"' => {
                idx = scan_double_quoted(input, idx);
            }
            '\'' => {
                idx = scan_single_quoted(input, idx);
            }
            _ => {
                idx += ch.len_utf8();
            }
        }
    }
    idx
}

/// Tokenizes a launch command or Steam Launch Options string into lossless,
/// contiguous [`CommandToken`] spans.
///
/// Handles:
/// - Leading `KEY=VALUE` environment assignments (including quoted values)
/// - Known wrappers (`gamescope`, `mangohud`, `gamemoderun`, `umu-run`, `env`, `unshare`, …)
/// - Primary executable and subsequent flags/arguments
/// - Steam `%command%` placeholder
/// - Single- and double-quoted strings, escaped quotes (`\"`), and unterminated quotes
/// - Contiguous whitespace runs
///
/// Every returned token range is guaranteed to lie on valid UTF-8 char
/// boundaries and concatenating all token slices reproduces `input` verbatim.
pub fn tokenize_command(input: &str) -> Vec<CommandToken> {
    let mut tokens = Vec::new();
    let mut stage = CommandStage::CommandPrefix;
    let mut idx = 0;

    while idx < input.len() {
        let ch = input[idx..].chars().next().expect("valid char boundary");
        if ch.is_whitespace() {
            let start = idx;
            while idx < input.len() {
                let next = input[idx..].chars().next().expect("valid char boundary");
                if !next.is_whitespace() {
                    break;
                }
                idx += next.len_utf8();
            }
            tokens.push(CommandToken {
                kind: CommandTokenKind::Whitespace,
                range: start..idx,
            });
            continue;
        }

        let start = idx;
        let (end, is_quoted) = scan_word(input, start);
        let text = &input[start..end];
        idx = end;

        let kind = classify_token(text, is_quoted, &mut stage);
        tokens.push(CommandToken {
            kind,
            range: start..end,
        });
    }

    tokens
}

fn classify_token(text: &str, is_quoted: bool, stage: &mut CommandStage) -> CommandTokenKind {
    if text == "%command%" {
        *stage = CommandStage::Arguments;
        return CommandTokenKind::CommandPlaceholder;
    }

    if is_quoted {
        if matches!(
            stage,
            CommandStage::CommandPrefix
                | CommandStage::WrapperFlagsBeforeCommand
                | CommandStage::EnvCommandPrefix
        ) {
            *stage = CommandStage::Arguments;
        }
        return CommandTokenKind::QuotedString;
    }

    match *stage {
        CommandStage::CommandPrefix => classify_command_head(text, stage, true),
        CommandStage::EnvCommandPrefix => {
            if is_flag_token(text) {
                CommandTokenKind::Flag
            } else if text == "--" {
                *stage = CommandStage::CommandPrefix;
                CommandTokenKind::Argument
            } else {
                classify_command_head(text, stage, true)
            }
        }
        CommandStage::WrapperUntilDoubleDash => {
            if text == "--" {
                *stage = CommandStage::CommandPrefix;
                CommandTokenKind::Argument
            } else if is_flag_token(text) {
                CommandTokenKind::Flag
            } else {
                CommandTokenKind::Argument
            }
        }
        CommandStage::WrapperFlagsBeforeCommand => {
            if text == "--" {
                *stage = CommandStage::CommandPrefix;
                CommandTokenKind::Argument
            } else if is_flag_token(text) {
                CommandTokenKind::Flag
            } else {
                classify_command_head(text, stage, false)
            }
        }
        CommandStage::Arguments => {
            if is_flag_token(text) {
                CommandTokenKind::Flag
            } else {
                CommandTokenKind::Argument
            }
        }
    }
}

fn classify_command_head(
    text: &str,
    stage: &mut CommandStage,
    allow_env_assignment: bool,
) -> CommandTokenKind {
    if allow_env_assignment && is_env_assignment(text) {
        return CommandTokenKind::EnvAssignment;
    }

    if let Some(behavior) = wrapper_behavior(text) {
        *stage = match behavior {
            WrapperBehavior::Immediate => CommandStage::CommandPrefix,
            WrapperBehavior::UntilDoubleDash => CommandStage::WrapperUntilDoubleDash,
            WrapperBehavior::FlagsBeforeCommand => CommandStage::WrapperFlagsBeforeCommand,
            WrapperBehavior::EnvPrefix => CommandStage::EnvCommandPrefix,
        };
        return CommandTokenKind::Wrapper;
    }

    if is_flag_token(text) {
        *stage = CommandStage::Arguments;
        return CommandTokenKind::Flag;
    }

    *stage = CommandStage::Arguments;
    CommandTokenKind::Executable
}
