use std::collections::HashMap;
use std::ffi::{OsStr, OsString};

use super::*;
use crate::bootstrap::host::PreRuntimeHost;

#[derive(Default)]
struct RecordingHost {
    threads: Option<u32>,
    flatpak: bool,
    opt_in: bool,
    migration_error: Option<String>,
    calls: Vec<String>,
    env: HashMap<OsString, OsString>,
}

impl RecordingHost {
    fn isolated() -> Self {
        Self {
            threads: Some(1),
            flatpak: true,
            ..Self::default()
        }
    }
}

impl PreRuntimeHost for RecordingHost {
    fn thread_count(&self) -> Option<u32> {
        self.threads
    }

    fn is_flatpak(&self) -> bool {
        self.flatpak
    }

    fn host_xdg_opt_in(&self) -> bool {
        self.opt_in
    }

    fn override_xdg(&mut self) {
        self.calls.push("override_xdg".to_owned());
    }

    fn run_flatpak_migration(&mut self) -> Result<FlatpakMigrationReport, String> {
        self.calls.push("flatpak_migration".to_owned());
        match &self.migration_error {
            Some(error) => Err(error.clone()),
            None => Ok(FlatpakMigrationReport {
                imported_config: true,
                imported_subtrees: vec!["crosshook/community".to_owned()],
                skipped_subtrees: Vec::new(),
                error: None,
            }),
        }
    }

    fn migrate_legacy_app_id(&mut self) {
        self.calls.push("legacy_app_id".to_owned());
    }

    fn env_var(&self, key: &OsStr) -> Option<OsString> {
        self.env.get(key).cloned()
    }

    fn set_env_var(&mut self, key: &OsStr, value: &OsStr) {
        self.calls.push(format!("set:{}", key.to_string_lossy()));
        self.env.insert(key.to_owned(), value.to_owned());
    }

    fn remove_env_var(&mut self, key: &OsStr) {
        self.calls.push(format!("remove:{}", key.to_string_lossy()));
        self.env.remove(key);
    }
}

struct RecordingPolicy;

impl EnvPolicy for RecordingPolicy {
    fn apply(&self, env: &mut PreRuntimeEnv<'_>) {
        env.set("CROSSHOOK_TEST_SET", "1");
        env.remove("CROSSHOOK_TEST_REMOVE");
    }
}

fn steps_of(pre: &PreRuntime) -> Vec<BootstrapStep> {
    pre.steps()
        .records()
        .iter()
        .map(|record| record.step)
        .collect()
}

fn outcomes_of(pre: &PreRuntime) -> Vec<StepOutcome> {
    pre.steps()
        .records()
        .iter()
        .map(|record| record.outcome)
        .collect()
}

#[test]
fn native_run_records_all_steps_in_order() {
    let mut host = RecordingHost {
        threads: Some(1),
        ..RecordingHost::default()
    };
    let pre = pre_runtime_with_host(PreRuntimeOptions::new(), &mut host)
        .expect("single-threaded native run");
    assert_eq!(pre.flatpak_mode(), FlatpakMode::Native);
    assert!(pre.migration().is_none());
    assert_eq!(steps_of(&pre), BootstrapStep::PRE_RUNTIME_ORDER);
    assert_eq!(
        outcomes_of(&pre),
        vec![
            StepOutcome::Skipped("not running under Flatpak"),
            StepOutcome::Skipped("not in Flatpak isolated mode"),
            StepOutcome::Ran,
            StepOutcome::Skipped("no env policy"),
        ]
    );
    assert_eq!(pre.steps().verify_pre_runtime_order(), Ok(()));
    assert_eq!(host.calls, vec!["legacy_app_id"]);
}

#[test]
fn isolated_run_migrates_before_legacy_migration() {
    let mut host = RecordingHost::isolated();
    let pre = pre_runtime_with_host(PreRuntimeOptions::new(), &mut host)
        .expect("single-threaded isolated run");
    assert_eq!(pre.flatpak_mode(), FlatpakMode::Isolated);
    let migration = pre.migration().expect("migration report");
    assert!(migration.imported_config);
    assert_eq!(migration.imported_subtrees, ["crosshook/community"]);
    assert_eq!(steps_of(&pre), BootstrapStep::PRE_RUNTIME_ORDER);
    assert_eq!(
        outcomes_of(&pre),
        vec![
            StepOutcome::Ran,
            StepOutcome::Ran,
            StepOutcome::Ran,
            StepOutcome::Skipped("no env policy"),
        ]
    );
    assert_eq!(pre.steps().verify_pre_runtime_order(), Ok(()));
    assert_eq!(host.calls, vec!["flatpak_migration", "legacy_app_id"]);
}

