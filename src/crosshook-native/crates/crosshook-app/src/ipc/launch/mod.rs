//! Launch wire contract: the `launch_game`/`launch_trainer` result and the
//! pure mapping of service [`LaunchEvent`]s onto the `launch-log`,
//! `launch-diagnostic`, `launch-complete` and `injection-log` events, adding
//! the session id to each payload.

pub mod payloads;
mod result;
#[cfg(test)]
mod tests;

use crate::log_tail::LogBatch;
use crate::services::launch::LaunchEvent;
use crosshook_core::launch::DiagnosticReport;

pub use self::payloads::LaunchLogRelayState;
use self::payloads::{
    suppression_summary_line, transform_launch_log_line_for_ui, InjectionLogEvent,
    LaunchCompletePayload, LaunchDiagnosticPayload, LaunchLogPayload,
};
pub use self::result::LaunchResult;
use super::WireEvent;

/// `launch-log` event name.
pub const LAUNCH_LOG_EVENT: &str = "launch-log";
/// `launch-diagnostic` event name.
pub const LAUNCH_DIAGNOSTIC_EVENT: &str = "launch-diagnostic";
/// `launch-complete` event name.
pub const LAUNCH_COMPLETE_EVENT: &str = "launch-complete";
/// `injection-log` event name.
pub const INJECTION_LOG_EVENT: &str = "injection-log";

/// One wire emission derived from a [`LaunchEvent`]. Owned, so the borrowed
/// payload DTOs are built from it at serialization time.
#[derive(Debug, Clone)]
pub enum RelayEmit {
    /// `launch-log` with `{line, session_id}`.
    Log {
        /// The console line.
        line: String,
        /// Owning session.
        session_id: String,
    },
    /// `launch-diagnostic`: the report plus `session_id`.
    Diagnostic {
        /// The diagnostic report.
        report: Box<DiagnosticReport>,
        /// Owning session.
        session_id: String,
    },
    /// `launch-complete`: `{code, signal, session_id}`.
    Complete(LaunchCompletePayload),
    /// `injection-log`, unchanged shape.
    Injection(InjectionLogEvent),
}

impl RelayEmit {
    /// The Tauri event name this emission goes to.
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::Log { .. } => LAUNCH_LOG_EVENT,
            Self::Diagnostic { .. } => LAUNCH_DIAGNOSTIC_EVENT,
            Self::Complete(_) => LAUNCH_COMPLETE_EVENT,
            Self::Injection(_) => INJECTION_LOG_EVENT,
        }
    }

    /// Serializes the emission into its named wire event.
    pub fn to_wire(&self) -> Option<WireEvent> {
        let name = self.event_name();
        match self {
            Self::Log { line, session_id } => {
                WireEvent::new(name, &LaunchLogPayload { line, session_id })
            }
            Self::Diagnostic { report, session_id } => {
                WireEvent::new(name, &LaunchDiagnosticPayload { report, session_id })
            }
            Self::Complete(payload) => WireEvent::new(name, payload),
            Self::Injection(payload) => WireEvent::new(name, payload),
        }
    }

    /// The wire payload as a JSON value (tests).
    #[cfg(test)]
    pub(crate) fn payload_value(&self) -> serde_json::Value {
        self.to_wire().expect("payload serializes").value()
    }
}

/// The wire events one service event produces, in emission order.
pub fn wire_events(state: &mut LaunchLogRelayState, event: LaunchEvent) -> Vec<WireEvent> {
    map_event(state, event)
        .iter()
        .filter_map(RelayEmit::to_wire)
        .collect()
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
pub fn map_event(state: &mut LaunchLogRelayState, event: LaunchEvent) -> Vec<RelayEmit> {
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
