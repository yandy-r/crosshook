# Plan: Flatpak Standardization (Phase 3.5)

## Summary

Remove AppImage as a distribution format and standardize on Flatpak as the sole GUI artifact. Delete AppImage-only scripts, strip AppImage logic from the build pipeline and Rust startup, simplify the CI release workflow to produce Flatpak + CLI tarball only, create `dev-flatpak.sh` for one-command sandbox iteration, and update all documentation. The native `dev-native.sh` hot-reload workflow is untouched.

## User Story

As the CrossHook developer shipping before any users exist on AppImage, I want to eliminate the dual-format runtime divergence now — before it creates "works on mine" bug reports — so that every future feature only needs to consider one production path (Flatpak sandbox) and one dev path (native binary).

## Problem → Solution

CrossHook maintains two production distribution formats (AppImage + Flatpak) with divergent runtime behavior. Every feature touching process execution, filesystem access, or environment detection carries two code paths. Phase 3 proved the cost: Proton detection, launch commands, and utility availability all required Flatpak-specific fixes. Standardizing on Flatpak pre-launch eliminates this divergence at zero user migration cost.

## Metadata

- **Complexity**: Medium-Large
- **Source PRD**: `docs/prps/prds/flatpak-standardization.prd.md`
- **PRD Phase**: Phase 3.5 — Flatpak Standardization
- **Tracking Issue**: `#220`
- **Sub-Issues**: `#215` (Step 1), `#216` (Step 2), `#217` (Step 3), `#218` (Step 4), `#219` (Step 5)
- **Estimated Files**: ~30 touched (4 delete, 1 rename, 1 create, ~24 modify)
- **Research**: `docs/prps/research/flatpak-standardization-research.md`

## Persistence & Usability

### Storage Boundary

| Datum / behavior                      | Classification         | Notes                                                       |
| ------------------------------------- | ---------------------- | ----------------------------------------------------------- |
| Build scripts, CI config              | Build-time only        | No runtime persistence impact                               |
| AppImage GPU re-exec (`lib.rs:35-77`) | Runtime-only (removed) | Dead code removal; no persisted state                       |
| XDG override (`platform.rs`)          | Runtime-only (kept)    | Stays in Phase 3.5; Phase 4 replaces with per-app isolation |
| DMA-BUF / GDK_BACKEND workarounds     | Runtime-only (kept)    | Format-agnostic; comments updated only                      |

### Persistence & Usability Notes

- No TOML migration planned. No user settings are affected.
- No SQLite migration planned. No metadata schema changes.
- XDG override stays — the Flatpak still uses host XDG paths (`~/.config/crosshook/`, etc.) rather than sandbox-local paths. Phase 4 replaces this.
- `is_flatpak()` branching stays — it now represents dev-vs-prod, not dual-production-runtime.

---

## Gaps Discovered During Research

These items were NOT in the PRD but were identified by codebase research:

| #   | Gap                                                                                                              | Impact                                                                                                                                                                                                     | Resolution                                                                                                                 |
| --- | ---------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| G1  | `packaging/PKGBUILD:35` calls `sync-tauri-icons.sh` which PRD marks for deletion                                 | PKGBUILD build breaks                                                                                                                                                                                      | Remove `sync-tauri-icons.sh` call from PKGBUILD; icon copy is unnecessary with `targets: []` since Tauri no longer bundles |
| G2  | `packaging/PKGBUILD:38` runs `tauri build` without `--no-bundle`                                                 | With `targets: []`, `tauri build` produces binary-only (no bundle) — functionally equivalent to `--no-bundle`. PKGBUILD still works because it searches for the binary via `find`, not for AppImage output | No code change needed; `targets: []` handles it. Add `--no-bundle` for clarity                                             |
| G3  | `lib.rs:86-95` — `linuxdeploy` GDK_BACKEND workaround comment references AppImage tooling                        | Comment is stale; code is harmless (only fires if `GDK_BACKEND=x11`, which linuxdeploy set)                                                                                                                | Update comment; keep code (defensive, harmless)                                                                            |
| G4  | `docs/prps/specs/flatpak-distribution-spec.md` has 9 AppImage refs including "AppImage remains primary" non-goal | Stale spec contradicts Phase 3.5                                                                                                                                                                           | Add supersession note (same treatment as parent PRD)                                                                       |
| G5  | `docs/internal-docs/profile-collections-browser-mocks.md:41` says "after the AppImage build"                     | Stale wording                                                                                                                                                                                              | Update to "after the binary build"                                                                                         |
| G6  | `build-flatpak.sh:122-129` duplicates `arch_suffix_for_triple` from `build-native.sh:25-43`                      | Duplication persists after rename; the function in `build-binary.sh` would be dead code (only used in AppImage naming)                                                                                     | Remove `arch_suffix_for_triple` from `build-binary.sh` since it's AppImage-specific; `build-flatpak.sh` keeps its own copy |

