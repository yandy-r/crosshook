# Security Research: UI Enhancements (Profiles Page Restructuring)

**Feature**: Restructure the Profiles page Advanced section — visual containers, sub-tabs, or section promotion.
**Scope**: UI-only restructuring. No new attack surface. No new IPC commands anticipated.
**Analyst**: security-researcher
**Date**: 2026-03-31

---

## Executive Summary

This is a low-risk UI restructuring task. CrossHook is a **native Linux desktop app** (Tauri v2) — it is not a web app, not multi-user, and not network-exposed. The threat model is a single authenticated local user operating their own machine.

The existing codebase already has solid security foundations: profile name validation prevents path traversal in `validate_name()` (`toml_store.rs:497-521`), environment variable keys are validated for `=` and NUL characters (`CustomEnvironmentVariablesSection.tsx:38-53`), reserved env keys are protected (`RESERVED_CUSTOM_ENV_KEYS`), Tauri capabilities are minimal (`core:default`, `dialog:default`, `shell:open-url` only), and ProtonDB-sourced env vars are sanitized in the Rust backend before they reach the frontend (`aggregation.rs:254-311`).

The primary security risks introduced by a UI restructuring are **data loss from unsaved changes during tab/section transitions** and **ensuring any new IPC commands follow the existing pattern**. Neither is a hard blocker — both are manageable with the practices described below.

---

## Findings by Severity

### CRITICAL — Hard Stop

| #   | Finding              | Location | Rationale                                                 |
| --- | -------------------- | -------- | --------------------------------------------------------- |
| —   | No critical findings | —        | No new attack surface introduced by UI-only restructuring |

### WARNING — Must Address

| #   | Finding                                                                                                                                                                                           | Location                                                                    | Rationale                                                                                                                                                                                                                                                                                              |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| W1  | Unsaved changes lost on tab switch — **partially mitigated** by props-only section decomposition; residual risk only for `CustomEnvironmentVariablesSection` which holds local `rows` draft state | `useProfile.ts:461`, `CustomEnvironmentVariablesSection.tsx`                | All proposed section components are stateless props-renderers except `EnvVarsSection`. That component must be CSS show/hide (not conditionally unmounted) on tab switch. All other sections can safely unmount.                                                                                        |
| W2  | ~~sessionStorage key namespacing~~ — **Eliminated**: tech-designer confirmed active tab state uses local `useState`, not sessionStorage. No new sessionStorage keys are introduced.               | —                                                                           | Only applies if a future change opts to persist tab state across sessions — document that decision explicitly if it ever occurs, and use `crosshook.*` prefix.                                                                                                                                         |
| W3  | `injection.dll_paths` and `injection.inject_on_launch` fields are present in `GameProfile` but intentionally not exposed in the UI — the restructuring must not accidentally surface them         | `src/crosshook-native/src/types/profile.ts:98-100`, `useProfile.ts:424-426` | These fields are managed by the install/migration pipeline, not user-facing forms. The community export sanitizer explicitly clears them (`exchange.rs:259`). Exposing DLL path inputs in a reorganized form would reintroduce a removed capability. Keep `injection.*` absent from all form sections. |

### ADVISORY — Best Practice

| #   | Finding                                                                                                                                                                                                                                                 | Location                                        | Rationale                                                                                                                                                                                                                                      |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1  | File path inputs have no client-side traversal indicator — paths like `../../etc/passwd` are accepted by the text field and only rejected at the Rust `validate_name()` boundary for profile names; game/executable paths are not validated client-side | `ProfileFormSections.tsx:124-163`               | For game executable paths (not profile names), validation is purely backend-enforced. This is architecturally correct (trust the backend) but displaying inline feedback for obviously malformed paths improves UX without weakening security. |
| A2  | Environment variable values have no length limit client-side                                                                                                                                                                                            | `CustomEnvironmentVariablesSection.tsx:181-192` | Extremely long env var values (e.g. megabytes) would be serialized into the profile TOML and passed over IPC. In practice this is a self-inflicted user action, not an attack, but a soft character limit advisory is a usability improvement. |
| A3  | ~~Dependency risk if a tab library is added~~ — **Resolved**: `@radix-ui/react-tabs` v1.1.13 and `react-resizable-panels` v4.7.6 are already in `package.json`. Zero new dependency risk.                                                               | `package.json`                                  | No new attack surface. All evaluated Radix UI primitives are pure DOM/CSS — no Tauri IPC, no shell access, no network calls. `@radix-ui/react-accordion` (same vendor) would also be minimal risk if chosen.                                   |

