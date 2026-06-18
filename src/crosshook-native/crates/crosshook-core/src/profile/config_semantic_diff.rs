//! TOML-aware semantic diff for config revision snapshots.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use toml::Value;

/// Maximum semantic changes returned in a single diff response.
pub const MAX_SEMANTIC_CHANGES: usize = 500;

/// Kind of semantic change at a dotted TOML path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticChangeKind {
    Added,
    Removed,
    Changed,
}

/// A single field-level change between two parsed TOML snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticChange {
    pub path: String,
    pub change_type: SemanticChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<String>,
}

/// Result of a semantic diff operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDiffResult {
    pub changes: Vec<SemanticChange>,
    pub truncated: bool,
    pub parse_failed: bool,
}

/// Compare two TOML snapshot strings semantically. On parse failure returns
/// `parse_failed = true` with an empty change list so callers can fall back to
/// unified diff output.
pub fn compute_semantic_diff(old_text: &str, new_text: &str) -> SemanticDiffResult {
    let old_value = match toml::from_str::<Value>(old_text) {
        Ok(v) => v,
        Err(_) => {
            return SemanticDiffResult {
                changes: Vec::new(),
                truncated: false,
                parse_failed: true,
            };
        }
    };
    let new_value = match toml::from_str::<Value>(new_text) {
        Ok(v) => v,
        Err(_) => {
            return SemanticDiffResult {
                changes: Vec::new(),
                truncated: false,
                parse_failed: true,
            };
        }
    };

    if old_value == new_value {
        return SemanticDiffResult {
            changes: Vec::new(),
            truncated: false,
            parse_failed: false,
        };
    }

    let mut changes = Vec::new();
    collect_changes("", &old_value, &new_value, &mut changes);
    changes.sort_by(|a, b| a.path.cmp(&b.path));

    let truncated = changes.len() > MAX_SEMANTIC_CHANGES;
    if truncated {
        changes.truncate(MAX_SEMANTIC_CHANGES);
    }

    SemanticDiffResult {
        changes,
        truncated,
        parse_failed: false,
    }
}

fn collect_changes(path: &str, old: &Value, new: &Value, out: &mut Vec<SemanticChange>) {
    if old == new {
        return;
    }

    match (old, new) {
        (Value::Table(old_table), Value::Table(new_table)) => {
            let keys: BTreeSet<&String> = old_table.keys().chain(new_table.keys()).collect();
            for key in keys {
                let child_path = join_path(path, key);
                match (old_table.get(key), new_table.get(key)) {
                    (None, Some(new_val)) => out.push(SemanticChange {
                        path: child_path,
                        change_type: SemanticChangeKind::Added,
                        old_value: None,
                        new_value: Some(value_to_display(new_val)),
                    }),
                    (Some(old_val), None) => out.push(SemanticChange {
                        path: child_path,
                        change_type: SemanticChangeKind::Removed,
                        old_value: Some(value_to_display(old_val)),
                        new_value: None,
                    }),
                    (Some(old_val), Some(new_val)) => {
                        collect_changes(&child_path, old_val, new_val, out);
                    }
                    (None, None) => {}
                }
            }
        }
        _ => out.push(SemanticChange {
            path: path.to_string(),
            change_type: SemanticChangeKind::Changed,
            old_value: Some(value_to_display(old)),
            new_value: Some(value_to_display(new)),
        }),
    }
}

fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

fn value_to_display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => toml::to_string(other)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorder_only_keys_produces_no_changes() {
        let a = "[game]\nname = \"Test\"\n[launch]\nmethod = \"proton_run\"\n";
        let b = "[launch]\nmethod = \"proton_run\"\n[game]\nname = \"Test\"\n";
        let result = compute_semantic_diff(a, b);
        assert!(!result.parse_failed);
        assert!(result.changes.is_empty());
    }

    #[test]
    fn detects_changed_scalar_field() {
        let old = "[game]\nname = \"Before\"\n";
        let new = "[game]\nname = \"After\"\n";
        let result = compute_semantic_diff(old, new);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].path, "game.name");
        assert_eq!(result.changes[0].change_type, SemanticChangeKind::Changed);
        assert_eq!(result.changes[0].old_value.as_deref(), Some("Before"));
        assert_eq!(result.changes[0].new_value.as_deref(), Some("After"));
    }

    #[test]
    fn detects_added_and_removed_sections() {
        let old = "[game]\nname = \"Test\"\n";
        let new = "[game]\nname = \"Test\"\n[trainer]\npath = \"trainer.exe\"\n";
        let result = compute_semantic_diff(old, new);
        assert!(result
            .changes
            .iter()
            .any(|c| c.path == "trainer" && c.change_type == SemanticChangeKind::Added));
    }

    #[test]
    fn parse_failure_sets_flag() {
        let result = compute_semantic_diff("not valid toml {{{", "[game]\nname = \"ok\"\n");
        assert!(result.parse_failed);
        assert!(result.changes.is_empty());
    }
}
