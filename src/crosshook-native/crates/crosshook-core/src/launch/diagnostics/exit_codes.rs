use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use super::models::{ExitCodeInfo, FailureMode};
use crate::launch::request::{ValidationSeverity, METHOD_STEAM_APPLAUNCH};

pub fn analyze_exit_status(exit_status: Option<ExitStatus>, method: &str) -> ExitCodeInfo {
    match exit_status {
        Some(status) => analyze_exit_status_raw(status, method),
        None => ExitCodeInfo {
            code: None,
            signal: None,
            signal_name: None,
            core_dumped: false,
            failure_mode: FailureMode::Unknown,
            description: "No exit status was captured.".to_string(),
            severity: ValidationSeverity::Warning,
        },
    }
}

fn analyze_exit_status_raw(exit_status: ExitStatus, method: &str) -> ExitCodeInfo {
    let code = exit_status.code();
    let signal = exit_status.signal();
    let core_dumped = signal.is_some() && exit_status.core_dumped();

    match signal {
        Some(signal) => build_signal_exit_info(signal, core_dumped),
        None => match code {
            Some(0) if method == METHOD_STEAM_APPLAUNCH => ExitCodeInfo {
                code,
                signal,
                signal_name: None,
                core_dumped,
                failure_mode: FailureMode::Indeterminate,
                description:
                    "Steam launch helper exited cleanly; Steam may still be running the game."
                        .to_string(),
                severity: ValidationSeverity::Info,
            },
            Some(0) => ExitCodeInfo {
                code,
                signal,
                signal_name: None,
                core_dumped,
                failure_mode: FailureMode::CleanExit,
                description: "Process exited successfully.".to_string(),
                severity: ValidationSeverity::Info,
            },
            Some(127) => ExitCodeInfo {
                code,
                signal,
                signal_name: None,
                core_dumped,
                failure_mode: FailureMode::CommandNotFound,
                description: "The launched command could not be found.".to_string(),
                severity: ValidationSeverity::Fatal,
            },
            Some(126) => ExitCodeInfo {
                code,
                signal,
                signal_name: None,
                core_dumped,
                failure_mode: FailureMode::PermissionDenied,
                description: "The launched command could not be executed due to permissions."
                    .to_string(),
                severity: ValidationSeverity::Fatal,
            },
            Some(other_code) => ExitCodeInfo {
                code: Some(other_code),
                signal: None,
                signal_name: None,
                core_dumped: false,
                failure_mode: FailureMode::NonZeroExit,
                description: format!("Process exited with code {other_code}."),
                severity: ValidationSeverity::Warning,
            },
            None => ExitCodeInfo {
                code: None,
                signal: None,
                signal_name: None,
                core_dumped: false,
                failure_mode: FailureMode::Unknown,
                description: "Process exit status could not be classified.".to_string(),
                severity: ValidationSeverity::Warning,
            },
        },
    }
}

fn build_signal_exit_info(signal: i32, core_dumped: bool) -> ExitCodeInfo {
    let signal_name = signal_name_from_number(signal).to_string();
    let description = if core_dumped {
        format!("Process terminated by {signal_name} and produced a core dump.")
    } else {
        format!("Process terminated by {signal_name}.")
    };

    let (failure_mode, severity) = match signal {
        11 => (FailureMode::Segfault, ValidationSeverity::Fatal),
        6 => (FailureMode::Abort, ValidationSeverity::Fatal),
        9 => (FailureMode::Kill, ValidationSeverity::Warning),
        15 => (FailureMode::Terminated, ValidationSeverity::Warning),
        7 => (FailureMode::BusError, ValidationSeverity::Fatal),
        4 => (FailureMode::IllegalInstruction, ValidationSeverity::Fatal),
        8 => (
            FailureMode::FloatingPointException,
            ValidationSeverity::Fatal,
        ),
        13 => (FailureMode::BrokenPipe, ValidationSeverity::Warning),
        _ => (FailureMode::UnknownSignal, ValidationSeverity::Warning),
    };

    ExitCodeInfo {
        code: None,
        signal: Some(signal),
        signal_name: Some(signal_name),
        core_dumped,
        failure_mode,
        description,
        severity,
    }
}