---

## State Management Security

**Current model**: Profile state is centralized in `useProfile` hook (exposed via `ProfileContext`). The `dirty` flag is set when `updateProfile` is called and cleared on save or profile switch. Local-only UI state (env var rows) is buffered in `CustomEnvironmentVariablesSection` and flushed to context via `onUpdateProfile` on every change.

**Tab navigation risk**: If the restructuring introduces sub-tabs that conditionally render form sections, React will unmount components that go out of view. `CustomEnvironmentVariablesSection` holds local `rows` state synchronized from `customEnvVars` via a `useEffect`. If this component unmounts mid-edit (tab switch before blur), the `applyRows` call on the last keystroke may not have fired, and a partially entered row will be lost on unmount.

**Recommendation (W1 mitigation)**:

- Prefer `display: none` / CSS visibility toggling over conditional rendering (`{activeTab === 'x' && <Section />}`) so components stay mounted and state is preserved.
- If unmount is unavoidable, add a `useEffect` cleanup in `CustomEnvironmentVariablesSection` that calls `applyRows(rows)` on unmount to flush current draft state to context.
- The `dirty` indicator in `ProfileActions` already covers the top-level save state — extend the guard to prompt before switching profiles (already present via `selectProfile` which checks `dirty`).

**Cross-profile data leakage**: Not a concern. Profile state is always loaded fresh from the backend on profile selection (`profile_load` IPC command). There is no shared mutable state between profiles in the frontend.

---

## Input Validation

**Profile names**: Fully validated at `crosshook-core::profile::validate_name()` before any filesystem operation. Rejects: empty, `.`, `..`, absolute paths, slashes, backslashes, colons, and all Windows reserved path characters. This function is called by `profile_path()` which gates every read/write/delete — path traversal via profile name is not possible.

**Game/executable paths**: Accepted as free-form strings and stored in TOML. They are passed to the OS/Wine/Proton launcher process via the backend. The backend does not perform additional path validation beyond what the OS enforces. This is the correct design for a launcher tool — restricting what paths a user can configure would break legitimate use cases (custom Wine prefixes, non-standard install locations). No change needed.

**Environment variable keys**: Client-side validation blocks `=` characters, NUL bytes, and reserved CrossHook-managed keys (`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`). These constraints mirror `RESERVED_CUSTOM_ENV_KEYS` defined in `crosshook-core launch/request.rs` — the frontend comment explicitly states this. This is a good pattern; if the restructuring moves this component, ensure this mirroring contract is preserved.

**Environment variable injection via values**: Custom env var values are passed to the child process environment. A user can set any value. This is intentional — it is a feature. The risk is self-inflicted (user configuring their own launcher). No mitigation needed.

**ProtonDB-sourced environment variables**: This is the one externally-sourced data path worth reviewing. The chain is:

1. `aggregation.rs::safe_env_var_suggestions()` (lines 254–311) parses raw ProtonDB report launch-option strings and applies three filters before any suggestion reaches the IPC layer:
   - `is_safe_env_key()`: Keys must match `[A-Z_][A-Z0-9_]*` — rejecting lowercase, shell metacharacters, and empty keys.
   - `is_safe_env_value()`: Values must contain no NUL, whitespace, or shell metacharacters (`$`, `;`, `"`, `'`, `\`, `` ` ``, `|`, `&`, `<`, `>`, `(`, `)`, `%`).
   - `RESERVED_ENV_KEYS` check: `WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH` are stripped.
2. The filtered `ProtonDbEnvVarSuggestion` list is serialized by the backend and sent over IPC.
3. Frontend `mergeProtonDbEnvVarGroup()` (`utils/protondb.ts`) performs conflict detection and merges — it does not re-filter reserved keys because the backend already guarantees they are absent.

**Finding**: The backend sanitization is sound and defense-in-depth is adequate. The frontend does not need to duplicate the key allowlist from `aggregation.rs` because the data is backend-normalized before crossing the IPC boundary. **However**, this is a documented trust assumption: if ProtonDB data were ever surfaced through a different code path that bypasses `safe_env_var_suggestions`, the frontend's `CustomEnvironmentVariablesSection` reserved-key check would be the only remaining guard. This contract should be preserved — do not remove or weaken `RESERVED_CUSTOM_ENV_KEYS` from the frontend during restructuring.

