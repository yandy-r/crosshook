use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::process::Command;
use std::sync::Arc;

use tempfile::tempdir;
use zbus::zvariant::{OwnedValue, Value};

use super::file::{attempt, open_all};
use super::*;

fn mode_of(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// Takes a lock that should be free. Other tests fork concurrently, and a
/// forked child shares our lock descriptors until it execs; the wait wakes
/// on the final close (`IN_CLOSE`) instead of racing it.
fn acquire_soon(paths: &InstanceLockPaths, what: &str) -> InstanceLock {
    InstanceLock::acquire_with_wait(paths, Duration::from_secs(2))
        .unwrap_or_else(|error| panic!("{what}: {error:?}"))
}

fn assert_contended(result: Result<InstanceLock, AcquireError>) {
    assert!(
        matches!(result, Err(AcquireError::Contended)),
        "expected Contended, got {result:?}"
    );
}

#[test]
fn resolve_native_and_flatpak_layouts() {
    let native = InstanceLockPaths::resolve(Some(OsStr::new("/run/user/1000")), false).unwrap();
    assert_eq!(
        native.file(),
        Path::new("/run/user/1000/crosshook/instance.lock")
    );
    assert_eq!(
        native.shared_file(),
        Some(Path::new(
            "/run/user/1000/app/dev.crosshook.CrossHook/instance.lock"
        ))
    );
    assert_eq!(native, InstanceLockPaths::in_dir("/run/user/1000"));
    let flatpak = InstanceLockPaths::resolve(Some(OsStr::new("/run/user/1000")), true).unwrap();
    assert_eq!(
        flatpak.file(),
        Path::new("/run/user/1000/app/dev.crosshook.CrossHook/instance.lock")
    );
    assert_eq!(flatpak.shared_file(), None);
}

#[test]
fn resolve_rejects_missing_empty_and_relative_runtime_dirs() {
    for runtime_dir in [None, Some(""), Some("relative/run")] {
        assert_eq!(
            InstanceLockPaths::resolve(runtime_dir.map(OsStr::new), false),
            Err(LockUnavailable::RuntimeDirUnavailable),
            "{runtime_dir:?}"
        );
    }
    assert_eq!(
        LockUnavailable::RuntimeDirUnavailable.reason(),
        "runtime dir unavailable"
    );
    assert_eq!(
        LockUnavailable::RuntimeDirUnsafe.reason(),
        "runtime dir unsafe"
    );
    assert_eq!(LockUnavailable::Io.reason(), "instance lock unavailable");
}

#[test]
fn acquire_creates_private_dir_and_empty_file() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());
    let lock = InstanceLock::try_acquire(&paths).unwrap();
    assert_eq!(mode_of(paths.dir()), 0o700);
    assert_eq!(mode_of(paths.file()), 0o600);
    assert_eq!(std::fs::metadata(paths.file()).unwrap().len(), 0);
    assert_eq!(lock.paths(), &paths);
}

#[test]
fn flatpak_layout_creates_missing_parents() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::resolve(Some(runtime.path().as_os_str()), true).unwrap();
    let _lock = InstanceLock::try_acquire(&paths).unwrap();
    assert!(paths.file().is_file());
    assert_eq!(mode_of(paths.dir()), 0o700);
}

#[test]
fn native_also_locks_the_flatpak_app_dir() {
    let runtime = tempdir().unwrap();
    let native = InstanceLockPaths::in_dir(runtime.path());
    let flatpak = InstanceLockPaths::resolve(Some(runtime.path().as_os_str()), true).unwrap();
    let shared = native.shared_file().unwrap().to_owned();
    assert_eq!(shared, flatpak.file());

    let native_lock = InstanceLock::try_acquire(&native).unwrap();
    assert!(shared.is_file());
    assert_eq!(mode_of(shared.parent().unwrap()), 0o700);
    assert_contended(InstanceLock::try_acquire(&flatpak));
    drop(native_lock);

    let flatpak_lock = acquire_soon(&flatpak, "flatpak lock after native drop");
    assert_contended(InstanceLock::try_acquire(&native));
    // A failed attempt must not leave its primary lock behind.
    drop(flatpak_lock);
    acquire_soon(&native, "both locks free again");
}

