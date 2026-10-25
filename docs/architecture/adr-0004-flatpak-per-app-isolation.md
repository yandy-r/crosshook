# ADR-0004: Flatpak per-app data isolation

**Status**: Accepted — 2026-04-20

---

## Context

Phase 1 of the Flatpak distribution effort (`override_xdg_for_flatpak_host_access()`,
ADR-0001 table row) was a deliberate stop-gap: it rewrites `XDG_CONFIG_HOME`,
`XDG_DATA_HOME`, and `XDG_CACHE_HOME` back to their host defaults so the Flatpak
could share the legacy host/AppImage-era on-disk tree. That avoided a silent
data-loss first-run experience (empty UI for users migrating from the old
AppImage-era install path to the Flatpak) and was the correct Phase 1 call. It
is, however, not how Flathub apps are expected to behave.

Three pressures make the stop-gap unsustainable for Phase 4 (Flathub submission):

1. **Flathub reviewer stance.** Flathub enforces per-app data directories
   (`~/.var/app/dev.crosshook.CrossHook/{config,data}/`) as the sandbox contract.
   Rewiring XDG back to the host violates that contract and is a known blocker for
   acceptance.

2. **Fedora Silverblue / Bazzite / SteamOS user expectations.** On immutable
   distros, users expect Flatpak apps to be self-contained. Silently writing to
   `~/.config/crosshook/` (outside the sandbox data directory) surprises users and
   breaks backup/restore tooling that keys on `~/.var/app/`.

3. **Legacy data preservation.** A user migrating from the old host/AppImage-era
   data tree to the Flatpak for the first time must not lose their profiles,
   game metadata, and community taps. A one-way import on first run preserves
   that data inside the sandbox without requiring the host-override hack.

Wine prefix sizes (10–100 GB typical) make a full copy infeasible. The prefix root
must stay on the host filesystem regardless of sandbox XDG.

---

## Decision

### Default behaviour: per-app isolation

When CrossHook runs inside a Flatpak sandbox, it uses the standard Flatpak per-app
data directories by default:

- Config: `~/.var/app/dev.crosshook.CrossHook/config/crosshook/`
- Data: `~/.var/app/dev.crosshook.CrossHook/data/crosshook/`
- Cache: `~/.var/app/dev.crosshook.CrossHook/cache/crosshook/`

`override_xdg_for_flatpak_host_access()` is **not** called on normal startup. The
Flatpak sandbox XDG vars are used as-is.

### First-run migration (one-way import)

On the first Flatpak launch, `crosshook_core::flatpak_migration::run()` detects
that the sandbox data directory is empty (filesystem-state driven — no sentinel
file, no DB flag) and imports the legacy host/AppImage-era tree into the
sandbox:

```
include subtrees: crosshook/community, crosshook/media, crosshook/launchers
include files:    crosshook/metadata.db, crosshook/metadata.db-wal, crosshook/metadata.db-shm, crosshook/config_history_hmac.key
skip subtrees:    crosshook/prefixes, crosshook/artifacts, crosshook/cache, crosshook/logs, crosshook/runtime-helpers
```

Config (`~/.config/crosshook/`) is copied verbatim. Data subtrees are copied
selectively per the manifest above. The migration is idempotent: if the sandbox
data directory is already populated (re-run after sandbox reset), the import is
skipped.

### Wine prefix root stays on host

Wine prefixes remain at `~/.local/share/crosshook/prefixes/` on the host
filesystem regardless of sandbox XDG. `crosshook_core::flatpak_migration::host_prefix_root()`
derives this path from `$HOME/.local/share/crosshook/prefixes/` and overrides the
prefix root in all install/service and ad-hoc launch paths.

### Shared mode (opt-in only)

Setting `CROSSHOOK_FLATPAK_HOST_XDG=1` before launch restores the Phase 1
shared-mode behaviour: `override_xdg_for_flatpak_host_access()` is called at
startup and all stores resolve to host paths. This opt-in is:

- Not the default for any distributed build.
- Not documented on Flathub.
- Intended for advanced users who explicitly prefer a single shared data tree.

### Startup entry point

The Flatpak gate, the first-run migration, and the legacy Tauri app-id migration run
in `crosshook_app::bootstrap::pre_runtime`, before any spawned thread, any Tokio
runtime, and GTK. Every call site reaches the base directories through
`crosshook_core::platform::base_dirs()`, the single choke point. The ordering
guarantee is precise:

- The gate, including its XDG override, completes before the first `base_dirs()` lookup.
- The two migrations are the first lookups (`flatpak_migration::run` and
  `migrate_legacy_tauri_app_id_xdg_directories` call `base_dirs()` themselves), and the
  legacy app-id migration follows the first-run migration.
