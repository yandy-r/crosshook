use std::path::Path;

use crosshook_core::launch::{DiagnosticReport, METHOD_PROTON_RUN, METHOD_STEAM_APPLAUNCH};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::commands::shared::sanitize_display_path;

pub(crate) async fn safe_read_tail(path: &Path, max_bytes: u64) -> String {
    let file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "failed to open launch log tail");
            return String::new();
        }
    };

    let metadata = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "failed to read launch log metadata");
            return String::new();
        }
    };

    let mut file = file;
    if metadata.len() > max_bytes {
        let offset = -(max_bytes as i64);
        if let Err(error) = file.seek(std::io::SeekFrom::End(offset)).await {
            tracing::warn!(%error, path = %path.display(), "failed to seek launch log tail");
            return String::new();
        }
    }

    let mut buffer = Vec::new();
    if let Err(error) = file.read_to_end(&mut buffer).await {
        tracing::warn!(%error, path = %path.display(), "failed to read launch log tail");
        return String::new();
    }

    String::from_utf8_lossy(&buffer).into_owned()
}

pub(crate) fn sanitize_diagnostic_report(mut report: DiagnosticReport) -> DiagnosticReport {
    report.summary = sanitize_display_path(&report.summary);
    report.exit_info.description = sanitize_display_path(&report.exit_info.description);
    report.launch_method = sanitize_display_path(&report.launch_method);
    report.log_tail_path = report.log_tail_path.as_deref().map(sanitize_display_path);

    for pattern_match in &mut report.pattern_matches {
        pattern_match.summary = sanitize_display_path(&pattern_match.summary);
        pattern_match.suggestion = sanitize_display_path(&pattern_match.suggestion);
        pattern_match.matched_line = pattern_match
            .matched_line
            .as_deref()
            .map(sanitize_display_path);
    }

    for suggestion in &mut report.suggestions {
        suggestion.title = sanitize_display_path(&suggestion.title);
        suggestion.description = sanitize_display_path(&suggestion.description);
    }

    report
}

pub(crate) fn diagnostic_method_for_log(method: &'static str, log_tail: &str) -> &'static str {
    if method == METHOD_STEAM_APPLAUNCH
        && log_tail.contains("[steam-trainer-runner]")
        && log_tail.contains("trainer_launch_mode=")
    {
        METHOD_PROTON_RUN
    } else {
        method
    }
}
