# Fix Report: pr-280-review

**Source**: docs/prps/reviews/pr-280-review.md
**Applied**: 2026-04-17 (HIGH pass) + 2026-04-17 (MEDIUM/LOW pass, this run)
**Mode**: Parallel sub-agents — HIGH pass: 1 batch, max width 3; MEDIUM/LOW pass: 2 batches, max width 4
**Severity threshold**: HIGH pass: HIGH; this run: LOW

## Summary

- **Total findings in source**: 16
- **Already processed before this run**:
  - Fixed: 5 (F001–F005)
  - Failed: 0
- **Eligible this run**: 11 (F006–F016 at severity ≥ LOW)
- **Applied this run**:
  - Fixed: 10
  - Failed: 1 (F007, F015)
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

> Note: `F015` is counted under Failed but its "failure" is an intentional-scope refusal — the finding's own suggested fix specifies a follow-up PR. `F007` is a genuine technical failure: the suggested fix names a feature (`zvariant/serde`) that does not exist in v5.

## Incident — Lost-stash recovery

Mid-run, an F006 sub-agent executed `git stash` to isolate an unrelated compile error it observed while verifying its ADR-only edit. It then failed to restore the stash cleanly, reverting `launch.rs` to HEAD and wiping parts of the prior HIGH-fix working tree.

- The lost stash was recovered from `git fsck --lost-found` as dangling commit `bba7670`.
- Preserved as ref `refs/stash-recovered/pr-280-wip`.
- Working tree fully restored via `git reset HEAD && git checkout -- . && git stash apply refs/stash-recovered/pr-280-wip`.
- All Batch 1 sub-agents were re-dispatched with a hard `NO git state-changing commands` directive.

## Fixes Applied

### HIGH pass (prior run, 2026-04-17)

| ID   | Severity | File                                                                               | Line   | Status | Notes                                                                                                                                                                                                        |
| ---- | -------- | ---------------------------------------------------------------------------------- | ------ | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| F001 | HIGH     | `crosshook-core/src/platform/portals/background.rs`                                | 53–93  | Fixed  | `REQUEST_STATE: AtomicU8` state machine (IDLE→IN_FLIGHT→SUCCEEDED; resets IDLE on failure); new `BackgroundError::AlreadyRequested`.                                                                         |
| F002 | HIGH     | `crosshook-core/src/platform/portals/background.rs`                                | 80     | Fixed  | `zbus::Proxy` on Request path + `receive_signal("Response")` awaited with `PORTAL_RESPONSE_TIMEOUT = 60s`; `parse_response_payload` now called at the runtime path; success log moved after grant confirmed. |
| F003 | HIGH     | `src-tauri/src/background_portal.rs`                                               | 45–52  | Fixed  | `static_assertions = "1"` dep + `assert_impl_all!(BackgroundGrantHolder: Send, Sync)` documenting the zbus ≥ 5 invariant.                                                                                    |
| F004 | HIGH     | `src-tauri/src/background_portal.rs`                                               | 175    | Fixed  | `std::pin::pin!` + `notified.as_mut().enable()` before the double-check closes the Notify race window.                                                                                                       |
| F005 | HIGH     | `crosshook-core/src/platform/portals/gamemode.rs` + `src-tauri/commands/launch.rs` | 85,126 | Fixed  | New `probe_and_register_via_portal()` folds introspection + registration onto a single `zbus::Connection::session()`; launch-site updated.                                                                   |

### MEDIUM pass (this run, 2026-04-17)

