//! Drop-safe session finalizer (`SessionGuard`) and the D5 step order.
//!
//! [`SessionGuard::finish`] publishes diagnostics and the trainer injection
//! wrap-up before `Exited`, then cascades, runs post-exit hooks, deregisters,
//! and unregisters the portal. If the session task panics or is dropped
//! first, [`Drop`] still runs the teardown steps that had not finished
//! (`Exited`, cascade, hooks, deregister, portal). Earlier steps are not
//! replayed, and `Exited` is published at most once.

use std::os::unix::process::ExitStatusExt;
use std::panic::AssertUnwindSafe;
use std::process::ExitStatus;
use std::sync::Arc;

use crosshook_core::launch::diagnostics::{FailureMode, MAX_LOG_TAIL_BYTES};
use crosshook_core::launch::{
    analyze, run_post_exit_hooks, should_surface_report, DiagnosticReport,
    LaunchHookExecutionContext, SessionKind, TeardownReason, ValidationSeverity,
};
use crosshook_core::profile::LaunchHook;
use tokio::runtime::Handle;

use super::diagnostics::{diagnostic_method_for_log, read_log_tail, sanitize_diagnostic_report};
use super::run_blocking;
use super::snapshot::record_success_effects;
use super::types::{
    EventPublisher, InjectionLevel, InjectionLogData, InjectionSource, SessionContext,
};
use crate::presenter::display_path_tilde;

/// Owns one session's post-exit work and finishes it on drop.
///
/// The session task holds the guard from its first line. [`Self::finish`]
/// runs the full sequence; unwinding or dropping the task early still runs
/// the remaining teardown steps.
pub(super) struct SessionGuard {
    ctx: SessionContext,
    publisher: EventPublisher,
    exited: bool,
    cascaded: bool,
    hooks_ran: bool,
    deregistered: bool,
    portal_unregistered: bool,
}

impl SessionGuard {
    pub(super) fn new(ctx: SessionContext, publisher: EventPublisher) -> Self {
        Self {
            ctx,
            publisher,
            exited: false,
            cascaded: false,
            hooks_ran: false,
            deregistered: false,
            portal_unregistered: false,
        }
    }

    /// Runs finalizer steps 1–13. `status` is the child exit status, or
    /// `None` when the process could not be waited on.
    pub(super) async fn finish(mut self, status: Option<ExitStatus>) {
        let exit_code = status.as_ref().and_then(ExitStatus::code);
        let signal = status.as_ref().and_then(ExitStatusExt::signal);
        let report = self.build_report(status).await;

        self.record_finished(exit_code, signal, &report).await;
        if self.success_effects_apply(&report) {
            record_success_effects(&self.ctx).await;
        }
        if should_surface_report(&report) {
            self.publisher.diagnostic(report.clone());
        }
        self.publish_trainer_wrap_up(&report);
        self.publish_exited(exit_code, signal);
        self.cascade();
        self.run_hooks().await;
        self.deregister();
        self.unregister_portal().await;
    }

    async fn build_report(&self, status: Option<ExitStatus>) -> DiagnosticReport {
        let log_tail = read_log_tail(&self.ctx.log_path, MAX_LOG_TAIL_BYTES).await;
        let method = diagnostic_method_for_log(self.ctx.method, &log_tail);
        let mut report = analyze(status, &log_tail, method);
        report.log_tail_path = Some(display_path_tilde(&self.ctx.log_path.to_string_lossy()));
        let mut report = sanitize_diagnostic_report(report);

        if self.ctx.watchdog_outcome.was_killed() {
            let message = match self.ctx.kind {
                SessionKind::Trainer => "Trainer exited; gamescope compositor cleaned up.",
                SessionKind::Game => "Game exited; gamescope compositor cleaned up.",
            };
            report.exit_info.failure_mode = FailureMode::CleanExit;
            report.summary = message.to_string();
            report.exit_info.description = message.to_string();
            report.exit_info.severity = ValidationSeverity::Info;
            report.suggestions.clear();
        }

        report.teardown_reason = self
            .ctx
            .watchdog_outcome
            .reason()
            .or(Some(TeardownReason::NaturalExit));
        report.coexistence_advisories = self.ctx.coexistence.clone();
        report
    }

    fn success_effects_apply(&self, report: &DiagnosticReport) -> bool {
        self.ctx.profile_name.is_some()
            && matches!(
                report.exit_info.failure_mode,
                FailureMode::CleanExit | FailureMode::Indeterminate
            )
    }

    /// History keeps the raw exit code and signal. The outcome column follows
    /// the report, including a watchdog rewrite to a clean exit.
    async fn record_finished(
        &self,
        exit_code: Option<i32>,
        signal: Option<i32>,
        report: &DiagnosticReport,
    ) {
        let Some(operation_id) = self.ctx.operation_id.clone() else {
            return;
        };
        let metadata = self.ctx.metadata.clone();
        let report = report.clone();
        let operation_for_log = operation_id.clone();
        let joined = run_blocking(move || {
            if let Err(error) =
                metadata.record_launch_finished(&operation_id, exit_code, signal, &report)
            {
                tracing::warn!(
                    %error,
                    operation_id = %operation_id,
                    "record_launch_finished failed"
                );
            }
            Ok(())
        })
        .await;
        if let Err(error) = joined {
            tracing::warn!(
                %error,
                operation_id = %operation_for_log,
                "record_launch_finished join failed"
            );
        }
    }

