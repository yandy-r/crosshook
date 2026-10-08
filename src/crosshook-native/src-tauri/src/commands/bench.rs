//! Startup-benchmark hooks (YAN-782 / #509). Inert unless `CROSSHOOK_BENCH=1`.
//!
//! Env contract:
//! - `CROSSHOOK_BENCH=1`                  enable hooks (anything else: no-op)
//! - `CROSSHOOK_BENCH_EXIT_AFTER_READY=1` exit(0) right after the READY marker
//! - `CROSSHOOK_BENCH_ROOT`               absolute fixture root dir (required with `_LOG`)
//! - `CROSSHOOK_BENCH_LOG`                absolute log file under the root; replayed as `launch-log`
//! - `CROSSHOOK_BENCH_LOG_RATE`           lines/s, 1..=5000 (default 5000)
//!
//! READY marker: one `READY <CLOCK_MONOTONIC ns>` line on stderr, once per process.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nix::time::{clock_gettime, ClockId};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncReadExt;

const ENV_BENCH: &str = "CROSSHOOK_BENCH";
const ENV_EXIT: &str = "CROSSHOOK_BENCH_EXIT_AFTER_READY";
const ENV_ROOT: &str = "CROSSHOOK_BENCH_ROOT";
const ENV_LOG: &str = "CROSSHOOK_BENCH_LOG";
const ENV_RATE: &str = "CROSSHOOK_BENCH_LOG_RATE";

const MAX_REPLAY_LINES: usize = 50_000;
const MAX_REPLAY_BYTES: u64 = 16 * 1024 * 1024;
const MIN_RATE: u32 = 1;
const MAX_RATE: u32 = 5000;
const TICK: Duration = Duration::from_millis(10);

static READY_FIRED: AtomicBool = AtomicBool::new(false);

/// True when benchmark mode is on (`CROSSHOOK_BENCH=1`).
pub fn bench_enabled() -> bool {
    std::env::var(ENV_BENCH).is_ok_and(|v| v == "1")
}

#[derive(Debug, Default)]
struct BenchEnv {
    bench: Option<String>,
    exit_after_ready: Option<String>,
    root: Option<String>,
    log: Option<String>,
    rate: Option<String>,
}

impl BenchEnv {
    fn from_process() -> Self {
        let get = |k: &str| std::env::var(k).ok();
        Self {
            bench: get(ENV_BENCH),
            exit_after_ready: get(ENV_EXIT),
            root: get(ENV_ROOT),
            log: get(ENV_LOG),
            rate: get(ENV_RATE),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BenchMode {
    Disabled,
    Ready {
        exit_after_ready: bool,
        replay: bool,
    },
}

/// Pure env decision. Replay only when a log is configured and we are not exiting.
fn decide(bench: Option<&str>, exit_after_ready: Option<&str>, log: Option<&str>) -> BenchMode {
    if bench != Some("1") {
        return BenchMode::Disabled;
    }
    let exit_after_ready = exit_after_ready == Some("1");
    BenchMode::Ready {
        exit_after_ready,
        replay: log.is_some() && !exit_after_ready,
    }
}

/// Replay input. Holds the already-opened log handle so replay never reopens the path
/// (no check/use race); `log` is the canonical path, kept for diagnostics only.
#[derive(Debug)]
struct ReplayPlan {
    file: File,
    log: PathBuf,
    rate: u32,
}

fn parse_rate(raw: Option<&str>) -> Result<u32, String> {
    let Some(raw) = raw else { return Ok(MAX_RATE) };
    match raw.parse::<u32>() {
        Ok(rate) if (MIN_RATE..=MAX_RATE).contains(&rate) => Ok(rate),
        _ => Err(format!(
            "{ENV_RATE} must be an integer in {MIN_RATE}..={MAX_RATE}, got {raw:?}"
        )),
    }
}

fn require_absolute<'a>(name: &str, value: &'a str) -> Result<&'a Path, String> {
    let path = Path::new(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!("{name} must be an absolute path"))
    }
}

/// Reject a root that is `/`, the user's HOME, or any ancestor of HOME (too broad to
/// be a fixture dir). `root` and `home` must already be canonical.
fn validate_root(root: &Path, home: Option<&Path>) -> Result<(), String> {
    if root == Path::new("/") || home.is_some_and(|h| h.starts_with(root)) {
        return Err(format!(
            "{ENV_ROOT} is too broad (/, HOME, or a parent of HOME)"
        ));
    }
    Ok(())
}