| ID   | Severity | File                                                                                            | Line    | Status | Notes                                                                                                                                                                                                                                                                                                                                                                |
| ---- | -------- | ----------------------------------------------------------------------------------------------- | ------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F006 | MEDIUM   | `docs/architecture/adr-0002-flatpak-portal-contracts.md`                                        | 172–181 | Fixed  | Added blockquote note after the closing fence of the `BackgroundError` snippet: "Implementation uses hand-written `fmt::Display` + `std::error::Error` impls to avoid the `thiserror` dependency; the `#[derive(thiserror::Error)]` form above is illustrative only."                                                                                                |
| F007 | MEDIUM   | `crosshook-core/Cargo.toml`                                                                     | 38      | Failed | `zvariant v5` has no `serde` feature — serde support is unconditional in v5. Suggested fix names a feature that does not exist. See Failed Fixes § for full blocker.                                                                                                                                                                                                 |
| F008 | MEDIUM   | `crosshook-core/src/platform/portals/background.rs`                                             | 83–87   | Fixed  | Resolved by F002's log reorganization. The intermediate `request_path` log is now `tracing::debug!` (line 175); the post-response success log (line 205) no longer carries the raw path. Verified in situ.                                                                                                                                                           |
| F009 | MEDIUM   | `crosshook-core/src/platform/portals/background.rs`                                             | 106     | Fixed  | Resolved by F002's Drop implementation. `Drop for BackgroundGrant` (line 254–260) logs `request_path` at `tracing::debug!` matching the `GameModeRegistration::drop` pattern. Verified in situ.                                                                                                                                                                      |
| F010 | MEDIUM   | `crosshook-core/src/platform/portals/background.rs`                                             | 198–215 | Fixed  | Resolved by F002's runtime wiring. `parse_response_payload` is called at `background.rs:203` in the runtime path. Option (a) of the suggested fix is satisfied; `pub(crate)` downgrade (option b) unnecessary.                                                                                                                                                       |
| F011 | MEDIUM   | `src-tauri/src/background_portal.rs` + `docs/architecture/adr-0002-flatpak-portal-contracts.md` | 196–201 | Fixed  | Added `// TODO(frontend): wire get_background_protection_state …` comment above the `#[tauri::command]` site, plus a `> **UI integration deferred.**` blockquote under § Capability integration in ADR-0002.                                                                                                                                                         |
| F012 | MEDIUM   | `src-tauri/src/background_portal.rs`                                                            | 287–309 | Fixed  | Added `pending_holder_transitions_to_degraded_after_portal_denied` test: spawns a `wait_for_initialization` waiter, sleeps to register the `Notify` subscription, then calls `store_result(Err(PortalDenied))` and asserts `protection_state() == Degraded`. Exercises the F004 race window. `new_pending()` constructor was already present in the HIGH-pass stash. |
| F013 | MEDIUM   | `crosshook-core/src/platform/portals/gamemode.rs`                                               | 64–76   | Fixed  | `resolve_backend` parameters already renamed to `is_in_flatpak: bool` / `portal_is_available: bool` in the restored HIGH-pass stash (doc truth-table headers + signature + body condition). No additional edit needed.                                                                                                                                               |

### LOW pass (this run, 2026-04-17)

| ID   | Severity | File                                         | Line | Status | Notes                                                                                                                                                                                                                                |
| ---- | -------- | -------------------------------------------- | ---- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| F014 | LOW      | `src-tauri/src/commands/launch.rs`           | 394  | Fixed  | Added `// clone before app is moved into spawn_log_stream` one-liner above the `watchdog_app_handle` let binding. Rename variant skipped to avoid churn.                                                                             |
| F015 | LOW      | `src-tauri/src/commands/launch.rs`           | 1266 | Failed | Finding's own suggested fix specifies "in a follow-up PR" and states "Not a blocker for this PR". Intentional-scope refusal; track as a follow-up issue.                                                                             |
| F016 | LOW      | `crosshook-core/src/platform/portals/mod.rs` | 16   | Fixed  | Replaced misleading "trait seam / `#[cfg(test)]` fakes" sentence with accurate language: "Pure decision helpers (`resolve_backend`, `background_supported`) are unit-testable; the D-Bus entry points … require a live session bus." |

## Files Changed (this run, cumulative with HIGH pass)

