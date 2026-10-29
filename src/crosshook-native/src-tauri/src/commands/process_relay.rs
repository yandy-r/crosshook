//! Shared helpers for relaying a process job's log stream to IPC events.

use crosshook_app::log_tail::LogBatch;

/// The event lines one log batch relays: the non-empty lines of `Lines`, or
/// one `[crosshook]` notice line for a lag (`Dropped`) or a cut-short final
/// drain (`Truncated`). The log file itself always stays complete.
pub(crate) fn batch_lines(batch: LogBatch) -> Vec<String> {
    match batch {
        LogBatch::Lines(lines) => lines.into_iter().filter(|line| !line.is_empty()).collect(),
        LogBatch::Dropped { n } => vec![format!(
            "[crosshook] Output skipped {n} log batches; the log file is complete."
        )],
        LogBatch::Truncated { bytes } => vec![format!(
            "[crosshook] Output truncated: about {bytes} bytes were not shown; the log file is complete."
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_keep_order_and_drop_empty_lines() {
        let batch = LogBatch::Lines(vec!["a".into(), String::new(), "b".into()]);
        assert_eq!(batch_lines(batch), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn dropped_maps_to_one_notice_line() {
        assert_eq!(
            batch_lines(LogBatch::Dropped { n: 7 }),
            vec!["[crosshook] Output skipped 7 log batches; the log file is complete.".to_string()]
        );
    }

    #[test]
    fn truncated_maps_to_one_notice_line() {
        assert_eq!(
            batch_lines(LogBatch::Truncated { bytes: 4096 }),
            vec![
                "[crosshook] Output truncated: about 4096 bytes were not shown; the log file is complete."
                    .to_string()
            ]
        );
    }
}
