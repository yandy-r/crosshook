## Executive Summary

CrossHook can satisfy `protonup-integration` acceptance criteria fastest by integrating with `protonup-rs` as the primary engine (library-first if licensing/size constraints allow, CLI wrapper fallback otherwise), using upstream GitHub release metadata for version discovery, and writing into Steam/Lutris compatibility tool paths that are already encoded in `libprotonup`.

Key outcomes:

- **List available versions**: Query release metadata from upstream projects (`proton-ge-custom`, `wine-ge-custom`) via `protonup-rs`/`libprotonup` release listing flow.
- **Install from CrossHook**: Execute `protonup-rs` non-interactive mode or call `libprotonup` download/unpack APIs to install into Steam/Lutris/native or Flatpak paths.
- **Community requirement suggestions**: Compare community-required Proton family/version against installed filesystem versions and cached release list; surface install suggestion in UI.

Recommended trust model:

- Prefer release checksums (`.sha512sum` / `.sha256`) when provided and verify before install.
- Treat GitHub asset `digest` values as additional integrity signal, not as publisher signature.
- For strong provenance on CrossHook-bundled artifacts (if any), leverage signed attestations where available (e.g., `protonup-rs` release includes `multiple.intoto.jsonl`).

### Candidate APIs and Services

#### 1) protonup-rs (primary candidate)

Official sources:

- Repository: <https://github.com/auyer/Protonup-rs>
- Library docs: <https://docs.rs/libprotonup/latest/libprotonup/>
- Project docs: <https://raw.githubusercontent.com/auyer/Protonup-rs/main/docs/docs.md>

Capabilities relevant to CrossHook:

- CLI non-interactive installs (`--tool`, `--version`, `--for`, `--force`) in addition to interactive mode.
- Tool/release listing via GitHub releases API abstraction (`list_releases` in `libprotonup::downloads`).
- Native + Flatpak app detection (Steam/Lutris) and installation directory resolution.
- Archive unpack support for `.tar.gz`, `.tar.xz`, `.tar.zst`.
- Hash verification support (`sha512` and `sha256`) through `libprotonup::hashing`.

Installation modes (from official docs + source):

- `--for steam` / `--for lutris` / custom path.
- App install targets include:
- Steam native: `~/.steam/steam/compatibilitytools.d/`
- Steam Flatpak: `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d/`
- Lutris native: `~/.local/share/lutris/` (subfolders by tool type)
- Lutris Flatpak: `~/.var/app/net.lutris.Lutris/data/lutris/`

Compatibility notes:

- `protonup-rs` is not affiliated with the original Python ProtonUp project; it is a Rust implementation.
- Supports both Proton-family (Steam) and Wine-family (Lutris/non-Steam runner) tools, matching CrossHook’s target use case.
- Has both library and CLI surface, enabling staged integration.

#### 2) ProtonUp-Qt (desktop UX reference + fallback interoperability)

Official sources:

- Repository: <https://github.com/DavidoTek/ProtonUp-Qt>
- Website: <https://davidotek.github.io/protonup-qt>
- Wiki: <https://github.com/DavidoTek/ProtonUp-Qt/wiki>

Relevance:

- Mature desktop UX reference for selecting app target and installing GE-Proton/Wine-GE.
- Useful to validate expected user flows and naming conventions in CrossHook.
- Not the best direct embedding target for CrossHook (Python/Qt stack mismatch vs Rust core/Tauri).

Compatibility notes:

- Declares itself independent from launcher/tool creators.
- Distributed as Flatpak/AppImage; useful for behavioral parity checks, not core integration library.

#### 3) Steam compatibility tools behavior (path/layout contract)

Official sources:

- Proton README (Valve): <https://raw.githubusercontent.com/ValveSoftware/Proton/master/README.md>
- `compatibilitytool.vdf` template: <https://raw.githubusercontent.com/ValveSoftware/Proton/master/compatibilitytool.vdf.template>

Behavior references:

- Steam local compatibility tools live under `~/.steam/root/compatibilitytools.d/` (with common symlinked equivalents such as `~/.steam/steam/...`).
- Tool package must include expected layout (e.g., `compatibilitytool.vdf`, `toolmanifest.vdf`, `proton`, etc.).
- Steam typically needs restart to discover newly installed compatibility tools.

Compatibility notes:

- Path aliases (`~/.steam/root` vs `~/.steam/steam`) can vary by distro packaging/symlink setup.
- `protonup-rs` defaults to `~/.steam/steam/` for native Steam; this generally works when symlinks are standard.

#### 4) GE-Proton / Wine-GE release metadata + integrity signals

Trusted upstream metadata endpoints:

- GE-Proton releases: <https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases>
- Wine-GE releases: <https://api.github.com/repos/GloriousEggroll/wine-ge-custom/releases>

Examples:

- Latest GE-Proton release includes `.tar.gz` and `.sha512sum` assets.
- Latest Wine-GE release also publishes `.tar.xz` and `.sha512sum` assets.

Integrity/signature notes:

- Checksums are available and supported by `libprotonup` verification flow.
- GitHub release assets now expose `digest` fields (sha256) in API responses, but this is not equivalent to author cryptographic signing (e.g., GPG/minisign/cosign key ownership proof).
- `protonup-rs` own release pipeline currently ships `multiple.intoto.jsonl` attestation artifact, which can inform higher-assurance supply-chain checks if CrossHook chooses to consume external binaries.

