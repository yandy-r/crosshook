# Security Research: Health Dashboard Page

**Date**: 2026-03-28
**Scope**: Health Dashboard as a new top-level tab in CrossHook (Tauri v2 + React 18)
**Attack Surface**: Local desktop app, no remote API, no auth, no network connectivity from dashboard

---

## Executive Summary

| Severity | Count | Finding                                                                       |
| -------- | ----- | ----------------------------------------------------------------------------- |
| CRITICAL | 0     | None                                                                          |
| WARNING  | 2     | XSS via profile name rendering; CSP missing style-src                         |
| ADVISORY | 3     | Search input length cap; error message leak hardening; CSP img-src tightening |

---

## Findings by Severity

### CRITICAL Findings

None identified.

**Rationale**: The dashboard is read-only, consumes existing IPC commands (`batch_validate_profiles`, `get_cached_health_snapshots`), makes no new filesystem calls, and runs in a sandboxed WebView with Tauri's capability model. The backend already sanitizes filesystem paths in `sanitize_display_path` (replaces `$HOME` with `~`) before they cross the IPC boundary. No path traversal, command injection, or remote-code-execution vectors exist in the proposed feature.

---

## WARNING Findings

### W-01 — XSS via Profile Name in Dashboard Table

**Severity**: WARNING

**Description**: Profile names are stored as user-provided strings in TOML files under `~/.config/crosshook/`. The health dashboard will render profile names in table cells, filter chips, and summary cards. If any rendering path uses `innerHTML`, `dangerouslySetInnerHTML`, or an unsanitized `title` attribute, a profile name containing `<script>alert(1)</script>` or `<img onerror=...>` could execute in the WebView.

**What the code does today**: `HealthBadge.tsx` renders `STATUS_LABEL[resolvedStatus]` (a fixed enum string — safe) and the `tooltip` prop via the HTML `title` attribute (safe, title is text-only). Existing pages use JSX interpolation (`{profileName}`) for profile names, which React escapes automatically.

**Risk for the new dashboard**: The dashboard adds a sortable/filterable table where profile names surface in more locations: column cells, filter pills, "recent failures" rows, and a search input placeholder. The risk is not in React interpolation (which escapes by default) but in:

1. Any component that falls back to `innerHTML` for performance or rich formatting.
2. Tooltip libraries or third-party table components that bypass React's escaping.
3. The `title` attribute on path-display `<span>` elements — safe in browsers but worth auditing.

**Mitigation**:

- Never use `dangerouslySetInnerHTML` for profile names or paths anywhere in the dashboard.
- Audit any new table/tooltip library introduced (see W-02 / Dependency Security section) for unsafe rendering.
- The existing JSX interpolation pattern (e.g., `<td>{profile.name}</td>`) is safe — standardize on it.
- Apply a `sanitizeProfileName` guard (simple string truncation, no HTML) at the data boundary when preparing table rows — not as a security control but as a defense-in-depth signal that the field is untrusted user input.

**Confidence**: High — based on direct code inspection of existing rendering patterns and Tauri WebView behavior.

---

### W-02 — CSP Missing `style-src` Directive

**Severity**: WARNING

**Description**: The current CSP in `tauri.conf.json` is:

```
default-src 'self'; script-src 'self'
```

This omits an explicit `style-src`. Per the CSP spec, when `default-src` is present and `style-src` is absent, `style-src` falls back to `default-src 'self'`. Inline styles (`style="..."` attributes, which the existing UI uses extensively — see `HealthBadge.tsx` lines 50, 66-73, 77-91) will be **blocked** by a strict CSP unless `'unsafe-inline'` is added to `style-src`.

**Current state**: Tauri's default WebView may not enforce this CSP strictly in development (devUrl points to `http://localhost:5173`), and the existing app works because Tauri's built-in WebView does not apply the CSP to the dev server in the same way as production. However, in the bundled AppImage the CSP is applied to the `asset://` protocol. If inline styles are blocked by a future tightening of the CSP, the dashboard could render as unstyled or broken.

