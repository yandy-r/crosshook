# Security Research: library-home

## Executive Summary

Security analysis of the library-home poster grid feature. The feature has a low-risk security profile: cover art is loaded through existing Rust IPC (not direct browser fetches), search is client-side React filtering, and favorites use the existing parameterized SQLite layer. No critical findings; a few advisory items around custom cover art paths and error message hygiene.

## Findings by Severity

| #    | Area                 | Finding                                                                                                                                                                                                                                           | Severity | Mitigation Path                                                                                                                       |
| ---- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| S-01 | Image Loading        | `custom_cover_art_path` passed directly to `convertFileSrc` without scope enforcement in capabilities                                                                                                                                             | WARNING  | Add `fs:allow-read-file` scope for user image dirs; validate extension                                                                |
| S-02 | Image Loading        | `img-src` CSP does not include Steam CDN — confirmed CDN is `cdn.cloudflare.steamstatic.com` (legacy `steamcdn-a.akamaihd.net` also in use)                                                                                                       | WARNING  | Images served via `asset://` locally so CDN not in `<img src>` — no CSP change needed unless a fallback renders CDN URLs directly     |
| S-03 | Image Loading        | ~~No MIME type validation on cached images~~ — **RESOLVED**: magic-byte validation already implemented in `game_images/client.rs`; JPEG/PNG/WebP accepted, SVG/HTML rejected; 5 MB hard cap and `safe_image_cache_path` canonicalization in place | RESOLVED | No action needed                                                                                                                      |
| S-04 | Input Validation     | Client-side search filter operates only on React-rendered text — no `dangerouslySetInnerHTML`                                                                                                                                                     | ADVISORY | Confirm no future use of `innerHTML` for profile names; document convention                                                           |
| S-05 | Input Validation     | Profile names originate from user-controlled TOML files; React default escaping protects display                                                                                                                                                  | ADVISORY | Enforce max-length and character allowlist on profile name at save time                                                               |
| S-06 | Tauri Security Model | Asset protocol scope is `$LOCALDATA/crosshook/cache/images/**` — custom paths outside this scope will 403                                                                                                                                         | WARNING  | Either validate `custom_cover_art_path` resolves inside a known safe dir, or broker local images via IPC                              |
| S-07 | Tauri Security Model | `fs:allow-read-file` permission is limited to cache dir — consistent with least privilege                                                                                                                                                         | ADVISORY | No change needed; document this intent                                                                                                |
| S-08 | SQLite Security      | `set_profile_favorite` and all image cache queries use `rusqlite` `params![]` macros — parameterized                                                                                                                                              | ADVISORY | No SQL injection risk; no change needed                                                                                               |
| S-09 | SQLite Security      | If search moves server-side in the future, profile name input must use parameterized queries                                                                                                                                                      | ADVISORY | Add note to implementation spec: never interpolate search strings into SQL                                                            |
| S-10 | Dependency Security  | Lazy-loading via native `loading="lazy"` or `IntersectionObserver` avoids third-party deps                                                                                                                                                        | ADVISORY | Prefer built-in browser lazy loading over npm packages to reduce attack surface                                                       |
| S-11 | Dependency Security  | No new high-risk npm packages required for the grid feature itself                                                                                                                                                                                | ADVISORY | If virtual scrolling is added, prefer `@tanstack/react-virtual` (audited, actively maintained)                                        |
| S-12 | IPC / New Command    | `profile_list_summaries` DTO returns `custom_cover_art_path` raw filesystem path to frontend over IPC                                                                                                                                             | WARNING  | Never render path as visible text; filter null bytes and `../` sequences in Rust before returning DTO                                 |
| S-13 | Credential Handling  | SteamGridDB API key (90-day rotating, 2FA-gated) must never be echoed back in any IPC response                                                                                                                                                    | WARNING  | Audit all `fetch_game_cover_art`-related IPC responses to confirm key is not included; key confirmed not logged or returned currently |

