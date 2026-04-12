# PRD: Flatpak Standardization (Phase 3.5)

**Status**: Ready for planning
**Date**: 2026-04-12
**Parent PRD**: [`flatpak-distribution.prd.md`](flatpak-distribution.prd.md)
**Research**: [`docs/prps/research/flatpak-standardization-research.md`](../research/flatpak-standardization-research.md)

---

## 1. Problem

CrossHook maintains two production distribution formats (AppImage + Flatpak) with divergent runtime behavior. Every feature touching process execution, filesystem access, or environment detection carries two code paths — the native path and the `is_flatpak()` / `host_command()` sandbox path. Phase 3 demonstrated the cost: Proton detection, launch commands, and utility availability all required Flatpak-specific fixes that consumed significant development time. Each fix for one runtime risks regression in the other.

**Hypothesis**: Standardizing on Flatpak as the sole distribution format before the app has a user base eliminates cross-format runtime divergence, reduces the code surface area, and prevents the "works on mine (AppImage) but not yours (Flatpak)" bug report category before it starts.

This is a **pre-marketing window decision** — there are no AppImage users to migrate, no download telemetry to weigh, and no backward-compatibility contract to honor. The cost of switching later (after users exist on both formats) is strictly higher.

---

## 2. Goals & Success Criteria

### 2.1 Goals

| #   | Goal                                                                                                      |
| --- | --------------------------------------------------------------------------------------------------------- |
| G1  | Remove all AppImage-specific code, scripts, CI steps, and documentation references                        |
| G2  | Flatpak (`.flatpak` bundle) becomes the sole GUI distribution artifact in GitHub Releases                 |
| G3  | Create `dev-flatpak.sh` — one-command Flatpak iteration workflow (build binary + package + install + run) |
| G4  | Rename `build-native.sh` to `build-binary.sh` — binary-only build, no packaging                           |
| G5  | Delete `build-native-container.sh` and `generate-crosshook-desktop.sh` (AppImage-only scripts)            |
| G6  | Update all documentation (CLAUDE.md, AGENTS.md, README, CONTRIBUTING, quickstart, etc.)                   |

### 2.2 Success Criteria

| Metric                               | Target                                                                       | Measurement                                                                  |
| ------------------------------------ | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| AppImage code removed                | Zero files reference AppImage as a build/ship target                         | `grep -r AppImage` returns only historical docs (CHANGELOG, completed plans) |
| CI produces Flatpak only             | `release.yml` uploads `.flatpak` + CLI tarball, no `.AppImage`               | Release artifact list                                                        |
| Dev iteration works                  | `./scripts/dev-flatpak.sh` builds, installs, and runs Flatpak in one command | Manual test                                                                  |
| `dev-native.sh` unchanged            | Hot-reload dev workflow still works for daily iteration                      | Manual test                                                                  |
| All 802 unit tests pass              | `cargo test -p crosshook-core` green                                         | CI                                                                           |
| Flatpak bundle installs and launches | `flatpak install --user` + `flatpak run dev.crosshook.CrossHook` works       | Manual test                                                                  |

### 2.3 Non-Goals

- Writing the Phase 4 build-from-source Flathub manifest (separate effort)
- Implementing Flatpak per-app XDG isolation (Phase 4 scope)
- Adding automated Flatpak integration tests (desirable but separate)
- Supporting Snap, Nix, or other package formats

---

## 3. Key Decisions

### 3.1 Timing: Phase 3.5, Not Phase 4

Do this as a standalone cleanup before Flathub submission. Rationale: shake out any issues from removing AppImage _before_ also dealing with build-from-source manifests and Flathub reviewer feedback. Two simultaneous transitions multiply risk.

### 3.2 `build-native.sh` -> `build-binary.sh`

Rename to reflect its new sole purpose: building the release binary via `tauri build --no-bundle`. Remove all AppImage naming, searching, copying logic. Remove `patchelf` dependency check. `build-flatpak.sh` already calls `build-native.sh --binary-only` — update to call `build-binary.sh`.