**For the dashboard specifically**: The feature description calls for color-coded summary cards and trend arrows, which are likely implemented with inline style overrides (color, background, etc.). These will silently break under a stricter `style-src 'self'`.

**Mitigation**:

- Add `style-src 'self' 'unsafe-inline'` explicitly to acknowledge that inline styles are intentional.
- OR migrate inline style props to CSS custom property overrides and CSS classes so `'unsafe-inline'` is not required. This is the better long-term approach for a component library, but not required before shipping.
- Document the inline-style decision in `tauri.conf.json` with a comment.

**Confidence**: High — CSP fallback behavior is well-specified; inline style usage is confirmed in existing component code.

---

## ADVISORY Findings

### A-01 — No Maximum Length on Search/Filter Input

**Severity**: ADVISORY

**Description**: The profile table's search/filter text input has no defined maximum length. While the input is used only for client-side filtering (no IPC call is triggered from it — filtering happens in-memory in React), an extremely long input could cause:

- Minor UI jank if the filter regex is complex.
- Unexpected behavior if the filter value is ever persisted to `localStorage` or sent via IPC (neither is planned, but worth guarding against drift).

**Mitigation**: Add `maxLength={200}` (or similar) to the search `<input>`. This is a one-liner and prevents any future accidental persistence of an unbounded string.

**Confidence**: Medium — low impact in current design; worth doing as a hygiene item.

---

### A-02 — `HealthIssue.message` Contains Unsanitized Paths (WARNING-adjacent)

**Severity**: ADVISORY (elevated from initial assessment — gap confirmed by code inspection)

**Description**: `sanitize_issues()` in `commands/health.rs:43-51` applies `sanitize_display_path()` to `issue.path` only. `issue.message` is not sanitized. The core health check functions in `crates/crosshook-core/src/profile/health.rs` build `message` fields by embedding the raw path directly via `format!()`:

```rust
// health.rs:97
message: format!("Path does not exist: {path}"),
// health.rs:109
message: format!("Path is not accessible (permission denied): {path}"),
// health.rs:122 — err may itself embed the path
message: format!("Could not access path: {err}"),
```

This means `HealthIssue.message` crosses the IPC boundary with raw `$HOME`-prefixed paths. The same gap applies to top-level IPC error strings returned via `map_err(|e| e.to_string())`.

**Impact**: A local desktop app — the user is the only person who sees the UI. If the dashboard renders `issue.message` in detail panels (likely, as it is the human-readable description), raw home directory paths appear in the UI. Privacy concern is low for the user themselves; the risk is path exposure in bug report screenshots.

**Two mitigations — pick one**:

Option A (backend fix — preferred): Extend `sanitize_issues()` to also sanitize `issue.message`:

```rust
fn sanitize_issues(issues: Vec<HealthIssue>) -> Vec<HealthIssue> {
    issues.into_iter().map(|mut issue| {
        issue.path = sanitize_display_path(&issue.path);
        issue.message = sanitize_display_path(&issue.message);  // add this line
        issue
    }).collect()
}
```

Option B (frontend display policy): Display `issue.path` (already sanitized) as the primary path reference. Display `issue.message` as supplementary text and accept it may contain raw paths. Do not apply frontend path transformation (double-substitution risk).

Option A is the cleaner fix — one line in the backend, complete coverage, no frontend complexity.

**Confidence**: High — confirmed by reading `health.rs` message construction and the `sanitize_issues()` implementation directly.

---

### A-03 — CSP `img-src` Not Restricted

**Severity**: ADVISORY

**Description**: The current CSP has no explicit `img-src`. With `default-src 'self'`, image sources fall back to `'self'` only. Community profile taps could theoretically include profile names or issue messages that generate `<img>` tags if any rendering path ever uses `innerHTML` (guarded against by W-01). This is defense-in-depth against a W-01 regression.