#[test]
fn host_xdg_run_overrides_xdg_and_skips_migration() {
    let mut host = RecordingHost {
        opt_in: true,
        ..RecordingHost::isolated()
    };
    let pre = pre_runtime_with_host(PreRuntimeOptions::new(), &mut host)
        .expect("single-threaded host-xdg run");
    assert_eq!(pre.flatpak_mode(), FlatpakMode::HostXdgShared);
    assert!(pre.migration().is_none());
    assert_eq!(
        outcomes_of(&pre),
        vec![
            StepOutcome::Ran,
            StepOutcome::Skipped("not in Flatpak isolated mode"),
            StepOutcome::Ran,
            StepOutcome::Skipped("no env policy"),
        ]
    );
    assert_eq!(pre.steps().verify_pre_runtime_order(), Ok(()));
    assert_eq!(host.calls, vec!["override_xdg", "legacy_app_id"]);
}

#[test]
fn env_policy_runs_last_and_mutates_through_host() {
    let mut host = RecordingHost::isolated();
    host.env
        .insert(OsString::from("CROSSHOOK_TEST_REMOVE"), OsString::from("x"));
    let opts = PreRuntimeOptions::new().with_env_policy(RecordingPolicy);
    let pre = pre_runtime_with_host(opts, &mut host).expect("policy run");
    assert_eq!(outcomes_of(&pre)[3], StepOutcome::Ran);
    assert_eq!(
        host.env.get(OsStr::new("CROSSHOOK_TEST_SET")),
        Some(&OsString::from("1"))
    );
    assert!(!host.env.contains_key(OsStr::new("CROSSHOOK_TEST_REMOVE")));
    assert_eq!(
        host.calls,
        vec![
            "flatpak_migration",
            "legacy_app_id",
            "set:CROSSHOOK_TEST_SET",
            "remove:CROSSHOOK_TEST_REMOVE",
        ]
    );
}

#[test]
fn migration_failure_degrades_instead_of_failing() {
    let mut host = RecordingHost {
        migration_error: Some("disk full".to_owned()),
        ..RecordingHost::isolated()
    };
    let pre = pre_runtime_with_host(PreRuntimeOptions::new(), &mut host)
        .expect("migration failure is not fatal");
    assert_eq!(
        outcomes_of(&pre)[1],
        StepOutcome::Degraded("flatpak migration failed")
    );
    let migration = pre.migration().expect("degraded report");
    assert!(!migration.imported_config);
    assert_eq!(migration.error.as_deref(), Some("disk full"));
    assert_eq!(pre.steps().verify_pre_runtime_order(), Ok(()));
}

#[test]
fn multithreaded_entry_degrades_env_step_but_runs_everything() {
    let mut host = RecordingHost {
        threads: Some(3),
        ..RecordingHost::isolated()
    };
    host.env
        .insert(OsString::from("CROSSHOOK_TEST_REMOVE"), OsString::from("x"));
    let opts = PreRuntimeOptions::new().with_env_policy(RecordingPolicy);
    let pre = pre_runtime_with_host(opts, &mut host).expect("multi-threaded entry is not fatal");
    assert_eq!(steps_of(&pre), BootstrapStep::PRE_RUNTIME_ORDER);
    assert_eq!(
        outcomes_of(&pre),
        vec![
            StepOutcome::Ran,
            StepOutcome::Ran,
            StepOutcome::Ran,
            StepOutcome::Degraded("multi-threaded at entry"),
        ]
    );
    assert_eq!(pre.steps().verify_pre_runtime_order(), Ok(()));
    assert_eq!(
        host.calls,
        vec![
            "flatpak_migration",
            "legacy_app_id",
            "set:CROSSHOOK_TEST_SET",
            "remove:CROSSHOOK_TEST_REMOVE",
        ]
    );
}

#[test]
fn multithreaded_entry_without_policy_still_degrades_env_step() {
    let mut host = RecordingHost {
        threads: Some(2),
        ..RecordingHost::default()
    };
    let pre = pre_runtime_with_host(PreRuntimeOptions::new(), &mut host)
        .expect("multi-threaded entry is not fatal");
    assert_eq!(
        outcomes_of(&pre)[3],
        StepOutcome::Degraded("multi-threaded at entry")
    );
    assert_eq!(host.calls, vec!["legacy_app_id"]);
}

#[test]
fn unknown_thread_count_proceeds() {
    let mut host = RecordingHost {
        threads: None,
        ..RecordingHost::default()
    };
    let pre = pre_runtime_with_host(PreRuntimeOptions::new(), &mut host)
        .expect("unverifiable thread count only warns");
    assert_eq!(pre.flatpak_mode(), FlatpakMode::Native);
}

#[test]
fn misordered_execution_fails_the_shared_order_checker() {
    let mut host = RecordingHost::isolated();
    let pre = pre_runtime_misordered_with_host(PreRuntimeOptions::new(), &mut host)
        .expect("misordered run still completes");
    assert_eq!(
        steps_of(&pre),
        vec![
            BootstrapStep::LegacyAppIdMigration,
            BootstrapStep::FlatpakGate,
            BootstrapStep::FlatpakMigration,
            BootstrapStep::EnvPolicy,
        ]
    );
    assert!(pre.steps().verify_pre_runtime_order().is_err());
    assert_eq!(host.calls, vec!["legacy_app_id", "flatpak_migration"]);
}