**Injection fields (`injection.dll_paths`, `injection.inject_on_launch`)**: These fields are present in the `GameProfile` Rust model and persisted to TOML, but are **intentionally not surfaced in `ProfileFormSections`** — confirmed by searching the entire component tree. They are populated via the install/migration pipeline, not user-facing forms. The community export sanitizer (`exchange.rs:259`) explicitly clears `dll_paths` before sharing profiles externally. The UI restructuring must **not** accidentally expose these fields — do not add form inputs for `injection.*` during the restructuring. If the fields appear in a profile review modal context, they should remain read-only display only.

---

## Navigation Guards

**Existing guard**: `selectProfile` in `useProfile.ts` enforces a `dirty` check before switching profiles. The frontend shows an "Unsaved changes" indicator in `ProfileActions`. This covers the most dangerous path (switching profiles with unsaved edits).

**Gap with sub-tab navigation**: Sub-tab switching within the same profile does not need a dirty guard — the user has not navigated away from the profile. State persists in context. The only risk is local component state loss (see W1 above).

**Delete confirmation**: A confirmation dialog exists in `ProfilesPage.tsx:818-845` before `profile_delete` is invoked. This is sufficient for a destructive IPC action.

**Recommendations**:

- No additional navigation guards needed beyond what exists.
- If a "Discard changes" button is added during restructuring (common in tabbed UIs), ensure it clears `dirty` and reloads profile from backend rather than resetting to a stale in-memory snapshot.

---

## Dependency Security

**Update (from api-researcher)**: `@radix-ui/react-tabs` v1.1.13 and `react-resizable-panels` v4.7.6 are already installed in `package.json`. This changes the dependency risk picture from "prefer avoiding new dependencies" to "zero marginal risk" for the primary tab candidates.

| Library                             | Status                                            | Risk Assessment                                                                                                   |
| ----------------------------------- | ------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `@radix-ui/react-tabs` v1.1.13      | **Already installed**                             | Zero new attack surface. Pure DOM/CSS — no Tauri IPC, no shell access, no network calls.                          |
| `react-resizable-panels` v4.7.6     | **Already installed**                             | Same. Suitable for panel-based layouts if that direction is chosen.                                               |
| `@radix-ui/react-accordion` v1.2.12 | Potential new addition — same WorkOS/Radix vendor | Minimal risk. Same maintenance profile as installed Radix packages. Acceptable if accordion pattern is preferred. |
| Custom CSS / `CollapsibleSection`   | In-repo, zero dependencies                        | Lowest risk. Still a valid choice if tab/accordion libraries are not needed.                                      |

**Recommendation**: Using `@radix-ui/react-tabs` (already installed) for sub-tabs carries zero new security risk. No supply chain audit required — it is already part of the dependency tree.

---

## Tauri IPC Security

**Current capability surface** (`capabilities/default.json`):

```json
{
  "permissions": ["core:default", "dialog:default", "shell:open-url"]
}
```

This is appropriately minimal. `shell:open-url` allows opening URLs in the default browser (used for external links), not arbitrary shell execution.

**IPC for UI restructuring**: No new IPC commands are anticipated for a UI-only restructuring. The existing commands (`profile_save`, `profile_load`, `profile_list`, etc.) are sufficient. All existing profile IPC commands follow the established pattern: `snake_case` names, Serde-serialized parameters, `State<'_, ProfileStore>` injection.

**If new IPC commands ARE added** (e.g. for tab state persistence):

- Must follow `snake_case` naming convention per `CLAUDE.md`
- Must not expand capability surface in `default.json` unless strictly necessary
- Tab UI state (active tab, collapse state) should use `sessionStorage` in the frontend — no IPC needed
- If tab preferences need to persist across sessions, store in `AppSettingsData` via existing `settings_save` command rather than creating new IPC endpoints

**No new capabilities should be added** to `default.json` for this feature.

---

## Secure Coding Guidelines

These apply specifically to the implementation of the UI restructuring:

1. **Component mounting strategy**: Use CSS-based show/hide for tab content rather than conditional rendering where components hold local draft state. This is the primary implementation guard against data loss.

