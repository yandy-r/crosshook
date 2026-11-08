//! Wire mapping for process-job relays (run-executable, update, prefix
//! dependencies): the shared log-line mapping and one module per flow.

pub mod prefix_deps;
pub mod run_executable;
pub mod update;

use crate::log_tail::LogBatch;

/// The event lines one log batch relays: the non-empty lines of `Lines`, or
/// one `[crosshook]` notice line for a lag (`Dropped`) or a cut-short final
/// drain (`Truncated`). The log file itself always stays complete.
pub fn batch_lines(batch: LogBatch) -> Vec<String> {
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
mod tests;
