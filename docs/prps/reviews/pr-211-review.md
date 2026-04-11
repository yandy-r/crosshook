# PR #211 Code Review — Phase 1 Flatpak Distribution (MVP Bundle)

- **Branch:** `feat/flatpak`
- **Head SHA:** `5864e62`
- **Scope:** 16 files, +2150 / −10
- **Reviewer:** `ycc:code-review --parallel` (3 agents: correctness, security, quality) + synthesis
- **Run date:** 2026-04-11
- **Decision:** COMMENT — strong Phase 1 foundation with several non-blocking improvements and one data-integrity concern worth fixing before wider distribution

---

## Validation results

| Check                                                                                   | Result                                                                                    |
| --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `cargo test -p crosshook-core -- app_id_migration platform`                             | **16/16 pass** (all new tests)                                                            |
| `shellcheck scripts/build-flatpak.sh`                                                   | **clean**                                                                                 |
| `desktop-file-validate packaging/flatpak/dev.crosshook.CrossHook.desktop`               | **pass** (1 pedantic hint)                                                                |
| `appstreamcli validate packaging/flatpak/dev.crosshook.CrossHook.metainfo.xml`          | **pass**                                                                                  |
| Empirical sandbox behavior probe (`flatpak run --command=bash dev.crosshook.CrossHook`) | see **Finding 2**                                                                         |
| PR labels                                                                               | `type:feature`, `type:build`, `area:build`, `platform:linux`, `priority:high` — compliant |
| Linked issues                                                                           | Closes #196, #197, #198, #199, #207 — compliant                                           |
| `origin/main` identifier history                                                        | Only `com.crosshook.native` ever reached `main`; single-hop migration is sufficient       |

---

## What's strong here

- **Test hygiene.** The `ScopedEnv` + `FLATPAK_ID_LOCK` pattern in `platform.rs` correctly serialises env mutation across concurrent tests, and the `FakeEnv`/`EnvSink` indirection lets the XDG override logic be unit-tested without touching the real process environment. Migration tests run in tempdirs with explicit fixtures for every branch (missing source, missing dest, empty dest, non-empty dest, one-root failure, symlink-to-file preservation).
- **Doc comments with SAFETY notes** on every `unsafe` boundary in `platform.rs`, with a clear statement of the single-threaded startup precondition.
- **Scaffolded, not half-implemented.** `host_command()` is scaffolded for Phase 3 migration and unit-tested for both branches, but no existing `Command::new` call sites were migrated in this PR. That's the right scope discipline.
- **`/app/resources` fallback** in `paths.rs` correctly guards with `flatpak_path.exists()` so a missing script fails loudly at spawn time rather than returning a silently bogus `Some`.
- **PRD §10 meets CLAUDE.md persistence requirements** — storage boundary table classifying each datum (TOML / SQLite / runtime-only), migration & backward compatibility, offline, degraded, and concurrent-write caveats.
- **Validator compliance chain documented.** The rationale for `dev.crosshook.CrossHook` over `io.github.yandy_r.CrossHook` is captured in PRD:74 with the specific validator (`cid-rdns-contains-hyphen`) that rejected the earlier candidates.
- **Ordering is correct.** `override_xdg_for_flatpak_host_access()` is the first statement in `run()`, ahead of every `BaseDirs::new()` call site and ahead of the app-id migration. The AppImage re-exec path also re-runs the override in the child.

---

## Findings

Severity scale: **CRITICAL** (must fix before merge) · **HIGH** (fix before merge unless explicitly deferred) · **MEDIUM** (fix before merge, or track as follow-up) · **LOW** (non-blocking) · **NIT** (style/polish).

All findings below are **Open** unless marked otherwise.

---

### [1] PR description references stale app-ID and stale test count — LOW · Status: Open

**Where:** PR #211 body.

