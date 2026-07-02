# Code Review — PR #24: fix(launch): restore launch status reset for game and trainer sessions

- **Mode**: PR review (`/ycc:code-review --parallel`)
- **PR**: https://git.home.rfamily.dev/yandy/crosshook/pulls/24 (Closes #22)
- **Branch**: `fix/launch-status-reset`
- **Reviewers**: correctness-reviewer, security-reviewer, quality-reviewer (3 parallel sub-agents, findings merged and de-duplicated)
- **Validation**: `cargo test -p crosshook-core` (full), `cargo test -p crosshook-native` (full), `cargo clippy --all-targets` (zero warnings), `cargo fmt --check`, `./scripts/check-host-gateway.sh`, `npx vitest run` (full), `npm run typecheck`
- **Decision**: APPROVE (after fixes below applied on-branch)

## Findings

### PR24-S1 — Anonymous-sentinel key collision reachable from the frontend

- **Severity**: major (Security / session-ownership invariant)
- **Files**: `crates/crosshook-core/src/profile/toml_store/utils.rs`, `crates/crosshook-core/src/profile/legacy.rs`
- **Status**: Fixed
- **Issue**: A profile could legally be named `__crosshook_anonymous_profile__`, colliding with the session registry's anonymous key. `launch_reset_sessions` made that collision user-invokable: resetting such a profile would cancel every anonymous session, violating "never kill another session's process tree."
- **Fix applied**: The sentinel name is now rejected by profile-name validation on **both** validation paths — `toml_store/utils.rs` (the live path used by `ProfileStore::save`/`rename`/`duplicate`; the reviewer's cited `legacy.rs` path has no live callers) and `legacy.rs` (public API surface, defense in depth). Tests added on both.

### PR24-C2 — Register-after-spawn race lets Reset miss a live process; stale launch promise resurrects state

- **Severity**: major (Correctness)
- **Files**: `src-tauri/src/commands/launch/execution.rs`, `src/hooks/useLaunchState.ts`
- **Status**: Fixed
- **Issue**: (a) `launch_game` registered the session after `command.spawn()` and the launch-history write, so a Reset in that window found nothing to cancel; (b) the in-flight `launch_game` promise then resolved and unconditionally dispatched `game-success`, silently undoing the user's Reset.
- **Fix applied**: (a) session registration now precedes spawn, with deregistration on spawn failure (`launch_trainer` already had the correct ordering); (b) `useLaunchState` gained a launch-generation counter bumped by `reset()` — in-flight `launchGame`/`launchTrainer` resolutions detect a stale generation and skip all state dispatches. Regression test: reset during a pending launch keeps phase `Idle` when the launch later resolves.

### PR24-C1 — Reset does not terminate the game process for non-gamescope launches

- **Severity**: major (Completeness / UX honesty)
- **File**: `src/components/library/launch/HeroLaunchCommandSection.tsx`
- **Status**: Fixed (scope made explicit; behavior intentionally unchanged)
- **Issue**: Cancelling a session tears down gamescope watchdogs but does not kill the game/trainer process for native or non-gamescope launches; the button copy ("Reset launch status…") implied more.
- **Resolution**: Killing the process on Reset would be wrong UX — the primary recovery scenario is "trainer crashed, game still running, re-arm the trainer without closing the game," and the pre-Hero reset never killed processes either. The button title now reads "Reset launch tracking for game and trainer — does not close a running game," and the component documents the contract.

### PR24-Q1 — Mutating command misfiled in the read-only queries module

- **Severity**: major (Pattern compliance)
- **Files**: `src-tauri/src/commands/launch/{queries.rs,execution.rs,mod.rs}`, `src-tauri/src/lib.rs`
- **Status**: Fixed
- **Issue**: `launch_reset_sessions` mutates registry state but lived in `queries.rs`, whose doc contract is "lightweight read-only launch commands."
- **Fix applied**: Command moved to `execution.rs` (session-lifecycle home); `mod.rs` inventory doc and `lib.rs` invoke-handler grouping updated.

### PR24-Q2 — Profile-key derivation belonged in crosshook-core

- **Severity**: minor (Architecture)
- **Files**: `crates/crosshook-core/src/launch/session/keys.rs` (new), `src-tauri/src/commands/launch/execution.rs`
- **Status**: Fixed
- **Issue**: The registry-key rule (trim + anonymous fallback) lived in the Tauri layer as a `pub(super)` helper reached into from a sibling module, despite being a core-domain invariant.
- **Fix applied**: `ANONYMOUS_PROFILE_KEY` and `session_profile_key_for_name` moved to `crosshook_core::launch::session` with unit tests; src-tauri keeps only a thin request wrapper. The redundant src-tauri `tests/session_key.rs` was removed.

### PR24-C3 — Doc comment cited the wrong linking helper

- **Severity**: minor (Documentation)
- **File**: `crates/crosshook-core/src/launch/session/registry.rs`
- **Status**: Fixed — doc now cites both `link_to_parent` and the production `register_and_link_to_parent_of_kind`.

### PR24-Q3 — registry.rs drifted past the ~500-line soft cap

- **Severity**: minor (Maintainability)
- **File**: `crates/crosshook-core/src/launch/session/registry.rs`
- **Status**: Fixed — inline tests extracted to `registry_tests.rs` via `#[path]`; production file now 254 lines.

### PR24-S2 — Whitespace-variant profile names sharing a session key

- **Severity**: minor (Security, pre-existing)
- **Status**: No change needed (moot)
- **Evidence**: `ProfileStore::save` builds the on-disk path from the **trimmed** validated name, so whitespace variants overwrite the same `Foo.toml` — two distinct saved profiles differing only by whitespace cannot exist. (Adjacent observation for a possible follow-up: `write_mangohud_conf` receives the raw untrimmed name, a cosmetic orphan-file risk only.)

## Post-fix validation

- `cargo test -p crosshook-core` → 1338 passed + all integration binaries
- `cargo test -p crosshook-native` → 58 passed
- `cargo clippy -p crosshook-core -p crosshook-native --all-targets` → zero warnings; `cargo fmt --check` clean
- `./scripts/check-host-gateway.sh` → pass
- `npx vitest run` → 500/500; `npm run typecheck` → clean