#[test]
fn unsafe_shared_dir_only_skips_the_shared_lock() {
    let runtime = tempdir().unwrap();
    let native = InstanceLockPaths::in_dir(runtime.path());
    let shared_dir = native.shared_file().unwrap().parent().unwrap().to_owned();
    std::fs::create_dir_all(&shared_dir).unwrap();
    std::fs::set_permissions(&shared_dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    let _lock = InstanceLock::try_acquire(&native).expect("primary lock still taken");
    assert!(!shared_dir.join("instance.lock").exists());
    assert_contended(InstanceLock::try_acquire(&native));
}

#[test]
fn second_acquire_is_contended() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());
    let _first = InstanceLock::try_acquire(&paths).unwrap();
    assert_contended(InstanceLock::try_acquire(&paths));
}

#[test]
fn drop_releases() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());
    let first = InstanceLock::try_acquire(&paths).unwrap();
    drop(first);
    let _second = acquire_soon(&paths, "lock free after drop");
}

#[test]
fn rejects_unsafe_dir() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());

    std::fs::create_dir(paths.dir()).unwrap();
    std::fs::set_permissions(paths.dir(), std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(matches!(
        InstanceLock::try_acquire(&paths),
        Err(AcquireError::Unavailable(
            LockUnavailable::RuntimeDirUnsafe,
            _
        ))
    ));
    std::fs::remove_dir(paths.dir()).unwrap();

    let elsewhere = tempdir().unwrap();
    symlink(elsewhere.path(), paths.dir()).unwrap();
    assert!(matches!(
        InstanceLock::try_acquire(&paths),
        Err(AcquireError::Unavailable(
            LockUnavailable::RuntimeDirUnsafe,
            _
        ))
    ));
    std::fs::remove_file(paths.dir()).unwrap();

    std::fs::create_dir(paths.dir()).unwrap();
    std::fs::set_permissions(paths.dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink(elsewhere.path().join("target"), paths.file()).unwrap();
    assert!(matches!(
        InstanceLock::try_acquire(&paths),
        Err(AcquireError::Unavailable(LockUnavailable::Io, Some(_)))
    ));
    assert!(!elsewhere.path().join("target").exists());
}

#[test]
fn wait_succeeds_when_holder_releases() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());
    let first = InstanceLock::try_acquire(&paths).unwrap();
    let waiter_paths = paths.clone();
    let started = Instant::now();
    let waiter =
        std::thread::spawn(move || InstanceLock::acquire_with_wait(&waiter_paths, HANDOFF_WAIT));
    std::thread::sleep(Duration::from_millis(200));
    drop(first);
    let second = waiter.join().unwrap();
    assert!(second.is_ok(), "waiter must get the lock: {second:?}");
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn wait_times_out_while_held() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());
    let _first = InstanceLock::try_acquire(&paths).unwrap();
    let started = Instant::now();
    assert_contended(InstanceLock::acquire_with_wait(
        &paths,
        Duration::from_millis(300),
    ));
    assert!(started.elapsed() >= Duration::from_millis(300));
}

/// Fail closed: the shared dir is unusable (so that file is never opened or
/// watched) and the primary is held. The wait must report `Contended`, not
/// `Unavailable`, or a handed-off launch would start beside the holder.
#[test]
fn wait_with_unsafe_shared_dir_and_held_primary_is_contended() {
    let runtime = tempdir().unwrap();
    let native = InstanceLockPaths::in_dir(runtime.path());
    let shared_dir = native.shared_file().unwrap().parent().unwrap().to_owned();
    std::fs::create_dir_all(&shared_dir).unwrap();
    std::fs::set_permissions(&shared_dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    let _held = InstanceLock::try_acquire(&native).expect("primary lock");
    assert_contended(InstanceLock::acquire_with_wait(
        &native,
        Duration::from_millis(100),
    ));
}

/// A lock file replaced at its path while another process holds the old
/// inode: the holder's lock is invisible at the path, but a new acquire must
/// lock the file the path names, not a stale descriptor. Simulated in-process:
/// open (stale), replace the file, then lock — the identity check must
/// notice and reopen.
#[test]
fn replaced_lock_file_is_reopened_not_locked_stale() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());
    let opened = open_all(&paths).unwrap();
    std::fs::remove_file(paths.file()).unwrap();
    std::fs::write(paths.file(), b"").unwrap();
    std::fs::set_permissions(paths.file(), std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        matches!(attempt(&opened), file::Attempt::Stale),
        "a replaced file must not count as locked"
    );
    drop(opened);
    // A holder of the replacement now blocks a fresh acquire.
    let holder = InstanceLock::try_acquire(&paths).expect("lock the current file");
    assert_contended(InstanceLock::try_acquire(&paths));
    assert_contended(InstanceLock::acquire_with_wait(
        &paths,
        Duration::from_millis(100),
    ));
    drop(holder);
}

