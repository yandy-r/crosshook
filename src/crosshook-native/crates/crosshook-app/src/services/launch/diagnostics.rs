//! Bounded log tail reads and diagnostic report helpers.

use std::io::SeekFrom;
use std::path::Path;

use crosshook_core::launch::{DiagnosticReport, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::presenter::display_path_tilde;

/// Reads at most the last `max_bytes` of the file at `path`, decoded lossily.
///
/// When the file is larger than `max_bytes` the read starts at
/// `End(-max_bytes)`, so the first character can be cut and decode as U+FFFD.
/// Any I/O error is logged (without the path) and yields an empty string, so
/// callers never fail a launch over an unreadable log.
pub async fn read_log_tail(path: &Path, max_bytes: u64) -> String {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) => {
            tracing::warn!(%error, "failed to open launch log tail");
            return String::new();
        }
    };

    let len = match file.metadata().await {
        Ok(metadata) => metadata.len(),
        Err(error) => {
            tracing::warn!(%error, "failed to read launch log metadata");
            return String::new();
        }
    };

    if len > max_bytes {
        // `max_bytes < len <= i64::MAX`, so the conversion cannot fail.
        let Ok(back) = i64::try_from(max_bytes) else {
            return String::new();
        };
        if let Err(error) = file.seek(SeekFrom::End(-back)).await {
            tracing::warn!(%error, "failed to seek launch log tail");
            return String::new();
        }
    }

    let mut buffer = Vec::new();
    if let Err(error) = file.take(max_bytes).read_to_end(&mut buffer).await {
        tracing::warn!(%error, "failed to read launch log tail");
        return String::new();
    }

    String::from_utf8_lossy(&buffer).into_owned()
}

/// Rewrites home-directory prefixes in every user-visible string of the
/// report to `~/`.
///
/// `log_tail_path` is rewritten too, so callers must set it from the raw log
/// path *before* sanitizing; the UI matches it against the tilde form.
pub(super) fn sanitize_diagnostic_report(mut report: DiagnosticReport) -> DiagnosticReport {
    report.summary = display_path_tilde(&report.summary);
    report.exit_info.description = display_path_tilde(&report.exit_info.description);
    report.launch_method = display_path_tilde(&report.launch_method);
    report.log_tail_path = report.log_tail_path.as_deref().map(display_path_tilde);

    for pattern_match in &mut report.pattern_matches {
        pattern_match.summary = display_path_tilde(&pattern_match.summary);
        pattern_match.suggestion = display_path_tilde(&pattern_match.suggestion);
        pattern_match.matched_line = pattern_match
            .matched_line
            .as_deref()
            .map(display_path_tilde);
    }

    for suggestion in &mut report.suggestions {
        suggestion.title = display_path_tilde(&suggestion.title);
        suggestion.description = display_path_tilde(&suggestion.description);
    }

    report
}

/// A Steam trainer leg that the helper delegated to the direct-Proton runner
/// is diagnosed as `proton_run`; every other method is kept.
pub(super) fn diagnostic_method_for_log(method: &'static str, log_tail: &str) -> &'static str {
    if method == METHOD_STEAM_APPLAUNCH
        && log_tail.contains("[steam-trainer-runner]")
        && log_tail.contains("trainer_launch_mode=")
    {
        METHOD_PROTON_RUN
    } else {
        method
    }
}
