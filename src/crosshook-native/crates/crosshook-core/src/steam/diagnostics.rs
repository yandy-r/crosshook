use std::collections::HashSet;

use tracing::info;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticCollector {
    pub diagnostics: Vec<String>,
    pub manual_hints: Vec<String>,
}

impl DiagnosticCollector {
    pub fn add_diagnostic(&mut self, message: impl Into<String>) {
        let message = message.into();
        info!(diagnostic = %message, "steam diagnostic");
        self.diagnostics.push(message);
    }

    pub fn add_hint(&mut self, message: impl Into<String>) {
        let message = message.into();
        info!(hint = %message, "steam manual hint");
        self.manual_hints.push(message);
    }

    pub fn finalize(self) -> (Vec<String>, Vec<String>) {
        (
            dedupe_preserving_order(self.diagnostics),
            dedupe_preserving_order(self.manual_hints),
        )
    }
}

fn dedupe_preserving_order(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::with_capacity(values.len());

    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::DiagnosticCollector;

    #[test]
    fn deduplicates_while_preserving_order() {
        let mut collector = DiagnosticCollector::default();
        collector.add_diagnostic("one");
        collector.add_diagnostic("two");
        collector.add_diagnostic("one");
        collector.add_hint("alpha");
        collector.add_hint("beta");
        collector.add_hint("alpha");

        let (diagnostics, manual_hints) = collector.finalize();

        assert_eq!(diagnostics, vec!["one".to_string(), "two".to_string()]);
        assert_eq!(manual_hints, vec!["alpha".to_string(), "beta".to_string()]);
    }
}
