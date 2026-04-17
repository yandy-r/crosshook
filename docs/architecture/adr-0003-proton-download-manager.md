# ADR-0003: Native Proton download manager architecture

**Status**: Accepted — 2026-04-17

---

## Context

CrossHook is a native Linux desktop application (Tauri v2, AppImage today,
Flatpak target tracked under issue [#276]). Issue [#274] adds a native Proton
version manager — download, verify, and extract GE-Proton and Proton-CachyOS
releases — that works consistently in both AppImage and Flatpak without
bundling a sandbox helper or shelling out to a host-side tool.

The design is grounded in three research anchors from the Flatpak deep-research
series:

- `docs/research/flatpak-bundling/14-recommendations.md` §3.1 (Proton version
  download manager) and §3.2 (Flatpak Steam write access negotiation): §3.1
  defines the GitHub API client + extract-to-`compatibilitytools.d/` approach
  that this ADR implements; §3.2 establishes the write-access constraint that
  makes install-root resolution non-trivial under Flatpak Steam.
- `docs/research/flatpak-bundling/13-opportunities.md` SO-2 (Build Proton
  version management as a native feature): identifies this as a high-effort /
  high-impact strategic opportunity that eliminates the `protonup-qt` host
  dependency without requiring `flatpak-spawn --host`.
- `docs/research/flatpak-bundling/11-patterns.md` Tier 3 (Should Build as
  Native Feature): classifies Proton download/management as in-process Rust
  work, not host-tool delegation, explicitly noting that
  `protonup_binary_path` was the placeholder setting.

Today the repo has partial scaffolding in
`src/crosshook-native/crates/crosshook-core/src/protonup/`: `catalog.rs`
handles cache-first GitHub Releases API fetch for GE-Proton and
Proton-CachyOS, `install.rs` handles SHA-512 verification and archive
extraction, and `matching.rs` provides community-version-to-install-name
matching. `mod.rs` defines the shared DTOs (`ProtonUpProvider`,
`ProtonUpAvailableVersion`, `ProtonUpInstallRequest`, `ProtonUpInstallResult`,
etc.) that cross the Tauri IPC boundary via Serde. Settings fields
(`protonup_default_provider`, `protonup_default_install_root`,
`protonup_include_prereleases`) already exist in
`src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`.

This ADR captures the architecture for completing that scaffolding to full
parity: the provider trait model, install-root resolution, the host-gateway
exemption, the SQLite cache boundary, and the rescan-is-truth rule for
installed inventory.

[#274]: https://github.com/yandy-r/crosshook/issues/274
[#276]: https://github.com/yandy-r/crosshook/issues/276

---

## Decision

### Provider trait

An async `ProtonReleaseProvider` trait lives in
`crosshook-core::protonup::providers`. Each upstream is a separate module
that implements this trait (`ge_proton.rs`, `proton_cachyos.rs`, and any
future providers). The trait declares two capability methods in addition to
catalog fetch and download URL resolution:

- `supports_install() -> bool` — signals whether the provider has enough
  metadata (download URL, file size) to drive the install orchestrator in
  `install.rs`. Providers with incomplete API responses set this to `false`
  and are surfaced as browse-only in the UI. Providers such as Proton-EM
  can still opt into installation when the catalog supplies the required
  archive metadata even if checksum coverage is unavailable.
- `checksum_kind() -> ChecksumKind` — declares one of `Sha512Sidecar`
  (GE-Proton: a separate `.sha512sum` sidecar file), `Sha256Manifest`
  (a provider that embeds checksums in the release body), or `None`
  (provider supplies no checksum; install is still allowed, but the
  verification phase emits a warning and continues). `None` is a
  first-class, explicitly flagged install path today and is used by
  Proton-EM. The install orchestrator in `install.rs` reads this
  discriminant and selects the matching verification path, keeping the
  orchestrator free of per-provider conditionals.

The existing `ProtonUpProvider` enum in `protonup/mod.rs` and the
`CatalogProviderConfig` struct in `catalog.rs` are refactored to delegate
to the trait implementations rather than using a `match` arm per provider.

### Install-root resolution

An environment-aware resolver returns an ordered list of
`InstallRootCandidate` values. Each candidate carries the path, a display
label, and a `writable: bool` determined by probing the filesystem at
resolution time (not cached).

Two canonical roots are checked:

1. **Native Steam**: `~/.local/share/Steam/compatibilitytools.d` — always
   writable if the directory exists (CrossHook's Flatpak manifest declares
   `--filesystem=home`).
2. **Flatpak Steam**: `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d`
   — the Flathub manifest for Steam mounts this path `:ro` in some
   configurations. The resolver probes writability at runtime; if the path
   is read-only it is included in the candidate list with `writable: false`
   so the UI can surface it, but the install orchestrator refuses to write
   there. When both candidates are writable, the native-Steam candidate
   sorts first (closer to the host filesystem, less mount-remapping
   ambiguity). The §3.2 research task (negotiating a Flatpak Steam `:rw`
   upgrade on Flathub) is tracked under [#274] and does not block this ADR;
   the resolver handles both present-day `:ro` and future `:rw` without a
   code change.

### Host-gateway exemption

Download (`reqwest`), decompression (`flate2` for `.tar.gz`, `xz2` for
`.tar.xz`), and extraction (`tar`) are in-sandbox library code — they run
inside the CrossHook process, not as host subprocesses. They are explicitly
outside the scope of [ADR-0001]'s host-command gateway contract and its
denylist enforced by `scripts/check-host-gateway.sh`.

No Proton install step shells out to a host-side `tar`, `unzstd`, or
similar binary. All archive I/O is handled by Rust crates linked into
`crosshook-core`. This is what makes the feature work identically in
AppImage and Flatpak without a sandbox helper.

[ADR-0001]: ./adr-0001-platform-host-gateway.md

### SQLite cache boundary

Remote catalog metadata (versions, download URLs, checksums, fetch
timestamps, TTL) lives in a dedicated `proton_release_catalog` table added
in the SQLite metadata DB at schema version 22
(`src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`,
migration `migrate_21_to_22`). This is operational/history metadata — the
same classification as `external_cache_entries` — and lives in
`~/.local/share/crosshook/metadata.db` (WAL, `0600`).

The persistence boundary is explicit:

| Datum                                       | Storage       |
| ------------------------------------------- | ------------- |
| Release catalog (versions, URLs, checksums) | SQLite v22    |
| Default provider                            | TOML settings |
| Default install root                        | TOML settings |
| Include prereleases flag                    | TOML settings |
| Installed Proton inventory                  | Runtime only  |

User preferences (`protonup_default_provider`, `protonup_default_install_root`,
`protonup_include_prereleases`) already exist in
`src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` and remain
there; no preference moves into SQLite.

### Rescan-is-truth rule

`discover_compat_tools_with_roots` in
`src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` is
authoritative for the installed Proton inventory. The download manager does
not maintain a persistent "installed tools" table in SQLite; it would diverge
from the filesystem if the user manually adds, removes, or renames a
compatibility tool directory.

After any install or uninstall operation the orchestrator triggers a rescan
via the existing `discover_compat_tools_with_roots` call path and returns the
fresh result to the IPC caller. The UI always reflects the filesystem state,
not a stale cache.

---

## Consequences

### AppImage + Flatpak parity

Both packaging targets work out of the box. `reqwest`, `flate2`, `xz2`, and
`tar` link into `crosshook-core` and run in-process in both environments.
Flatpak users whose only writable compat-tools path is native Steam
(`~/.local/share/Steam/compatibilitytools.d`) get that path automatically;
no manual configuration is required.

### Degraded mode

If the resolver finds no writable install root — for example, a Flatpak
Steam `:ro` mount with no native Steam present — the UI surfaces a
"read-only" badge on every candidate root and disables the install button.
The install orchestrator never silently writes to a path that the writability
probe marked non-writable. The feature degrades visibly, not silently.

### Checksum heterogeneity

Per-provider `ChecksumKind` keeps the install orchestrator in `install.rs`
simple: it reads the discriminant and calls the matching verification path.
Providers without a checksum (e.g. a future Proton-EM integration) are
`ChecksumKind::None`, and the orchestrator logs a warning and proceeds
rather than failing — making the absence of a checksum an explicit, auditable
choice at the provider level rather than a silent skip in orchestration logic.

### No sandbox bundling

CrossHook does not ship any Proton build inside its Flatpak or AppImage.
Bundling Proton into the app package is explicitly out of scope: it would
make the package multi-gigabytes, require legal review of each Proton
variant's license, and re-create a maintenance burden that ProtonUp-Qt
already owns for its users. This ADR rejects that approach permanently.

---

## Alternatives considered

### Reuse ProtonUp-Qt / protonup-rs

Invoking `protonup-qt` or a wrapped `protonup-rs` binary from CrossHook
was the approach the scaffolded `protonup_binary_path` setting anticipated.
`docs/research/flatpak-bundling/14-recommendations.md` §3.1 rejects this in
favor of a narrower native capability: host-tool invocation adds a Flatpak
boundary crossing (requiring `flatpak-spawn --host` for a sandbox-installed
`protonup-qt`, or a separate Flatpak portal for the Flathub variant), a
hard external dependency that degrades the UX when the tool is absent, and
version coupling to an external project's release cadence. The native Rust
implementation has none of these drawbacks and is CrossHook-native code that
the team maintains directly.

### Persist installed tool inventory as source of truth

Storing a `proton_installed_tools` SQLite table and mutating it on
install/uninstall was considered. Rejected: the filesystem is the ground
truth; a persistent table drifts the moment a user manually installs,
removes, or moves a compatibility tool directory outside CrossHook.
Rescan via `discover_compat_tools_with_roots` is cheap (a few directory
reads) and always accurate. A redundant persistent table adds migration
burden and a class of consistency bugs with no compensating benefit.

---

## Related

- [ADR-0001 — `platform.rs` host-command gateway](./adr-0001-platform-host-gateway.md):
  establishes the host-tool denylist and `flatpak-spawn` gateway contract.
  This ADR's host-gateway exemption (in-process library code) is grounded in
  ADR-0001's scope boundary section, which explicitly excludes in-sandbox
  subprocess and library code from the gateway rule.
- [ADR-0002 — Flatpak portal contracts](./adr-0002-flatpak-portal-contracts.md):
  additive portal integrations; not directly related to Proton download, but
  shares the same Flatpak packaging context.
- Issue [#274] — native Proton download manager (this feature).
- Issue [#276] — parent Flatpak distribution tracker.

---

## References

- `docs/research/flatpak-bundling/14-recommendations.md` §3.1 and §3.2 —
  task definitions for the Proton manager and Flatpak Steam write access
- `docs/research/flatpak-bundling/13-opportunities.md` SO-2 — strategic
  rationale for building Proton management as a native capability
- `docs/research/flatpak-bundling/11-patterns.md` Tier 3 — architecture tier
  guidance confirming in-process Rust (not host delegation) is correct
- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog.rs` —
  existing cache-first GitHub Releases catalog fetcher
- `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs` —
  existing SHA-512 verify + extract orchestrator
- `src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs` —
  shared DTOs and `ProtonUpProvider` enum
- `src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` —
  `discover_compat_tools_with_roots` (rescan-is-truth implementation)
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` —
  SQLite migration chain; v22 adds `proton_release_catalog`
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` —
  TOML settings fields for provider, install root, and prerelease preference
- `src/crosshook-native/crates/crosshook-core/src/platform/mod.rs` —
  ADR-0001 gateway; exemption reasoning is in the scope boundary section