### 3.3 Delete `build-native-container.sh`

Exists only to work around AppImage toolchain issues (`linuxdeploy`, `patchelf`) on hosts where those tools don't work. With no AppImage bundling, the container build has no purpose. The Dockerfile (`build-native-container.Dockerfile`) is also deleted.

### 3.4 Delete `generate-crosshook-desktop.sh`

This 240-line script generates a desktop entry from an AppImage, including extracting and installing the embedded icon via `--appimage-extract`. Flatpak has its own committed desktop entry at `packaging/flatpak/dev.crosshook.CrossHook.desktop`. The script is entirely AppImage-specific.

### 3.5 Remove `tauri.conf.json` Bundle Targets

Change `"targets": ["appimage"]` to `"targets": []`. Tauri still builds the binary; it just produces no bundle artifact. `tauri dev` is unaffected (it never produces bundles). `tauri build --no-bundle` is already the path `build-flatpak.sh` uses.

### 3.6 Remove AppImage GPU Re-exec Hack

Delete `lib.rs:35-77` — the `APPIMAGE` env var check and system WebKitGTK re-exec path. This code only executes inside an AppImage. The Flatpak manifest handles GPU compatibility via `--env=WEBKIT_DISABLE_DMABUF_RENDERER=1` in `finish-args`. The DMA-BUF fallback at `lib.rs:80+` stays (applies to both native dev and Flatpak).

### 3.7 Remove XDG Override Bridge

Delete `platform.rs:172-210` (`override_xdg_for_flatpak_host_access()`). This function exists to share XDG state between AppImage and Flatpak — with no AppImage, there is no second consumer. Phase 4 per-app isolation replaces this with proper sandbox-local state.

**Wait — not yet.** The XDG override is still needed for Phase 3.5. It ensures the Flatpak uses host XDG paths (`~/.config/crosshook/`, etc.) rather than sandbox-local paths (`~/.var/app/dev.crosshook.CrossHook/...`). Without AppImage, the override still serves the same purpose: making the Flatpak see the user's existing data at standard host paths. Phase 4 replaces this with per-app isolation + first-run migration. **Keep the override in Phase 3.5; remove in Phase 4.**

### 3.8 Create `dev-flatpak.sh`

One-command Flatpak iteration workflow:

```bash
./scripts/dev-flatpak.sh           # Rebuild binary + Flatpak + install + run
./scripts/dev-flatpak.sh --run     # Skip build, just run installed Flatpak
./scripts/dev-flatpak.sh --shell   # Open bash inside the Flatpak sandbox
```

Implementation: chains `build-binary.sh` -> `build-flatpak.sh --rebuild --install` -> `flatpak run dev.crosshook.CrossHook`. Captures stdout/stderr for debugging.

### 3.9 Keep `lib/build-paths.sh` With Cleanup

Remove `crosshook_appimage_bundle_dirs()` (AppImage-specific). Keep `crosshook_build_paths_init()` — it resolves `DIST_DIR` and `CARGO_TARGET_DIR` for any build output, not just AppImage.

### 3.10 Keep `install-native-build-deps.sh` With Cleanup

Remove `patchelf` from all package lists (pacman, apt, dnf, zypper). Update usage text to say "Tauri binary" instead of "Tauri/AppImage target."

---

## 4. Scope of Changes

### 4.1 Files to Delete

| File                                        | Reason                                           |
| ------------------------------------------- | ------------------------------------------------ |
| `scripts/build-native-container.sh`         | AppImage container build only                    |
| `scripts/build-native-container.Dockerfile` | AppImage container image                         |
| `scripts/generate-crosshook-desktop.sh`     | Generates desktop entry from AppImage            |
| `scripts/lib/sync-tauri-icons.sh`           | Syncs icons into Tauri AppImage bundle structure |

### 4.2 Files to Rename

| From                      | To                        | Changes                                                              |
| ------------------------- | ------------------------- | -------------------------------------------------------------------- |
| `scripts/build-native.sh` | `scripts/build-binary.sh` | Remove AppImage logic (lines 16-42, 117-204); keep binary-only build |