---

## Tasks

### Task 1: Script Cleanup — Foundation

**Issue**: `#215`
**File ownership**: `scripts/` directory

#### Task 1.1: Delete AppImage-only scripts

Delete these 4 files entirely:

| File                                        | Lines | Reason                                                                                                        |
| ------------------------------------------- | ----- | ------------------------------------------------------------------------------------------------------------- |
| `scripts/build-native-container.sh`         | 204   | AppImage container build wrapper; no non-doc callers                                                          |
| `scripts/build-native-container.Dockerfile` | 52    | AppImage container image; only caller is `build-native-container.sh:17`                                       |
| `scripts/generate-crosshook-desktop.sh`     | 240   | Generates desktop entry from AppImage via `--appimage-extract`; no non-doc callers                            |
| `scripts/lib/sync-tauri-icons.sh`           | 29    | Syncs icons into Tauri AppImage bundle structure; called by `build-native.sh:164` and `packaging/PKGBUILD:35` |

**Verification**: `git status` shows 4 deleted files. No script or CI workflow references remain (docs updated in Task 4).

#### Task 1.2: Rename `build-native.sh` → `build-binary.sh` and strip AppImage logic

**Source**: `scripts/build-native.sh` (205 lines)
**Target**: `scripts/build-binary.sh`

**Lines to DELETE** (AppImage-specific logic):

| Lines   | Content                                                                                    |
| ------- | ------------------------------------------------------------------------------------------ |
| 10      | `export APPIMAGE_EXTRACT_AND_RUN="${APPIMAGE_EXTRACT_AND_RUN:-1}"`                         |
| 16-23   | `stable_appimage_name()` function                                                          |
| 25-42   | `appimage_arch_suffix()` function                                                          |
| 118     | `patchelf` check: `command -v patchelf >/dev/null 2>&1 \|\| die "patchelf is required..."` |
| 162-164 | `generate-assets.sh` and `sync-tauri-icons.sh` calls (AppImage-only pre-build steps)       |
| 166     | Echo "Building CrossHook Native AppImage..."                                               |
| 169-177 | Full `tauri build` invocation (non-`--no-bundle` path)                                     |
| 179-204 | AppImage search loop, copy, stable alias creation                                          |

**Lines to UPDATE**:

| Line    | Current                                                      | New                                                                        |
| ------- | ------------------------------------------------------------ | -------------------------------------------------------------------------- |
| 51      | `--binary-only` help text mentioning "AppImage bundling"     | Remove `--binary-only` flag entirely — the script now ONLY builds binaries |
| 58      | Usage text mentioning "binary/AppImage copies"               | Update to "release binary" only                                            |
| 117-119 | `if (( ! BINARY_ONLY ))` guard around patchelf check         | Remove guard — there is no non-binary-only path anymore                    |
| 138     | Comment referencing "AppImage or inside the Flatpak sandbox" | Update to "native dev build or inside the Flatpak sandbox"                 |

**Result**: `build-binary.sh` is the binary-only build path (lines 128-159 of the original), with cleaner entry/exit. No `--binary-only` flag needed since that's the only mode. The script should still accept `--print-paths`, `--install-deps`, `--yes` flags.

