//! Unified diff helpers for config revision snapshots.

use similar::{ChangeTag, TextDiff};

/// Context lines around each hunk in unified diff output.
pub const DIFF_CONTEXT_LINES: usize = 3;
/// Maximum lines per side considered for the diff.
pub const DIFF_MAX_LINES: usize = 2000;
/// Maximum byte length of diff output returned across the IPC boundary.
pub const MAX_DIFF_OUTPUT_BYTES: usize = 512 * 1024;

/// Result of a unified diff between two text snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedDiffResult {
    pub diff_text: String,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub truncated: bool,
}

/// Compute a unified diff between two text strings using the `similar` crate.
/// Returns empty `diff_text` when inputs are identical. Sets `truncated` when
/// either side exceeds [`DIFF_MAX_LINES`].
pub fn compute_unified_diff(
    old_label: &str,
    new_label: &str,
    old_text: &str,
    new_text: &str,
) -> UnifiedDiffResult {
    if old_text == new_text {
        return UnifiedDiffResult {
            diff_text: String::new(),
            added_lines: 0,
            removed_lines: 0,
            truncated: false,
        };
    }

    let old_total = old_text.lines().count();
    let new_total = new_text.lines().count();
    let truncated = old_total > DIFF_MAX_LINES || new_total > DIFF_MAX_LINES;

    let old_truncated = truncate_lines(old_text, DIFF_MAX_LINES);
    let new_truncated = truncate_lines(new_text, DIFF_MAX_LINES);

    let diff = TextDiff::from_lines(&old_truncated, &new_truncated);

    let mut added_lines = 0usize;
    let mut removed_lines = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added_lines += 1,
            ChangeTag::Delete => removed_lines += 1,
            ChangeTag::Equal => {}
        }
    }

    let diff_text = diff
        .unified_diff()
        .context_radius(DIFF_CONTEXT_LINES)
        .header(old_label, new_label)
        .to_string();

    UnifiedDiffResult {
        diff_text,
        added_lines,
        removed_lines,
        truncated,
    }
}

/// Truncate text to at most `max_lines` lines, preserving content without a
/// trailing newline on the last retained line.
fn truncate_lines(text: &str, max_lines: usize) -> String {
    text.lines().take(max_lines).collect::<Vec<_>>().join("\n")
}

/// Apply the IPC byte cap to unified diff output, marking truncated when cut.
pub fn cap_diff_output_bytes(mut diff_text: String, mut truncated: bool) -> (String, bool) {
    if diff_text.len() > MAX_DIFF_OUTPUT_BYTES {
        let mut truncate_at = MAX_DIFF_OUTPUT_BYTES;
        while truncate_at > 0 && !diff_text.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }
        diff_text.truncate(truncate_at);
        truncated = true;
    }
    (diff_text, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_input_produces_empty_diff() {
        let text = "[game]\nname = \"Test\"\n";
        let result = compute_unified_diff("old", "new", text, text);
        assert_eq!(result.diff_text, "");
        assert_eq!(result.added_lines, 0);
        assert_eq!(result.removed_lines, 0);
        assert!(!result.truncated);
    }

    #[test]
    fn detects_added_and_removed_lines() {
        let old = "a\nb\nc\n";
        let new = "a\nx\nc\n";
        let result = compute_unified_diff("old", "new", old, new);
        assert!(result.added_lines >= 1);
        assert!(result.removed_lines >= 1);
        assert!(result.diff_text.contains("--- old"));
        assert!(result.diff_text.contains("+++ new"));
    }

    #[test]
    fn truncation_flag_when_input_exceeds_line_limit() {
        let old = (0..DIFF_MAX_LINES + 5)
            .map(|i| format!("old-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let new = old.clone();
        let mut new_with_change = new.clone();
        new_with_change.push_str("\nchanged\n");
        let result = compute_unified_diff("old", "new", &old, &new_with_change);
        assert!(result.truncated);
    }

    #[test]
    fn byte_cap_truncates_oversized_output() {
        let huge = "+\n".repeat(MAX_DIFF_OUTPUT_BYTES / 2 + 1);
        let (capped, truncated) = cap_diff_output_bytes(huge, false);
        assert!(truncated);
        assert!(capped.len() <= MAX_DIFF_OUTPUT_BYTES);
    }
}