/// The original holder keeps its (now unlinked) primary inode locked and
/// the file is recreated: a new launch must not succeed against the fresh
/// inode, because the holder still holds the Flatpak-shared lock too.
#[test]
fn recreated_primary_while_held_does_not_let_a_second_launch_in() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());
    let holder = InstanceLock::try_acquire(&paths).unwrap();
    std::fs::remove_file(paths.file()).unwrap();
    std::fs::write(paths.file(), b"").unwrap();
    std::fs::set_permissions(paths.file(), std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_contended(InstanceLock::try_acquire(&paths));
    assert_contended(InstanceLock::acquire_with_wait(
        &paths,
        Duration::from_millis(100),
    ));
    drop(holder);
    acquire_soon(&paths, "free once the holder exits");
}

#[test]
fn covers_flatpak_reflects_the_shared_lock() {
    let runtime = tempdir().unwrap();
    let native = InstanceLockPaths::in_dir(runtime.path());
    assert!(InstanceLock::try_acquire(&native).unwrap().covers_flatpak());
    let flatpak = InstanceLockPaths::resolve(Some(runtime.path().as_os_str()), true).unwrap();
    let other = tempdir().unwrap();
    let flatpak_other = InstanceLockPaths::resolve(Some(other.path().as_os_str()), true).unwrap();
    assert!(InstanceLock::try_acquire(&flatpak_other)
        .unwrap()
        .covers_flatpak());
    drop(flatpak);
    let shared_dir = native.shared_file().unwrap().parent().unwrap().to_owned();
    std::fs::set_permissions(&shared_dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    let partial = InstanceLock::try_acquire(&native).expect("primary only");
    assert!(!partial.covers_flatpak());
}

#[test]
fn wait_on_free_lock_acquires_immediately() {
    let runtime = tempdir().unwrap();
    let paths = InstanceLockPaths::in_dir(runtime.path());
    InstanceLock::acquire_with_wait(&paths, Duration::ZERO).expect("free lock");
}

fn relay() -> Arc<ActivationRelay> {
    Arc::new(ActivationRelay::new(tokio::runtime::Handle::current()))
}

fn held_guard(runtime: &Path) -> InstanceGuard {
    let lock = InstanceLock::try_acquire(&InstanceLockPaths::in_dir(runtime)).unwrap();
    InstanceGuard::new(InstanceLockState::Held(lock), None, relay(), false)
}

#[tokio::test]
async fn guard_handoff_without_lock_is_not_held() {
    let guard = InstanceGuard::new(InstanceLockState::Unavailable("test"), None, relay(), true);
    let result = guard.release_and_handoff(Command::new("true")).await;
    assert!(matches!(result, Err(HandoffError::NotHeld)), "{result:?}");
    assert!(!guard.handoff_pending());
}

#[tokio::test]
async fn guard_handoff_spawn_failure_keeps_everything() {
    let runtime = tempdir().unwrap();
    let guard = held_guard(runtime.path());
    let missing = Command::new(runtime.path().join("no-such-binary"));
    let result = guard.release_and_handoff(missing).await;
    assert!(matches!(result, Err(HandoffError::Spawn(_))), "{result:?}");
    assert!(guard.holds_lock());
    assert!(!guard.handoff_pending());
    let paths = InstanceLockPaths::in_dir(runtime.path());
    assert_contended(InstanceLock::try_acquire(&paths));
}

#[tokio::test]
async fn guard_second_handoff_is_already_pending() {
    let runtime = tempdir().unwrap();
    let guard = held_guard(runtime.path());
    let mut child = guard
        .release_and_handoff(Command::new("true"))
        .await
        .expect("first handoff spawns");
    let again = guard.release_and_handoff(Command::new("true")).await;
    assert!(
        matches!(again, Err(HandoffError::AlreadyPending)),
        "{again:?}"
    );
    assert!(guard.handoff_pending());
    assert!(child.wait().unwrap().success());
}

#[tokio::test]
async fn guard_handoff_retry_after_spawn_failure_is_allowed() {
    let runtime = tempdir().unwrap();
    let guard = held_guard(runtime.path());
    let missing = Command::new(runtime.path().join("no-such-binary"));
    assert!(matches!(
        guard.release_and_handoff(missing).await,
        Err(HandoffError::Spawn(_))
    ));
    assert!(
        !guard.handoff_pending(),
        "a failed spawn resets the pending state"
    );
    let mut child = guard
        .release_and_handoff(Command::new("true"))
        .await
        .expect("retry spawns");
    assert!(child.wait().unwrap().success());
    assert!(guard.handoff_pending());
}

#[tokio::test]
async fn guard_handoff_keeps_the_lock_until_exit() {
    let runtime = tempdir().unwrap();
    let guard = held_guard(runtime.path());
    let mut child = guard
        .release_and_handoff(Command::new("true"))
        .await
        .expect("spawn");
    assert!(child.wait().unwrap().success());
    assert!(guard.handoff_pending());
    assert!(guard.holds_lock());
    assert!(!guard.owns_bus_name());
    let paths = InstanceLockPaths::in_dir(runtime.path());
    assert_contended(InstanceLock::try_acquire(&paths));
}

fn owned(value: Value<'_>) -> OwnedValue {
    value.try_to_owned().unwrap()
}

#[test]
fn platform_data_extracts_token_and_startup_id() {
    let data = HashMap::from([
        ("activation-token".to_owned(), owned(Value::from("tok"))),
        (
            "desktop-startup-id".to_owned(),
            owned(Value::from("id_TIME1")),
        ),
    ]);
    let parsed = ActivationRequest::from_platform_data(&data);
    assert_eq!(parsed.activation_token.as_deref(), Some("tok"));
    assert_eq!(parsed.desktop_startup_id.as_deref(), Some("id_TIME1"));
    assert_eq!(parsed.startup_id(), Some("tok"));
    let only_id = ActivationRequest {
        activation_token: None,
        ..parsed
    };
    assert_eq!(only_id.startup_id(), Some("id_TIME1"));
}

#[test]
fn platform_data_drops_untrusted_values() {
    let data = HashMap::from([
        ("activation-token".to_owned(), owned(Value::from(42u32))),
        (
            "desktop-startup-id".to_owned(),
            owned(Value::from("x".repeat(513).as_str())),
        ),
        ("unknown".to_owned(), owned(Value::from("tok"))),
    ]);
    assert_eq!(
        ActivationRequest::from_platform_data(&data),
        ActivationRequest::default()
    );
    let control = HashMap::from([(
        "activation-token".to_owned(),
        owned(Value::from("tok\ninjected")),
    )]);
    assert_eq!(
        ActivationRequest::from_platform_data(&control),
        ActivationRequest::default()
    );
    let max = HashMap::from([(
        "activation-token".to_owned(),
        owned(Value::from("x".repeat(512).as_str())),
    )]);
    assert!(ActivationRequest::from_platform_data(&max)
        .activation_token
        .is_some());
}

#[test]
fn platform_data_round_trips_through_the_wire_map() {
    let request = ActivationRequest {
        activation_token: Some("tok".to_owned()),
        desktop_startup_id: None,
    };
    let wire: HashMap<String, OwnedValue> = request
        .platform_data()
        .into_iter()
        .map(|(key, value)| (key.to_owned(), owned(value)))
        .collect();
    assert_eq!(wire.len(), 1);
    assert_eq!(ActivationRequest::from_platform_data(&wire), request);
}