**Verification**: `./scripts/build-binary.sh` produces binary at `$DIST_DIR/crosshook-native`. No AppImage strings remain: `grep -i appimage scripts/build-binary.sh` returns nothing.

#### Task 1.3: Clean up `scripts/lib/build-paths.sh`

**File**: `scripts/lib/build-paths.sh` (80 lines)

**DELETE**: Lines 66-80 — `crosshook_appimage_bundle_dirs()` function and its preceding comment. Only caller was `build-native.sh:189` (now deleted).

**KEEP**: `crosshook_build_paths_init()` (lines 10-64) — resolves `DIST_DIR` and `CARGO_TARGET_DIR`. Used by `build-binary.sh`, `build-flatpak.sh`, `dev-native.sh`.

**Verification**: `grep -n appimage scripts/lib/build-paths.sh` returns nothing.

#### Task 1.4: Clean up `scripts/install-native-build-deps.sh`

**File**: `scripts/install-native-build-deps.sh` (162 lines)

**Remove `patchelf`** from these 6 locations:

| Line | Context                                                                                                                              |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------ |
| 10   | Usage text: "Install the host packages required to build the native Tauri/AppImage target." → "...to build the native Tauri binary." |
| 61   | pacman packages array: remove `patchelf`                                                                                             |
| 85   | apt packages array: remove `patchelf`                                                                                                |
| 112  | dnf packages array: remove `patchelf`                                                                                                |
| 136  | zypper packages array: remove `patchelf`                                                                                             |
| 160  | Fallback error message: remove `patchelf` from manual install list                                                                   |

**Verification**: `grep -n patchelf scripts/install-native-build-deps.sh` returns nothing.

#### Task 1.5: Update `scripts/build-flatpak.sh` references

**File**: `scripts/build-flatpak.sh` (297 lines)

Update all references from `build-native.sh` to `build-binary.sh`:

| Line | Current                                               | New                                         |
| ---- | ----------------------------------------------------- | ------------------------------------------- |
| 53   | Help text referencing `build-native.sh`               | `build-binary.sh`                           |
| 60   | Help text referencing `build-native.sh`               | `build-binary.sh`                           |
| 63   | Help text referencing `build-native.sh --binary-only` | `build-binary.sh` (no `--binary-only` flag) |
| 211  | `"$ROOT_DIR/scripts/build-native.sh" --binary-only`   | `"$ROOT_DIR/scripts/build-binary.sh"`       |
| 217  | `"$ROOT_DIR/scripts/build-native.sh" --binary-only`   | `"$ROOT_DIR/scripts/build-binary.sh"`       |
| 220  | Log message referencing `build-native.sh`             | `build-binary.sh`                           |

**Verification**: `grep -n build-native scripts/build-flatpak.sh` returns nothing.

#### Task 1.6: Create `scripts/dev-flatpak.sh`

**New file**: `scripts/dev-flatpak.sh` (per PRD §6)

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
    echo "Usage: dev-flatpak.sh [--run | --shell | --skip-binary]" >&2
    echo "" >&2
    echo "  (no args)      Full cycle: build binary + Flatpak + install + run" >&2
    echo "  --run           Run already-installed Flatpak (skip build)" >&2
    echo "  --shell         Open bash inside the Flatpak sandbox" >&2
    echo "  --skip-binary   Rebuild Flatpak from cached binary (skip Rust compilation)" >&2
    exit 1
    ;;