**Mitigation**: Add `img-src 'self' data:` to the CSP to explicitly allow only local and data-URI images. This matches the existing icon usage and prevents out-of-band image exfiltration if XSS were ever introduced.

**Confidence**: Low — requires W-01 to be exploitable first; included for completeness.

---

## Authentication and Authorization

Tauri v2 uses a capability-based permission model. The health dashboard consumes three existing IPC commands:

| Command                       | Registered in `invoke_handler`? | Capability in `default.json`? |
| ----------------------------- | ------------------------------- | ----------------------------- |
| `batch_validate_profiles`     | Yes (`lib.rs:166`)              | Implicitly via `core:default` |
| `get_profile_health`          | Yes (`lib.rs:167`)              | Implicitly via `core:default` |
| `get_cached_health_snapshots` | Yes (`lib.rs:168`)              | Implicitly via `core:default` |

**Assessment**: No new permissions are required. The `core:default` capability grants access to all `invoke_handler`-registered commands from the `main` window. Since the dashboard is a new tab in the same `main` window, it inherits the existing permission set automatically.

**No authentication is required or applicable.** This is a local single-user desktop app. The WebView is sandboxed by the OS (the AppImage runs as the user, not root) and all data is already owned by the same user running the app.

**No authorization model is needed.** All profiles are the user's own files. The dashboard does not expose any admin or elevated operation.

---

## Data Protection

### Filesystem Path Exposure

Profile data includes full filesystem paths (`executable_path`, `compatdata_path`, `trainer.path`, Proton install paths). These are stored in TOML files under `~/.config/crosshook/` and are already accessible by the user. Rendering them in the dashboard does not increase exposure.

**Existing protection**: `sanitize_display_path` in `commands/shared.rs` replaces `$HOME` with `~` on all `HealthIssue.path` fields before IPC serialization. This is applied in `sanitize_report()` called from both `batch_validate_profiles` and `get_profile_health`.

**Gap**: The sanitization applies to `issues[*].path` but not `issues[*].message` or top-level IPC error strings (see A-02).

### SQL Injection — Confirmed Not Possible

All MetadataStore queries used by the health commands (`query_failure_trends`, `query_last_success_per_profile`, `query_total_launches_for_profiles`, `upsert_health_snapshot`, `load_health_snapshots`) use rusqlite's `params![]` macro with positional bind parameters (`?1`, `?2`, etc.). No string interpolation into SQL statements occurs anywhere in the health or metadata code paths. SQL injection is structurally impossible.

### Profile Name Sensitivity

Profile names are user-defined strings. They are not treated as secrets. The worst case is an inadvertent display of a game title or trainer name that the user would prefer to keep private in a screenshot. This is inherent to the app's purpose and not a meaningful threat to address.

### No New Data Persistence

The health dashboard is read-only and introduces no new write paths, no new IPC write commands. If `sessionStorage` is used for dashboard UI state (e.g., dismissed banners), it must store only boolean flag values keyed by fixed constant strings — following the existing pattern in `ProfilesPage.tsx` (`'crosshook.healthBannerDismissed'` → `'1'`). Never store profile names, paths, or health data in `sessionStorage`.

---

## Dependency Security

### Current Frontend Dependencies

```json
"@radix-ui/react-select": "^2.2.6"
"@radix-ui/react-tabs": "^1.1.13"
"@tauri-apps/api": "^2.0.0"
"@tauri-apps/plugin-dialog": "^2.0.0"
"@tauri-apps/plugin-fs": "^2.0.0"
"@tauri-apps/plugin-shell": "^2.0.0"
"react": "^18.3.1"
"react-dom": "^18.3.1"
"react-resizable-panels": "^4.7.6"
```

**Assessment**: The feature description states "no new backend work." One new frontend dependency has been proposed and evaluated.

### Dependency Risk Recommendation