/// Open without following a final-component symlink and check the opened handle
/// (fstat, not path stat): regular file within the size cap.
fn open_log(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("{ENV_LOG} unreadable: {e}"))?;
    let meta = file
        .metadata()
        .map_err(|e| format!("{ENV_LOG} unreadable: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{ENV_LOG} is not a regular file"));
    }
    if meta.len() > MAX_REPLAY_BYTES {
        return Err(format!("{ENV_LOG} exceeds {MAX_REPLAY_BYTES} bytes"));
    }
    Ok(file)
}

/// Validate replay inputs. Log must canonicalize to a regular file strictly under the
/// canonical root (symlink escapes resolve outside the root and are rejected).
/// `home` is the (uncanonicalized) HOME used for the root breadth check.
fn resolve_replay_plan(
    root: Option<&str>,
    log: &str,
    rate: Option<&str>,
    home: Option<&Path>,
) -> Result<ReplayPlan, String> {
    let rate = parse_rate(rate)?;
    let root = root.ok_or_else(|| format!("{ENV_ROOT} is required when {ENV_LOG} is set"))?;
    let root = require_absolute(ENV_ROOT, root)?
        .canonicalize()
        .map_err(|e| format!("{ENV_ROOT} unusable: {e}"))?;
    if !root.is_dir() {
        return Err(format!("{ENV_ROOT} is not a directory"));
    }
    validate_root(&root, home.and_then(|h| h.canonicalize().ok()).as_deref())?;
    let requested = require_absolute(ENV_LOG, log)?;
    let log = requested
        .canonicalize()
        .map_err(|e| format!("{ENV_LOG} unusable: {e}"))?;
    if log == root || !log.starts_with(&root) {
        return Err(format!("{ENV_LOG} must be inside {ENV_ROOT}"));
    }
    // Open the configured path, not its canonical target: O_NOFOLLOW then rejects a
    // final-component symlink even when it resolves inside the root.
    let file = open_log(requested)?;
    Ok(ReplayPlan { file, log, rate })
}

fn timespec_to_ns(sec: i64, nsec: i64) -> Option<u64> {
    u64::try_from(sec)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u64::try_from(nsec).ok()?)
}

fn monotonic_ns() -> Result<u64, String> {
    let ts = clock_gettime(ClockId::CLOCK_MONOTONIC)
        .map_err(|e| format!("clock_gettime(CLOCK_MONOTONIC) failed: {e}"))?;
    timespec_to_ns(ts.tv_sec(), ts.tv_nsec())
        .ok_or_else(|| "CLOCK_MONOTONIC value out of u64 nanosecond range".to_string())
}

/// True for exactly one caller per flag.
fn claim(flag: &AtomicBool) -> bool {
    !flag.swap(true, Ordering::AcqRel)
}

#[derive(Debug)]
struct Started {
    ns: u64,
    exit_after_ready: bool,
    replay: Option<ReplayPlan>,
}

/// Validate everything, then claim the one-shot. Any error leaves the flag unclaimed.
/// `Ok(None)`: disabled or already fired.
fn begin(flag: &AtomicBool, env: &BenchEnv) -> Result<Option<Started>, String> {
    let BenchMode::Ready {
        exit_after_ready,
        replay,
    } = decide(
        env.bench.as_deref(),
        env.exit_after_ready.as_deref(),
        env.log.as_deref(),
    )
    else {
        return Ok(None);
    };
    if flag.load(Ordering::Acquire) {
        return Ok(None);
    }
    let replay = match env.log.as_deref().filter(|_| replay) {
        Some(log) => Some(resolve_replay_plan(
            env.root.as_deref(),
            log,
            env.rate.as_deref(),
            std::env::var_os("HOME").as_deref().map(Path::new),
        )?),
        None => None,
    };
    let ns = monotonic_ns()?;
    if !claim(flag) {
        return Ok(None);
    }
    Ok(Some(Started {
        ns,
        exit_after_ready,
        replay,
    }))
}

/// Frontend-ready signal for the startup benchmark. No-op unless `CROSSHOOK_BENCH=1`.
#[tauri::command]
pub async fn bench_ready(app: AppHandle) -> Result<(), String> {
    // Surface config errors on stderr so the runner sees why READY never came.
    // Messages carry env var names and OS error text only, never file contents.
    let started = begin(&READY_FIRED, &BenchEnv::from_process()).map_err(|error| {
        eprintln!("BENCH_ERROR: {error}");
        error
    })?;
    let Some(started) = started else {
        return Ok(());
    };
    {
        let mut err = std::io::stderr().lock();
        writeln!(err, "READY {}", started.ns)
            .and_then(|()| err.flush())
            .map_err(|e| format!("failed to write READY marker: {e}"))?;
    }
    if started.exit_after_ready {
        app.exit(0);
    } else if let Some(plan) = started.replay {
        tauri::async_runtime::spawn(replay_log(app, plan));
    }
    Ok(())
}