esac
```

Mark executable: `chmod +x scripts/dev-flatpak.sh`

**Verification**: `./scripts/dev-flatpak.sh --run` launches the Flatpak (if installed). Script is executable.

#### Task 1.7: Update `packaging/PKGBUILD` (Gap G1/G2)

**File**: `packaging/PKGBUILD` (66 lines)

| Line | Current                                                             | New                                                                                                                                                |
| ---- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| 35   | `./scripts/lib/sync-tauri-icons.sh`                                 | DELETE this line (script deleted in Task 1.1)                                                                                                      |
| 38   | `./node_modules/.bin/tauri build --target x86_64-unknown-linux-gnu` | `./node_modules/.bin/tauri build --no-bundle --target x86_64-unknown-linux-gnu` (explicit; `targets: []` handles it, but `--no-bundle` is clearer) |

**Verification**: `grep sync-tauri packaging/PKGBUILD` returns nothing. PKGBUILD still builds and finds binary via `find`.

---

### Task 2: Rust/Tauri Cleanup

**Issue**: `#216`
**File ownership**: `src/crosshook-native/`

#### Task 2.1: Delete AppImage GPU re-exec hack from `lib.rs`

**File**: `src/crosshook-native/src-tauri/src/lib.rs` (416 lines)

**DELETE**: Lines 35-77 (entire block) — the `// --- AppImage GPU compatibility` comment block and the `#[cfg(target_os = "linux")] if std::env::var_os("APPIMAGE").is_some()` re-exec hack.

**UPDATE line 25** (XDG override comment):

- Current: `// Phase 1 shares data between AppImage and Flatpak via --filesystem=home.`
- New: `// Phase 1 shares data with host via --filesystem=home (Flatpak-only since Phase 3.5).`

**UPDATE line 81** (DMA-BUF comment):

- Current: `// environment, but the AppImage needs it set before WebKit initializes.`
- New: `// environment, but the binary needs it set before WebKit initializes.`

**UPDATE lines 86-90** (GDK_BACKEND comment — Gap G3):

- Current: `// The linuxdeploy GTK plugin forces GDK_BACKEND=x11 to work around Wayland`
- New: `// Some build environments force GDK_BACKEND=x11 to work around Wayland`
- Keep lines 87-90 otherwise as-is; the workaround is harmless and defensive.

**Verification**: `grep -n -i appimage src/crosshook-native/src-tauri/src/lib.rs` returns nothing. `grep -n linuxdeploy src/crosshook-native/src-tauri/src/lib.rs` returns nothing. `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native` passes.

#### Task 2.2: Set `tauri.conf.json` targets to empty

**File**: `src/crosshook-native/src-tauri/tauri.conf.json` (36 lines)

**UPDATE line 32**:

- Current: `"targets": ["appimage"],`
- New: `"targets": [],`

**Verification**: `grep appimage src/crosshook-native/src-tauri/tauri.conf.json` returns nothing.

#### Task 2.3: Update `args.rs` help text

**File**: `src/crosshook-native/crates/crosshook-cli/src/args.rs` (319 lines)

**UPDATE line 72**:

- Current: `/// Override the helper scripts directory (default: bundled AppImage scripts)`
- New: `/// Override the helper scripts directory (default: bundled runtime scripts)`

**Verification**: `grep -i appimage src/crosshook-native/crates/crosshook-cli/src/args.rs` returns nothing.

#### Task 2.4: Update `platform.rs` doc comments

**File**: `src/crosshook-native/crates/crosshook-core/src/platform.rs` (1241 lines)

All changes are **doc comment only** — no runtime code changes.

| Line | Current text                                                                   | New text                                                                         |
| ---- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- |
| 3    | `//! CrossHook runs both as a native Linux binary (AppImage, dev build) and`   | `//! CrossHook runs both as a native Linux binary (dev build) and`               |
| 39   | `/// build continue to work when reused later by the native/AppImage build.`   | `/// build continue to work when reused later by the native dev build.`          |
| 149  | `/// code behaves correctly in both AppImage and Flatpak deployments.`         | `/// code behaves correctly in both native dev and Flatpak deployments.`         |
| 180  | `/// Flatpak build and the AppImage share the same data on disk.`              | `/// Flatpak build sees the same data on disk as a native dev build.`            |
| 197  | `/// build and the AppImage share the same data on disk. Called from the very` | `/// Flatpak build sees the host's real XDG paths on disk. Called from the very` |

