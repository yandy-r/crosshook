# Plan: Profile schema — pre/post launch hooks (`LaunchHook`, `HookStage`) (Phase 3)

## Summary

Add `LaunchHook { id, name, path, stage, enabled }` and `HookStage::PreLaunch | PostExit` to `crosshook-core/src/profile/models/` (new `hooks.rs`), plus two top-level `GameProfile` fields — `pre_launch_hooks: Vec<LaunchHook>` and `post_exit_hooks: Vec<LaunchHook>` — with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`. Round-trip TOML tests lock the contract. ts-rs exports `LaunchHook`/`HookStage` to `src/crosshook-native/src/types/generated/launch_hooks.ts` and the hand-written `GameProfile` TS interface gains the matching optional arrays. Community-exchange export strips hooks and import force-disables them (security hardening, fail-open denylist gap). **No launcher consumption** — a `// TODO(hooks-runtime): …` comment plus a tracked follow-up issue defer runtime execution.

## User Story

As a profile maintainer, I want hook entries to serialize in profile.toml without breaking older builds, so the Phase 6 UI (`HookListPanel`, issue #471) has a stable, frozen data contract.

## Problem → Solution

**Current state**: `GameProfile` (`profile/models/profile.rs:9-25`) has no hook concept. No `LaunchHook`, `HookStage`, `pre_launch_hooks`, or `post_exit_hooks` exists anywhere in the workspace (grep-confirmed). Phase 6 (#471) is blocked on this schema. The community-exchange sanitizer (`profile/exchange/utils.rs:20-36`) is a fail-open denylist — any new path-bearing field leaks into exports unless explicitly cleared.

**Desired state**: Profile TOML round-trips two new optional arrays with zero migration burden. Old profiles deserialize with empty vecs; new profiles with empty vecs serialize byte-identically to old ones. `LaunchHook`/`HookStage` are exported as TS types and reachable from the frontend barrel. Community export strips hooks; community import lands hooks disabled. The launcher remains untouched except for TODO breadcrumbs.

## Metadata

- **Complexity**: Medium (1 new Rust module, ~10 ripple sites, exchange hardening, ts-rs wiring, frontend type mirror)
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md` (Phase 3: lines 317–327; schema block: lines 186–202; persistence: lines 233–247; open question: line 75)
- **PRD Phase**: Phase 3 — Pre/post hook schema (crosshook-core additive fields)
- **GitHub Issue**: [#468](https://github.com/yandy-r/crosshook/issues/468) (tracker: #478; downstream consumer: #471 Phase 6)
- **Estimated Files**: 5 new + ~12 edited (≈8 of those are mechanical struct-literal fixes the compiler enumerates)
- **Scope boundary**: Schema + tests + TS export + exchange hardening ONLY. No launcher execution, no UI, no SQLite/`migrations.rs` change, no IPC signature change (`profile_save`/`profile_load` piggyback `GameProfile` serde).

## Resolved Decisions (this plan freezes the Phase 6 contract)

These six items are the frozen interface #471 builds against. Changing any of them after merge forces a Phase 6 rework.

1. **Schema shape** (answers the issue's "Open question gating"): **single `LaunchHook` struct + `HookStage` enum** — unanimous across research. Two split structs would break the `HookListPanel` prop contract (`{ hooks: LaunchHook[]; stage: HookStage }`, PRD line 368) and double the serde/ts-rs/test surface.
2. **Field home** (user-confirmed): **top-level on `GameProfile`**, after `local_override` (last position). TOML shape is `[[pre_launch_hooks]]` / `[[post_exit_hooks]]`, matching the PRD AC text and Phase 6's `updateProfile({ ...profile, pre_launch_hooks, post_exit_hooks })` spread. Cost accepted: every exhaustive `GameProfile { ... }` literal must be fixed (compiler enumerates them; see Task 2.1).
3. **Derives** (overrides the PRD snippet, which would not compile): `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]` on both types, plus `Copy` on `HookStage`. `GameProfile` derives `PartialEq, Eq` (`profile.rs:9`), so the new types must too, and the round-trip test's `assert_eq!(parsed, profile)` requires it.
4. **Wire format**: `HookStage` serializes kebab-case — `"pre-launch"` / `"post-exit"` — via `#[serde(rename_all = "kebab-case")]` (PRD line 198; precedent: `MangoHudPosition`, `models/mangohud.rs:3-5`). `#[default] PreLaunch`.
5. **Tolerant deserialization**: struct-level `#[serde(default)]` on `LaunchHook` (per PRD snippet line 188 and the PRD's degraded-fallback contract: a malformed hook renders as an "Invalid hook" row in Phase 6 rather than failing the whole profile load). A hand-edited hook table missing `id`/`name`/`path` deserializes with empty strings — it does NOT poison the profile. Matches the codebase-wide tolerant-default convention (e.g. `trainer.rs:43-44`).
6. **`id` ownership**: frontend-minted at attach time (Phase 6) via the guarded `crypto.randomUUID()` pattern (`CollectionLaunchDefaultsEditor.tsx:21-27` precedent — covers non-secure `http://` browser dev mode). Backend treats `id` as an opaque persisted string and never mints it. Documented in the `LaunchHook` doc comment.

**Stage invariant** (documented, not type-enforced): every element of `pre_launch_hooks` must carry `stage: PreLaunch` and every element of `post_exit_hooks` must carry `stage: PostExit`. The **containing vec is authoritative**; `stage` mirrors it so a Phase 6 row can render its pill without knowing its parent array. Producers (Phase 6 UI) are responsible for keeping them aligned. Load-time normalization is deferred to the runtime follow-up, where the discrepancy would actually matter.

---

## Storage Boundary & Persistence

Per CLAUDE.md persistence-planning rules:

| Datum                                     | Classification                                 | Notes                                                                                                                                                                       |
| ----------------------------------------- | ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pre_launch_hooks: Vec<LaunchHook>`       | **TOML settings** (per-profile, user-editable) | `profile.toml` via `toml_store`; serialized as `[[pre_launch_hooks]]` array-of-tables                                                                                       |
| `post_exit_hooks: Vec<LaunchHook>`        | **TOML settings** (per-profile, user-editable) | Same                                                                                                                                                                        |
| `LaunchHook.{id,name,path,stage,enabled}` | **TOML settings**                              | `path` is a host-side absolute path (normalized via `normalize_flatpak_host_path` at future execution; ADR-0001 scope boundary — user variable, not a denylisted tool name) |
| SQLite metadata                           | **None**                                       | `migrations.rs` untouched; schema stays v23                                                                                                                                 |
| Runtime-only state                        | **None introduced**                            | UI/session state is Phases 5–7                                                                                                                                              |

- **Migration / backward compatibility**: additive only — no migration. Old profiles load with empty vecs (`#[serde(default)]`); new-but-empty profiles serialize byte-identically to old ones (`skip_serializing_if = "Vec::is_empty"`); old builds reading new profiles skip the unknown keys (no `deny_unknown_fields` anywhere in the `GameProfile` tree — confirmed).
- **Offline expectations**: fully offline; local `profile.toml` only.
- **Degraded fallback**: malformed hook tables deserialize with defaulted fields (tolerant struct-level `#[serde(default)]`); Phase 6 renders "Invalid hook" rows for empty-`id` entries. A profile never fails to load because of a bad hook entry.
- **User visibility / editability**: fields are user-editable per profile (hand-edit TOML today; Hero Detail Launch tab in Phase 6). Phase 3 ships **no** user-facing surface. Runtime execution being deferred is surfaced in-product in Phase 6 (banner), not here.

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2      | —          | 2              |
| B2    | 2.1           | B1         | 1              |
| B3    | 3.1, 3.2, 3.3 | B2         | 3              |
| B4    | 4.1           | B3         | 1              |

- **Total tasks**: 7
- **Total batches**: 4
- **Max parallel width**: 3

Same-file collision check: B1 splits Rust module creation (1.1: `hooks.rs`, `models/mod.rs`, `profile/mod.rs`) vs. GitHub issue creation (1.2: no repo files). B3 splits model tests (3.1: `models/tests/hooks.rs`, `models/tests/mod.rs`) vs. exchange hardening (3.2: `exchange/utils.rs`, exchange tests) vs. ts-rs wiring (3.3: `ts_rs_exports.rs`, `types/generated/launch_hooks.ts`). No two tasks in a batch touch the same file.

---

## UX Design

No UI change. This phase is backend schema + type exports only. The Phase 6 `HookListPanel` (issue #471) is the consumer.

---

## Mandatory Reading

Files that MUST be read before implementing. All paths relative to repo root; Rust paths relative to `src/crosshook-native/crates/crosshook-core/src/` unless noted.

| Priority | File                                                                        | Lines                     | Why                                                                                                                      |
| -------- | --------------------------------------------------------------------------- | ------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| P0       | `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`           | 186–202, 233–247, 317–327 | Schema block, persistence contract, Phase 3 scope                                                                        |
| P0       | `profile/models/profile.rs`                                                 | all (208 lines)           | The struct gaining two fields; derive set; `effective_profile_with`/`storage_profile` field-merge warning at lines 71–76 |
| P0       | `profile/models/mod.rs`                                                     | all                       | Module declaration + re-export pattern to extend                                                                         |
| P0       | `profile/models/trainer.rs`                                                 | 4–19, 43–55               | `#[default]` enum precedent (`TrainerLoadingMode`); `Vec` + `skip_serializing_if` field precedent                        |
| P0       | `profile/models/mangohud.rs`                                                | 3–14                      | kebab-case enum precedent (`MangoHudPosition`)                                                                           |
| P0       | `profile/models/tests/launch_section.rs`                                    | 21–116                    | Round-trip + empty-omission test idioms to mirror exactly                                                                |
| P0       | `profile/exchange/utils.rs`                                                 | 20–80                     | `sanitize_profile_for_community_export` denylist + `hydrate_imported_profile` — both gain hook handling                  |
| P1       | `profile/models/legacy.rs`                                                  | 37–78                     | Exhaustive `GameProfile { ... }` literal that WILL break compilation — primary ripple site                               |
| P1       | `ts_rs_exports.rs`                                                          | all                       | Manual export registry — types are a silent no-op unless registered here                                                 |
| P1       | `profile/health/types.rs`                                                   | 1–11                      | ts-rs `cfg_attr` gating pattern to copy verbatim                                                                         |
| P1       | `profile/models/tests/fixtures.rs` + `profile/toml_store/tests/fixtures.rs` | all                       | Exhaustive fixture literals to extend                                                                                    |
| P1       | `src/crosshook-native/src/types/profile.ts` (frontend)                      | 93–172, 215–286, 343–348  | Hand-written `GameProfile` interface, `DEFAULT_*`, normalizer, the "use `?`, never `\| null`" convention                 |
| P2       | `src/crosshook-native/src/types/onboarding.ts` (frontend)                   | 1–12                      | The funnel pattern for re-exporting generated types through a hand-written module                                        |
| P2       | `docs/prps/specs/ts-rs-evaluation-spec.md`                                  | 13–21                     | Regeneration command + known ts-rs wrinkles                                                                              |
| P2       | `docs/architecture/adr-0001-platform-host-gateway.md`                       | scope-boundary section    | Why hook `path` is a user variable, not a gateway concern — wording for the doc comment                                  |

## External Documentation

- ts-rs 10.x: <https://docs.rs/ts-rs/10> — `#[ts(export, export_to = "...")]`, `serde-compat` honors `rename_all`. The repo's own evaluation spec (`docs/prps/specs/ts-rs-evaluation-spec.md`) is authoritative for local wiring.
- serde field attributes: <https://serde.rs/field-attrs.html> — `default`, `skip_serializing_if` semantics.

## Patterns to Mirror

**Enum with `#[default]` variant** (`profile/models/trainer.rs:4-10`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrainerLoadingMode {
    #[default]
    SourceDirectory,
    CopyToPrefix,
}
```

**kebab-case enum serde** (`profile/models/mangohud.rs:3-5`): `#[serde(rename_all = "kebab-case")]` on `MangoHudPosition` → `"top-left"` etc.

**Vec field attribute** (`profile/models/trainer.rs:54-55`) — the exact attribute the issue mandates:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub required_protontricks: Vec<String>,
```

**ts-rs gating** (`profile/health/types.rs:1-11`):

```rust
#[cfg(feature = "ts-rs")]
use ts_rs::TS;
// ...
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/health.ts"))]
```

**Empty-omission round-trip test** (`profile/models/tests/launch_section.rs:88-96`):

```rust
#[test]
fn custom_env_vars_empty_omitted_from_toml_and_roundtrips() {
    let profile = sample_profile();
    let serialized = toml::to_string_pretty(&profile).expect("serialize");
    assert!(!serialized.contains("custom_env_vars"), "expected empty map skipped: {serialized}");
    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert!(parsed.launch.custom_env_vars.is_empty());
}
```

**Generated-type funnel re-export** (`src/types/onboarding.ts:1-12`): `export type { X } from './generated/onboarding';`, then `index.ts` re-exports the hand-written module.

**Lint gotchas to honor** (workspace lints, `src/crosshook-native/Cargo.toml:12-31`, all `-D warnings` in CI with `--all-targets`): inline format args (`format!("{x}")`), no `use HookStage::*;` (`enum_glob_use = "deny"`), `copied()` over `cloned()` for `Copy` types.

## Files to Change

| File                                                                            | Change                                                                     |
| ------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/models/hooks.rs`                             | **NEW** — `LaunchHook`, `HookStage`                                        |
| `crates/crosshook-core/src/profile/models/mod.rs`                               | `mod hooks;` + `pub use hooks::{HookStage, LaunchHook};`                   |
| `crates/crosshook-core/src/profile/mod.rs`                                      | Add `HookStage, LaunchHook` to the `pub use models::{...}` list            |
| `crates/crosshook-core/src/profile/models/profile.rs`                           | Two fields (last position) + import + TODO comment                         |
| `crates/crosshook-core/src/launch/script_runner/proton_game.rs`                 | TODO(hooks-runtime) breadcrumb only (no logic)                             |
| `crates/crosshook-core/src/profile/models/legacy.rs`                            | Fix exhaustive `GameProfile` literal (lines 41–77)                         |
| ~8 fixture/test/production files with exhaustive `GameProfile { ... }` literals | Mechanical: append fields or `..Default::default()` (compiler enumerates)  |
| `crates/crosshook-core/src/profile/models/tests/mod.rs`                         | `mod hooks;`                                                               |
| `crates/crosshook-core/src/profile/models/tests/hooks.rs`                       | **NEW** — round-trip + compat tests                                        |
| `crates/crosshook-core/src/profile/exchange/utils.rs`                           | Export strips hooks; import force-disables hooks                           |
| exchange test file(s)                                                           | **NEW tests** — export-strip + import-disable assertions                   |
| `crates/crosshook-core/src/ts_rs_exports.rs`                                    | Register `HookStage::export()` + `LaunchHook::export()`                    |
| `src/crosshook-native/src/types/generated/launch_hooks.ts`                      | **GENERATED** — commit after running exporter + format                     |
| `src/crosshook-native/src/types/profile.ts`                                     | Optional fields on `GameProfile` interface + normalizer + funnel re-export |

## NOT Building

- **No launcher consumption** — `launch/` gains only a TODO comment. `grep -rn "pre_launch_hooks\|post_exit_hooks" crates/crosshook-core/src/launch/` must return only the TODO line.
- **No `LaunchRequest` plumbing** — the launcher consumes `LaunchRequest`, not `GameProfile`; threading hooks into `launch/request/models.rs` is the runtime follow-up's job (named in the TODO).
- **No UI** — `HookListPanel` is #471 (Phase 6).
- **No SQLite change** — `metadata/migrations.rs` untouched.
- **No IPC change** — `profile_save`/`profile_load` (`src-tauri/src/commands/profile/lifecycle.rs:38,100`) pass `GameProfile` by serde; fields piggyback.
- **No `CollectionDefaultsSection` hook overrides** — collection-level hook defaults are out of scope (consistent with no-runtime).
- **No raw-TOML-share sanitization** — `profile_export_toml` stays verbatim; the consent/strip UX for that path belongs with the runtime follow-up (documented in the follow-up issue, Task 1.2).
- **No vec size caps or load-time stage normalization** — no codebase precedent; deferred to the runtime follow-up where they matter.

---

## Step-by-Step Tasks

### Task 1.1: Create `models/hooks.rs` + module wiring — Depends on [none]

**Files**: `crates/crosshook-core/src/profile/models/hooks.rs` (new), `crates/crosshook-core/src/profile/models/mod.rs`, `crates/crosshook-core/src/profile/mod.rs`

Create `hooks.rs`:

```rust
use serde::{Deserialize, Serialize};

#[cfg(feature = "ts-rs")]
use ts_rs::TS;

/// When a launch hook fires relative to the game lifecycle.
///
/// Serializes kebab-case: `"pre-launch"` / `"post-exit"` — this exact wire
/// format is the Phase 6 stage-pill contract (issue #471).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/launch_hooks.ts"))]
pub enum HookStage {
    /// Runs before the launch command is executed.
    #[default]
    PreLaunch,
    /// Runs after the game process exits.
    PostExit,
}

/// A user-declared script invoked around the launch lifecycle.
///
/// Declared, not yet executed — runtime execution is tracked in the
/// hooks-runtime follow-up issue (see TODO(hooks-runtime) markers).
///
/// - `id` is an opaque client-minted identifier (frontend `crypto.randomUUID()`
///   at attach time); the backend never mints or interprets it.
/// - `path` is a host-side absolute path. Per ADR-0001's scope boundary it is a
///   user variable (not a denylisted tool name); future execution must apply
///   `normalize_flatpak_host_path` and route through the host gateway.
/// - `stage` mirrors the containing profile vec (`pre_launch_hooks` /
///   `post_exit_hooks`), which is authoritative; producers keep them aligned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "generated/launch_hooks.ts"))]
pub struct LaunchHook {
    pub id: String,
    pub name: String,
    pub path: String,
    pub stage: HookStage,
    pub enabled: bool,
}
```

Wire `models/mod.rs`: add `mod hooks;` (alphabetical, between `gamescope` and `launch`) and `pub use hooks::{HookStage, LaunchHook};` (alphabetical in the re-export block at lines 17–29). Wire `profile/mod.rs`: add `HookStage, LaunchHook` to the `pub use models::{...}` list (lines 30–37) so they're reachable as `crosshook_core::profile::LaunchHook`.

**Validation**: `cargo build --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` and `cargo build … -p crosshook-core --features ts-rs` both compile.

### Task 1.2: Create the hooks-runtime follow-up GitHub issue — Depends on [none]

The follow-up issue does **not** exist (verified — only #471 and #478 match hook searches). The PRD mandates it (lines 202, 325: "Add a tracked follow-up issue") and the Phase 6 banner (#471) needs the same issue number.

Create via `gh` using the repo's YAML form templates under `.github/ISSUE_TEMPLATE/` (or GitHub API mirroring the form fields if `--template` fails — never title-only):

- **Title**: `Runtime execution of pre/post launch hooks (consume LaunchHook in launcher)`
- **Body must include**: scope (execute `pre_launch_hooks` before spawn, `post_exit_hooks` on session teardown via `LaunchSessionRegistry`/watchdog); `LaunchRequest` plumbing gap (`launch/request/models.rs` carries no hooks); ADR-0001 host-gateway + `normalize_flatpak_host_path` + `host_command_with_env` requirements; raw-TOML-share consent/strip UX; whether imported hooks may ever auto-enable; timeout/failure/ordering semantics; use-time path health check (mirror `health/checks.rs:178` `check_required_executable`); a **Storage boundary** subsection (consumes Phase 3 TOML fields; persists nothing new) and **Persistence & usability** subsection per CLAUDE.md.
- **Labels** (existing taxonomy only): `type:feature`, `area:launch`, `priority:medium`, `feat:hero-detail-consolidation`.
- Reference: `Part of #478`, `Depends on #468`.

**Output**: record the new issue number — Task 2.1's TODO comments reference it.

**Validation**: `gh issue view <new-number>` shows the body with both persistence subsections.

### Task 2.1: Add fields to `GameProfile` + fix ripple sites + TODO breadcrumbs — Depends on [1.1, 1.2]

**Files**: `crates/crosshook-core/src/profile/models/profile.rs`, `crates/crosshook-core/src/profile/models/legacy.rs`, `crates/crosshook-core/src/launch/script_runner/proton_game.rs`, plus every exhaustive `GameProfile { ... }` literal the compiler flags.

1. In `profile.rs`: add `use super::hooks::LaunchHook;` and append to `GameProfile` (LAST position, after `local_override`):

```rust
    // TODO(hooks-runtime): consume in launcher — see issue #<N from Task 1.2>.
    // Declared-only in Phase 3 (#468); the containing vec is authoritative for
    // stage. Keep these fields last; new scalar fields must go before them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_launch_hooks: Vec<LaunchHook>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_exit_hooks: Vec<LaunchHook>,
```

Note: toml 1.1 (the pinned version) auto-orders scalars before tables, so position is a readability choice, not a correctness requirement — but keep them last anyway and keep the comment.

2. Also note in the `effective_profile_with`/`storage_profile` doc area (`profile.rs:71-76` warns new fields must be audited): hooks are carried by `self.clone()`/`effective.clone()` untouched — **no merge handling needed**; they are not part of `local_override` or `CollectionDefaultsSection`. Add one line to that comment block recording the audit.

3. Fix exhaustive `GameProfile { ... }` literals. Run `cargo build -p crosshook-core` (and `cargo test -p crosshook-core --no-run`; also build `src-tauri` tests) and fix every error by appending the two fields as `Vec::new()` (or `..Default::default()` where stylistically consistent). Known/likely sites (the compiler is the authoritative list):
   - `profile/models/legacy.rs:41-77` (production — `From<LegacyProfileData>`; legacy profiles get empty vecs)
   - `profile/models/tests/fixtures.rs:6-20`, `profile/toml_store/tests/fixtures.rs:4-47`
   - `profile/health/tests/fixtures.rs:37, 77`, `profile/exchange/mod.rs:27`, `install/models.rs:193`
   - `metadata/profile_sync.rs:348, 372`, `metadata/test_support.rs:16`
   - `crates/crosshook-core/tests/collections_jtbd_integration.rs:17`
   - `src-tauri/src/commands/profile/tests/helpers.rs:7`, `…/lifecycle.rs:61`, `…/optimizations.rs:74`

4. Launcher breadcrumb in `launch/script_runner/proton_game.rs` (`build_proton_game_command`, before the final `Ok(command)` around lines 140–142) — comment only, no logic:

```rust
    // TODO(hooks-runtime): consume in launcher — run profile.pre_launch_hooks here
    // (before spawn) and register profile.post_exit_hooks with session teardown
    // (launch/session/, launch/watchdog/). Requires plumbing hooks from GameProfile
    // into LaunchRequest (launch/request/models.rs) and equivalent treatment in the
    // native/trainer builders. See issue #<N from Task 1.2>.
```

**Validation**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` green (existing tests only at this point); `cargo build --manifest-path src/crosshook-native/Cargo.toml --workspace` compiles.

### Task 3.1: Round-trip + compatibility tests — Depends on [2.1]

**Files**: `crates/crosshook-core/src/profile/models/tests/hooks.rs` (new), `crates/crosshook-core/src/profile/models/tests/mod.rs` (add `mod hooks;`)

File opens with `#![cfg(test)]`, imports `use super::super::*;` + `use super::fixtures::*;` (the `models/tests/*` convention). Test fn naming: behavior-first snake*case, no `test*` prefix. Tests:

1. `launch_hooks_two_each_toml_roundtrip` — profile with 2 pre + 2 post hooks → `toml::to_string_pretty` → assert output contains `[[pre_launch_hooks]]`, `[[post_exit_hooks]]`, `stage = "pre-launch"`, `stage = "post-exit"` → `toml::from_str::<GameProfile>` → `assert_eq!(parsed, profile)` (whole-profile equality).
2. `empty_hook_vecs_omitted_from_toml` — `sample_profile()` serialized; assert `!serialized.contains("pre_launch_hooks")` and `!…("post_exit_hooks")` (substring check, NOT the bracketed header — guards against a false pass if the field home ever changes); parsed vecs empty.
3. `legacy_profile_without_hook_keys_defaults_to_empty` — raw `r#"…"#` TOML with only `[game]`/`[launch]` → both vecs empty (AC3).
4. `unknown_stage_value_is_rejected` — `stage = "mid-flight"` → `toml::from_str` errors (serde enums reject unknown variants; locks the variant set).
5. `stage_defaults_to_pre_launch_when_omitted` — hook table without `stage` → `HookStage::PreLaunch`; also `assert_eq!(HookStage::default(), HookStage::PreLaunch)`.
6. `malformed_hook_missing_fields_tolerated` — hook table with only `path` → deserializes (struct-level `#[serde(default)]`), `id`/`name` empty, `enabled == false`. Locks Decision 5.

Use a local `fn sample_hook(id: &str, stage: HookStage) -> LaunchHook` helper. Remember workspace lints: inline format args in assert messages (`"{serialized}"`).

**Validation**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core hooks` — all new tests pass.

### Task 3.2: Exchange hardening (export strip + import disable) — Depends on [2.1]

**Files**: `crates/crosshook-core/src/profile/exchange/utils.rs`, exchange tests (follow existing test layout under `profile/exchange/`)

Security rationale (user-approved scope): the community-export sanitizer is a fail-open **denylist** — `sanitize_profile_for_community_export` (`exchange/utils.rs:20-36`) clears only enumerated path fields, so hooks would silently leak; import (`hydrate_imported_profile`, `utils.rs:38-80`) performs no field stripping. Once runtime lands, imported attacker-controlled hook paths become an execution vector.

1. **Export**: in `sanitize_profile_for_community_export`, clear both vecs (`profile.pre_launch_hooks.clear(); profile.post_exit_hooks.clear();`) alongside the existing `dll_paths`/`proton_path` clears. Add a comment noting the denylist invariant: any new path-bearing field must be cleared here.
2. **Import**: in `hydrate_imported_profile`, force-disable any hooks that arrive: `for h in profile.pre_launch_hooks.iter_mut().chain(profile.post_exit_hooks.iter_mut()) { h.enabled = false; }`. Imported profiles can never auto-execute a hook even after runtime lands.
3. **Tests** (mirror existing exchange test style):
   - `community_export_strips_launch_hooks` — profile with enabled hooks → exported manifest JSON contains no hook paths / empty vecs.
   - `community_import_force_disables_launch_hooks` — manifest embedding `enabled = true` hooks → hydrated profile has all hooks `enabled == false` (entries retained, paths intact, just disabled).
   - Regression guard: collection-preset exchange (`collection_exchange/`) is allowlist-based and carries no hooks — no change needed; add an assertion only if an existing descriptor round-trip test is cheap to extend.

**Validation**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core exchange` green.

### Task 3.3: ts-rs export registration + regeneration — Depends on [2.1]

**Files**: `crates/crosshook-core/src/ts_rs_exports.rs`, `src/crosshook-native/src/types/generated/launch_hooks.ts` (generated, committed)

ts-rs is a **manual allowlist** — the `cfg_attr` derives from Task 1.1 emit nothing until registered:

1. In `ts_rs_exports.rs`: import `crate::profile::models::{HookStage, LaunchHook}` (gated module is already `#![cfg(feature = "ts-rs")]`), add:

```rust
fn export_launch_hooks() -> Result<(), Box<dyn std::error::Error>> {
    use crate::profile::{HookStage, LaunchHook};
    HookStage::export()?; // dependency first (LaunchHook references it)
    LaunchHook::export()?;
    Ok(())
}
```

and call `export_launch_hooks()?;` from `export_ts_types()` (alongside `export_onboarding()?;` at line ~35).

2. Regenerate:

```bash
cargo run --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --features ts-rs --bin ts_rs_export
```

3. Format the generated file (the exporter emits compact double-quoted output; checked-in files are Biome-formatted — known wrinkle from PR-355 review F003): `./scripts/format.sh` (or the TS-scoped variant), then commit `generated/launch_hooks.ts`. Expected content: `export type HookStage = 'pre-launch' | 'post-exit';` and `export type LaunchHook = { id: string; name: string; path: string; stage: HookStage; enabled: boolean; };` (snake_case fields verbatim — no rename on the struct).

4. Clippy under the feature (PR-355 review F002 precedent — the feature path can fail `-D warnings` independently): `cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --features ts-rs --all-targets -- -D warnings`.

**Validation**: generated file exists with the expected unions/fields; clippy + tests green with `--features ts-rs`. Note: there is **no CI drift sentinel** for `generated/` — committing the regenerated file is a manual, unenforced step; do not skip it.

### Task 4.1: Frontend type mirror + barrel export — Depends on [3.1, 3.2, 3.3]

**Files**: `src/crosshook-native/src/types/profile.ts`

The frontend hand-maintains `GameProfile` (`profile.ts:93-172`); generated files cover only the element types. Freeze the Phase 6 frontend contract:

1. Import + funnel re-export (the `onboarding.ts` pattern): `import type { LaunchHook } from './generated/launch_hooks';` and `export type { LaunchHook, HookStage } from './generated/launch_hooks';` — making both importable from the `types` barrel (`index.ts` already does `export * from './profile';`).
2. Add to the `GameProfile` interface at **top level** (after `local_override`), using `?` per the repo convention (`profile.ts:343-348`: serde drops empty vecs → field absent → `undefined`; never `| null`):

```ts
  pre_launch_hooks?: LaunchHook[];
  post_exit_hooks?: LaunchHook[];
```

3. In `normalizeSerializedGameProfile` (`profile.ts:226-286`), default + deep-copy both arrays, mirroring the `required_protontricks` idiom: `pre_launch_hooks: [...(profile.pre_launch_hooks ?? [])]` (and same for post). Leave `DEFAULT_*` constants untouched (optional fields stay absent, consistent with `gamescope?`/`mangohud?`).

**Validation**: `npm run typecheck` green; `npm test` green (no behavioral frontend change expected).

---

## Testing Strategy

- **Unit (Rust)**: Task 3.1's six tests lock serialization shape, kebab-case wire values, default behavior, unknown-variant rejection, tolerance, and whole-profile equality. Task 3.2's tests lock the exchange security invariants. Existing `toml_store` save/load tests now exercise the fields implicitly via fixtures.
- **Compat directions**: old→new (test 3), new-empty→old (test 2: byte-identical output), new-populated→old (mechanism: serde skips unknown keys; no `deny_unknown_fields` in the tree — assert via test comment, not a stripped-struct fixture).
- **Frontend**: typecheck-only — no behavioral change; Phase 6 (#471) adds the RTL coverage.
- **No frontend test framework gap**: Vitest exists (`npm test`) but nothing new to assert here beyond compilation.

## Validation Commands

Run in order; all must pass before the task/PR is done.

```bash
# Level 1 — static
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --features ts-rs --all-targets -- -D warnings
./scripts/lint.sh --rust

# Level 2 — unit
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --features ts-rs

# Level 3 — build / generation
cargo run --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --features ts-rs --bin ts_rs_export
git diff --exit-code src/crosshook-native/src/types/generated/   # no drift after regen+format
npm run typecheck
npm test

# Level 4 — integration / scope guards
cargo build --manifest-path src/crosshook-native/Cargo.toml --workspace
grep -rn "pre_launch_hooks\|post_exit_hooks" src/crosshook-native/crates/crosshook-core/src/launch/ \
  | grep -v "TODO(hooks-runtime)" | wc -l   # must be 0 — no launcher consumption

# Level 5 — edge (covered by unit tests; spot-check)
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core hooks
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core exchange
```

`./scripts/check-host-gateway.sh` is not applicable (schema spawns nothing) but is harmless to run.

## Acceptance Criteria

From issue #468, made testable:

- [ ] `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` green (AC1)
- [ ] `LaunchHook` + `HookStage` emitted in `src/crosshook-native/src/types/generated/launch_hooks.ts` AND re-exported from the `types` barrel (AC2)
- [ ] Old profiles (no hook keys) deserialize with empty vecs (AC3 — test 3)
- [ ] New profiles with empty vecs emit no `pre_launch_hooks`/`post_exit_hooks` keys at all (AC4 — test 2)
- [ ] Two-hook round-trip equality with kebab-case stage values on the wire (AC5 — test 1)
- [ ] Zero hook references in `launch/` beyond the TODO breadcrumb (AC6 — Level 4 grep)
- [ ] Open question resolved and recorded: single struct + stage enum; top-level field home (this plan, Resolved Decisions)
- [ ] Community export strips hooks; community import force-disables them (user-approved hardening scope)
- [ ] Hooks-runtime follow-up issue exists with persistence subsections; both TODO comments reference it

## Completion Checklist

- [ ] All 7 tasks complete; all Level 1–5 validation commands pass
- [ ] `generated/launch_hooks.ts` committed (no CI sentinel will catch a miss — manual gate)
- [ ] PR title: `feat(profiles): add pre/post launch hook schema (LaunchHook, HookStage)` (Conventional Commits; squash-merge lands it in CHANGELOG.md verbatim)
- [ ] PR body: `Closes #468`, `Part of #478`; includes Storage boundary + Persistence & usability subsections (copy from this plan); labels `type:feature`, `area:profiles`, `phase:3`, `feat:hero-detail-consolidation`, `priority:high`
- [ ] PRD Implementation Phases table: Phase 3 row → `in-progress`/`complete` + PRP link (commit as `docs(internal): …` separately, along with this plan file)
- [ ] Update issue #471 (Phase 6) with a comment confirming the frozen contract (type names, field names, kebab-case wire values, top-level home, `id` minting convention)

## Risks

| Risk                                                                                 | Likelihood             | Mitigation                                                                                                      |
| ------------------------------------------------------------------------------------ | ---------------------- | --------------------------------------------------------------------------------------------------------------- |
| Missed exhaustive `GameProfile { ... }` literal                                      | L                      | Compiler enumerates every site; build `--workspace` including src-tauri tests                                   |
| ts-rs export silently produces nothing                                               | M                      | Task 3.3 registers in `ts_rs_exports.rs` AND Level 3 runs the exporter + `git diff --exit-code` on `generated/` |
| Generated file format drifts from Biome style                                        | M                      | `./scripts/format.sh` after regen (PR-355 F003 precedent), then diff-check                                      |
| PRD snippet copied verbatim (missing `PartialEq, Eq`) breaks `GameProfile`'s derives | H if ignored           | Decision 3 overrides the PRD; tests won't compile otherwise — fails fast                                        |
| Stage/vec inconsistency in hand-edited TOML                                          | L (inert — no runtime) | Documented invariant; normalization deferred to runtime follow-up                                               |
| Exchange hardening missed on a future path-bearing field                             | M (future)             | Denylist-invariant comment added in `sanitize_profile_for_community_export`                                     |
| `clippy --features ts-rs` failure not caught by default CI lane                      | M                      | Explicit Level 1 command (PR-355 F002 precedent)                                                                |

## Notes

- **toml ordering non-issue**: the pinned `toml 1.1` auto-orders scalars before tables (empirically verified during research) — the legacy "values must be emitted before tables" error is unreachable. Field placement at the end of `GameProfile` is for readable output and future-proofing only.
- **`HookStage` kebab vs snake**: `TrainerLoadingMode` uses snake_case but the PRD explicitly specifies kebab-case (`"pre-launch"`/`"post-exit"`) and `MangoHudPosition` is the kebab precedent. Kebab-case is part of the frozen Phase 6 contract — do not "fix" it to snake_case.
- **Why struct-level `#[serde(default)]` on `LaunchHook`** (vs. required `id`/`name`/`path`): the PRD's degraded-fallback contract expects a malformed hook to render as an "Invalid hook" row (PRD line 245), which requires the profile to still load. This trades CLAUDE.md's fail-fast preference for the PRD's explicit usability contract and the codebase-wide tolerant-default convention.
- **Parallelism with sibling phases**: per the PRD, Phase 3 has no upstream deps. Phases 1 (#466) and 2 (#467) are already closed; #471 (Phase 6) is the blocked consumer.
- Research for this plan was produced by 7 parallel researchers (api/business/tech/ux/security/practices/recommendations); key file:line evidence is embedded above rather than kept as separate artifacts.