async fn replay_log(app: AppHandle, plan: ReplayPlan) {
    let mut buf = Vec::new();
    let read = async {
        let file = tokio::fs::File::from_std(plan.file);
        file.take(MAX_REPLAY_BYTES).read_to_end(&mut buf).await
    };
    if let Err(error) = read.await {
        tracing::warn!(%error, log = %plan.log.display(), "bench log replay: read failed");
        return;
    }
    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.is_empty())
        .take(MAX_REPLAY_LINES)
        .collect();

    let start = Instant::now();
    let mut ticker = tokio::time::interval(TICK);
    let mut sent = 0usize;
    while sent < lines.len() {
        ticker.tick().await;
        let due = usize::try_from(
            u128::from(plan.rate).saturating_mul(start.elapsed().as_millis()) / 1000,
        )
        .unwrap_or(usize::MAX)
        .min(lines.len());
        for line in &lines[sent..due] {
            if let Err(error) = app.emit("launch-log", *line) {
                tracing::warn!(%error, "bench log replay: emit failed; stopping");
                return;
            }
        }
        sent = sent.max(due);
    }
    tracing::info!(lines = sent, "bench log replay complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn env(bench: &str) -> BenchEnv {
        BenchEnv {
            bench: Some(bench.to_string()),
            ..BenchEnv::default()
        }
    }

    #[test]
    fn decide_gates_on_exact_one() {
        assert_eq!(decide(None, None, None), BenchMode::Disabled);
        assert_eq!(
            decide(Some("0"), Some("1"), Some("/x")),
            BenchMode::Disabled
        );
        assert_eq!(decide(Some("true"), None, None), BenchMode::Disabled);
        assert_eq!(
            decide(Some("1"), None, None),
            BenchMode::Ready {
                exit_after_ready: false,
                replay: false
            }
        );
    }

    #[test]
    fn decide_exit_flag_and_replay() {
        assert_eq!(
            decide(Some("1"), Some("1"), None),
            BenchMode::Ready {
                exit_after_ready: true,
                replay: false
            }
        );
        assert_eq!(
            decide(Some("1"), Some("0"), Some("/l")),
            BenchMode::Ready {
                exit_after_ready: false,
                replay: true
            }
        );
        // exit wins over replay
        assert_eq!(
            decide(Some("1"), Some("1"), Some("/l")),
            BenchMode::Ready {
                exit_after_ready: true,
                replay: false
            }
        );
    }

    #[test]
    fn claim_is_one_shot() {
        let flag = AtomicBool::new(false);
        assert!(claim(&flag));
        assert!(!claim(&flag));
    }

    #[test]
    fn begin_disabled_does_not_claim() {
        let flag = AtomicBool::new(false);
        assert!(begin(&flag, &env("0")).unwrap().is_none());
        assert!(!flag.load(Ordering::SeqCst));
    }

    #[test]
    fn begin_fires_once() {
        let flag = AtomicBool::new(false);
        let first = begin(&flag, &env("1")).unwrap().expect("first fires");
        assert!(first.ns > 0 && first.replay.is_none() && !first.exit_after_ready);
        assert!(begin(&flag, &env("1")).unwrap().is_none());
    }

    #[test]
    fn begin_error_does_not_consume_one_shot() {
        let flag = AtomicBool::new(false);
        let mut bad = env("1");
        bad.log = Some("/nonexistent/launch.log".into()); // root missing
        assert!(begin(&flag, &bad).is_err());
        assert!(!flag.load(Ordering::SeqCst));
        assert!(begin(&flag, &env("1")).unwrap().is_some());
    }

    #[test]
    fn timespec_conversion_is_checked() {
        assert_eq!(timespec_to_ns(2, 5), Some(2_000_000_005));
        assert_eq!(timespec_to_ns(-1, 0), None);
        assert_eq!(timespec_to_ns(0, -1), None);
        assert_eq!(timespec_to_ns(i64::MAX, 0), None);
    }

    #[test]
    fn rate_bounds() {
        assert_eq!(parse_rate(None), Ok(5000));
        assert_eq!(parse_rate(Some("1")), Ok(1));
        assert_eq!(parse_rate(Some("5000")), Ok(5000));
        for bad in ["0", "5001", "-1", "abc", "", "1.5"] {
            assert!(parse_rate(Some(bad)).is_err(), "{bad}");
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        log: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir_all(root.join("logs")).unwrap();
        let log = root.join("logs/launch-50k.log");
        std::fs::write(&log, "a\nb\n").unwrap();
        Fixture {
            _dir: dir,
            root,
            log,
        }
    }

    fn plan(f: &Fixture, log: &Path, rate: Option<&str>) -> Result<ReplayPlan, String> {
        resolve_replay_plan(f.root.to_str(), log.to_str().unwrap(), rate, None)
    }

    fn plan_parts(p: Result<ReplayPlan, String>) -> (PathBuf, u32) {
        let p = p.unwrap();
        (p.log, p.rate)
    }

    #[test]
    fn plan_accepts_log_under_root() {
        let f = fixture();
        let (log, rate) = plan_parts(plan(&f, &f.log, Some("100")));
        assert_eq!(rate, 100);
        assert_eq!(log, f.log.canonicalize().unwrap());
    }

    #[test]
    fn plan_requires_root_absolute_and_existing() {
        let f = fixture();
        let log = f.log.to_str().unwrap();
        assert!(resolve_replay_plan(None, log, None, None).is_err());
        assert!(resolve_replay_plan(Some("relative/root"), log, None, None).is_err());
        assert!(resolve_replay_plan(Some("/nonexistent/root"), log, None, None).is_err());
        assert!(resolve_replay_plan(f.log.to_str(), log, None, None).is_err()); // root is a file
    }

    #[test]
    fn plan_rejects_bad_log() {
        let f = fixture();
        assert!(plan(&f, &f.root.join("logs/missing.log"), None).is_err());
        assert!(plan(&f, Path::new("logs/launch-50k.log"), None).is_err()); // relative
        assert!(plan(&f, &f.root.join("logs"), None).is_err()); // directory
        assert!(plan(&f, &f.root, None).is_err()); // root itself
        let outside = f._dir.path().join("outside.log");
        std::fs::write(&outside, "x\n").unwrap();
        assert!(plan(&f, &outside, None).is_err());
        assert!(plan(&f, &f.log, Some("0")).is_err());
    }

    #[test]
    fn plan_rejects_symlink_escape() {
        let f = fixture();
        let outside = f._dir.path().join("secret.txt");
        std::fs::write(&outside, "secret\n").unwrap();
        let link = f.root.join("logs/escape.log");
        symlink(&outside, &link).unwrap();
        assert!(plan(&f, &link, None).is_err());
        let dir_link = f.root.join("outdir");
        symlink(f._dir.path(), &dir_link).unwrap();
        assert!(plan(&f, &dir_link.join("outside.log"), None).is_err());
    }

    #[test]
    fn plan_rejects_symlink_swap_to_inside_root() {
        // Canonicalize validates the real target; O_NOFOLLOW re-checks the path at
        // open time, so a swap to an in-root link is still rejected.
        let f = fixture();
        let link = f.root.join("logs/swap.log");
        symlink(&f.log, &link).unwrap();
        assert!(plan(&f, &link, None).is_err());
    }

    #[test]
    fn plan_rejects_broad_root() {
        let f = fixture();
        let log = f.log.to_str().unwrap();
        assert!(resolve_replay_plan(Some("/"), log, None, None).is_err());
        // HOME ancestor and exact-HOME roots are too broad to trust as fixture dirs.
        assert!(resolve_replay_plan(f._dir.path().to_str(), log, None, Some(&f.root)).is_err());
        assert!(resolve_replay_plan(f.root.to_str(), log, None, Some(&f.root)).is_err());
        // Unrelated or nested-under-root HOME is fine.
        assert!(plan_parts(resolve_replay_plan(
            f.root.to_str(),
            log,
            None,
            Some(&f._dir.path().join("home"))
        ))
        .0
        .starts_with(&f.root));
    }

    #[test]
    fn plan_rejects_oversize_log() {
        let f = fixture();
        let big = f.root.join("logs/big.log");
        std::fs::File::create(&big)
            .unwrap()
            .set_len(MAX_REPLAY_BYTES + 1)
            .unwrap();
        assert!(plan(&f, &big, None).is_err());
    }
}
