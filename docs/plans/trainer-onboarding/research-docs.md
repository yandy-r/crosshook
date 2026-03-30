# Documentation Research: Trainer Onboarding

Comprehensive map of all documentation relevant to implementing the trainer-onboarding feature (#37).
Read the **Must-Read** list first to orient quickly; the rest supports deep dives.

---

## Architecture Docs

| File                                                  | Status       | Key Content                                                                               |
| ----------------------------------------------------- | ------------ | ----------------------------------------------------------------------------------------- |
| `docs/plans/trainer-onboarding/feature-spec.md`       | **REQUIRED** | Definitive spec: data models, API design, 8 files to create, 6 to modify, 4-phase rollout |
| `docs/plans/trainer-onboarding/research-technical.md` | **REQUIRED** | Architecture diagram, component relationships, IPC sequence, data-flow                    |
| `CLAUDE.md` (root)                                    | **REQUIRED** | Project overview, architecture map, all module paths, build commands                      |

The feature-spec defines the full architecture in `docs/plans/trainer-onboarding/feature-spec.md:108-127`. A component diagram is available in `research-technical.md:13-53`.

---

## Feature Spec and Research

All 7 research artifacts from the feature-research phase live in `docs/plans/trainer-onboarding/`:

| File                          | Summary                                                                                                                       |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `feature-spec.md`             | Master spec synthesizing all research. Start here.                                                                            |
| `research-technical.md`       | Full architecture: IPC commands, data models, readiness function signatures, files to create/modify, startup event emission   |
| `research-business.md`        | Domain rules (BR-1 through BR-10), user stories (U1–U7), edge cases (EC-1–EC-6), workflows, integration constraints           |
| `research-ux.md`              | Wizard flow, per-step UX patterns, Steam Deck gamepad risks, progressive disclosure, inline validation, accessibility         |
| `research-security.md`        | W-1 (git branch injection), W-2 (URL scheme allowlist), 8 advisory items; community-tap hardening must ship before onboarding |
| `research-practices.md`       | Reusable code inventory, KISS assessment, build-vs-depend decisions, module boundary design, testability patterns             |
| `research-external.md`        | Trainer distribution sources (FLiNG, WeMod), Steam Store API, ProtonDB API, PE magic check, zero new dependencies for v1      |
| `research-recommendations.md` | Phasing plan (Phase 0–4), technology decisions table, risk assessment, alternative approaches, key decisions needed           |

---

## Configuration Files

### `src/crosshook-native/src-tauri/tauri.conf.json`

- **Window**: 1280×800, dark theme, single `main` window
- **CSP**: `default-src 'self'; script-src 'self'` — no inline scripts, no external URLs in JS
- **Bundle target**: `appimage` only; resources includes `runtime-helpers/*.sh`
- **Onboarding impact**: No changes needed; wizard is a React component, not a new window

### `src/crosshook-native/src-tauri/capabilities/default.json`

Current state: `core:default` + `dialog:default`. The spec requires registering 3 new commands:

```json
"check_readiness", "dismiss_onboarding", "get_trainer_guidance"
```

No additional FS shell access needed for guidance/status commands (spec:298-299).

### `src/crosshook-native/Cargo.toml`

Workspace with 3 members: `crosshook-core`, `crosshook-cli`, `src-tauri`. Current version `0.2.4`.
New `onboarding/` module adds to `crosshook-core` only.

### `src/crosshook-native/package.json`

Frontend dependencies relevant to onboarding:

- `@tauri-apps/api: ^2.0.0` — `invoke()` for IPC commands
- `@tauri-apps/plugin-dialog: ^2.0.0` — file picker (`chooseFile`, `chooseDirectory`), already used
- `@tauri-apps/plugin-shell: ^2.0.0` — used by existing features (not needed for onboarding v1)
- `@tauri-apps/plugin-fs: ^2.0.0` — present; wizard uses dialog plugin path, not direct FS access

---

## Code Documentation (Well-Documented Modules)

### Settings Persistence — `crates/crosshook-core/src/settings/mod.rs:1`

**Current `AppSettingsData` struct (line 21-25):**

```rust
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    // MISSING: onboarding_completed / onboarding_dismissed: bool
}
```

The struct uses `#[serde(default)]` (line 20) — adding `onboarding_completed: bool` is backward-compatible; existing `settings.toml` files will default to `false`. The `load`/`save` methods (lines 87-103) are simple `toml::from_str` / `toml::to_string_pretty` — no migration needed.

Tests at lines 111-168 demonstrate the round-trip pattern and `#[serde(default)]` behavior.

### Profile Domain Types — `crates/crosshook-core/src/profile/models.rs:51`

`TrainerLoadingMode` enum (line 51-57):

- `SourceDirectory` (default) — trainer runs from host path; field must point to a directory
- `CopyToPrefix` — trainer staged into Wine prefix before launch; field must point to `.exe`

This type is used as-is in onboarding guidance (`research-practices.md:16`). No extension needed.

### Health Types — `crates/crosshook-core/src/profile/health.rs:31` + `src/types/health.ts:6`

The Rust source has doc comments on every struct (lines 11, 20, 29, 39, 49). **`HealthIssue` is reused directly for readiness check results** — same fields, same serde serialization, same frontend type:

```rust
/// A single path-field issue found during health check.
pub struct HealthIssue { field, path, message, remediation, severity }
```

Note: `HealthIssueSeverity` comment at line 20 clarifies it is _distinct_ from `ValidationSeverity` (which is always Fatal) — important for onboarding readiness severity levels.

TypeScript mirror — `src/types/health.ts:6` — `HealthIssue` interface — **reused directly for readiness check results**:

```typescript
interface HealthIssue {
  field: string; // e.g., "steam_installed", "proton_available"
  path: string;
  message: string;
  remediation: string;
  severity: HealthIssueSeverity; // 'error' | 'warning' | 'info'
}
```

The readiness check `ReadinessCheckResult` wraps `HealthIssue[]` (feature-spec.md:147-154). No new type needed — re-export from `health.ts`.

### Reference Stage Machine — `src/crosshook-native/src/hooks/useInstallGame.ts:1`

The `useInstallGame` hook is the canonical pattern for wizard state. Key structural elements:

- `UseInstallGameResult` interface (line 19): `stage`, `statusText`, `hintText`, `actionLabel`
- Pure derivation functions `deriveStatusText` / `deriveHintText` extracted to module scope
- `useCallback` + `useState` (not `useReducer`) for simple linear stages
- Error handling via `setFieldError` / `setGeneralError` (not exception throwing)

`useOnboarding.ts` should mirror this interface shape exactly.

### Tauri App Setup — `src/crosshook-native/src-tauri/src/lib.rs:1`

Documents how stores are registered and startup events are emitted. Key patterns:

- All stores `.manage()`'d before `invoke_handler` (line 16-35)
- Startup events emitted via `tauri::async_runtime::spawn` with `sleep(350ms)` delay (line 61-69)
- `MetadataStore::disabled()` fallback for SQLite unavailability (line 33-35)
- New `onboarding-check` event should follow the same delayed-emit pattern as `auto-load-profile`

### Core Module Root — `crates/crosshook-core/src/lib.rs:1`

Module declarations. New line `pub mod onboarding;` must be added here.

Current modules: `community`, `export`, `install`, `launch`, `logging`, `metadata`, `profile`, `settings`, `steam`, `update`.

---

## Must-Read Documents (Prioritized)

Read in this order for a complete implementation context:

1. **`docs/plans/trainer-onboarding/feature-spec.md`** — single source of truth. Has data models, API signatures, files to create/modify, and phasing. Do not skip.

2. **`CLAUDE.md`** — architecture, directory structure, code conventions, build commands. Especially the Key Patterns section.

3. **`docs/plans/trainer-onboarding/research-technical.md`** — detailed architecture: component diagram, IPC commands, startup event flow, data models with serialization notes.

4. **`docs/plans/trainer-onboarding/research-business.md`** — business rules BR-1 through BR-10 are implementation requirements, not soft guidance. BR-2 (loading mode) and BR-8 (state persistence) are most critical.

5. **`docs/plans/trainer-onboarding/research-recommendations.md`** — phasing plan and the technology decisions table. Read the "Architectural Decisions" table before writing any code.

6. **`crates/crosshook-core/src/settings/mod.rs`** — understand the existing `AppSettingsData` struct before adding the `onboarding_completed` field.

7. **`src/crosshook-native/src/hooks/useInstallGame.ts`** — reference implementation for the `useOnboarding` stage-machine hook.

### Nice-to-Have (supporting context)

- `docs/plans/trainer-onboarding/research-ux.md` — Steam Deck gamepad requirements, 56px touch targets, B=back rule, inline validation patterns
- `docs/plans/trainer-onboarding/research-security.md` — W-1 and W-2 detail. Needed before Phase 0 security hardening.
- `docs/plans/trainer-onboarding/research-practices.md` — reusable code inventory and KISS assessment; useful when making "build vs reuse" decisions
- `docs/plans/trainer-onboarding/research-external.md` — trainer source details, PE magic check implementation, future API references

---

## Documentation Gaps

1. **No Tauri v2 modal/focus-trap documentation internally.** The feature-spec describes focus trap requirements (`data-crosshook-focus-root="modal"`, `role="dialog"`, `aria-modal="true"`) but there is no existing CrossHook component that implements a modal focus trap. Implementers must reference Tauri v2 docs and ARIA authoring practices externally.

2. **`research-technical.md` cross-references a `research-tech-specs.md` filename in `research-recommendations.md:388`** — this is the same file under an earlier working title. No separate file exists; `research-technical.md` is the correct file.

3. **`onboarding_dismissed` vs `onboarding_completed` naming conflict.** `research-business.md` prefers `onboarding_dismissed` (line 83); `feature-spec.md` uses `onboarding_completed` (line 139). The canonical name in `feature-spec.md` is `onboarding_completed`. Use that.

4. **No external Tauri v2 permissions docs are referenced internally.** When registering the 3 new commands in `capabilities/default.json`, refer to the Tauri v2 permissions system docs externally (schema at `src-tauri/gen/schemas/desktop-schema.json`).

5. **Gamepad `ShowFloatingGamepadTextInput` has no internal reference.** The UX spec calls for Steam Deck keyboard input via this Steam Input API. No example exists in the codebase; implementers must find this externally.

6. **No existing modal component to reference.** The `ProfileReviewModal` and `LauncherPreviewModal` use a promise-resolver pattern (noted in `research-recommendations.md:206-208` as explicitly NOT to follow for the wizard). The wizard needs a different pattern but no clean example exists yet.