### 4.3 Files to Modify

| File                                                         | Change                                                                                                                              |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src-tauri/src/lib.rs`                  | Delete lines 35-77 (AppImage GPU re-exec hack); update comment at line 24-28                                                        |
| `src/crosshook-native/src-tauri/tauri.conf.json`             | `"targets": ["appimage"]` -> `"targets": []`                                                                                        |
| `scripts/build-flatpak.sh`                                   | Update reference from `build-native.sh` to `build-binary.sh`                                                                        |
| `scripts/lib/build-paths.sh`                                 | Remove `crosshook_appimage_bundle_dirs()` function                                                                                  |
| `scripts/install-native-build-deps.sh`                       | Remove `patchelf` from all package lists; update usage text                                                                         |
| `scripts/generate-assets.sh`                                 | No change (icons are used by Flatpak too)                                                                                           |
| `scripts/dev-native.sh`                                      | No change (runs `tauri dev`, unrelated to AppImage packaging)                                                                       |
| `.github/workflows/release.yml`                              | Remove AppImage artifact from build-native job; remove AppImage from publish asset list; simplify build-native to binary + CLI only |
| `.github/pull_request_template.md`                           | Remove AppImage-specific checklist items                                                                                            |
| `src/crosshook-native/crates/crosshook-cli/src/args.rs`      | Update "bundled AppImage scripts" help text                                                                                         |
| `src/crosshook-native/crates/crosshook-core/src/platform.rs` | Update doc comments referencing AppImage; keep `is_flatpak()`, `host_command()`, and XDG override                                   |
| `.gitignore`                                                 | Remove any AppImage-specific entries                                                                                                |

### 4.4 Files to Create

| File                     | Purpose                                    |
| ------------------------ | ------------------------------------------ |
| `scripts/dev-flatpak.sh` | One-command Flatpak dev iteration workflow |

### 4.5 Documentation to Update

| File                                                    | Change                                                                      |
| ------------------------------------------------------- | --------------------------------------------------------------------------- |
| `CLAUDE.md`                                             | Remove AppImage references from commands section; update build scripts list |
| `AGENTS.md`                                             | Update stack overview, build scripts, distribution format references        |
| `.cursorrules`                                          | Update if present — same as AGENTS.md                                       |
| `README.md`                                             | Update installation instructions (Flatpak only); update download links      |
| `CONTRIBUTING.md`                                       | Update build instructions; remove AppImage toolchain references             |
| `docs/getting-started/quickstart.md`                    | Flatpak-only installation instructions                                      |
| `docs/internal-docs/local-build-publish.md`             | Update local build workflow to Flatpak-only                                 |
| `docs/internal-docs/steam-deck-validation-checklist.md` | Update to Flatpak-only validation                                           |
| `packaging/flatpak/README.md`                           | Update to reflect Flatpak as sole format (not secondary)                    |
| `docs/prps/prds/flatpak-distribution.prd.md`            | Add note that Phase 3.5 supersedes the "AppImage remains primary" non-goal  |

---

## 5. Architecture Impact

### 5.1 What Changes in the Runtime

**Before (dual-format)**:

```
User installs AppImage -> runs natively -> direct Command::new()
User installs Flatpak  -> runs in sandbox -> flatpak-spawn --host
```

Both paths are production paths requiring full verification.

**After (Flatpak-only)**:

```
Developer runs tauri dev   -> runs natively -> direct Command::new() (dev only)
User installs Flatpak      -> runs in sandbox -> flatpak-spawn --host (production)
```

One production path. The native path is dev-only convenience.

### 5.2 `is_flatpak()` Branching Stays

The `is_flatpak()` / `host_command()` branching in `platform.rs` remains because:

- `dev-native.sh` runs the binary natively (outside Flatpak), so `is_flatpak()` = false
- Production Flatpak runs in sandbox, so `is_flatpak()` = true
- The branching now represents a dev-vs-prod distinction, not a dual-production-runtime distinction

This is a standard, manageable gap — identical to any sandboxed app's dev workflow.

### 5.3 XDG Override Stays (Until Phase 4)

`override_xdg_for_flatpak_host_access()` is still needed. It ensures the Flatpak uses host XDG paths rather than sandbox-local paths. Without it, the Flatpak would silently start with empty data. Phase 4 replaces this with per-app isolation + first-run migration.

### 5.4 CI Pipeline Simplification

**Before** (3 jobs):

```
build-native (AppImage + binary + CLI) -> build-flatpak (Flatpak from binary) -> publish (3 artifacts)
```

**After** (3 jobs, simplified first job):

```
build-binary (binary + CLI only) -> build-flatpak (Flatpak from binary) -> publish (2 artifacts)
```

The `build-binary` job drops: `patchelf` dependency, AppImage search/copy, `generate-assets.sh` call (moved to `build-flatpak` which already handles icons), `sync-tauri-icons.sh` call. The mock-code verification step stays (checks `dist/assets/*.js`).

---

## 6. Dev Workflow: `dev-flatpak.sh`

### 6.1 Purpose

Fill the gap between `dev-native.sh` (fast hot-reload, no sandbox) and manual Flatpak build/install/run. When you need to test sandbox-specific behavior, one command does the full cycle.

### 6.2 Interface

```bash
./scripts/dev-flatpak.sh                   # Full cycle: build binary + Flatpak + install + run
./scripts/dev-flatpak.sh --run             # Run already-installed Flatpak (skip build)
./scripts/dev-flatpak.sh --shell           # Open bash inside the Flatpak sandbox for debugging
./scripts/dev-flatpak.sh --skip-binary     # Rebuild Flatpak from cached binary (skip Rust compilation)
```

### 6.3 Implementation

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ID="dev.crosshook.CrossHook"

case "${1:-}" in
  --run)
    exec flatpak run "$APP_ID"
    ;;
  --shell)
    exec flatpak run --command=bash "$APP_ID"
    ;;
  --skip-binary)
    "$ROOT_DIR/scripts/build-flatpak.sh" --skip-build --install
    exec flatpak run "$APP_ID"
    ;;
  "")
    "$ROOT_DIR/scripts/build-flatpak.sh" --rebuild --install
    exec flatpak run "$APP_ID"
    ;;
  *)
    echo "Error: unknown argument: $1" >&2
    exit 1
    ;;
esac
```

### 6.4 Iteration Speed

| Scenario                    | Approximate Time                            | Command                        |
| --------------------------- | ------------------------------------------- | ------------------------------ |
| Frontend change only        | ~30s (Vite rebuild + Flatpak repackage)     | `dev-flatpak.sh --skip-binary` |
| Rust change                 | ~60-90s (cargo release + Flatpak repackage) | `dev-flatpak.sh`               |
| No code change, just test   | <1s                                         | `dev-flatpak.sh --run`         |
| Debug inside sandbox        | <1s                                         | `dev-flatpak.sh --shell`       |
| Hot-reload dev (no sandbox) | Instant                                     | `dev-native.sh` (unchanged)    |

---

## 7. Testing Strategy

### 7.1 Pre-Merge Verification

| #   | Test                                                             | Expected                                                 | Priority |
| --- | ---------------------------------------------------------------- | -------------------------------------------------------- | -------- |
| T1  | `cargo test -p crosshook-core`                                   | All 802+ tests pass                                      | P0       |
| T2  | `./scripts/build-binary.sh` produces binary                      | Binary at `$DIST_DIR/crosshook-native`                   | P0       |
| T3  | `./scripts/build-flatpak.sh` produces `.flatpak`                 | Bundle at `$DIST_DIR/CrossHook_amd64.flatpak`            | P0       |
| T4  | `flatpak install --user` + `flatpak run dev.crosshook.CrossHook` | App launches, UI renders                                 | P0       |
| T5  | `./scripts/dev-native.sh` hot-reload still works                 | Vite + Tauri dev server starts                           | P0       |
| T6  | `./scripts/dev-native.sh --browser` still works                  | Vite dev server at localhost:5173                        | P0       |
| T7  | `./scripts/dev-flatpak.sh` full cycle                            | Builds, installs, runs Flatpak                           | P0       |
| T8  | `./scripts/dev-flatpak.sh --shell`                               | Opens bash inside sandbox                                | P1       |
| T9  | `grep -r 'AppImage' scripts/`                                    | No hits (only CHANGELOG, completed plans, research docs) | P0       |
| T10 | Steam library detection inside Flatpak                           | Discovers `~/.local/share/Steam`                         | P0       |
| T11 | Profile launch inside Flatpak                                    | Game launches via `flatpak-spawn --host`                 | P0       |

### 7.2 Phase 3 Manual Verification Matrix

The Phase 3 manual verification matrix (T1-T17 from the parent PRD) should be executed as part of Phase 3.5 validation. This was deferred in Phase 3 and remains a gap.

---

## 8. Risks & Mitigations

### High Risk

| Risk                                                   | Impact                                               | Mitigation                                                                                                    |
| ------------------------------------------------------ | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| **Something assumed AppImage-only is actually shared** | Removing code breaks Flatpak too                     | Each deletion verified by grepping for callers; unit tests + manual Flatpak test after                        |
| **`tauri build --no-bundle` doesn't embed frontend**   | Flatpak launches with blank screen (devUrl fallback) | Already tested — `build-flatpak.sh` uses `--no-bundle` today via `build-native.sh --binary-only` and it works |

### Medium Risk

| Risk                                                    | Impact                                    | Mitigation                                                                                                      |
| ------------------------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| **Phase 4 Flathub requires changes to scripts**         | Phase 3.5 script cleanup may need re-work | Phase 3.5 keeps `build-binary.sh` generic; Phase 4 build-from-source manifest is separate from the binary build |
| **Users on non-Flatpak distros (Arch, Gentoo minimal)** | Must install Flatpak runtime first        | Pre-marketing; no current users; document Flatpak setup in quickstart. CLI tarball remains available.           |

### Low Risk

| Risk                                   | Impact                                        | Mitigation                                                                                 |
| -------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------ |
| **Debugging is harder inside Flatpak** | Need SDK + `.Debug` extensions for GDB/strace | Daily dev uses `dev-native.sh` (native binary, full host tools); Flatpak debugging is rare |
| **CHANGELOG references AppImage**      | Historical entries mention AppImage           | Leave historical entries; they're accurate for their release                               |

---

## 9. Implementation Phases

This is a single-phase effort with logical ordering:

### Step 1: Script Cleanup (Foundation)

1. Rename `build-native.sh` to `build-binary.sh`; strip AppImage logic
2. Delete `build-native-container.sh` + `build-native-container.Dockerfile`
3. Delete `generate-crosshook-desktop.sh`
4. Delete `scripts/lib/sync-tauri-icons.sh`
5. Clean up `scripts/lib/build-paths.sh` — remove `crosshook_appimage_bundle_dirs()`
6. Clean up `scripts/install-native-build-deps.sh` — remove `patchelf`
7. Update `scripts/build-flatpak.sh` reference from `build-native.sh` to `build-binary.sh`
8. Create `scripts/dev-flatpak.sh`

### Step 2: Rust/Tauri Cleanup

1. Delete `lib.rs:35-77` (AppImage GPU re-exec hack)
2. Update `lib.rs` comment at lines 24-28 (remove AppImage reference)
3. Set `tauri.conf.json` `"targets": []`
4. Update `args.rs` help text
5. Update `platform.rs` doc comments (remove AppImage references; keep all runtime code)

### Step 3: CI Pipeline

1. Simplify `build-native` job in `release.yml` — rename to `build-binary`, remove AppImage artifact steps
2. Remove AppImage from `publish-release` asset list
3. Remove `patchelf` from CI build prerequisites
4. Verify mock-code sentinel check still works on `dist/assets/*.js`

### Step 4: Documentation

1. Update `CLAUDE.md` — commands section, build scripts list
2. Update `AGENTS.md` — stack overview, build scripts, distribution format
3. Update `.cursorrules` — same as AGENTS.md
4. Update `README.md` — installation instructions, download links
5. Update `CONTRIBUTING.md` — build instructions, toolchain references
6. Update `docs/getting-started/quickstart.md` — Flatpak-only install
7. Update `docs/internal-docs/local-build-publish.md` — local build workflow
8. Update `docs/internal-docs/steam-deck-validation-checklist.md` — Flatpak-only
9. Update `packaging/flatpak/README.md` — Flatpak is now sole format
10. Update `.github/pull_request_template.md` — remove AppImage checklist items
11. Add supersession note to `flatpak-distribution.prd.md`

### Step 5: Verification

1. Run `cargo test -p crosshook-core`
2. Run `./scripts/build-binary.sh`
3. Run `./scripts/build-flatpak.sh --rebuild --install`
4. Run `flatpak run dev.crosshook.CrossHook` — verify app launches
5. Run `./scripts/dev-native.sh` — verify hot-reload still works
6. Run `./scripts/dev-flatpak.sh` — verify one-command cycle works
7. Verify `grep -r 'AppImage' scripts/` returns no active-code hits

---

## 10. Affected Files Summary

| Category         | Count | Files                                                                                                                                                                                                                         |
| ---------------- | ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Delete           | 4     | `build-native-container.sh`, `build-native-container.Dockerfile`, `generate-crosshook-desktop.sh`, `lib/sync-tauri-icons.sh`                                                                                                  |
| Rename           | 1     | `build-native.sh` -> `build-binary.sh`                                                                                                                                                                                        |
| Create           | 1     | `dev-flatpak.sh`                                                                                                                                                                                                              |
| Modify (code)    | 5     | `lib.rs`, `tauri.conf.json`, `args.rs`, `platform.rs` (comments only), `build-paths.sh`                                                                                                                                       |
| Modify (scripts) | 3     | `build-flatpak.sh`, `install-native-build-deps.sh`, `dev-native.sh` (if referencing build-native)                                                                                                                             |
| Modify (CI)      | 1     | `release.yml`                                                                                                                                                                                                                 |
| Modify (docs)    | 11    | CLAUDE.md, AGENTS.md, .cursorrules, README.md, CONTRIBUTING.md, quickstart.md, local-build-publish.md, steam-deck-validation-checklist.md, packaging/flatpak/README.md, pull_request_template.md, flatpak-distribution.prd.md |

**Total**: ~26 files touched

---

## 11. References

### Internal

- Parent PRD: [`docs/prps/prds/flatpak-distribution.prd.md`](flatpak-distribution.prd.md)
- Research: [`docs/prps/research/flatpak-standardization-research.md`](../research/flatpak-standardization-research.md)
- Phase 3 report: [`docs/prps/reports/flatpak-phase-3-process-execution-hardening-report.md`](../reports/flatpak-phase-3-process-execution-hardening-report.md)
- Flatpak manifest: [`packaging/flatpak/dev.crosshook.CrossHook.yml`](../../../packaging/flatpak/dev.crosshook.CrossHook.yml)

### External

- [DuckStation: Dropped AppImage for Flatpak](https://pulsegeek.com/articles/duckstation-appimage-vs-flatpak-on-linux/)
- [Heroic: Multi-format "best decision"](https://www.gamingonlinux.com/2023/01/an-interview-with-the-creator-of-the-heroic-games-launcher/)
- [AppImageKit FUSE 3 issue (#1120) — open since 2021](https://github.com/AppImage/AppImageKit/issues/1120)
- [Pomodorolm: Tauri v2 Flatpak submission experience](https://vincent.jousse.org/blog/en/packaging-tauri-v2-flatpak-snapcraft-elm/)
- [Flatpak development restarts (2025)](https://linuxiac.com/flatpak-development-restarts-with-fresh-energy-and-clear-direction/)