## Libraries and SDKs

### Recommended

1. **`libprotonup` (Rust crate, from `protonup-rs`)**

- Docs: <https://docs.rs/libprotonup/latest/libprotonup/>
- Why: Native Rust APIs for release listing, app detection, download streaming, archive unpack, hash verification.
- Fit: Aligns with CrossHook’s Rust core (`crosshook-core`) architecture.

2. **`protonup-rs` CLI**

- Repo: <https://github.com/auyer/Protonup-rs>
- Why: Fastest path to ship install functionality; usable as subprocess with progress capture.
- Fit: Good fallback if direct crate embedding introduces dependency/licensing risk.

### Optional / contextual

3. **GitHub Releases REST API (direct integration)**

- Docs: <https://docs.github.com/en/rest/releases/releases>
- Why: Could power independent listing/cache if CrossHook avoids `libprotonup`.
- Tradeoff: Re-implements filtering, path logic, hash handling already present upstream.

4. **ProtonUp-Qt codebase**

- Repo: <https://github.com/DavidoTek/ProtonUp-Qt>
- Why: UX/behavioral reference only.
- Tradeoff: Python/Qt stack unsuitable for direct embedding in Rust core.

### Integration Patterns

#### Pattern A — Direct `libprotonup` embedding (preferred long-term)

Flow:

1. Discover installed app targets (`Steam`/`SteamFlatpak`/`Lutris`/`LutrisFlatpak`) using `libprotonup::apps`.
2. List releases (`libprotonup::downloads::list_releases`) per tool family.
3. Cache normalized release list in CrossHook SQLite external cache entries (with TTL + ETag/updated_at metadata if feasible).
4. On install:

- Resolve install directory from app installation + tool type.
- Download selected asset with progress callback.
- Verify hash (prefer `.sha512sum`/`.sha256` when present).
- Unpack into install path and refresh local installed-version scan.

5. Surface suggestion if community profile requires version not installed.

Pros:

- Unified Rust control path, easier testability.
- No subprocess parsing fragility.

Cons:

- Dependency footprint and update cadence tied to external crate.

#### Pattern B — `protonup-rs` subprocess adapter (fastest shipping)

Flow:

1. Use CrossHook logic for release listing cache OR parse `protonup-rs` output where stable.
2. Execute `protonup-rs --tool ... --version ... --for ... [--force]`.
3. Capture stdout/stderr for progress/status and map to UI state.
4. Validate expected installation path post-install by filesystem scan.

Pros:

- Minimal code in CrossHook for install mechanics.
- Upstream behavior parity.

Cons:

- CLI output may change across versions.
- Harder to provide granular progress unless upstream offers machine-readable output contract.

#### Pattern C — Hybrid

Use `libprotonup` for listing/detection and keep CLI fallback for installation edge-cases (or vice versa). This de-risks first release while preserving migration path to pure in-process API.

## Constraints and Gotchas

1. **Source-of-truth drift for Wine-GE**

- `wine-ge-custom` README now states repo is effectively sunset in favor of newer launcher approaches; release cadence is old compared to GE-Proton.
- Integration should treat Wine-GE support as best-effort and avoid hard-coding assumptions about frequent new tags.

2. **Path/environment variance**

- Steam and Lutris native vs Flatpak paths differ; distro-specific symlink layouts can hide incorrect assumptions.
- Must prefer detected install mode over static path defaults.

3. **Integrity != signature**

- `.sha512sum`/`.sha256` verification protects against corruption/tampering in transit if checksums are trusted from source channel.
- Without publisher key verification/signatures, this is not full provenance assurance.

4. **Large artifacts + UX**

- GE-Proton tarballs are very large (hundreds of MB), making cancellation/resume/failure UX important.
- Download progress should remain runtime-only as planned; consider resumable strategy later.

5. **Steam refresh behavior**

- New tools may require Steam restart before appearing in compatibility dropdown.
- CrossHook should explicitly communicate this post-install.

6. **Licensing and update policy**

- `protonup-rs` and ProtonUp-Qt are GPL-3.0 projects.
- If statically linking or vendoring code/binaries, CrossHook maintainers need an explicit legal/compliance decision.

## Open Decisions

1. **Integration path selection**

- Choose one: `libprotonup` embedding, `protonup-rs` CLI adapter, or hybrid phased approach.

2. **Trust policy**

- Minimum: checksum verification from upstream release assets.
- Optional stronger mode: require additional provenance signals (e.g., attestation validation for bundled helper binaries).

3. **Tool scope for v1**

- GE-Proton only (Steam) vs GE-Proton + Wine-GE (Lutris) on day one.

4. **Suggestion matching strictness**

- Exact version requirement matching vs family-compatible nearest/newer suggestion policy.

5. **Cache refresh strategy**

- TTL-only refresh vs conditional refresh with release metadata (`updated_at`) and manual “refresh now”.

6. **Progress/telemetry contract**

- Define install state model for UI (queued/downloading/verifying/unpacking/done/failed) and whether progress comes from in-process APIs or subprocess output parsing.