    fn publish_trainer_wrap_up(&self, report: &DiagnosticReport) {
        if self.ctx.kind != SessionKind::Trainer {
            return;
        }
        let profile = self.ctx.profile_name.as_deref();
        let kind = self.ctx.kind;
        if report
            .teardown_reason
            .is_some_and(|reason| reason != TeardownReason::NaturalExit)
        {
            self.publisher.injection(InjectionLogData::new(
                profile,
                kind,
                InjectionLevel::Warning,
                InjectionSource::Runtime,
                "Trainer session teardown was recorded.",
                false,
            ));
        }

        let natural_clean = report.teardown_reason == Some(TeardownReason::NaturalExit)
            && report.exit_info.failure_mode == FailureMode::CleanExit;
        let (level, source, message) = if natural_clean {
            (
                InjectionLevel::Info,
                InjectionSource::Trainer,
                "Trainer launch completed.",
            )
        } else if report.exit_info.failure_mode == FailureMode::Indeterminate {
            (
                InjectionLevel::Warning,
                InjectionSource::Runtime,
                "Trainer launch outcome is indeterminate.",
            )
        } else {
            (
                InjectionLevel::Error,
                InjectionSource::Trainer,
                "Trainer launch ended with diagnostics.",
            )
        };
        self.publisher
            .injection(InjectionLogData::new(profile, kind, level, source, message, false));
    }

    fn publish_exited(&mut self, code: Option<i32>, signal: Option<i32>) {
        if self.exited {
            return;
        }
        self.exited = true;
        self.publisher.exited(code, signal);
    }

    fn cascade(&mut self) {
        if self.cascaded {
            return;
        }
        if self.ctx.kind == SessionKind::Game {
            let signalled = self.ctx.registry.cancel_linked_children(
                self.ctx.session_id,
                TeardownReason::LinkedSessionExit,
            );
            if signalled > 0 {
                tracing::info!(
                    game_session_id = %self.ctx.session_id,
                    linked_trainers = signalled,
                    "launch session: cascading LinkedSessionExit to trainer children"
                );
            }
        }
        self.cascaded = true;
    }

    async fn run_hooks(&mut self) {
        if self.hooks_ran {
            return;
        }
        self.hooks_ran = true;
        let hooks = self.ctx.post_exit_hooks.clone();
        let hook_context = self.ctx.hook_context.clone();
        let joined = run_blocking(move || {
            log_hook_warnings(&hooks, &hook_context);
            Ok(())
        })
        .await;
        if let Err(error) = joined {
            tracing::warn!(%error, "post-exit launch hooks failed");
        }
    }

    fn deregister(&mut self) {
        if self.deregistered {
            return;
        }
        self.ctx.registry.deregister(self.ctx.session_id);
        self.deregistered = true;
    }

    async fn unregister_portal(&mut self) {
        if self.portal_unregistered {
            return;
        }
        self.portal_unregistered = true;
        let Some(portal) = self.ctx.portal_guard.take() else {
            return;
        };
        if let Err(error) = portal.unregister().await {
            tracing::warn!(%error, "gamemode portal unregister failed");
        }
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.publish_exited(None, None);
        self.cascade_from_drop();
        self.hooks_from_drop();
        self.deregister_from_drop();
        self.portal_from_drop();
    }
}

impl SessionGuard {
    fn cascade_from_drop(&mut self) {
        if self.cascaded {
            return;
        }
        self.cascaded = true;
        if self.ctx.kind != SessionKind::Game {
            return;
        }
        let registry = Arc::clone(&self.ctx.registry);
        let session_id = self.ctx.session_id;
        catch_registry(move || {
            let signalled =
                registry.cancel_linked_children(session_id, TeardownReason::LinkedSessionExit);
            if signalled > 0 {
                tracing::info!(
                    game_session_id = %session_id,
                    linked_trainers = signalled,
                    "launch session: cascading LinkedSessionExit to trainer children"
                );
            }
        });
    }

    fn hooks_from_drop(&mut self) {
        if self.hooks_ran {
            return;
        }
        self.hooks_ran = true;
        let hooks = self.ctx.post_exit_hooks.clone();
        let hook_context = self.ctx.hook_context.clone();
        let Ok(handle) = Handle::try_current() else {
            return;
        };
        handle.spawn_blocking(move || log_hook_warnings(&hooks, &hook_context));
    }

    fn deregister_from_drop(&mut self) {
        if self.deregistered {
            return;
        }
        self.deregistered = true;
        let registry = Arc::clone(&self.ctx.registry);
        let session_id = self.ctx.session_id;
        catch_registry(move || registry.deregister(session_id));
    }

    fn portal_from_drop(&mut self) {
        if self.portal_unregistered {
            return;
        }
        self.portal_unregistered = true;
        let Some(portal) = self.ctx.portal_guard.take() else {
            return;
        };
        let Ok(handle) = Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            if let Err(error) = portal.unregister().await {
                tracing::warn!(%error, "gamemode portal unregister failed");
            }
        });
    }
}

fn log_hook_warnings(hooks: &[LaunchHook], context: &LaunchHookExecutionContext) {
    for warning in run_post_exit_hooks(hooks, context) {
        tracing::warn!(
            hook_id = warning.hook_id.as_deref().unwrap_or(""),
            code = warning.code.as_deref().unwrap_or(""),
            message = %warning.message,
            "post-exit launch hook warning"
        );
    }
}

/// Registry locks panic on poison. Teardown must not turn that into an abort
/// while the session task is already unwinding.
fn catch_registry(work: impl FnOnce()) {
    let _ = std::panic::catch_unwind(AssertUnwindSafe(work));
}

