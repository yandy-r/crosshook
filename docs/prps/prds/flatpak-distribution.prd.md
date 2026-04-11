# PRD: Flatpak Distribution Target

**Issue**: [#69](https://github.com/yandy-r/crosshook/issues/69)
**Status**: Ready for planning
**Date**: 2026-04-10
**Spec**: [`docs/prps/specs/flatpak-distribution-spec.md`](../specs/flatpak-distribution-spec.md)

---

## 1. Problem

CrossHook is distributed exclusively as an AppImage. This limits reach for three growing user segments:

1. **Immutable distro users** (Fedora Silverblue/Kinoite, Bazzite, SteamOS, Vanilla OS, Universal Blue) where Flatpak is the primary — often only — app installation mechanism. Many of these users lack FUSE support required by AppImage.
2. **Multi-drive gamers** who store Steam libraries on `/mnt/nvme1`, `/run/media/user/SSD`, or other non-`$HOME` mount points. The previous Flatpak prototype failed because it could not detect Proton versions or launch games from these external paths — a sandbox permission design problem, not a Flatpak limitation.
3. **Discoverability-driven users** who find software through Flathub (the largest Linux app store) rather than GitHub Releases.

**Hypothesis**: Offering CrossHook as a Flatpak will capture ≥10% of total downloads within two release cycles, driven primarily by immutable distro and SteamOS users who currently cannot use CrossHook at all.

AppImage remains the primary distribution format. Flatpak is a secondary target.

---

## 2. Users & Personas

| Persona                         | Context                                                                                       | Key Need                                                                           |
| ------------------------------- | --------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| **SteamOS / Bazzite gamer**     | Read-only root, Flatpak is the sideload mechanism. Games on internal NVMe + external USB SSD. | Install from `.flatpak` bundle; launch trainers against games on any mounted drive |
| **Fedora Silverblue developer** | Policy-avoids AppImage; trusts Flatpak sandboxing model. Single NVMe, Steam native.           | `flatpak install` from GitHub Releases; all features work within sandbox           |
| **Multi-drive power user**      | Any distro; 3+ Steam library folders across `/mnt/nvme1`, `/mnt/ssd-games`, etc.              | CrossHook discovers all Steam libraries and launches Proton from any drive         |
| **Flathub browser**             | Discovers tools via Flathub search. No prior awareness of CrossHook.                          | Find, install, and run CrossHook from Flathub (Phase 4)                            |

---

## 3. Goals & Success Criteria

### 3.1 Goals

| #   | Goal                                                                                                                                           | Phase |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| G1  | Produce a working `.flatpak` bundle installable via `flatpak install --user`                                                                   | 1     |
| G2  | All core workflows function inside the Flatpak sandbox — Steam discovery (all drives), profile launch, trainer injection, GE-Proton management | 1–3   |
| G3  | Automate Flatpak builds in CI alongside AppImage                                                                                               | 2     |
| G4  | All 12 external binary calls work reliably via centralized host-spawn abstraction                                                              | 3     |
| G5  | Publish to Flathub for discoverability                                                                                                         | 4     |

### 3.2 Success Criteria

| Metric                        | Target                                                           | Measurement                                                                  |
| ----------------------------- | ---------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Flatpak download share        | ≥10% of total downloads                                          | GitHub Releases download counts (AppImage vs Flatpak) after 2 release cycles |
| Core workflow pass rate       | 100% of P0 test cases pass                                       | Manual verification matrix (§8)                                              |
| Sandbox escape                | Zero — no `--filesystem=host` in final manifest                  | Manifest audit                                                               |
| CI build time impact          | Flatpak job runs in parallel; total release time increase ≤5 min | CI job duration                                                              |
| Flathub submission acceptance | Accepted within 3 review rounds                                  | Flathub PR status (Phase 4)                                                  |

### 3.3 Non-Goals

- Replacing AppImage as the primary distribution format
- Supporting Snap, Nix, or other package formats in this effort
- Running a self-hosted flat-manager Flatpak repository (bundles via GitHub Releases only)
- Bundling Wine/Proton inside the Flatpak (host Proton is used via `flatpak-spawn --host`)

---

## 4. Key Decisions

### 4.1 App ID: `dev.crosshook.CrossHook`

Use the owned-domain reverse-DNS ID from the start. The `crosshook.dev` domain was acquired during Phase 1 planning, which maps cleanly to `dev.crosshook.CrossHook` under Flathub conventions. This avoids a later breaking migration (app ID change = user data path change) and gives CrossHook first-class Flathub standing without the GitHub-ID compatibility warnings that plagued earlier attempts.

**Impact**: The Tauri `identifier` in `tauri.conf.json` changes globally from `com.crosshook.native` to `dev.crosshook.CrossHook`. This affects XDG data paths, D-Bus names, and the bundle identifier. All references across `Cargo.toml` files, desktop entries, metainfo, CI, and documentation must be updated.

**Validator compliance**: `dev.crosshook.CrossHook` passes (a) the Tauri CLI bundle identifier validator (alphanumerics + `.`), (b) `appstreamcli validate` (no `cid-rdns-contains-hyphen` warning), and (c) `flatpak install`'s D-Bus-name parser (no hyphens in middle segments). Earlier attempts at GitHub-derived identifiers — `io.github.yandy-r.crosshook`, `io.github.yandy_r.CrossHook`, `io.github.yandy-r.CrossHook` — each failed at least one of these validators. See the unreleased `feat/flatpak` branch history for the receipts.

**Migration**: The XDG migration chain is `com.crosshook.native → dev.crosshook.CrossHook`. The intermediate GitHub-derived identifiers never shipped to users (they existed only on the unreleased `feat/flatpak` branch), so a single migration hop is sufficient.

### 4.2 Filesystem Permissions: `--filesystem=home` + Explicit Mount Paths

**Not** `--filesystem=host`. The Lutris precedent proves that gaming tools with broad filesystem needs are accepted on Flathub using targeted permissions:

```yaml
finish-args:
  # Home directory (Steam default, config, downloads)
  - --filesystem=home

  # External/additional drives where Steam libraries live
  - --filesystem=/mnt
  - --filesystem=/run/media
  - --filesystem=/media

  # Flatpak Steam installation (read-only discovery)
  - --filesystem=~/.var/app/com.valvesoftware.Steam:ro
```

This covers the critical multi-drive use case (games on `/mnt/nvme1`, USB SSDs at `/run/media/user/`, etc.) without the "enormous hole" of `--filesystem=host`.

### 4.3 Process Execution: Centralized `host_command()`

All external binary calls go through a single abstraction in `crosshook-core/src/platform.rs` that conditionally wraps commands with `flatpak-spawn --host` when `is_flatpak()` returns true.

**Rationale**: The ~140ms D-Bus overhead per call is negligible relative to Proton startup time (seconds). Uniform wrapping is simpler and more predictable than selective wrapping.

**12 binaries requiring host execution**:

| Binary                    | Source File                              | Notes                           |
| ------------------------- | ---------------------------------------- | ------------------------------- |
| `git`                     | `community/taps.rs`                      | Tap clone/fetch/diff            |
| `unshare`                 | `script_runner.rs`, `runtime_helpers.rs` | PID/network namespace isolation |
| `gamescope`               | `runtime_helpers.rs`                     | Compositor wrapper              |
| `lspci`                   | `diagnostics.rs`                         | GPU detection                   |
| `getent`                  | `settings/mod.rs`                        | Home dir resolution             |
| `/bin/bash`               | `script_runner.rs`                       | Script execution                |
| `kill`                    | `run_executable.rs`, `update.rs`         | Signal processes                |
| Proton (dynamic)          | `runtime_helpers.rs`                     | Proton runtime                  |
| Game exe (dynamic)        | `script_runner.rs`                       | Game executable                 |
| Wrapper cmds (dynamic)    | `runtime_helpers.rs`                     | User-specified wrappers         |
| `crosshook-native` (self) | `lib.rs`                                 | Single-instance restart         |
| Helper scripts            | `script_runner.rs`                       | Bundled shell scripts           |

### 4.4 Helper Script Flatpak Awareness

The Rust core detects the Flatpak sandbox via `FLATPAK_ID` and `/.flatpak-info` (`crosshook-core/src/platform.rs`, `is_flatpak()`). Process spawning that must reach the host uses `host_command()` / `flatpak-spawn --host` from the core layer. The bundled helper scripts do not set or read `FLATPAK=1`; the manifest does not add `--env=FLATPAK=1` (Flatpak sets `FLATPAK_ID` automatically).

### 4.5 `/usr/bin/rm` Hardcode Removal

Replace the hardcoded `/usr/bin/rm` calls in `run_executable.rs:21,278` with `std::fs::remove_dir_all`. This eliminates a process spawn for a standard library operation and removes a Flatpak sandbox path ambiguity.

### 4.6 GNOME Runtime: Track Latest Stable

The committed manifest uses **`runtime-version: "50"`** in `packaging/flatpak/dev.crosshook.CrossHook.yml` (GNOME 50 / `org.gnome.Platform` 50). Track the latest stable release — bump when a new stable lands. Document the upgrade path:

1. Update `runtime-version` in the manifest (keep `scripts/build-flatpak.sh` default `CROSSHOOK_FLATPAK_RUNTIME_VERSION` aligned, or override via env)
2. Test WebKitGTK rendering on GNOME 50 (NVIDIA DMABUF workaround may change between runtimes)
3. Verify GNOME 50 SDK provides all build dependencies for any future build-from-source Flathub work
4. Update CI container image / `flatpak-github-actions` image tags when a Flatpak job pins a GNOME major

### 4.7 Prototype Cleanup

**Done — [#195](https://github.com/yandy-r/crosshook/issues/195).** `packaging/flatpak/build-dir/` and `packaging/flatpak/repo/` were removed. The prototype was a `flatpak build-init` output and ad hoc local repo, not a reproducible build definition. New committed artifacts replace them.

---

## 5. Feature Requirements

### 5.1 Phase 1 — Flatpak Build Infrastructure (MVP)

**Goal**: Produce a working `.flatpak` bundle from the pre-built binary.

| #     | Requirement                                                                                                                                                                   | Priority |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| F1.1  | Remove `packaging/flatpak/build-dir/` and `packaging/flatpak/repo/` (**done — [#195](https://github.com/yandy-r/crosshook/issues/195)**)                                        | P0       |
| F1.2  | Create committed `packaging/flatpak/dev.crosshook.CrossHook.yml` manifest with `--filesystem=home` + explicit mount paths                                                 | P0       |
| F1.3  | Create committed `packaging/flatpak/dev.crosshook.CrossHook.desktop` static desktop entry                                                                                 | P0       |
| F1.4  | Create committed `packaging/flatpak/dev.crosshook.CrossHook.metainfo.xml` with all Flathub-required fields (developer, screenshots placeholder, content rating, releases) | P0       |
| F1.5  | Create `scripts/build-flatpak.sh` — stages binary + resources, runs `flatpak-builder`, produces `.flatpak` bundle                                                             | P0       |
| F1.6  | Add `is_flatpak()` detection to `crosshook-core/src/platform.rs` (checks `FLATPAK_ID` env var and `/.flatpak-info` file)                                                      | P0       |
| F1.7  | Add `/app/resources/` fallback to resource path resolution in `paths.rs`                                                                                                      | P0       |
| F1.8  | Add 128x128 icon size to `scripts/generate-assets.sh` pipeline                                                                                                                | P1       |
| F1.9  | Change Tauri `identifier` from `com.crosshook.native` to `dev.crosshook.CrossHook` across all config files                                                                | P0       |
| F1.10 | Update all `Cargo.toml`, desktop entry, metainfo, and CI references to new app ID                                                                                             | P0       |

### 5.2 Phase 2 — CI Integration

**Goal**: Automate Flatpak builds in the release pipeline.

| #    | Requirement                                                                      | Priority |
| ---- | -------------------------------------------------------------------------------- | -------- |
| F2.1 | Add `flatpak` job to `release.yml` running in parallel with AppImage job         | P0       |
| F2.2 | Use `flatpak/flatpak-github-actions/flatpak-builder@v6` with GNOME SDK container | P0       |
| F2.3 | Upload `.flatpak` bundle as GitHub Release artifact alongside AppImage           | P0       |
| F2.4 | Add `appstreamcli validate` and `desktop-file-validate` CI steps                 | P1       |
| F2.5 | Document GNOME runtime version upgrade path in `packaging/flatpak/README.md`     | P1       |

### 5.3 Phase 3 — Process Execution Hardening

**Goal**: All external binary calls work reliably within the sandbox.

| #    | Requirement                                                                                                                                                                                                                    | Priority |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| F3.1 | Implement `host_command()` in `crosshook-core/src/platform.rs` — conditionally wraps `Command::new` with `flatpak-spawn --host` when `is_flatpak()` is true                                                              | P0       |
| F3.2 | Migrate all 12 external binary call sites to use `host_command()`                                                                                                                                                        | P0       |
| F3.3 | Replace `/usr/bin/rm` calls in `run_executable.rs` with `std::fs::remove_dir_all`                                                                                                                                              | P0       |
| F3.4 | Add `FLATPAK_ID` detection to all three bundled helper scripts; prefix host commands with `flatpak-spawn --host`                                                                                                               | P0       |
| F3.5 | Make Proton/compatibility-tool discovery Flatpak-aware — system paths (`/usr/share/steam/compatibilitytools.d/`, `/usr/lib/steam/`) are invisible inside the sandbox; use `flatpak-spawn --host` to enumerate and resolve them | P0       |
| F3.6 | Implement graceful degradation for `unshare --user --net` — when seccomp blocks it, show persistent badge/icon on affected profiles indicating network isolation is unavailable                                                | P1       |
| F3.7 | Test each external binary in sandbox: `git`, `unshare`, `gamescope`, `lspci`, `getent`, `/bin/bash`, `kill`, Proton, game executables, wrapper commands, self-restart, helper scripts                                          | P0       |

### 5.4 Phase 4 — Flathub Submission

**Goal**: Publish to Flathub for discoverability.

| #    | Requirement                                                                                                         | Priority |
| ---- | ------------------------------------------------------------------------------------------------------------------- | -------- |
| F4.1 | Create publishable screenshots for metainfo (profile editor, launch monitor, settings)                              | P0       |
| F4.2 | Generate OARS content rating via https://hughsie.github.io/oars/                                                    | P0       |
| F4.3 | Prepare build-from-source manifest using `flatpak-cargo-generator.py` + `flatpak-node-generator` for offline builds | P0       |
| F4.4 | Fork `flathub/flathub`, create manifest PR targeting `new-pr` branch                                                | P0       |
| F4.5 | Respond to reviewer feedback — expect 2–3 rounds based on Tauri v2 precedent (Pomodorolm)                           | P0       |
| F4.6 | If filesystem permissions are challenged, present Lutris precedent and CrossHook's multi-drive justification        | P1       |

---

## 6. Sandbox & Permissions Design

### 6.1 Complete `finish-args`

```yaml
finish-args:
  # Display
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --device=dri

  # Audio (games via Proton)
  - --socket=pulseaudio

  # Network (ProtonDB, SteamGridDB, GE-Proton downloads, git clone)
  - --share=network

  # Host command execution (Proton, Wine, Steam, gamescope, etc.)
  - --talk-name=org.freedesktop.Flatpak

  # WebKitGTK NVIDIA workaround
  - --env=WEBKIT_DISABLE_DMABUF_RENDERER=1

  # Filesystem — home + external drives
  - --filesystem=home
  - --filesystem=/mnt
  - --filesystem=/run/media
  - --filesystem=/media

  # Flatpak Steam discovery (read-only)
  - --filesystem=~/.var/app/com.valvesoftware.Steam:ro
```

### 6.2 Permission Justification Matrix

| Permission                               | Justification                                                                                                                                                                                                                                                                                                                                                                        | Flathub Precedent                    |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------ |
| `--filesystem=home`                      | Steam default library, user config, downloads                                                                                                                                                                                                                                                                                                                                        | Lutris, Heroic                       |
| `--filesystem=/mnt`                      | User Steam libraries on additional NVMe/SSD drives                                                                                                                                                                                                                                                                                                                                   | Lutris (`/run/media`, `/media`)      |
| `--filesystem=/run/media`                | Removable media mount point (USB, external drives)                                                                                                                                                                                                                                                                                                                                   | Lutris, Heroic                       |
| `--filesystem=/media`                    | Legacy removable media mount point                                                                                                                                                                                                                                                                                                                                                   | Lutris, Heroic                       |
| `--talk-name=org.freedesktop.Flatpak`    | Host process execution via `flatpak-spawn --host` for Proton, Wine, Steam, gamescope, git, system binaries. **Also required** for discovering system-installed Proton at `/usr/share/steam/compatibilitytools.d/` — Flatpak mounts the runtime's `/usr` over the host's, so these paths are invisible via `--filesystem` permissions and must be accessed via `flatpak-spawn --host` | Lutris (only gaming tool using this) |
| `--socket=pulseaudio`                    | Games require audio via Wine/Proton                                                                                                                                                                                                                                                                                                                                                  | Lutris, Heroic, Bottles              |
| `--share=network`                        | ProtonDB API, SteamGridDB, GE-Proton downloads, community tap git clone                                                                                                                                                                                                                                                                                                              | All gaming tools                     |
| `--env=WEBKIT_DISABLE_DMABUF_RENDERER=1` | Prevents blank screen on NVIDIA with Wayland (Tauri/WebKitGTK issue)                                                                                                                                                                                                                                                                                                                 | Lutris uses same workaround          |

---

## 7. Architecture Changes

### 7.1 New Module: `crosshook-core/src/platform.rs`

```rust
use std::path::Path;
use tokio::process::Command;

/// Returns true when running inside a Flatpak sandbox.
pub fn is_flatpak() -> bool {
    std::env::var_os("FLATPAK_ID").is_some()
        || Path::new("/.flatpak-info").exists()
}

/// Creates a Command that runs on the host when inside Flatpak.
/// Outside Flatpak, returns a normal Command.
pub fn host_command(program: &str) -> Command {
    if is_flatpak() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(program);
        cmd
    } else {
        Command::new(program)
    }
}
```

### 7.2 Host Filesystem Discovery

Flatpak mounts the runtime's `/usr` over the host's `/usr`. System-installed Proton (e.g., `proton-cachyos-slr` at `/usr/share/steam/compatibilitytools.d/`) is invisible via `--filesystem` permissions. Discovery must go through `flatpak-spawn --host`:

```rust
/// Lists entries in a host directory that may be invisible inside the sandbox.
/// Falls back to direct std::fs::read_dir outside Flatpak.
pub async fn host_read_dir(path: &Path) -> Result<Vec<String>> {
    if is_flatpak() {
        let output = host_command("ls")
            .arg(path)
            .output().await?;
        // parse stdout lines into entries
    } else {
        // direct filesystem read
    }
}
```

**Affected code paths**:

- Proton/compatibility-tool discovery scanning `/usr/share/steam/compatibilitytools.d/`
- Any future discovery of system-installed Steam data under `/usr/lib/steam/`

The Proton discovery code must call `host_read_dir()` for system paths and direct `read_dir()` for user paths (`~/.steam/`, Steam library folders on mounted drives).

### 7.3 Resource Path Fallback in `paths.rs`

Add a Flatpak-aware branch to the resolution chain:

```rust
// After standard Tauri resource resolution fails:
if crate::platform::is_flatpak() {
    let flatpak_path = PathBuf::from("/app/resources").join(script_name);
    if flatpak_path.exists() {
        return Some(flatpak_path);
    }
}
```

### 7.3 Helper Script Pattern

Each bundled shell script adds a Flatpak-aware command wrapper:

```bash
# Flatpak host execution wrapper
if [ -n "$FLATPAK_ID" ]; then
    run_host() { flatpak-spawn --host "$@"; }
else
    run_host() { "$@"; }
fi

# Usage: replace direct calls
run_host pgrep -f "$game_name"
run_host steam steam://rungameid/"$app_id"
```

### 7.4 Graceful Degradation: Network Isolation

When `unshare --user --net` fails inside the Flatpak sandbox (seccomp blocks it), the existing `is_unshare_net_available()` probe catches this. The Flatpak-specific behavior:

- The probe returns `false` inside the sandbox
- A persistent badge/icon appears on affected profiles in the UI, indicating "Network isolation unavailable in Flatpak"
- Launch proceeds without network isolation — the trainer runs with full network access
- No blocking dialog or toast — the badge is the notification mechanism

---

## 8. Testing Strategy

### 8.1 Manual Verification Matrix

| #    | Test Case                                                                    | Expected Result                                                         | Priority | Phase |
| ---- | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------- | -------- | ----- |
| T1   | Install `.flatpak` bundle, launch app                                        | App window opens, no blank screen                                       | P0       | 1     |
| T2   | Steam library auto-detection (native Steam, `$HOME`)                         | Discovers `~/.local/share/Steam`                                        | P0       | 1     |
| T3   | Steam library auto-detection (Flatpak Steam)                                 | Discovers `~/.var/app/com.valvesoftware.Steam/data/Steam`               | P0       | 1     |
| T4   | Steam library on external drive (`/mnt/nvme1`)                               | Discovers library, lists games, resolves Proton versions                | P0       | 1     |
| T5   | Steam library on removable media (`/run/media/user/SSD`)                     | Discovers library, lists games                                          | P0       | 1     |
| T5.1 | System-installed Proton discovery (`/usr/share/steam/compatibilitytools.d/`) | Proton versions found via `flatpak-spawn --host`; selectable in profile | P0       | 3     |
| T6   | Create profile, launch game via Proton (home drive)                          | Game launches, trainer launches after delay                             | P0       | 3     |
| T7   | Create profile, launch game via Proton (external drive)                      | Game launches from `/mnt/nvme1`, trainer works                          | P0       | 3     |
| T8   | `pgrep` game process detection in helper scripts                             | Process detected via `flatpak-spawn --host pgrep`                       | P0       | 3     |
| T9   | ProtonDB integration                                                         | Ratings load from network                                               | P1       | 1     |
| T10  | GE-Proton download and install                                               | Downloads and extracts to correct path                                  | P1       | 3     |
| T11  | Community tap clone                                                          | `git clone` succeeds via `flatpak-spawn --host git`                     | P1       | 3     |
| T12  | Trainer network isolation (`unshare`)                                        | Degrades gracefully — badge shown, launch proceeds                      | P1       | 3     |
| T13  | Settings persistence across restarts                                         | Settings at `~/.config/crosshook/`                                  | P1       | 1     |
| T14  | SQLite DB persistence                                                        | DB at `~/.local/share/crosshook/`                                   | P1       | 1     |
| T15  | NVIDIA GPU with Wayland                                                      | No blank screen (DMABUF workaround active)                              | P2       | 1     |
| T16  | `gamescope` wrapper launch                                                   | Compositor wraps game via `flatpak-spawn --host gamescope`              | P2       | 3     |
| T17  | Diagnostics export with `lspci`                                              | GPU info captured via `flatpak-spawn --host lspci`                      | P2       | 3     |

### 8.2 CI Validation

| Step                          | Tool                                                             | Phase |
| ----------------------------- | ---------------------------------------------------------------- | ----- |
| MetaInfo validation           | `appstreamcli validate dev.crosshook.CrossHook.metainfo.xml` | 2     |
| Desktop file validation       | `desktop-file-validate dev.crosshook.CrossHook.desktop`      | 2     |
| Bundle builds without error   | `flatpak-builder` exit code 0                                    | 2     |
| Bundle installs without error | `flatpak install --user <bundle>`                                | 2     |

---

## 9. Risks & Mitigations

### High Risk

| Risk                                                                                                                                                               | Impact                                                                                     | Mitigation                                                                                                                                                                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`flatpak-spawn --host` fails for specific binaries** — some host binaries may not be accessible even with D-Bus permission                                       | Core launch broken for those binaries                                                      | Test each of the 12 binaries individually in Phase 3. Fall back to documenting unsupported configurations.                                                                  |
| **System-installed Proton invisible** — `/usr/share/steam/compatibilitytools.d/` is masked by the runtime's `/usr`; `--filesystem` cannot expose host `/usr` paths | Distro-packaged Proton (e.g., `proton-cachyos-slr`) not discoverable, games fail to launch | Discovery code uses `flatpak-spawn --host` to read host `/usr` paths (§7.2). This is the only viable approach without `--filesystem=host-os`.                               |
| **Flathub rejects `--filesystem=/mnt` or `--talk-name=org.freedesktop.Flatpak`**                                                                                   | Cannot publish to Flathub                                                                  | Lutris uses both. Prepare justification document citing multi-drive Steam library discovery and Lutris precedent. Self-hosted distribution (GitHub Releases) is unaffected. |
| **Tauri app ID change breaks existing user data**                                                                                                                  | AppImage users lose settings/DB on upgrade                                                 | The ID change applies to the Flatpak target. AppImage users on the old ID path need a one-time migration. Document and implement data migration in Phase 1.                 |

### Medium Risk

| Risk                                                                  | Impact                                  | Mitigation                                                                                                        |
| --------------------------------------------------------------------- | --------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| **Tauri `BaseDirectory::Resource` doesn't resolve `/app/resources/`** | Bundled scripts not found, launch fails | Explicit `/app/resources/` fallback in `paths.rs` (F1.7)                                                          |
| **WebKitGTK rendering on NVIDIA**                                     | Blank screen or flickering              | `WEBKIT_DISABLE_DMABUF_RENDERER=1` in finish-args (matches Lutris)                                                |
| **Flathub review takes 3+ rounds**                                    | Delays Phase 4 timeline                 | Budget for extended review. Pomodorolm (Tauri v2) described review as "pretty serious."                           |
| **`unshare` blocked by seccomp**                                      | Network isolation unavailable           | Graceful degradation with persistent badge (F3.5). Existing `is_unshare_net_available()` probe handles detection. |

### Low Risk

| Risk                                            | Impact                               | Mitigation                                                                     |
| ----------------------------------------------- | ------------------------------------ | ------------------------------------------------------------------------------ |
| **CI build time increase**                      | Longer releases                      | Flatpak job runs in parallel with AppImage job                                 |
| **XDG path confusion**                          | Users look for config/data/cache in wrong place | Document that Phase 1 uses host-shared XDG paths: `~/.config/crosshook/`, `~/.local/share/crosshook/`, and `~/.cache/crosshook/` |
| **GNOME runtime version bump breaks something** | Rendering or dependency issue        | Document upgrade path; pin to specific version in manifest                     |

---

## 10. Persistence & Data

### 10.1 Data Classification

| Datum                               | Storage            | Location (Phase 1 Flatpak, host-shared)   |
| ----------------------------------- | ------------------ | ------------------------------------------ |
| User settings (TOML)                | TOML settings file | `~/.config/crosshook/` (host-shared)       |
| Game metadata, profiles (SQLite)    | SQLite metadata DB | `~/.local/share/crosshook/` (host-shared)  |
| Wine prefixes                       | On-disk prefix dir | `~/.local/share/crosshook/prefixes/` (host-shared) |
| Community taps                      | Git clone          | `~/.local/share/crosshook/community/`      |
| Image cache                         | Runtime cache      | `~/.cache/crosshook/` (host-shared)        |
| `is_flatpak()` result               | Runtime-only       | Memory (`FLATPAK_ID` / `/.flatpak-info` in `platform.rs`) |
| Flatpak host spawns                 | Runtime-only       | `host_command()` when `is_flatpak()` (not in shell helpers) |

### 10.2 Migration & Backward Compatibility (Phase 1 decision)

**Decision**: Phase 1 shares XDG state between the AppImage and the Flatpak build instead of following Flatpak's per-app isolation. On startup inside a Flatpak sandbox, `crosshook_core::platform::override_xdg_for_flatpak_host_access()` rewrites `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, and `XDG_CACHE_HOME` back to their host defaults (`$HOME/.config`, `$HOME/.local/share`, `$HOME/.cache`). `--filesystem=home` exposes those paths to the sandbox, and `directories::BaseDirs` resolves them on first access. The effect: every store (`SettingsStore`, `ProfileStore`, `MetadataStore`, `CommunityTapStore`, image cache, wine prefixes) points at the same directory tree the AppImage uses.

**Why not per-app isolation for Phase 1?** Per-app isolation would require (a) copying or symlinking `~/.config/crosshook/` into the sandbox, (b) choosing which of `~/.local/share/crosshook/`'s subtrees to migrate (the metadata DB and community taps are small; wine prefixes can be 10–100 GB), (c) a prefix root override so the Flatpak continues to use the host prefix location, and (d) a "flip the switch" mechanism for users who want to migrate between the two modes. That is a multi-file refactor that belongs in its own tracking issue, not Phase 1 MVP.

**Why not silent data-less first run?** The earlier text in this section — *"XDG paths change from `~/.config/crosshook/` to `~/.var/app/…/config/crosshook/`. Flatpak remaps automatically via `XDG_CONFIG_HOME` — no code change needed if the app uses `directories::BaseDirs`"* — was wrong in practice. The remap does happen, but it silently orphans the user's existing profiles, settings, game metadata, and wine prefixes. An AppImage user installing the Flatpak saw an empty UI even though their data was visible to the sandbox at the absolute host path. The override corrects this.

**Legacy app-id migration** (`com.crosshook.native → dev.crosshook.CrossHook`): The one-time XDG migration in `app_id_migration.rs` runs against whatever `BaseDirs` resolves to at the time. With the host override in place, both the source and destination are host paths. In practice this is a no-op for existing AppImage installs because `crosshook-core` has always written directly to `$XDG_CONFIG_HOME/crosshook/` (no app-id parent segment); the migration exists only to catch any stale Tauri-managed state at `$XDG_CONFIG_HOME/com.crosshook.native/`.

**Offline behavior**: All persistence is local. No network dependency for settings or DB.

**Degraded behavior**: If SQLite DB is inaccessible (permission issue), the app should fail with a clear error rather than silently losing data.

**Caveat**: Sharing data means the AppImage and Flatpak must not run concurrently with conflicting writes. Both use a process-local `io_lock` on settings; neither has cross-process file locking. Two concurrent instances can corrupt `settings.toml` (this is not a new risk — two AppImage instances have the same problem). Users running both should pick one as primary.

### 10.3 Phase 4 follow-up — per-app isolation (deferred)

Phase 4 (Flathub submission) will replace the host-shared model with proper per-app isolation:

1. On first Flatpak launch, copy `~/.config/crosshook/` into `$XDG_CONFIG_HOME/crosshook/` (inside the sandbox).
2. Selectively copy `~/.local/share/crosshook/` into `$XDG_DATA_HOME/crosshook/`, excluding large subtrees (`prefixes/`, `artifacts/`, `cache/`).
3. Override the wine prefix root so existing host prefixes remain usable without a multi-gigabyte copy.
4. Keep `override_xdg_for_flatpak_host_access()` as a fallback for users who prefer host-shared state (config flag or env var).

Flathub is expected to require isolation because it is the standard sandbox contract every other Flathub app honours. Tracked in the Phase 4 sub-issue created alongside the commit that introduces the Phase 1 override (see the issue linked from #210).

---

## 11. Implementation Phases

### Phase 1: Flatpak Build Infrastructure (MVP)

**Gate**: Working `.flatpak` bundle that installs, launches, discovers Steam libraries on all drives.

1. **Done (#195):** Removed `packaging/flatpak/build-dir/` and `packaging/flatpak/repo/`
2. Change Tauri app ID to `dev.crosshook.CrossHook` across all configs
3. Implement data migration for old app ID paths
4. Add `is_flatpak()` + `host_command()` to `crosshook-core/src/platform.rs`
5. Add `/app/resources/` fallback to `paths.rs`
6. Add 128x128 to icon generation pipeline
7. Create committed Flatpak manifest, desktop file, metainfo XML
8. Create `scripts/build-flatpak.sh`
9. Manual testing: T1–T5, T9, T13–T15

### Phase 2: CI Integration

**Gate**: Flatpak bundle is automatically built and published to GitHub Releases on tag push.

1. Add `flatpak` job to `release.yml` (parallel with AppImage)
2. Add metainfo + desktop file validation steps
3. Upload `.flatpak` bundle as release artifact
4. Document GNOME runtime upgrade path

### Phase 3: Process Execution Hardening

**Gate**: All 12 external binaries verified working inside sandbox. Helper scripts Flatpak-aware.

1. Implement `host_command()` abstraction
2. Migrate all `Command::new` call sites
3. Replace `/usr/bin/rm` with `std::fs::remove_dir_all`
4. Add `FLATPAK_ID` detection + `run_host()` wrapper to all three helper scripts
5. Implement `unshare` graceful degradation with persistent badge
6. Manual testing: T6–T8, T10–T12, T16–T17

### Phase 4: Flathub Submission

**Gate**: Accepted on Flathub.

1. Create screenshots for metainfo
2. Generate OARS content rating
3. Build from-source manifest with offline dependency generators
4. Submit Flathub PR
5. Address reviewer feedback (budget 2–3 rounds)

---

## 12. Comparable Projects

| Project                   | Filesystem Perms                           | Host Execution                   | Key Lesson                                                                                                     |
| ------------------------- | ------------------------------------------ | -------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| **Lutris**                | `home`, `/run/media`, `/media`, Steam path | `flatpak-spawn --host` via D-Bus | Closest precedent. Proves gaming tools with broad FS + host exec are Flathub-accepted.                         |
| **Heroic**                | Targeted paths per-tool                    | None (bundles Wine Manager)      | Tighter perms possible only by bundling Wine — not viable for CrossHook.                                       |
| **Bottles**               | Zero (portals only)                        | None (internal Wine)             | Maximum sandboxing requires reimplementing all file access — impractical for CrossHook.                        |
| **ProtonUp-Qt**           | Per-tool Steam/Lutris paths                | `flatpak-spawn --host`           | Targeted paths work when all locations are known. CrossHook's paths are more dynamic.                          |
| **Pomodorolm** (Tauri v2) | Minimal                                    | None                             | Only well-documented Tauri v2 Flathub case. Resource path `/app/` workaround documented. Review was extensive. |

---

## 13. Open Questions

| #   | Question                                                                                                                | Decision Needed By | Owner |
| --- | ----------------------------------------------------------------------------------------------------------------------- | ------------------ | ----- |
| 2   | Are publishable screenshots available, or do they need to be created for Phase 4?                                       | Phase 4 start      | Yandy |
| 4   | Is `/opt` a common Steam library mount point that should be added to filesystem permissions?                            | Phase 1 testing    | Yandy |

---

## 14. References

### Internal

- Issue: [#69 — Flatpak distribution target](https://github.com/yandy-r/crosshook/issues/69)
- Feature spec: [`docs/prps/specs/flatpak-distribution-spec.md`](../specs/flatpak-distribution-spec.md)
- Research: `docs/research/additional-features/deep-research-report.md`
- Implementation guide: `docs/research/additional-features/implementation-guide.md`

### External

- [Flathub Submission Requirements](https://docs.flathub.org/docs/for-app-authors/requirements)
- [Flathub MetaInfo Guidelines](https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines)
- [Tauri v2 Flatpak Guide](https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/distribute/flatpak.mdx)
- [Pomodorolm Flatpak Packaging Blog](https://vincent.jousse.org/blog/en/packaging-tauri-v2-flatpak-snapcraft-elm/)
- [Lutris Flathub Manifest](https://github.com/flathub/net.lutris.Lutris/blob/master/net.lutris.Lutris.yml)
- [Heroic Flathub Manifest](https://github.com/flathub/com.heroicgameslauncher.hgl/blob/master/com.heroicgameslauncher.hgl.yml)
- [Bottles Flathub Manifest](https://github.com/flathub/com.usebottles.bottles/blob/master/com.usebottles.bottles.yml)
- [Flatpak GitHub Actions](https://github.com/flatpak/flatpak-github-actions)
- [Flatpak Cargo Generator](https://github.com/flatpak/flatpak-builder-tools/tree/master/cargo)
- [flatpak-spawn man page](https://man7.org/linux/man-pages/man1/flatpak-spawn.1.html)
