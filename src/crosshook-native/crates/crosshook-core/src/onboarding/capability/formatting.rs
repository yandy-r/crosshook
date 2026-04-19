//! Formatting utilities for capability messages.

use crate::onboarding::HostToolCheckResult;

pub(super) fn format_tool_list(tool_checks: &[HostToolCheckResult]) -> String {
    let labels = tool_checks
        .iter()
        .map(|check| check.display_name.as_str())
        .collect::<Vec<_>>();
    join_list(&labels)
}

pub(super) fn join_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_string(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let mut combined = items[..items.len() - 1].join(", ");
            combined.push_str(", and ");
            combined.push_str(items[items.len() - 1]);
            combined
        }
    }
}
