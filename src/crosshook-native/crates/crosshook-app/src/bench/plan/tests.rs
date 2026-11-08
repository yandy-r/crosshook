use super::*;
use std::os::unix::fs::symlink;

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
    // Exact-HOME root is too broad to trust as a fixture dir.
    assert!(resolve_replay_plan(f.root.to_str(), log, None, Some(&f.root)).is_err());
    // Isolated HOME nested under the fixture root is the intended harness
    // layout (and likewise a root that is an ancestor of HOME): accepted.
    let home = f.root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    assert!(resolve_replay_plan(f.root.to_str(), log, None, Some(&home)).is_ok());
    assert!(resolve_replay_plan(f._dir.path().to_str(), log, None, Some(&f.root)).is_ok());
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