**What:** The description was written before commit `55179c6` renamed the app-ID from `io.github.yandy_r.CrossHook` to `dev.crosshook.CrossHook`. Every occurrence of the old ID in the PR body is now inconsistent with the shipped code. The description also claims "788 core tests + 4 integration tests" but `cargo test -p crosshook-core --lib` now runs 793 tests (16 new).

**Why it matters:** Future reviewers reading the PR body see a different identifier than what the code installs. Contributors searching for `io.github.yandy_r.CrossHook` in the issue/PR history will find this PR and be misled.

**Suggested fix:** Edit the PR description:

1. Replace every `io.github.yandy_r.CrossHook` with `dev.crosshook.CrossHook`.
2. Update the filename mentions in the "Packaging artifacts" section (`packaging/flatpak/dev.crosshook.CrossHook.{yml,desktop,metainfo.xml}`).
3. Update the "788 core tests + 4 integration" line to reflect current counts.
4. Note the subsequent fix commits (`41266e1` XDG redirect, `e213627` frontend embed, `8c55430` --rebuild flag, `5864e62` doc/env alignment) and, briefly, the T13/T14 results after those fixes.
5. Consider adding a note that the PR now also contains Phase 1 hardening commits on top of the original scope.

---

### [2] `override_xdg_for_flatpak_host_access` should prefer `HOST_XDG_*_HOME` env vars over `$HOME/.config` derivation — MEDIUM · Status: Fixed

**Where:** `src/crosshook-native/crates/crosshook-core/src/platform.rs:102-120` (`apply_xdg_host_override`)

**What:** Empirically verified on this machine against the installed `dev.crosshook.CrossHook` flatpak:

```text
HOME=/home/yandy
XDG_CONFIG_HOME=/home/yandy/.var/app/dev.crosshook.CrossHook/config
HOST_XDG_CONFIG_HOME=/home/yandy/.config
HOST_XDG_DATA_HOME=/home/yandy/.local/share
```

Two things this establishes:

1. **`$HOME` inside the sandbox IS the real host home** (not the per-app dir), so the current implementation works for users with a **default XDG layout**. The correctness reviewer's claim here is right and the PRD §10.2 validation was accurate.
2. **Flatpak exposes `HOST_XDG_CONFIG_HOME`, `HOST_XDG_DATA_HOME`, `HOST_XDG_CACHE_HOME`, `HOST_XDG_STATE_HOME`** specifically for cases like this one — they carry the host's actual XDG values, including user customizations. The current implementation derives XDG paths from `$HOME` and silently ignores them.

The user-visible consequence of the gap: if a user sets `XDG_CONFIG_HOME=/data/configs` on their host, their AppImage install writes settings to `/data/configs/crosshook/`, but the Flatpak install writes to `/home/user/.config/crosshook/`. The two installs **do not share data**, contradicting the documented goal. This is not theoretical — users who follow the XDG spec at all seriously do this.

**Why it matters:** The function docstring explicitly says its purpose is "so the Flatpak build and the AppImage share the same data on disk". That invariant is broken for any user with a customized XDG layout. The fix is small, easy to test, and removes a future user support escalation.

**Suggested fix:** Prefer `HOST_XDG_*_HOME` when set, fall back to the `$HOME`-derived default when absent. Roughly:

```rust
fn apply_xdg_host_override(home: Option<PathBuf>, sink: &mut dyn EnvSink) -> bool {
    fn host_or_default(host_var: &str, home: &Path, default_rel: &[&str]) -> OsString {
        if let Some(v) = std::env::var_os(host_var) {
            return v;
        }
        let mut p = home.to_path_buf();
        for s in default_rel { p.push(s); }
        p.into_os_string()
    }

    let Some(home) = home else { /* ... */ return false; };

    let config = host_or_default("HOST_XDG_CONFIG_HOME", &home, &[".config"]);
    let data   = host_or_default("HOST_XDG_DATA_HOME",   &home, &[".local", "share"]);
    let cache  = host_or_default("HOST_XDG_CACHE_HOME",  &home, &[".cache"]);

    sink.set("XDG_CONFIG_HOME", &config);
    sink.set("XDG_DATA_HOME",   &data);
    sink.set("XDG_CACHE_HOME",  &cache);
    // ...
}
```