pub fn signal_name_from_number(signal: i32) -> &'static str {
    match signal {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        5 => "SIGTRAP",
        6 => "SIGABRT",
        7 => "SIGBUS",
        8 => "SIGFPE",
        9 => "SIGKILL",
        10 => "SIGUSR1",
        11 => "SIGSEGV",
        12 => "SIGUSR2",
        13 => "SIGPIPE",
        14 => "SIGALRM",
        15 => "SIGTERM",
        16 => "SIGSTKFLT",
        17 => "SIGCHLD",
        18 => "SIGCONT",
        19 => "SIGSTOP",
        20 => "SIGTSTP",
        21 => "SIGTTIN",
        22 => "SIGTTOU",
        23 => "SIGURG",
        24 => "SIGXCPU",
        25 => "SIGXFSZ",
        26 => "SIGVTALRM",
        27 => "SIGPROF",
        28 => "SIGWINCH",
        29 => "SIGIO",
        30 => "SIGPWR",
        31 => "SIGSYS",
        _ => "SIGUNKNOWN",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::{METHOD_NATIVE, METHOD_PROTON_RUN};
    use std::os::unix::process::ExitStatusExt;

    fn exit_status_from_code(code: i32) -> ExitStatus {
        ExitStatus::from_raw(code << 8)
    }

    fn exit_status_from_signal(signal: i32, core_dumped: bool) -> ExitStatus {
        ExitStatus::from_raw(signal | if core_dumped { 0x80 } else { 0 })
    }

    #[test]
    fn analyze_exit_status_handles_expected_outcomes() {
        struct Case {
            status: ExitStatus,
            method: &'static str,
            expected_code: Option<i32>,
            expected_signal: Option<i32>,
            expected_signal_name: Option<&'static str>,
            expected_core_dumped: bool,
            expected_failure_mode: FailureMode,
            expected_severity: ValidationSeverity,
        }

        let cases = [
            Case {
                status: exit_status_from_code(0),
                method: METHOD_STEAM_APPLAUNCH,
                expected_code: Some(0),
                expected_signal: None,
                expected_signal_name: None,
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::Indeterminate,
                expected_severity: ValidationSeverity::Info,
            },
            Case {
                status: exit_status_from_code(0),
                method: METHOD_NATIVE,
                expected_code: Some(0),
                expected_signal: None,
                expected_signal_name: None,
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::CleanExit,
                expected_severity: ValidationSeverity::Info,
            },
            Case {
                status: exit_status_from_code(1),
                method: METHOD_PROTON_RUN,
                expected_code: Some(1),
                expected_signal: None,
                expected_signal_name: None,
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::NonZeroExit,
                expected_severity: ValidationSeverity::Warning,
            },
            Case {
                status: exit_status_from_signal(11, true),
                method: METHOD_PROTON_RUN,
                expected_code: None,
                expected_signal: Some(11),
                expected_signal_name: Some("SIGSEGV"),
                expected_core_dumped: true,
                expected_failure_mode: FailureMode::Segfault,
                expected_severity: ValidationSeverity::Fatal,
            },
            Case {
                status: exit_status_from_signal(6, false),
                method: METHOD_PROTON_RUN,
                expected_code: None,
                expected_signal: Some(6),
                expected_signal_name: Some("SIGABRT"),
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::Abort,
                expected_severity: ValidationSeverity::Fatal,
            },
            Case {
                status: exit_status_from_signal(9, false),
                method: METHOD_PROTON_RUN,
                expected_code: None,
                expected_signal: Some(9),
                expected_signal_name: Some("SIGKILL"),
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::Kill,
                expected_severity: ValidationSeverity::Warning,
            },
            Case {
                status: exit_status_from_signal(15, false),
                method: METHOD_PROTON_RUN,
                expected_code: None,
                expected_signal: Some(15),
                expected_signal_name: Some("SIGTERM"),
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::Terminated,
                expected_severity: ValidationSeverity::Warning,
            },
            Case {
                status: exit_status_from_code(127),
                method: METHOD_PROTON_RUN,
                expected_code: Some(127),
                expected_signal: None,
                expected_signal_name: None,
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::CommandNotFound,
                expected_severity: ValidationSeverity::Fatal,
            },
            Case {
                status: exit_status_from_code(126),
                method: METHOD_PROTON_RUN,
                expected_code: Some(126),
                expected_signal: None,
                expected_signal_name: None,
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::PermissionDenied,
                expected_severity: ValidationSeverity::Fatal,
            },
            Case {
                status: exit_status_from_code(42),
                method: METHOD_NATIVE,
                expected_code: Some(42),
                expected_signal: None,
                expected_signal_name: None,
                expected_core_dumped: false,
                expected_failure_mode: FailureMode::NonZeroExit,
                expected_severity: ValidationSeverity::Warning,
            },
        ];

        for case in cases {
            let info = analyze_exit_status(Some(case.status), case.method);

            assert_eq!(info.code, case.expected_code);
            assert_eq!(info.signal, case.expected_signal);
            assert_eq!(
                info.signal_name.as_deref(),
                case.expected_signal_name,
                "unexpected signal name for method {}",
                case.method
            );
            assert_eq!(info.core_dumped, case.expected_core_dumped);
            assert_eq!(info.failure_mode, case.expected_failure_mode);
            assert_eq!(info.severity, case.expected_severity);
        }
    }
}
