//! Per-session Tauri event relay: the only place that knows the Tauri event
//! names `launch-log`, `launch-diagnostic`, `launch-complete` and
//! `injection-log`, mapping service [`LaunchEvent`]s onto them and adding the
//! session id to each payload.

pub(crate) mod payloads;
#[cfg(test)]
mod tests;

use crosshook_app::log_tail::LogBatch;
use crosshook_app::services::launch::{LaunchEvent, LaunchEvents};
use crosshook_core::launch::DiagnosticReport;
use tauri::{AppHandle, Emitter};

use self::payloads::{
    suppression_summary_line, transform_launch_log_line_for_ui, InjectionLogEvent,
    LaunchCompletePayload, LaunchDiagnosticPayload, LaunchLogPayload, LaunchLogRelayState,
};

/// One wire emission derived from a [`LaunchEvent`]. Owned, so the borrowed
/// payload DTOs are built from it at emit time.
#[derive(Debug, Clone)]
pub(crate) enum RelayEmit {
    /// `launch-log` with `{line, session_id}`.
    Log { line: String, session_id: String },
    /// `launch-diagnostic`: the report plus `session_id`.
    Diagnostic {
        report: Box<DiagnosticReport>,
        session_id: String,
    },
    /// `launch-complete`: `{code, signal, session_id}`.
    Complete(LaunchCompletePayload),
    /// `injection-log`, unchanged shape.
    Injection(InjectionLogEvent),
}

impl RelayEmit {
    pub(crate) fn event_name(&self) -> &'static str {
        match self {
            Self::Log { .. } => "launch-log",
            Self::Diagnostic { .. } => "launch-diagnostic",
            Self::Complete(_) => "launch-complete",
            Self::Injection(_) => "injection-log",
        }
    }

    /// Serializes the wire payload (used by tests; `emit_one` emits the same
    /// DTOs without materializing a `Value`).
    #[cfg(test)]
    pub(crate) fn payload_value(&self) -> serde_json::Value {
        match self {
            Self::Log { line, session_id } => {
                serde_json::to_value(LaunchLogPayload { line, session_id })
                    .expect("log payload serializes")
            }
            Self::Diagnostic { report, session_id } => {
                serde_json::to_value(LaunchDiagnosticPayload { report, session_id })
                    .expect("diagnostic payload serializes")
            }
            Self::Complete(payload) => {
                serde_json::to_value(payload).expect("complete payload serializes")
            }
            Self::Injection(payload) => {
                serde_json::to_value(payload).expect("injection payload serializes")
            }
        }
    }
}

/// Pure mapping from a service event to its wire emissions.
///
/// `Log::Lines` becomes one [`RelayEmit::Log`] per non-empty line after the
/// relay transform (xdg_backend suppression per session); `Log::Dropped{n}`
/// becomes one visible dropped-batches line. The xdg_backend suppression
/// summary line is emitted right after the last log line, before the first
/// `Diagnostic` or `Injection` that follows any log line (D5 order); if
/// neither arrives, `Exited` emits it (if any) **then**
/// [`RelayEmit::Complete`]. It is emitted at most once per session.
pub(crate) fn map_event(state: &mut LaunchLogRelayState, event: LaunchEvent) -> Vec<RelayEmit> {
    match event {
        LaunchEvent::Log { session_id, batch } => {
            let session_id = session_id.to_string();
            match batch {
                LogBatch::Lines(lines) => lines
                    .iter()
                    .filter(|line| !line.is_empty())
                    .flat_map(|line| transform_launch_log_line_for_ui(state, line))
                    .map(|line| RelayEmit::Log {
                        line,
                        session_id: session_id.clone(),
                    })
                    .collect(),
                LogBatch::Dropped { n } => vec![RelayEmit::Log {
                    line: format!(
                        "[crosshook] Console skipped {n} log batches; the raw launch log is complete."
                    ),
                    session_id,
                }],
                LogBatch::Truncated { bytes } => vec![RelayEmit::Log {
                    line: format!(
                        "[crosshook] Console output truncated: about {bytes} bytes were not shown; the raw launch log is complete."
                    ),
                    session_id,
                }],
            }
        }
        LaunchEvent::Diagnostic { session_id, report } => {
            let session_id = session_id.to_string();
            let mut emits = pending_suppression_summary(state, &session_id);
            emits.push(RelayEmit::Diagnostic { report, session_id });
            emits
        }
        LaunchEvent::Injection { session_id, event } => {
            let mut emits = pending_suppression_summary(state, &session_id.to_string());
            emits.push(RelayEmit::Injection(InjectionLogEvent::from_data(
                session_id, &event,
            )));
            emits
        }
        LaunchEvent::Exited {
            session_id,
            code,
            signal,
        } => {
            let session_id = session_id.to_string();
            // Fallback: the summary normally precedes the first post-log
            // Diagnostic/Injection; emit it here when neither arrived.
            let mut emits = pending_suppression_summary(state, &session_id);
            emits.push(RelayEmit::Complete(LaunchCompletePayload {
                code,
                signal,
                session_id,
            }));
            emits
        }
    }
}

/// Emits the xdg_backend suppression summary line at most once per session,
/// so it lands right after the last log line and before the first
/// `Diagnostic`/`Injection` (D5 order); `Exited` is the fallback.
fn pending_suppression_summary(
    state: &mut LaunchLogRelayState,
    session_id: &str,
) -> Vec<RelayEmit> {
    if state.summary_emitted {
        return Vec::new();
    }
    // Only a returned summary consumes the once-per-session slot: trainer
    // sessions emit Injection rows before any log line, and suppression may
    // happen later, so `Exited` must still be able to emit it.
    let Some(line) = suppression_summary_line(state) else {
        return Vec::new();
    };
    state.summary_emitted = true;
    vec![RelayEmit::Log {
        line,
        session_id: session_id.to_string(),
    }]
}

fn emit_one(app: &AppHandle, emit: &RelayEmit) {
    let result = match emit {
        RelayEmit::Log { line, session_id } => {
            app.emit("launch-log", LaunchLogPayload { line, session_id })
        }
        RelayEmit::Diagnostic { report, session_id } => app.emit(
            "launch-diagnostic",
            LaunchDiagnosticPayload { report, session_id },
        ),
        RelayEmit::Complete(payload) => app.emit("launch-complete", payload),
        RelayEmit::Injection(payload) => app.emit("injection-log", payload),
    };
    if let Err(error) = result {
        tracing::warn!(%error, event = emit.event_name(), "failed to emit launch event");
    }
}

/// One relay task per session, draining the stream in order.
pub(crate) fn spawn(app: AppHandle, mut events: LaunchEvents) {
    tauri::async_runtime::spawn(async move {
        let mut state = LaunchLogRelayState::default();
        while let Some(event) = events.recv().await {
            for emit in map_event(&mut state, event) {
                emit_one(&app, &emit);
            }
        }
    });
}

/// Relays events published before a launch failed to start (pre-spawn
/// injection rows), so they are not lost with the `Err` return.
pub(crate) fn replay_start_error(app: &AppHandle, events: Vec<LaunchEvent>) {
    let mut state = LaunchLogRelayState::default();
    for event in events {
        for emit in map_event(&mut state, event) {
            emit_one(app, &emit);
        }
    }
}