Add a test that injects a `FakeEnv`-like reader for `HOST_XDG_CONFIG_HOME=/data/configs` and asserts the override writes `/data/configs` instead of `$HOME/.config`. Also add a doc comment note that the function preserves `$HOME` (which Flatpak does NOT remap) and that `HOST_XDG_*_HOME` is the Flatpak-provided path to the host's real XDG values.

---

### [3] `override_xdg_for_flatpak_host_access` doc comment should explicitly state that Flatpak preserves `$HOME` — LOW · Status: Open

**Where:** `src/crosshook-native/crates/crosshook-core/src/platform.rs:35-60`

**What:** The function doc explains that Flatpak remaps `XDG_*` vars but never states that `$HOME` itself is preserved pointing at the real host home. This is a counterintuitive Flatpak fact — several readers (including me on first pass) will default-assume `$HOME` is also remapped.

**Why it matters:** A future contributor looking at this code and thinking "wait, `$HOME` inside the sandbox is the per-app dir, this function is broken" will either remove it or add a broken workaround. The empirical output from Finding 2 is the ground truth; put it in the doc.

**Suggested fix:** Add one sentence at the top of the doc comment:

```rust
/// Note: Flatpak remaps `XDG_CONFIG_HOME` / `XDG_DATA_HOME` / `XDG_CACHE_HOME`
/// to per-app directories under `~/.var/app/<id>/`, but it does NOT remap
/// `$HOME` — that still points at the real user home (e.g. `/home/alice`).
/// This function relies on that distinction.
```

---

### [4] Cross-device migration failure leaves `new_root` in a partially-populated state that blocks future migrations — MEDIUM · Status: Fixed

**Where:** `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:99-115`

**What:** When `fs::rename` fails (cross-device), the fallback is `copy_dir_recursive(old_root, new_root)` followed by `fs::remove_dir_all(old_root)`. If `copy_dir_recursive` fails mid-way (disk full, permissions, ENOSPC on the target fs, interrupt), it returns `Err` — but `new_root` now contains a partial copy of the old data. On the next startup, `new_root.exists() && !dir_is_empty(new_root)` will evaluate true, the migration is silently skipped, `old_root` remains intact, and the user's data is permanently split between two trees.

**Why it matters:** This is a data-integrity failure mode that is invisible to the user: no error panel, no re-try on next startup, and the UI will quietly load a partial profile set. For any user whose `~/.config/` and `~/.local/share/` are on different filesystems (common on NAS-mounted homes, LUKS-encrypted data volumes, `/home` on one drive and custom XDG on another), this is a realistic single-failure scenario.

**Suggested fix:** Use a staged rename pattern so the partial state is never visible:

1. Stage the copy to a sibling temp path: `let stage = new_root.with_file_name(format!("{}.migrating", CURRENT_TAURI_APP_ID_DIR));`
2. `copy_dir_recursive(old_root, &stage)?` — if this fails, `fs::remove_dir_all(&stage)` and return `Err`.
3. After copy succeeds, `fs::rename(&stage, new_root)?` — cheap because it's same-parent rename.
4. Only then `fs::remove_dir_all(old_root)`.

This preserves the invariant "`new_root` non-empty ⇒ migration succeeded". Add a unit test using a filesystem error injector (e.g. mock write failure at a specific depth) to cover the partial-failure recovery path.

---

### [5] `flatpak-spawn --host` does not forward env vars set by the caller — document the contract on `host_command()` — MEDIUM · Status: Fixed

**Where:** `src/crosshook-native/crates/crosshook-core/src/platform.rs:73-82` (`host_command_with`)

**What:** `host_command()` returns `Command::new("flatpak-spawn").arg("--host").arg(program)`. If a caller in Phase 3 writes:

```rust
let mut cmd = host_command("proton");
cmd.env("STEAM_COMPAT_DATA_PATH", "/home/user/prefixes/foo");
cmd.env("PROTON_LOG", "1");
cmd.arg("run").arg("game.exe");
```

…those env vars are set on the `flatpak-spawn` process, not forwarded to the host `proton` process. `flatpak-spawn --host` requires explicit `--env=KEY=VALUE` arguments to propagate env vars to the host executable. The current implementation does not do this, and has no doc comment warning that `.env()` calls on the returned `Command` will silently fail.

**Why it matters:** Phase 3 will migrate Proton/Wine/Steam launch sites to `host_command()`. All of those launches depend on a specific set of `STEAM_COMPAT_*`, `PROTON_*`, `WINEPREFIX`, `MANGOHUD_CONFIG`, and `GAMESCOPE_*` env vars. If callers migrate naively and keep using `.env()`, games will launch with wrong prefixes, silently broken ProtonDB integration, broken MangoHud overlays, and wrong Steam app IDs — and the bugs will be attributed to Proton, not to this scaffold.

**Suggested fix:** Either:

**(a)** Add a strongly-worded doc comment on `host_command()`:

````rust
/// # Env var forwarding
///
/// `.env()` / `.envs()` calls on the returned `Command` set env vars on the
/// `flatpak-spawn` process, NOT on the host program. Inside Flatpak, you
/// MUST pass host env vars as `--env=KEY=VALUE` args before `program`:
///
/// ```ignore
/// let mut cmd = host_command("proton");
/// if is_flatpak() {
///     cmd.arg("--env=STEAM_COMPAT_DATA_PATH=/foo");
/// } else {
///     cmd.env("STEAM_COMPAT_DATA_PATH", "/foo");
/// }
/// ```
///
/// See https://docs.flatpak.org/en/latest/flatpak-command-reference.html#flatpak-spawn
````

**(b)** Add a `host_command_with_env(program, envs: &HashMap<String, String>)` helper that automatically threads envs through `--env` when in Flatpak and uses `.envs()` otherwise. Phase 3 callers use the helper unconditionally.

Option (b) is more ergonomic and avoids the footgun entirely. I'd pick (b).

---

### [6] `migrate_one_app_id_root` uses stringly-typed errors instead of typed variants — MEDIUM · Status: Fixed

**Where:** `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:69` and `186`

**What:** Public signature is `-> Result<(), String>`; the test helper returns `Vec<String>`. The dominant pattern in `crosshook-core` is typed error variants (`ProfileStoreError`, `CommunityTapError`, etc.) or `anyhow::Error`. The crate-level CLAUDE.md also says "errors via `Result` with `anyhow` or project error types".

**Why it matters:** Callers cannot match on error categories; every consumer must parse the string. In Phase 3/4, when a legitimate caller needs to distinguish "destination already migrated" from "disk full" from "permission denied", the surface must be broken-changed. The `tracing::warn!` inside already distinguishes categories — externalise that into a type now, costs little.