---

## 1. Image Loading Security

### 1.1 External CDN — Steam Cover Art

**Confidence**: High

Cover art is fetched by the Rust backend (`fetch_game_cover_art` → `download_and_cache_image`), written to `$LOCALDATA/crosshook/cache/images/`, and the cached local path is returned to the frontend. The frontend then calls `convertFileSrc(path)` to get an `asset://` URL.

**Key finding (S-02)**: The current `tauri.conf.json` CSP is:

```
default-src 'self'; script-src 'self'; img-src 'self' asset: http://asset.localhost
```

The primary Steam CDN is `https://cdn.cloudflare.steamstatic.com` (confirmed by api-researcher); the legacy Akamai URL `https://steamcdn-a.akamaihd.net` is also in use. Neither is in the current `img-src`. For the designed flow (cache-then-serve via `asset://`), no CSP change is required. If the implementation ever renders CDN `<img>` tags directly (e.g., as a fallback during cache miss), the CSP must be updated before shipping:

```json
"img-src": "'self' asset: http://asset.localhost https://cdn.cloudflare.steamstatic.com https://steamcdn-a.akamaihd.net"
```

**Threat model**: Images are fetched by the Rust backend, not the WebView — the CDN is contacted from the native process. This removes SSRF-from-WebView risk. Both CDN domains are Valve-owned and considered trusted first-party infrastructure.

### 1.2 Local File Images — `custom_cover_art_path`

**Confidence**: High (based on direct codebase review)

`custom_cover_art_path` is a user-controlled string stored in the profile TOML file (see `src/crosshook-native/crates/crosshook-core/src/profile/models.rs:193`). In `useGameCoverArt.ts:26`, when a `customCoverArtPath` is non-empty, it is passed directly to `convertFileSrc()` without any frontend validation.

**Path traversal risk (S-01, S-06)**: `convertFileSrc` converts a filesystem path to an `asset://localhost/...` URL. The asset protocol scope in `tauri.conf.json` is limited to `$LOCALDATA/crosshook/cache/images/**` and the corresponding `fs:allow-read-file` capability mirrors this scope. A path pointing **outside** this directory (e.g., a user manually editing their TOML to `/etc/passwd`) will receive a **403 Forbidden** response — the Tauri scope guard prevents the read.

However, the following conditions could expand exposure:

- If the asset protocol scope is ever widened (e.g., `$HOME/**`) to support arbitrary user image paths, path traversal becomes a real concern.
- The current design **silently fails** (returns `null`) for paths outside the allowed scope. This is secure but produces no user-visible error. The library-home UI must handle the fallback gracefully.

**Recommended mitigation**: Rather than expanding the asset protocol scope, broker custom cover art through an IPC command that validates the path resolves to an allowed directory (e.g., `$HOME/Pictures/**` or `$XDG_DATA_HOME/**`) and copies/symlinks it to the cache directory. This keeps the asset scope minimal.

### 1.3 MIME Type Validation — RESOLVED

**Confidence**: High (confirmed by api-researcher review of `game_images/client.rs`)

Magic-byte validation is already implemented in the Rust image cache layer:

- JPEG (`FF D8 FF`), PNG (`89 50 4E 47`), and WebP (`52 49 46 46 ... 57 45 42 50`) are accepted.
- SVG, HTML, and all other non-image content are rejected before writing to disk.
- A 5 MB hard cap is enforced per image.
- `safe_image_cache_path` validates `app_id` as decimal-only, filename as a single path component, and uses `canonicalize` + prefix assertion to prevent path escape.

S-03 is closed. No action required for the library-home feature.

---

## 2. Input Validation

### 2.1 Search Input — Client-Side Filtering

**Confidence**: High

Search filtering in the library-home page operates client-side on an array of profile names. The filter computes `profile.name.toLowerCase().includes(query.toLowerCase())` and passes matching profiles to React rendering. React's JSX auto-escaping converts all string values to text nodes — no `innerHTML` or `dangerouslySetInnerHTML` is involved in standard grid rendering.