**`@tanstack/react-table` v8.21.3 is acceptable.** Verified via `npm install --dry-run` on the project:

- Adds exactly 2 packages: `@tanstack/react-table` + `@tanstack/table-core`. Zero transitive deps beyond React.
- `npm audit` shows 0 new vulnerabilities introduced into the project.
- No CVEs found for either package. The December 2025 React RSC CVEs (CVE-2025-55182 et al.) affect React Server Components + Next.js — not applicable to a Tauri desktop app using React 18 client-side only.
- It is a **headless** library: no DOM manipulation, no rendering of its own. All rendering is done by the consumer via JSX, making XSS auditing straightforward and keeping W-01 compliance fully in the implementer's hands.

**Condition for safe use**: Column cell renderers must use JSX interpolation only — never `innerHTML` or `dangerouslySetInnerHTML` with values sourced from `cell.getValue()`.

**Confidence**: High — verified via dry-run install and npm audit on the actual project.

---

## Input Validation and Injection

### Search / Filter Input

The search input filters the profile health table client-side. It is not sent to the backend via IPC. Threat model for this input:

| Attack          | Mechanism                                | Risk                                               | Verdict                                                                         |
| --------------- | ---------------------------------------- | -------------------------------------------------- | ------------------------------------------------------------------------------- |
| XSS             | Search value rendered via `innerHTML`    | Only if React's escape is bypassed                 | Not a risk with JSX interpolation                                               |
| ReDoS           | Complex regex built from search string   | If `new RegExp(searchTerm)` is used                | Use `String.prototype.includes()` or `filter` on `.toLowerCase()`, not `RegExp` |
| IPC injection   | Search value sent to Rust commands       | Dashboard is read-only; search is client-side only | No risk in current design                                                       |
| State pollution | Search state persisted to `localStorage` | Not planned; if added later, limit length (A-01)   | Low risk                                                                        |

**Recommendation**: Implement table filtering with `value.toLowerCase().includes(term.toLowerCase())` — not `new RegExp(term)`. This avoids ReDoS from inputs like `(((` and is simpler.

### Profile Name Rendering

Covered under W-01. Summary: JSX interpolation is safe; avoid `innerHTML`/`dangerouslySetInnerHTML`.

### Path Display

`HealthIssue.path` arrives pre-sanitized from the backend (`~`-prefixed). Display as plain text in a `<code>` or `<span>` element via JSX interpolation. Do not use `innerHTML` for path rendering.

---

## Infrastructure and Configuration Security

### Tauri CSP

Current CSP: `default-src 'self'; script-src 'self'`

**Gaps**:

- No explicit `style-src` (see W-02)
- No explicit `img-src` (see A-03)
- No explicit `connect-src` (acceptable — no network requests from dashboard)
- No explicit `font-src` (acceptable — fonts are bundled in AppImage)

**Recommended CSP** (adding minimum needed for dashboard):

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"
```

### Tauri Capability Model

The `default.json` capability grants `core:default` and `dialog:default` to the `main` window. No new capabilities are needed for the health dashboard.

`tauri_plugin_fs` and `tauri_plugin_shell` are registered but their capability scope is not expanded by this feature. The dashboard does not call any `fs::*` or `shell::*` commands.

### WebView Sandbox

Tauri v2 on Linux uses WebKitGTK. The WebView runs in a sandboxed process. The AppImage runs as the local user — no elevated privileges are involved. This is the appropriate security posture for a local desktop app.

---

## Secure Coding Guidelines

The following patterns should be standardized for the health dashboard implementation:

### 1. Profile Name Rendering

```tsx
// CORRECT — React escapes automatically
<td>{profile.name}</td>
<span title={profile.name}>{profile.name}</span>

// WRONG — never do this
<td dangerouslySetInnerHTML={{ __html: profile.name }} />
```

### 2. Client-Side Filtering (no RegExp from user input)

```tsx
// CORRECT — substring match, no ReDoS risk
const filtered = profiles.filter((p) => p.name.toLowerCase().includes(searchTerm.toLowerCase()));