**Verification**: `grep -i appimage src/crosshook-native/crates/crosshook-core/src/platform.rs` returns nothing. All runtime code (`is_flatpak()`, `host_command()`, `override_xdg_for_flatpak_host_access()`) unchanged.

#### Task 2.5: Remove `*.AppImage` from `.gitignore`

**File**: `.gitignore`

**DELETE line 14**: `*.AppImage`

**Verification**: `grep -i appimage .gitignore` returns nothing.

#### Task 2.6: Run Rust verification

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cargo check --manifest-path src/crosshook-native/Cargo.toml
```

All 802+ tests must pass. No compilation errors.

---

### Task 3: CI Release Pipeline

**Issue**: `#217`
**File ownership**: `.github/workflows/release.yml`

#### Task 3.1: Simplify the `build-native` job

**File**: `.github/workflows/release.yml` (301 lines)

The `build-native` job (lines 16-193) needs these changes:

**a) Rename job ID** (optional but clarifying):

- Line 16: `build-native:` → `build-binary:`
- Update all `needs: [build-native]` references (lines 198, 250) to `needs: [build-binary]`

**b) Remove `APPIMAGE_CARGO_TARGET_DIR` env var**:

- DELETE line 20: `APPIMAGE_CARGO_TARGET_DIR: /tmp/crosshook-native-appimage-target`

**c) Remove `patchelf` from build prerequisites**:

- Line 60: remove `patchelf` from the `apt-get install` list

**d) Replace "Build native AppImage" step with "Build release binary"**:

- Lines 111-116: Replace entire step:
  - Current: step name "Build native AppImage", sets `CARGO_TARGET_DIR: ${{ env.APPIMAGE_CARGO_TARGET_DIR }}`, runs `./scripts/build-native.sh`
  - New: step name "Build release binary", sets `CARGO_TARGET_DIR` to a simple path like `/tmp/crosshook-cargo-target`, runs `./scripts/build-binary.sh`

**e) Update "Export Flatpak binary" step**:

- Lines 118-122: Update `binary_path` to use new `CARGO_TARGET_DIR` value instead of `${{ env.APPIMAGE_CARGO_TARGET_DIR }}`

**f) Remove AppImage from artifact upload**:

- Line 159: DELETE `dist/CrossHook_${{ env.RELEASE_VERSION }}_amd64.AppImage` from the upload paths

**g) Verify `verify:no-mocks` step** (lines 124-139) still works — it checks `src/crosshook-native/dist/assets/*.js`, independent of AppImage. No change needed.

#### Task 3.2: Remove AppImage from `publish-release` job

**File**: `.github/workflows/release.yml`

- Line 289: DELETE `dist/CrossHook_${{ env.RELEASE_VERSION }}_amd64.AppImage` from the release asset list
- Remaining assets: CLI tarball (`_linux_amd64.tar.gz`) and Flatpak (`.flatpak`)
- `fail_on_unmatched_files: true` stays — list must match exactly

#### Task 3.3: Update job dependencies

If job was renamed from `build-native` to `build-binary`:

- Line 198: `- build-native` → `- build-binary` (in `build-flatpak.needs`)
- Line 250: `- build-native` → `- build-binary` (in `publish-release.needs`)

**Verification**: The workflow YAML is valid. `grep -i appimage .github/workflows/release.yml` returns nothing except possibly in comments about what was removed.

---

### Task 4: Documentation Updates

**Issue**: `#218`
**File ownership**: docs, config, templates

#### Task 4.1: Update `CLAUDE.md`

**File**: `CLAUDE.md`

| Line | Change                                                                                |
| ---- | ------------------------------------------------------------------------------------- |
| 13   | `(Tauri v2, AppImage)` → `(Tauri v2, Flatpak)`                                        |
| 66   | `./scripts/build-native.sh` → `./scripts/build-binary.sh`                             |
| 67   | DELETE `./scripts/build-native-container.sh` line                                     |
| 68   | `./scripts/build-native.sh --binary-only` → DELETE (redundant with `build-binary.sh`) |
| —    | ADD `./scripts/dev-flatpak.sh` to commands list                                       |