**Assessment (S-04)**: Client-side-only search against locally loaded data presents no server-side injection risk. React's default escaping is the correct protection for display. There is no XSS risk as long as profile names are not interpolated into raw HTML.

If search is ever moved server-side (IPC command), the profile name query parameter must be sanitized at the IPC boundary and passed to SQLite via parameterized queries — never string-interpolated.

### 2.2 Profile Name Display — XSS via TOML

**Confidence**: High

Profile names originate from user-controlled TOML files. React renders them as text children (e.g., `<span>{profile.name}</span>`), which is safe by default. The risk arises only if:

1. A component uses `dangerouslySetInnerHTML` with a profile name — none currently do.
2. A profile name is used as an HTML `id`, `class`, or `href` attribute without sanitization.

For the library-home grid, profile names will appear in card titles, `aria-label` attributes, and potentially `data-*` attributes. All of these are safe when assigned as React props (React escapes attribute values).

**Recommended convention (S-05 ADVISORY)**: Enforce a max length (e.g., 128 chars) and a character allowlist (alphanumeric, spaces, hyphens, underscores, dots, parentheses) on profile names at save time in the Rust backend. This prevents edge-case attacks if a future component inadvertently mishandles a very long or specially crafted name.

---

## 3. Tauri Security Model

### 3.1 CSP Configuration

**Confidence**: High (reviewed `tauri.conf.json` directly)

Current CSP in `src/crosshook-native/src-tauri/tauri.conf.json:23`:

```json
"csp": "default-src 'self'; script-src 'self'; img-src 'self' asset: http://asset.localhost"
```

This is appropriately restrictive. The library-home feature needs to be implemented such that images are always served through the `asset:` protocol (from the local cache), not directly from the CDN. This is already how `useGameCoverArt.ts` works — the Rust backend downloads and caches images, the frontend only ever uses `convertFileSrc(cachedPath)`.

**If the implementation diverges** (e.g., shows CDN thumbnails during cache warmup), S-02 becomes a hard blocker — the CSP must be updated before shipping.

### 3.2 Asset Protocol Scope

**Confidence**: High (reviewed `capabilities/default.json` and `tauri.conf.json` directly)

The asset protocol is scoped to `$LOCALDATA/crosshook/cache/images/**`. The `fs:allow-read-file` capability mirrors this scope. This is correct least-privilege design.

**Gap**: `customCoverArtPath` can be any filesystem path (the user specifies it in the TOML). When `convertFileSrc` converts an out-of-scope path, Tauri returns a 403 — silently from the UI perspective. The library-home grid must degrade gracefully (show placeholder) rather than showing a broken image. This is S-06.

### 3.3 IPC Command Permissions

**Confidence**: High

The `profile_set_favorite` command exists and is registered (see `lib.rs:251`). The `fetch_game_cover_art` command is registered at `lib.rs`. Both commands are backend-only operations that:

- Use Tauri `State<>` injection for store access (no ambient state leakage)
- Return `Result<T, String>` — errors are stringified, not panicked to the frontend
- Require no additional capability permissions beyond `core:default`

No new IPC commands are inherently required for the library-home feature. Reusing existing commands (`profile_list`, `profile_set_favorite`, `fetch_game_cover_art`) keeps the IPC attack surface stable.

---

## 4. SQLite Security

### 4.1 Favorite Toggle — Parameterized Queries

**Confidence**: High (reviewed `collections.rs:169-186` directly)

```rust
conn.execute(
    "UPDATE profiles SET is_favorite = ?1, updated_at = ?2 \
     WHERE current_filename = ?3 AND deleted_at IS NULL",
    params![favorite as i32, now, profile_name],
)
```

This is correct. The profile name is bound as a parameter — no SQL injection risk (S-08).