**Suggested fix:** Small enum:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppIdMigrationError {
    #[error("io error at {path}: {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("destination {0} exists and is non-empty; migration skipped")]
    DestinationNotEmpty(PathBuf),
}
```

Or just use `anyhow::Result<()>` since this is a best-effort startup path and the `eprintln!` already swallows everything. Either is fine — just not stringly-typed.

---

### [7] Pre-existing `set_var` / `remove_var` at `src-tauri/src/lib.rs:83,94` lack `unsafe` blocks and safety comments — LOW (pre-existing, not introduced by this PR) · Status: Open (non-blocking)

**Where:** `src/crosshook-native/src-tauri/src/lib.rs:83` and `94`

**What:** Lines 83 and 94 call `std::env::set_var` / `std::env::remove_var` in plain safe Rust:

```rust
if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");   // :83
}
// ...
if std::env::var_os("WAYLAND_DISPLAY").is_some() && ... {
    std::env::remove_var("GDK_BACKEND");                         // :94
}
```

**Context:** `git show origin/main:src/crosshook-native/src-tauri/src/lib.rs` confirms both lines already exist on `main`. They are **not** introduced by this PR. Both `crosshook-core` and `crosshook-native` are on `edition = "2021"` (verified in `Cargo.toml`), so these compile cleanly today. However, the new code on line 30 (`unsafe { override_xdg_for_flatpak_host_access() }`) has an explicit SAFETY comment justifying the single-threaded precondition, while lines 83 and 94 have neither `unsafe` blocks nor safety comments — the inconsistency is new.

**Why it matters:** Concurrent getenv/setenv is UB in POSIX regardless of Rust's `unsafe` marker. The calls are safe **in practice** because they run in the same single-threaded startup window before `tauri::Builder::default()`, but that invariant is undocumented and fragile. In Rust 2024 (stabilised, and a likely near-term upgrade), these become hard compile errors. The inconsistency with the new `unsafe` block above makes the lack of justification conspicuous.

**Not a merge blocker for this PR** — these lines pre-date it. But since the PR author is already handling `unsafe { set_var }` hygiene in `platform.rs`, the cheap follow-up is to align the lib.rs call sites.

**Suggested fix:** Wrap both in `unsafe` blocks with an inline SAFETY comment:

```rust
// SAFETY: single-threaded startup before any Tauri runtime threads exist.
// Matches the precondition of `override_xdg_for_flatpak_host_access` at line 30.
unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1"); }
```

This makes the intent explicit, avoids a future edition upgrade breaking the build, and documents the single-threaded invariant in one place that doesn't rot silently.

---

### [8] `--filesystem=home` is a Flathub blocker — acknowledge explicitly in PR description and manifest — HIGH (deferred) · Status: Accepted as Phase 1 scope

**Where:** `packaging/flatpak/dev.crosshook.CrossHook.yml:48`

**What:** `--filesystem=home` grants read/write to the entire host home directory and is on Flathub's explicit rejection list. The PR's stated scope is "local build only; Flathub submission is Phase 4" (confirmed in the PR description), so this is a known deferred concern — but the PR description does not explicitly flag this as a Phase 4 blocker. Without a visible flag, Phase 4 will rediscover this during Flathub submission review and be surprised.

**Why it matters:** Reworking filesystem permissions is not a one-liner: it requires auditing every path CrossHook opens, replacing `BaseDirs`-derived paths with `--filesystem=xdg-config`/`xdg-data`/`xdg-cache` grants plus narrower grants for `/mnt`, `/run/media`, `/media`, and possibly `xdg-download` for game installs. The longer this lives as the accepted approach, the more Phase 3 code will silently depend on full-home access.

**Suggested fix:** Two cheap paper-trail fixes:

1. Add a YAML comment block above line 48 of the manifest:

   ```yaml
   # Phase 1 shortcut: --filesystem=home grants read/write to the entire host
   # home. Flathub will reject this at submission. Phase 4 must replace with:
   #   --filesystem=xdg-config
   #   --filesystem=xdg-data
   #   --filesystem=xdg-cache
   # plus explicit grants for /mnt, /run/media, /media where Steam libraries
   # live, and a portal-based file chooser for user-selected paths.
   # Tracking: (link to Phase 4 issue once filed)
   - --filesystem=home
   ```

2. Add a "Flathub blockers" subsection in the PR description listing this + the placeholder screenshot + empty OARS rating, so the delta to Flathub submission is explicit.

---

### [9] `eprintln!` inside `migrate_legacy_tauri_app_id_xdg_directories` is undocumented — LOW · Status: Open

**Where:** `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:149-153`

**What:** The function emits both `tracing::warn!` and `eprintln!` on the same error path. The doc comment on the function says only "Best-effort: logs warnings and continues". The `eprintln!` exists because `migrate_legacy_tauri_app_id_xdg_directories` is called at `src-tauri/src/lib.rs:33`, which is before `logging::init_logging` fires later in the Tauri setup closure.

**Why it matters:** A future contributor will look at the duplicate output on a non-Flatpak run and think the `eprintln!` is dead code, delete it, and silently drop the only human-visible diagnostic for a migration failure at startup.

**Suggested fix:** One-sentence doc comment addendum:

```rust
/// Uses `eprintln!` alongside `tracing::warn!` because this runs before
/// `logging::init_logging` initialises the tracing subscriber, so the
/// tracing output would otherwise be dropped.
```

---

### [10] `override_xdg_for_flatpak_host_access` public function has no test for the "not in Flatpak" branch — LOW · Status: Open

**Where:** `src/crosshook-native/crates/crosshook-core/src/platform.rs:62`

**What:** The public function short-circuits when `!is_flatpak()`, but no test exercises this exact path. `xdg_override_noop_when_home_unset` tests `apply_xdg_host_override(None, ...)` returns false; no test asserts the public-fn early return.

**Why it matters:** The test seam is already good (`apply_xdg_host_override` is tested with `FakeEnv`). The gap is the public contract — if someone inverts the early return, the suite stays green. Small gap, small fix.

**Suggested fix:** Either split the early-return into a separately testable helper:

```rust
fn should_apply_xdg_override(flatpak: bool) -> bool { flatpak }
```

…and test it for both values. Or add a doc comment explicitly noting the Flatpak-false path is covered by `is_flatpak_with` returning false in the four-case matrix.

---

### [11] `copy_dir_recursive` symlink coverage is partial (file only, not directory or broken) — LOW · Status: Open

**Where:** `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:46`

**What:** The test `copy_dir_recursive_preserves_symlink_to_file` covers symlinks pointing to files. The Unix branch in `copy_symlink` (`fs::read_link` + `std::os::unix::fs::symlink`) is generic and symlink-to-directory should also work, but there is no test asserting it. There is also no test for broken (dangling) symlinks.

**Why it matters:** Real `~/.config/crosshook/` and `~/.local/share/crosshook/` trees can contain symlinks to Proton prefixes (directories) and the occasional dangling link (cleanup-after-removal). Both should copy as symlinks, not as the dereferenced target.

**Suggested fix:** Add two tests:

1. `copy_dir_recursive_preserves_symlink_to_dir` — symlink from `src/link/` to `src/target_dir/`; assert `dst/link` is a symlink and `read_link` returns `target_dir`.
2. `copy_dir_recursive_preserves_broken_symlink` — symlink pointing to a non-existent path; assert the copy succeeds and the broken link is preserved at `dst/`.

---

### [12] `arch_suffix_for_triple` is duplicated between `build-flatpak.sh` and `build-native.sh` — LOW · Status: Open

**Where:** `scripts/build-flatpak.sh:122-129` vs `scripts/build-native.sh:25-43`

**What:** Both define `arch_suffix_for_triple` (differently named, identical logic) mapping `x86_64-*` → `amd64`, `aarch64-*|arm64-*` → `arm64`, `armv7-*` → `armv7`, fallback to the triple's prefix.

**Why it matters:** Adding a new target triple requires updating both functions. One missed update produces a misnamed bundle with no build error. Both scripts already source `scripts/lib/build-paths.sh`, which is the natural home for the helper.

**Suggested fix:** Move a single `crosshook_arch_suffix()` function into `scripts/lib/build-paths.sh` and have both scripts call it. Delete the per-script definitions.

---

### [13] `runtime-version` in manifest has no sync check against `CROSSHOOK_FLATPAK_RUNTIME_VERSION` — LOW · Status: Open

**Where:** `packaging/flatpak/dev.crosshook.CrossHook.yml:22` (hardcoded `"50"`) and `scripts/build-flatpak.sh:35` (default `50`)

**What:** The GNOME runtime version is hardcoded in two places. `CROSSHOOK_FLATPAK_RUNTIME_VERSION` is consumed only by `--install-deps` for `flatpak install …//50`. The manifest `flatpak-builder` reads is always the committed file.