#### Task 4.2: Update `AGENTS.md`

**File**: `AGENTS.md`

| Line | Change                                                                                                                               |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------ |
| 13   | `(Tauri v2, AppImage)` → `(Tauri v2, Flatpak)`                                                                                       |
| 70   | `./scripts/build-native.sh  # AppImage: runs generate-assets...` → `./scripts/build-binary.sh  # Build release binary (no bundling)` |
| 71   | DELETE `./scripts/build-native-container.sh` line                                                                                    |
| 72   | `./scripts/build-native.sh --binary-only` → DELETE (redundant)                                                                       |
| 84   | `after every AppImage build` → `after every release build`                                                                           |
| 92   | `packaged as AppImage` → `packaged as Flatpak`                                                                                       |
| —    | ADD `./scripts/dev-flatpak.sh` to commands list                                                                                      |

#### Task 4.3: Update `.cursorrules`

**File**: `.cursorrules`

Same changes as AGENTS.md — these files are kept in sync:

| Line | Change                                                                                                           |
| ---- | ---------------------------------------------------------------------------------------------------------------- |
| 13   | `(Tauri v2, AppImage)` → `(Tauri v2, Flatpak)`                                                                   |
| 57   | `./scripts/build-native.sh  # AppImage: ...` → `./scripts/build-binary.sh  # Build release binary (no bundling)` |
| 58   | DELETE `./scripts/build-native-container.sh` line                                                                |
| 59   | `./scripts/build-native.sh --binary-only` → DELETE                                                               |
| 68   | `after every AppImage build` → `after every release build`                                                       |
| 76   | `packaged as AppImage` → `packaged as Flatpak`                                                                   |
| —    | ADD `./scripts/dev-flatpak.sh` to commands list                                                                  |

#### Task 4.4: Update `README.md`

**File**: `README.md` (21 AppImage references)

Major sections to rewrite:

- **Line 3**: Download badge — change from AppImage to Flatpak download badge
- **Lines 30-38**: Download section — Flatpak install instructions (`flatpak install --user CrossHook_amd64.flatpak` or from Releases)
- **Lines 43-45**: Quick Start steps — Flatpak-based launch
- **Lines 171-192**: Build section — replace "Build the AppImage" with "Build the release binary"; use `build-binary.sh`; remove `patchelf`/`rsvg-convert` from AppImage-specific prereqs; remove `--binary-only` references
- **Lines 221-232**: Browser Dev Mode and Release Notes — replace AppImage references with Flatpak/binary

#### Task 4.5: Update `CONTRIBUTING.md`

**File**: `CONTRIBUTING.md` (6 references)

| Line | Change                                                                                        |
| ---- | --------------------------------------------------------------------------------------------- |
| 46   | `./scripts/build-native.sh` → `./scripts/build-binary.sh`                                     |
| 49   | Remove "Full AppImage builds" text; update to binary-only build story                         |
| 52   | `./scripts/build-native.sh --binary-only` → `./scripts/build-binary.sh`                       |
| 71   | Remove "building the AppImage" reference                                                      |
| 83   | `runtime-helpers/ \| Shell scripts bundled into the AppImage` → `...bundled into the Flatpak` |
| 156  | `./scripts/build-native.sh --binary-only` → `./scripts/build-binary.sh` in PR checklist       |

#### Task 4.6: Update `docs/getting-started/quickstart.md`

**File**: `docs/getting-started/quickstart.md` (14 AppImage references)

Rewrite the Linux Desktop and Steam Deck installation sections for Flatpak:

- **Linux Desktop** (lines 60-73): Flatpak install instructions (`flatpak install --user <path>.flatpak`, `flatpak run dev.crosshook.CrossHook`)
- **Steam Deck** (lines 78-91): Flatpak install in Desktop Mode (Flatpak is native to SteamOS)
- **Supported Environments table** (lines 45-46): Update "Run the AppImage" → "Install the Flatpak"
- **What You Need** (line 50): Update prerequisites