2. **Session key namespacing**: Any new `sessionStorage` keys must use the `crosshook.*` prefix (e.g. `crosshook.profilesActiveTab`). Do not use generic keys like `activeTab` that could collide.

3. **No new npm dependency risk for tabs**: `@radix-ui/react-tabs` is already installed — using it introduces zero new supply chain surface. If `@radix-ui/react-accordion` is needed, it is the same vendor and acceptable. Do not reach for unrelated or unvetted libraries.

4. **Do not introduce `dangerouslySetInnerHTML`**: No scenario in this UI restructuring requires raw HTML injection. Any error messages or labels should use JSX text nodes.

5. **Preserve `RESERVED_CUSTOM_ENV_KEYS` mirror contract**: If `CustomEnvironmentVariablesSection` is moved or refactored during restructuring, the client-side reserved key list must remain synchronized with `crosshook-core/src/launch/request.rs`.

6. **Confirmation before destructive actions**: The existing delete confirmation pattern must not be bypassed during restructuring. If sections are reorganized, ensure the delete button retains its `confirmDelete` → `executeDelete` two-step flow. Unsaved-changes prompts should use a modal overlay — not a browser-native `alert()` — consistent with the existing delete/rename dialog patterns.

7. **Compact view / progressive disclosure must not hide misconfiguration-critical fields**: If a "compact view" toggle or collapsed-by-default section is introduced, path fields that affect runtime security boundaries (`steam.compatdata_path`, `runtime.prefix_path`, `steam.proton_path`, `runtime.proton_path`) must remain visible or have their configured values summarized in the collapsed state. Hiding these fields entirely could leave users unaware of misconfigured Wine prefixes or stale Proton paths without any indicator.

---

## Trade-off Recommendations

| Trade-off                             | Recommendation                                                                                                                                                                                                    |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CSS show/hide vs. unmount for tabs    | Prefer CSS show/hide to preserve local component state. Accept slightly higher DOM size for safety.                                                                                                               |
| Tab library vs. custom CSS            | `@radix-ui/react-tabs` is already installed — using it carries zero new dependency risk. Custom CSS/`CollapsibleSection` remains viable. The choice is architectural, not a security concern.                     |
| Client-side path validation           | Do not add — paths are intentionally user-controlled. Display helpful error messages from backend validation results instead.                                                                                     |
| Unsaved-changes prompt implementation | Use a modal overlay (consistent with existing delete/rename dialogs). Do not use browser-native `alert()` — it is visually inconsistent and cannot be styled to match the destructive-action pattern.             |
| Compact view for path fields          | If implemented, show a summary of configured values in the collapsed state rather than hiding them entirely. Silently hiding `compatdata_path` or `proton_path` when misconfigured creates a silent failure mode. |
| Persisting active tab across sessions | Use `sessionStorage` for session scope. Do not use `localStorage` (would affect future sessions unexpectedly). Do not use IPC/backend for UI-only state.                                                          |

---

## Open Questions

1. **Tab vs. section cards decision**: The security posture is equivalent for both approaches. The choice is a UX/design decision, not a security one.
2. **Env var draft flushing on unmount**: If sub-tabs are implemented with unmounting, a decision is needed on whether to auto-flush draft state or warn the user. Auto-flush (write current draft to context) is simpler and safer.
3. **Does the restructuring require any new IPC commands?** If yes, enumerate them here for review before implementation. From a security perspective, no new IPC is expected.

---

## Sources

- [Tauri v2 Security Model](https://v2.tauri.app/security/) — IPC trust boundary and capability model
- [Tauri v2 IPC](https://v2.tauri.app/concept/inter-process-communication/) — Command architecture
- [React unsaved changes patterns](https://cloudscape.design/patterns/general/unsaved-changes/) — Cloudscape design system patterns for data loss prevention
- Codebase: `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs:497-521` — `validate_name()` path traversal prevention
- Codebase: `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx:6-53` — env var key validation and reserved key protection
- Codebase: `src/crosshook-native/src-tauri/capabilities/default.json` — Minimal capability surface
- Codebase: `src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs:254-311` — Backend sanitization of ProtonDB-sourced env var suggestions (`safe_env_var_suggestions`, `is_safe_env_key`, `is_safe_env_value`, `RESERVED_ENV_KEYS`)
- Codebase: `src/crosshook-native/src/utils/protondb.ts` — Frontend env var merge logic (conflict detection, no reserved-key re-check needed post-backend-sanitization)