**Why it matters:** A developer running `CROSSHOOK_FLATPAK_RUNTIME_VERSION=52 ./scripts/build-flatpak.sh --install-deps` installs the SDK 52 runtime, then `flatpak-builder` reads `"50"` from the manifest and either fails loudly (if 50 is absent) or succeeds silently against a stale runtime. Neither is great.

**Suggested fix:** Add a preflight check that extracts the runtime version from the manifest (`grep -E '^runtime-version:' "$MANIFEST_SRC" | sed 's/.*"\(.*\)".*/\1/'`) and asserts it equals `$RUNTIME_VERSION`, `die`-ing with a clear message on drift. Alternatively, document the coupling in a comment near line 35 of the script.

---

### [14] `dir_is_empty` + `remove_dir` TOCTOU race is theoretical — NIT · Status: Acknowledged

**Where:** `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:78-79`

**What:** Between `dir_is_empty(new_root)` returning `true` and `fs::remove_dir(new_root)`, another process could race to populate `new_root`. In practice this is a startup-once migration on a rare first-run state. The error is propagated correctly and the migration is skipped for that root. No action needed.

---

### [15] `is_flatpak()` caching — NIT · Status: Acknowledged

**Where:** `src/crosshook-native/src-tauri/src/paths.rs:42` (called from `resolve_bundled_script_path`)