### 4.2 Image Cache Queries

**Confidence**: High (reviewed `game_image_store.rs` directly)

All queries in `game_image_store.rs` use `params![]` macro with positional placeholders. The `upsert_game_image` function binds `steam_app_id`, `image_type`, `source`, `file_path`, `file_size`, `content_hash`, `mime_type`, `source_url`, and `expires_at` — all as parameters. No string interpolation occurs.

### 4.3 Database Configuration — Defense in Depth

**Confidence**: High (reviewed `db.rs` directly)

The DB connection is configured with:

- `PRAGMA secure_delete=ON` — overwrites deleted rows
- `PRAGMA foreign_keys=ON` — referential integrity
- Directory permissions `0o700`, file permissions `0o600`
- Symlink detection before opening (prevents symlink attacks on the DB file)

This is excellent existing security practice that the library-home feature inherits for free.

### 4.4 Future Risk: Server-Side Search

**Confidence**: Medium

If search is ever moved to an IPC command for performance (e.g., querying profiles table with a LIKE filter), the search string must be passed as a bound parameter:

```rust
// SAFE — parameterized
let query = format!("%{}%", search_term);
conn.query_map("SELECT name FROM profiles WHERE name LIKE ?1", params![query], ...)

// UNSAFE — never do this
conn.query_map(&format!("SELECT name FROM profiles WHERE name LIKE '%{}%'", search_term), ...)
```

This is a pre-emptive advisory (S-09), not a current risk.

---

## Dependency Security

### 5.1 Existing Dependencies — No New Risk

**Confidence**: High

The library-home grid feature can be implemented entirely with:

- React (existing) — grid layout via CSS Grid
- Existing `useGameCoverArt` hook — cover art loading
- Existing `profile_set_favorite` command — favorite toggle
- Native `loading="lazy"` attribute on `<img>` — lazy loading (browser-native, zero new dependencies)

No new npm packages are required for the core feature.

### 5.2 Virtual Scrolling (if needed for large libraries)

**Confidence**: Medium

If profile libraries grow large enough to warrant virtual scrolling (>200 profiles), a virtual list library would be needed. Security posture by candidate:

| Library                   | Last Audit                              | Known CVEs          | Recommendation      |
| ------------------------- | --------------------------------------- | ------------------- | ------------------- |
| `@tanstack/react-virtual` | Active, widely audited                  | None known          | Preferred if needed |
| `react-window`            | Maintained, slower updates              | None known          | Acceptable          |
| `react-lazyload`          | Inactive (no releases in 12+ months)    | None known (direct) | Avoid — abandoned   |
| `react-virtual` (legacy)  | Superseded by `@tanstack/react-virtual` | N/A                 | Avoid               |

**Recommendation (S-10, S-11)**: Start with CSS Grid and native `loading="lazy"`. Defer virtual scrolling until performance data shows it is needed. If added, use `@tanstack/react-virtual`.

### 5.3 Image Loading — No New Risk

**Confidence**: High

The `<img loading="lazy">` attribute is a WebKit/WebView-native feature — no JavaScript library required. This is the correct approach for a Tauri app and eliminates the entire class of risks associated with third-party lazy-loading libraries (prototype pollution, dependency supply chain, bundle bloat).

---

## 6. Secure Coding Guidelines

These patterns MUST be followed in the library-home implementation:

### 6.1 Image Sources

```tsx
// CORRECT — always use convertFileSrc for local paths
const url = convertFileSrc(cachedLocalPath);
<img src={url} loading="lazy" alt="Game cover art" />

// WRONG — never render CDN URLs directly in <img> without updating CSP
<img src={`https://steamcdn-a.akamaihd.net/steam/apps/${appId}/...`} />
```

### 6.2 Profile Name Rendering

```tsx
// CORRECT — React escapes by default
<span className="card-title">{profile.name}</span>