- Every Phase B lookup (stores, logging, catalogs) follows `pre:done`.

`pre_runtime` returns a `PreRuntime` token that `App::bootstrap` consumes, so Phase B
cannot run without it; `App::bootstrap` returns a `BootstrappedApp` (a cloneable
wrapper that derefs to `App` and carries the stores, services, startup handle, and
step log).

Fixed step order (always recorded in the step log; inapplicable steps are `Skipped`):

1. Flatpak gate — not Flatpak: native; `CROSSHOOK_FLATPAK_HOST_XDG=1`: shared mode and
   `override_xdg_for_flatpak_host_access()`; otherwise isolated.
2. First-run Flatpak migration (isolated mode only).
3. Legacy Tauri app-id migration.
4. UI `EnvPolicy` hook (renderer environment; the classic shell sets its WebKit vars here).
5. `pre:done`.

Because the gate rewrites process-global environment variables, `pre_runtime` checks
the thread count (`Threads:` from `/proc/self/status`) before mutating anything. A
multi-threaded process at entry is a contract violation, not a fatal error: production
warns (via `tracing` and `eprintln!`), records the environment step
as `Degraded("multi-threaded at entry")` in the step log, and continues, because
aborting startup would leave the user with no window. An unreadable `/proc` proceeds
with a warning. The ordering is enforced by the unit tests in
`crates/crosshook-app/src/bootstrap/` and by the integration test
`crates/crosshook-app/tests/bootstrap_ordering.rs`. That test runs as a
`harness = false` binary so libtest's worker threads do not exist; it asserts
`Threads: 1` for every recorded environment mutation (rule R1), records the
`base_dirs()` lookups with their thread counts, and asserts the order above (plus a
deliberately misordered scenario that must be rejected).

The frontend never races this startup: the migration outcome, auto-load profile, and
onboarding state are read once per page load through the `get_startup_report` command,
not sleep-delayed events. Job consumers (health, offline readiness) subscribe to the
terminal `startup-job-finished` event first, then read a fresh report; the report is
never memoized for jobs, because a job's state changes after the first read.

---

## Consequences

### Positive

- **Flathub eligibility unblocked.** Per-app isolation satisfies the standard
  Flathub sandbox contract; the XDG override is no longer a submission blocker.
- **User data preserved on upgrade.** Users migrating from legacy
  host/AppImage-era data see their profiles, game metadata, community taps, and
  settings intact on first run.
- **Host/sandbox trees cleanly separated.** Tools that back up `~/.var/app/` capture
  the full CrossHook state; legacy host state is unaffected by Flatpak resets.

### Negative

- **One-way migration — host edits don't sync.** After first import, changes made
  to the legacy host/AppImage-era tree are not reflected in the sandbox and vice
  versa. Users who still run an old AppImage alongside the Flatpak should
  designate one as primary.
- **Sandbox reset triggers re-import.** A `flatpak uninstall --delete-data` removes
  the sandbox tree; the next Flatpak launch re-imports from the current host tree.
  Any changes made only inside the sandbox are lost, and whatever exists on the host
  at that moment becomes the new source of truth. The reset itself does not
  duplicate community tap state — that can only happen if the host tree changed in
  the interim and the re-imported content differs from what the sandbox had before
  the reset.

### Neutral

- **Settings field UI toggle deferred.** There is no in-app UI to switch between
  isolated and shared mode. The env-var (`CROSSHOOK_FLATPAK_HOST_XDG=1`) is the
  only opt-in lever.
- **Host-gateway rules unchanged.** Host-tool invocation contracts (ADR-0001) are
  orthogonal — this ADR covers in-sandbox storage layout; host-tool invocation
  contracts are unchanged.

---

## References

- `docs/prps/plans/completed/flatpak-isolation.plan.md` — implementation plan (tasks 1.1–5.3)
- `docs/prps/prds/flatpak-distribution.prd.md` §10.3 — per-app isolation follow-up
- `docs/plans/bootstrap/plan.md` — startup bootstrap (`pre_runtime` → `App::bootstrap`) plan
- `crates/crosshook-app/tests/bootstrap_ordering.rs` — ordering integration test
- [ADR-0001 — `platform.rs` host-command gateway](./adr-0001-platform-host-gateway.md)
- [ADR-0002 — Flatpak portal contracts](./adr-0002-flatpak-portal-contracts.md)
- Issue [#276] — Flatpak distribution tracker

[#276]: https://github.com/yandy-r/crosshook/issues/276