**What:** Each call does an env var lookup + `stat("/.flatpak-info")`. Both are cheap. A `OnceLock<bool>` cache would be theoretically better but not necessary for Phase 1. No action.

---

### [16] `STAGE_DIR` cleanup `rm -rf` is safe — verified · Status: Resolved

**Where:** `scripts/build-flatpak.sh:238-244`

**What:** `set -u` is active; `STAGE_DIR` is assigned from `mktemp -d` (which cannot return an empty string under `set -e`) before the `cleanup` trap is installed. If `mktemp` fails, the script exits before the trap fires. The `rm -rf "$STAGE_DIR"` cannot expand to `rm -rf ""`.

---

## Summary

| Severity        | Count | Items                                                                                                                                                                                                                             |
| --------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CRITICAL        | 0     | —                                                                                                                                                                                                                                 |
| HIGH (deferred) | 1     | [8] `--filesystem=home` Flathub blocker (acknowledged Phase 1 scope; add paper trail)                                                                                                                                             |
| MEDIUM          | 4     | [2] `HOST_XDG_*_HOME` portability, [4] partial-copy split-brain, [5] `flatpak-spawn` env forwarding contract, [6] typed errors for migration                                                                                      |
| LOW             | 6     | [1] PR description drift, [3] `$HOME` doc clarification, [7] pre-existing `set_var` hygiene, [9] `eprintln!` rationale, [10] test coverage, [11] symlink test coverage, [12] shell helper duplication, [13] runtime version drift |
| NIT / Resolved  | 3     | [14] TOCTOU, [15] `is_flatpak` caching, [16] `STAGE_DIR` rm safety                                                                                                                                                                |

## Decision: COMMENT

This is a well-organized Phase 1 MVP that stays inside its declared scope, respects CLAUDE.md persistence planning requirements, ships with strong test isolation and validator-clean metadata, and correctly scaffolds Phase 3 without half-implementing it. None of the findings block merge in isolation.

**However**, two findings deserve pre-merge attention:

- **[4] partial-copy split-brain** is a silent data-integrity failure mode on a realistic single-failure input (cross-fs `.config` ↔ `.local/share` + mid-copy error). Low probability, high blast radius. Cheap to fix.
- **[2] `HOST_XDG_*_HOME`** breaks the documented "AppImage and Flatpak share data" invariant for any user with a customized XDG layout. Easy fix, narrow blast radius, silent failure mode.

Strongly recommended before merging: fix [4] and [2]; update the PR description per [1] and [8]; land the other LOW items as a short follow-up commit on this branch or track in a Phase 1.1 hardening issue.

The remaining LOW items are genuine maintenance hazards that will compound, but don't block the MVP.