// WRONG — never use dangerouslySetInnerHTML with user-controlled strings
<div dangerouslySetInnerHTML={{ __html: profile.name }} />
```

### 6.3 Favorite Toggle IPC

```tsx
// CORRECT — invoke with typed parameters
await invoke('profile_set_favorite', { name: profile.name, favorite: !profile.isFavorite });

// WRONG — constructing dynamic command names from user input
await invoke(`profile_${action}_favorite`, { ... });
```

### 6.4 Error Messages

Do not expose filesystem paths in user-visible error messages. If a custom cover art path fails to load:

```tsx
// CORRECT — generic message
'Cover art unavailable. Check the path in profile settings.'
// WRONG — leaks filesystem structure
`Failed to load image from ${customCoverArtPath}: 403 Forbidden`;
```

### 6.5 `custom_cover_art_path` Handling

The current design correctly handles out-of-scope paths via a 403 (Tauri asset scope enforcement). The library-home grid should use the same `useGameCoverArt` hook pattern — no special handling needed for custom paths. The hook already returns `null` on failure, which maps to a placeholder card.

---

## 7. Trade-off Recommendations

### 7.1 Custom Cover Art Path — Scope Expansion vs. IPC Brokering

**Option A — Expand asset protocol scope** (e.g., `$HOME/Pictures/**`):

- Pro: Simple, no new IPC command
- Con: Expands the file system attack surface; a malicious TOML from a community source could read arbitrary user images

**Option B — IPC-brokered copy** (copy user image to `$LOCALDATA/crosshook/cache/images/` on first use):

- Pro: Asset scope stays minimal; validated path enters the cache
- Con: One-time copy; UX must handle "importing" the image

**Option C — Keep current behavior** (out-of-scope paths silently fail with placeholder):

- Pro: Zero scope change; secure by default
- Con: Users with custom cover art stored outside the cache dir will see placeholders in the library-home grid

**Recommendation**: For the initial library-home implementation, use Option C. Custom cover art is an edge case — most users will rely on Steam CDN / SteamGridDB images fetched by the backend. Address Option B as a follow-up if user feedback identifies it as painful.

### 7.2 CDN Fallback — Cache Miss Handling

During cache warm-up (first launch of library-home), some profiles may not have cached cover art. Options:

- **A**: Show placeholder card until the cache download completes (background IPC call) — no CSP change needed
- **B**: Show CDN URL directly during loading — requires CSP update

**Recommendation**: Use Option A. The `useGameCoverArt` hook already supports a `loading` state. Display a skeleton/placeholder while `fetch_game_cover_art` runs in the background. This keeps the CSP restrictive and avoids the direct CDN render risk.

---

## 8. Open Questions

1. **Virtual scrolling threshold**: At what profile count does DOM performance become unacceptable? Recommend testing with 100, 300, 500 profiles to determine whether `@tanstack/react-virtual` is needed.

2. **Community-imported profiles**: Can community-sourced profiles include arbitrary `custom_cover_art_path` values? If so, a path like `/home/user/.ssh/id_rsa` would produce a 403 (safe), but developers should confirm community import validation strips or ignores `custom_cover_art_path` from imported profiles.

3. **SteamGridDB cover art cache path**: **RESOLVED** — confirmed by api-researcher that SteamGridDB images are cached under the same `$LOCALDATA/crosshook/cache/images/**` path as Steam CDN images. The existing asset protocol scope covers SteamGridDB images without any changes.

4. **Card action error disclosure**: If `profile_set_favorite` returns an error (e.g., profile not found in metadata DB), what should the grid card show? Ensure the error message does not include raw database error strings.

5. **SteamGridDB rate limiting on first load**: The library-home page will trigger `fetch_game_cover_art` for every visible card with a Steam app ID on first load (before cache warms). This burst of concurrent SteamGridDB requests risks rate-limiting or banning the user's API key. Confirmed context: SteamGridDB API keys rotate every 90 days and require 2FA — a banned key creates a meaningful user-facing disruption. Staggered/queued fetching is a pre-ship requirement, not an optional optimization. See S-13 for key hygiene requirements.

---

## 9. `profile_list_summaries` IPC Command — Security Review

_Added after tech-design review (2026-04-01)_

The proposed `profile_list_summaries` command returns a slim DTO to the frontend:

```
{ name, game_name, steam_app_id, custom_cover_art_path }
```

### S-12 — `custom_cover_art_path` in IPC DTO (WARNING)

This is the first time a raw filesystem path from a profile TOML is transmitted to the frontend over IPC for display purposes. The path is user-controlled and may originate from community-imported profiles.

**Required mitigations before shipping:**

1. **Rust-side sanitization before returning the DTO**: Reject `custom_cover_art_path` values containing null bytes (`\0`), path traversal sequences (`../`, `..\`), or lengths exceeding 4096 chars. Return an empty string for rejected values — not an error.

2. **Frontend must treat the path as opaque**: `custom_cover_art_path` must only be passed to `convertFileSrc()`. It must never be rendered as visible text (card title, tooltip, `alt` attribute, error message, or console log in production builds).

3. **Community import validation**: Strip `custom_cover_art_path` from community-imported profiles at import time. Community tap authors cannot predict valid local paths on another user's machine — the field is meaningless for imports and a potential path disclosure vector.

**Remaining DTO fields** (`name`, `game_name`, `steam_app_id`) are non-sensitive identifiers with no path disclosure risk.

---

## 10. SteamGridDB API Key — Credential Hygiene

_Added after api-researcher review (2026-04-01)_

### S-13 — API Key Echo Risk (WARNING)

The SteamGridDB API key:

- Is stored in `SettingsStore` (TOML settings file on disk)
- Is read by the Rust layer as `Option<&str>` and passed to the SteamGridDB HTTP client
- Rotates every 90 days and requires 2FA to generate
- Is **not** currently logged or returned to the frontend (confirmed)

**Required action before shipping `fetch_game_cover_art` in the library-home context:**

Audit every IPC response shape that touches `fetch_game_cover_art` or any settings-related command to confirm the `steamgriddb_api_key` field is never included in a response payload. Specifically:

1. `fetch_game_cover_art` returns `Option<String>` (a local file path) — key is not in scope of the return type. Confirmed safe.
2. Any `load_settings` or `get_settings` IPC command that returns the full settings struct must redact `steamgriddb_api_key` from the response, or return a boolean `has_api_key` instead of the raw value.
3. Error responses from SteamGridDB HTTP failures must not include the API key in the error string passed back to the frontend.

**Why this matters now**: The library-home page will trigger cover art fetching for potentially all profiles on first load. If any error handling path leaks the API key into a frontend-visible error message or DevTools log, it becomes visible in screenshots and bug reports.

---

## Sources

- [Tauri v2 CSP Documentation](https://v2.tauri.app/security/csp/)
- [Tauri v2 Security Overview](https://v2.tauri.app/security/)
- [Tauri v2 Permissions and Capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri v2 Asset Protocol Discussion](https://github.com/tauri-apps/tauri/discussions/11498)
- [Radically Open Security — Tauri 2.0 Penetration Test Report (Aug 2024)](https://fossies.org/linux/tauri/audits/Radically_Open_Security-v2-report.pdf)
- [rusqlite Params Documentation](https://docs.rs/rusqlite/latest/rusqlite/trait.Params.html)
- [React XSS Prevention Guide — StackHawk](https://www.stackhawk.com/blog/react-xss-guide-examples-and-prevention/)
- [MIME Sniffing Security — Coalfire](https://coalfire.com/the-coalfire-blog/mime-sniffing-in-browsers-and-the-security)
- [OWASP XSS Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html)
- [react-lazy-load-image-component Snyk Audit](https://security.snyk.io/package/npm/react-lazy-load-image-component)