// WRONG — user-controlled regex, ReDoS risk
const re = new RegExp(searchTerm, 'i');
const filtered = profiles.filter((p) => re.test(p.name));
```

### 3. Error Display (avoid leaking internal paths)

```tsx
// CORRECT — generic message, full error in console
if (error) {
  console.error('[HealthDashboard] scan failed:', error);
  return <ErrorBanner>Health scan failed. Check app logs for details.</ErrorBanner>;
}

// ACCEPTABLE (paths are sanitized before IPC, but still verbose)
if (error) {
  return <ErrorBanner>{error}</ErrorBanner>; // error string may contain raw paths
}
```

### 4. Search Input Length Cap

```tsx
<input
  type="search"
  value={searchTerm}
  onChange={(e) => setSearchTerm(e.target.value)}
  maxLength={200}
  placeholder="Filter profiles..."
/>
```

### 5. Path Display

```tsx
// CORRECT — paths arrive pre-sanitized (~/...) from backend; display as-received
<code className="crosshook-path">{issue.path}</code>

// The backend sanitize_display_path() already replaces $HOME with ~.
// Do NOT apply additional frontend path transformation — double substitution
// would corrupt paths that legitimately contain ~/. Trust the backend.
```

---

## Trade-off Recommendations

| Decision             | Recommended Choice                               | Rationale                                                             |
| -------------------- | ------------------------------------------------ | --------------------------------------------------------------------- |
| Table implementation | Hand-rolled `<table>` with `useMemo` sort/filter | Avoids new dependency; profile counts are small                       |
| Error display        | Generic message + console.error                  | Avoids edge case of unsanitized error strings in UI                   |
| Inline styles        | Keep for now; document in CSP                    | Migration to CSS classes is worth doing as a follow-up, not a blocker |
| Search filtering     | `String.includes()`, not `RegExp`                | Simpler and avoids ReDoS                                              |
| `style-src` in CSP   | Add `'unsafe-inline'` explicitly                 | Acknowledges existing code pattern; prevents silent style breakage    |

---

## Open Questions

1. **Will the dashboard introduce any `localStorage` persistence** (e.g., remembering the last sort column or active filter)? If yes, apply length limits to any stored string and do not store profile names directly as keys.

2. **Will any community-sourced data appear in the dashboard?** The feature description mentions "community import health." If community profile names are rendered, they pass through the same trust level as local profile names (user-imported, same risk). No additional treatment needed beyond W-01 compliance.

3. **Is there a plan to add a "download report" or "copy to clipboard" feature?** If health data is exported to clipboard or file, ensure paths are sanitized in the export (apply the same `~` substitution). This is not in scope for the initial feature but worth noting.

4. **Does the "recent failures panel" show launch log file paths or only counts?** If log paths are displayed, verify they also pass through `sanitize_display_path`.

---

## Sources

All findings are based on direct code inspection of the CrossHook repository. No external security databases were queried because:

- No new external dependencies are proposed.
- The identified risks (XSS via profile names, CSP inline style gaps) are well-understood web security patterns that do not require CVE research.
- Tauri v2's security model is documented in its official capability system, which was verified against the codebase.

Relevant code locations:

- CSP configuration: `src/crosshook-native/src-tauri/tauri.conf.json:23`
- Tauri capabilities: `src/crosshook-native/src-tauri/capabilities/default.json`
- Path sanitization: `src/crosshook-native/src-tauri/src/commands/shared.rs:20-33`
- Health IPC command registration: `src/crosshook-native/src-tauri/src/lib.rs:166-168`
- Health commands with sanitization: `src/crosshook-native/src-tauri/src/commands/health.rs:43-58`
- Existing safe rendering pattern: `src/crosshook-native/src/components/HealthBadge.tsx`
- Health types: `src/crosshook-native/src/types/health.ts`