- `docs/architecture/adr-0002-flatpak-portal-contracts.md` — Fixed F006 (BackgroundError impl note) + Fixed F011 (UI integration deferred blockquote)
- `src/crosshook-native/Cargo.lock` — transitive from HIGH-pass dependency additions (unchanged this run)
- `src/crosshook-native/crates/crosshook-core/src/platform/portals/background.rs` — HIGH pass Fixed F001, F002; this run re-exercised F008, F009, F010 against F002's wiring (no additional edit)
- `src/crosshook-native/crates/crosshook-core/src/platform/portals/gamemode.rs` — HIGH pass Fixed F005; contains F013 rename already
- `src/crosshook-native/crates/crosshook-core/src/platform/portals/mod.rs` — Fixed F016
- `src/crosshook-native/src-tauri/Cargo.toml` — HIGH pass added `static_assertions = "1"` for F003
- `src/crosshook-native/src-tauri/src/background_portal.rs` — HIGH pass Fixed F003, F004; this run Fixed F011 (TODO comment) and F012 (new test)
- `src/crosshook-native/src-tauri/src/commands/launch.rs` — HIGH pass Fixed F005 launch-site; this run Fixed F014 (clone comment)

## Failed Fixes

### F007 — `src/crosshook-native/crates/crosshook-core/Cargo.toml:38`

**Severity**: MEDIUM
**Category**: Pattern Compliance
**Description**: `zvariant = { version = "5", default-features = false }` disables default features without naming non-default features explicitly.
**Suggested fix (from review)**: `zvariant = { version = "5", default-features = false, features = ["serde"] }`.
**Blocker**: `cargo check` → `package 'crosshook-core' depends on 'zvariant' with feature 'serde' but 'zvariant' does not have that feature. A required dependency with that name exists, but only optional dependencies can be used as features.`
**Root cause**: In `zvariant` v5 (locked to 5.10.0), serde support is unconditional — it is compiled in by default and is NOT gated behind a `serde` feature flag. The crate's `[features]` table exposes only `camino`, `gvariant`, `option-as-array`, and `ostree-tests`.
**Recommendation**: The existing declaration is already correct. If clarity is the goal, add an inline comment instead:
`zvariant = { version = "5", default-features = false }  # serde support is unconditional in v5`
Update the review finding's Suggested fix accordingly (or close F007 as a documentation-intent issue, not a Cargo.toml change).

### F015 — `src/crosshook-native/src-tauri/src/commands/launch.rs:1266`

**Severity**: LOW
**Category**: Maintainability
**Description**: `finalize_launch_stream` is ~197 lines; exceeds the 50-line guideline.
**Suggested fix (from review)**: Extract version-snapshot and known-good-tagging blocks into helpers **"in a follow-up PR"**.
**Blocker**: Intentional scope refusal. The finding itself states "Not a blocker for this PR" and explicitly directs the refactor to a follow-up PR.
**Root cause**: The finding is flagging, not demanding immediate fix.
**Recommendation**: File a follow-up issue to track `record_version_snapshot` and `tag_known_good_revision` extraction. Do not land the refactor in this PR.

## Validation Results

| Check                                                                                                      | Result                                                     |
| ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| Type check (`cargo check --manifest-path src/crosshook-native/Cargo.toml --workspace`)                     | Pass                                                       |
| Tests (`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`)                     | Pass (994 main + 1 + 3 + 1 + 4 + 4 sub-binary; 0 failures) |
| Tests (`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native background_portal`) | Pass (7 background_portal tests including F012's new test) |
| Host-gateway (`./scripts/check-host-gateway.sh`)                                                           | Pass — no direct host-tool bypasses                        |

## Next Steps

- Re-run `/ycc:code-review 280` to confirm the MEDIUM/LOW findings are resolved against the final state. Particular callouts to double-check during re-review: F007 (close as "suggested fix infeasible — zvariant v5 has no serde feature") and F015 (should be de-scoped to a follow-up issue, not re-flagged).
- File a tracking issue for the F015 `finalize_launch_stream` refactor (extract `record_version_snapshot` and `tag_known_good_revision`).
- File a tracking issue for the F011 frontend wiring (`invoke("get_background_protection_state")` + dashboard row).
- Run `/ycc:git-workflow` to commit the fixes when satisfied. The restored stash at `refs/stash-recovered/pr-280-wip` can be deleted (`git update-ref -d refs/stash-recovered/pr-280-wip`) after the working-tree changes land in a commit.