#### Task 4.7: Update `docs/internal-docs/local-build-publish.md`

**File**: `docs/internal-docs/local-build-publish.md` (22+ references)

Major rewrite — this file is heavily AppImage-centric:

- Remove "AppImage icon and branding assets" section (lines 12-19)
- Rename "Build AppImage" section to "Build Release Binary"
- Replace all `build-native.sh` references with `build-binary.sh`
- Remove AppImage output descriptions (versioned + stable alias)
- Remove container build section (lines 84-87)
- Update CI/CD steps to reflect binary + Flatpak pipeline
- Update artifact shape section — no more `.AppImage` files
- Remove `--binary-only` flag references (that's now the only mode)
- Add `dev-flatpak.sh` usage section

#### Task 4.8: Update `docs/internal-docs/steam-deck-validation-checklist.md`

**File**: `docs/internal-docs/steam-deck-validation-checklist.md`

- **Line 11**: `gamescope -W 1280 -H 800 -r 60 -- ./CrossHook_amd64.AppImage` → `gamescope -W 1280 -H 800 -r 60 -- flatpak run dev.crosshook.CrossHook`

#### Task 4.9: Update `packaging/flatpak/README.md`

**File**: `packaging/flatpak/README.md`

- **Line 19**: `alongside AppImage and CLI assets` → `alongside the CLI tarball`
- Update any framing of Flatpak as "secondary" to "sole GUI distribution format"

#### Task 4.10: Update `.github/pull_request_template.md`

**File**: `.github/pull_request_template.md`

| Line | Change                                                                      |
| ---- | --------------------------------------------------------------------------- |
| 31   | `./scripts/build-native.sh --binary-only` → `./scripts/build-binary.sh`     |
| 33   | DELETE `./scripts/build-native.sh produces a valid AppImage` checklist item |

#### Task 4.11: Add supersession note to `flatpak-distribution.prd.md`

**File**: `docs/prps/prds/flatpak-distribution.prd.md`

Add a note at the top (after the title/frontmatter):

```markdown
> **Superseded (Phase 3.5)**: The non-goal "AppImage remains the primary distribution format"
> (§2.3) was reversed by [Phase 3.5](flatpak-standardization.prd.md). As of Phase 3.5,
> Flatpak is the sole GUI distribution format and all AppImage code has been removed.
> This PRD remains accurate for Phases 1-3 history.
```

#### Task 4.12: Add supersession note to `flatpak-distribution-spec.md` (Gap G4)

**File**: `docs/prps/specs/flatpak-distribution-spec.md`

Add a similar supersession note at the top referencing Phase 3.5.

#### Task 4.13: Update `docs/internal-docs/profile-collections-browser-mocks.md` (Gap G5)

**File**: `docs/internal-docs/profile-collections-browser-mocks.md`

- **Line 41**: `after the AppImage build` → `after the release build`

---

### Task 5: Verification and Acceptance Tests

**Issue**: `#219`

#### Task 5.1: Automated verification

```bash
# T1: Unit tests pass
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core

# T9: No AppImage in active scripts
grep -r 'AppImage' scripts/
# Expected: no hits

# Broader check: no AppImage in active code/config (exclude historical docs)
grep -ri 'appimage' --include='*.sh' --include='*.rs' --include='*.json' --include='*.yml' --include='*.toml' .
# Expected: only in CHANGELOG.md, completed plans, research docs, PRDs (historical)
```

#### Task 5.2: Build verification

```bash
# T2: Binary build produces artifact
./scripts/build-binary.sh
# Expected: binary at $DIST_DIR/crosshook-native

# T3: Flatpak build produces bundle
./scripts/build-flatpak.sh --rebuild --install
# Expected: bundle at $DIST_DIR/CrossHook_amd64.flatpak

# T4: Flatpak installs and runs
flatpak run dev.crosshook.CrossHook
# Expected: app launches, UI renders
```

#### Task 5.3: Dev workflow verification

```bash
# T5: dev-native.sh hot-reload
./scripts/dev-native.sh
# Expected: Vite + Tauri dev server starts

# T6: dev-native.sh browser mode
./scripts/dev-native.sh --browser
# Expected: Vite dev server at localhost:5173

# T7: dev-flatpak.sh full cycle
./scripts/dev-flatpak.sh
# Expected: builds, installs, runs Flatpak

# T8: dev-flatpak.sh shell
./scripts/dev-flatpak.sh --shell
# Expected: opens bash inside sandbox
```

#### Task 5.4: Flatpak functionality smoke test

```bash
# T10: Steam library detection inside Flatpak
# Verify Flatpak can discover ~/.local/share/Steam

# T11: Profile launch inside Flatpak
# Verify game launches via flatpak-spawn --host
```

---

## Execution Order and Dependencies

```
Task 1 (Scripts)  ──→  Task 2 (Rust/Tauri)  ──→  Task 3 (CI)  ──→  Task 4 (Docs)  ──→  Task 5 (Verification)
   #215                    #216                     #217               #218                  #219
```

Tasks MUST execute in order:

- Task 2 depends on Task 1 (Rust code references scripts; `build-flatpak.sh` must already point to `build-binary.sh`)
- Task 3 depends on Task 2 (`release.yml` references both scripts and Tauri config)
- Task 4 depends on Tasks 1-3 (docs must reflect the actual state of scripts, code, and CI)
- Task 5 depends on all prior tasks (verification runs against the final state)

## Files Changed Summary

| Category         | Count | Files                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Delete           | 4     | `build-native-container.sh`, `build-native-container.Dockerfile`, `generate-crosshook-desktop.sh`, `lib/sync-tauri-icons.sh`                                                                                                                                                                                                                                                 |
| Rename           | 1     | `build-native.sh` → `build-binary.sh`                                                                                                                                                                                                                                                                                                                                        |
| Create           | 1     | `dev-flatpak.sh`                                                                                                                                                                                                                                                                                                                                                             |
| Modify (Rust)    | 4     | `lib.rs`, `tauri.conf.json`, `args.rs`, `platform.rs`                                                                                                                                                                                                                                                                                                                        |
| Modify (scripts) | 4     | `build-flatpak.sh`, `build-paths.sh`, `install-native-build-deps.sh`, `PKGBUILD`                                                                                                                                                                                                                                                                                             |
| Modify (CI)      | 1     | `release.yml`                                                                                                                                                                                                                                                                                                                                                                |
| Modify (config)  | 1     | `.gitignore`                                                                                                                                                                                                                                                                                                                                                                 |
| Modify (docs)    | 14    | `CLAUDE.md`, `AGENTS.md`, `.cursorrules`, `README.md`, `CONTRIBUTING.md`, `quickstart.md`, `local-build-publish.md`, `steam-deck-validation-checklist.md`, `packaging/flatpak/README.md`, `pull_request_template.md`, `flatpak-distribution.prd.md`, `flatpak-distribution-spec.md`, `profile-collections-browser-mocks.md`, `dev-native.sh` (no change needed per research) |

**Total**: ~30 files touched

## Risk Checklist

| Risk                                                             | Likelihood | Impact | Mitigation                                                                           |
| ---------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------ |
| Deleting code that Flatpak also needs                            | Low        | High   | Each deletion verified by grepping for callers; only AppImage-exclusive code removed |
| `tauri build --no-bundle` doesn't embed frontend                 | Low        | High   | Already tested — `build-flatpak.sh` uses this path today                             |
| PKGBUILD breaks from `sync-tauri-icons.sh` deletion              | Medium     | Medium | Task 1.7 updates PKGBUILD to remove the call                                         |
| `release.yml` asset list mismatch with `fail_on_unmatched_files` | Medium     | High   | Task 3.2 ensures list matches exactly                                                |
| Phase 4 Flathub requires script re-work                          | Low        | Low    | `build-binary.sh` stays generic; Flathub manifest is separate                        |
