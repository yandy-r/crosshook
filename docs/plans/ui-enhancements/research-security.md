# Security Research: UI Enhancements (Profiles Page Restructuring + Game Metadata / Cover Art)

**Feature**: Restructure the Profiles page Advanced section AND add game metadata / cover art fetching via Steam Store API and SteamGridDB (issue #52).
**Scope**: UI restructuring (pass 1) + external API calls, binary image downloads, SQLite cache table, and webview image rendering (pass 2).
**Analyst**: security-researcher
**Date (pass 1)**: 2026-03-31
**Date (pass 2)**: 2026-04-01

---

## Executive Summary

**Pass 1 (UI-only restructuring)** remained low risk. No new attack surface. Pass 2 significantly expands the risk surface by adding external HTTP requests, binary file I/O to disk, a new SQLite table, and rendering externally-sourced images in the Tauri webview.

The revised **overall risk level is MEDIUM**. No single finding is a hard blocker, but three findings (I1, I2, K1) are WARNINGs that must be addressed before shipping. The remaining findings are ADVISORYs that follow secure-by-default practices and are safe to defer.

CrossHook remains a **native Linux desktop app** (Tauri v2, single authenticated user). It is not a multi-user web application. The threat model is: a malicious Steam CDN or SteamGridDB response attempting to exfiltrate data or execute code within the local Tauri webview. An attacker would need to compromise an upstream CDN or perform a MITM attack — plausible only on insecure networks.

Existing security foundations remain solid (see pass 1 findings W1, W3 below). The new attack surface is manageable with the mitigations described.

---

## Findings by Severity

### CRITICAL — Hard Stop

| #   | Finding              | Location | Rationale                                                                                  |
| --- | -------------------- | -------- | ------------------------------------------------------------------------------------------ |
| —   | No critical findings | —        | No single finding is an unconditional blocker given the desktop / single-user threat model |

### WARNING — Must Address

| #   | Finding                                                                                                                                                                                                                                                                                                           | Location                                                                    | Rationale                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| W1  | Unsaved changes lost on tab switch — **partially mitigated** by props-only section decomposition; residual risk only for `CustomEnvironmentVariablesSection` which holds local `rows` draft state                                                                                                                 | `useProfile.ts:461`, `CustomEnvironmentVariablesSection.tsx`                | All proposed section components are stateless props-renderers except `EnvVarsSection`. That component must be CSS show/hide (not conditionally unmounted) on tab switch. All other sections can safely unmount.                                                                                                                                                                                                                                                                                   |
| W3  | `injection.dll_paths` and `injection.inject_on_launch` fields are present in `GameProfile` but intentionally not exposed in the UI — the restructuring must not accidentally surface them                                                                                                                         | `src/crosshook-native/src/types/profile.ts:98-100`, `useProfile.ts:424-426` | These fields are managed by the install/migration pipeline, not user-facing forms. The community export sanitizer explicitly clears them (`exchange.rs:259`). Exposing DLL path inputs in a reorganized form would reintroduce a removed capability. Keep `injection.*` absent from all form sections.                                                                                                                                                                                            |
| I1  | **SVG downloads must be rejected at the Rust layer** — SVG files served from Steam CDN or SteamGridDB may contain embedded `<script>` tags, `<foreignObject>` XHTML injection, or XXE entity expansion. Rendering an SVG from `asset://` in the Tauri webview executes embedded JavaScript in the webview context | Image download handler (to be implemented)                                  | Steam CDN headers (`header.jpg`, `capsule_616x353.jpg`) are always JPEG; SteamGridDB can return PNG, JPEG, or WEBP. SVG is not a valid cover art format but could be served if SteamGridDB allows user-uploaded content. Reject any response whose magic bytes or `Content-Type` indicates SVG (`image/svg+xml`). Use the `infer` crate for magic-byte detection — do not rely solely on the URL extension or the `Content-Type` response header, both of which are attacker-controlled.          |
| I2  | **Path construction for the image cache directory must use `canonicalize` + prefix assertion** — a malicious URL component in `source_url` or a crafted `app_id` could cause the download handler to write a file outside `~/.local/share/crosshook/cache/images/`                                                | Image download handler (to be implemented)                                  | The download handler builds a file path from `{cache_dir}/{steam_app_id}/{filename}`. `steam_app_id` is a numeric field and should be validated as a pure decimal integer. The final resolved path must start with the expected base directory. Use `std::fs::canonicalize` on the base dir before joining, then assert the joined path starts with that canonical base. See mitigation pattern below.                                                                                            |
| K1  | **SteamGridDB API key stored in plaintext `settings.toml`** — if the user's home directory or dotfiles are shared/committed/synced (e.g. dotfiles repo, backup service with improper exclusions), the key is exposed                                                                                              | `AppSettingsData` in settings.toml                                          | The key grants full read access to SteamGridDB on behalf of the user's account. Loss of the key requires manual revocation on SteamGridDB's website. The OS Secret Service (via the `keyring` crate) is the correct storage location for user secrets on Linux desktops. However, Secret Service availability varies across minimal desktop environments and headless setups, so plaintext fallback with a clear UX warning is acceptable as an initial implementation. See migration path below. |

### ADVISORY — Best Practice

| #   | Finding                                                                                                                                                                                                                                                                                                                                                                                                | Location                                        | Rationale                                                                                                                                                                                                                                                                                                                         |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1  | File path inputs have no client-side traversal indicator                                                                                                                                                                                                                                                                                                                                               | `ProfileFormSections.tsx:124-163`               | Architecturally correct. Display inline feedback from backend validation results.                                                                                                                                                                                                                                                 |
| A2  | Environment variable values have no length limit client-side                                                                                                                                                                                                                                                                                                                                           | `CustomEnvironmentVariablesSection.tsx:181-192` | Self-inflicted in a single-user app. Soft character limit is a usability improvement.                                                                                                                                                                                                                                             |
| A3  | ~~Dependency risk if a tab library is added~~ — **Resolved**: Radix UI tabs already in `package.json`.                                                                                                                                                                                                                                                                                                 | `package.json`                                  | Zero new dependency risk.                                                                                                                                                                                                                                                                                                         |
| I3  | **MIME type validation should be defense-in-depth (magic bytes + allowlist)** — use the `infer` crate to detect image type from the first 12–16 bytes after download. Allowlist: `image/jpeg`, `image/png`, `image/webp`. Reject everything else, including `image/gif` and `image/bmp` (no cover art use case).                                                                                       | Image download handler                          | Steam CDN URLs return JPEG/PNG/WEBP. SteamGridDB API docs confirm same formats. Rejecting GIF eliminates the animated-GIF memory amplification attack class (decompression bomb).                                                                                                                                                 |
| I4  | **Image file size cap before write** — reject downloads that exceed a reasonable upper bound (recommended: 5 MB) before writing to disk. The `Content-Length` response header can be read before streaming the body. If absent, accumulate bytes in a bounded buffer and abort if exceeded.                                                                                                            | Image download handler                          | A single pathological 500 MB "image" would fill `/home` on constrained systems (Steam Deck). Steam capsule images are under 300 KB; SteamGridDB grids are under 1 MB. A 5 MB cap is conservative.                                                                                                                                 |
| I5  | **TOCTOU between download and render is low risk in this architecture** — the Rust backend writes the file, then the frontend reads it via `asset://`. Because the backend controls the directory and the webview cannot write to the cache directory, the race window is narrow. Standard defense: stat the file after write and verify size and MIME type before returning the path to the frontend. | Image download handler                          | The symlink attack vector is similarly low risk because `~/.local/share/crosshook/cache/images/` is under the user's own home — an attacker with access to that directory already has full user-level access. Record the SHA-256 checksum in `game_image_cache` at download time and verify on serve.                             |
| I6  | **Cache size growth** — no explicit cap is defined for the `game_image_cache` table or the `~/.local/share/crosshook/cache/images/` directory. An app fetching cover art for 1000+ games could accumulate several GB.                                                                                                                                                                                  | `game_image_cache` table (to be implemented)    | Enforce a TTL on `game_image_cache` entries (recommended: 30 days without access). On TTL expiry, delete the file and the DB row. Provide a "Clear image cache" action in Settings. Track per-entry `last_accessed_at` to enable LRU eviction if total cache size exceeds a configurable threshold (recommended default: 500 MB). |
| I7  | **Steam Store API response injection** — `store.steampowered.com/api/appdetails` returns JSON with fields like `name`, `short_description`, and `header_image`. If any metadata field is rendered with `dangerouslySetInnerHTML`, a compromised or spoofed API response could inject HTML/JS into the webview.                                                                                         | Frontend metadata rendering (to be implemented) | Use JSX text nodes for all metadata display. Never pass API-sourced strings to `dangerouslySetInnerHTML`. This is already a CLAUDE.md project rule — flag any deviation in code review.                                                                                                                                           |
| C1  | **Tauri capabilities require `assetProtocol` expansion** — to render locally cached images via `asset://`, `tauri.conf.json` must enable `assetProtocol` with an explicit scope pattern for the cache directory. The current capability surface (`core:default`, `dialog:default`, `shell:allow-open`) does not include this.                                                                          | `src-tauri/capabilities/default.json`           | The scope pattern must be as narrow as possible: `["$HOME/.local/share/crosshook/cache/images/**"]` or the Tauri-idiomatic `"$LOCALDATA/cache/images/**"`. Do not use wildcard `*/**` which grants the webview read access to all user files. See configuration pattern below.                                                    |
| C2  | **CSP `img-src` expansion required** — the current CSP does not include `asset:` or `http://asset.localhost`. Adding these is required to load cached images but must be scoped to `img-src` only, not `default-src`.                                                                                                                                                                                  | `tauri.conf.json` security.csp                  | Recommended addition: `"img-src": "'self' asset: http://asset.localhost"`. Do not add `blob:` or `data:` unless explicitly needed — data URIs have been used as XSS vectors.                                                                                                                                                      |
| C3  | **`shell:allow-open` URL allowlist needs expansion for SteamGridDB** — the current `shell:allow-open` permission only allows opening `https://www.protondb.com/**`. If the UI adds an "Open on SteamGridDB" external link, the allowlist must be updated.                                                                                                                                              | `src-tauri/capabilities/default.json:11`        | Add `https://www.steamgriddb.com/**` to the `shell:allow-open` allow list at the time the external link feature is added. Do not pre-emptively add Steam CDN domains — the CDN is used for image downloads in the Rust backend, not `shell:open-url`.                                                                             |

---

## Image Download Security

### W1 / I1: SVG Rejection and MIME Type Enforcement

SVG is the primary format-level risk for Tauri webview rendering. Unlike rasterized formats, SVG files contain XML that the browser engine executes. An SVG loaded via `asset://` in the webview is subject to normal browser parsing — including `<script>` execution, `<foreignObject>` XHTML injection, and CSS `url()` references. This is distinct from an SVG rendered inside an `<img>` tag, where browsers sandbox script execution.

**Recommended mitigation pattern (Rust download handler):**

```rust
use infer::Infer;

const ALLOWED_IMAGE_MIMES: &[&str] = &["image/jpeg", "image/png", "image/webp"];
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024; // 5 MB

fn validate_image_bytes(bytes: &[u8]) -> Result<(), ImageDownloadError> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(ImageDownloadError::TooLarge);
    }
    let infer = Infer::new();
    let mime = infer.get(bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");
    if !ALLOWED_IMAGE_MIMES.contains(&mime) {
        return Err(ImageDownloadError::ForbiddenFormat(mime.to_string()));
    }
    Ok(())
}
```

Call `validate_image_bytes` on the **full downloaded body** before writing any bytes to disk. Do not rely on `Content-Type` from the HTTP response header alone.

**Confidence**: High — `infer` crate magic-byte detection is the standard Rust approach. SVG-in-webview XSS is a documented class of vulnerability in Electron/Tauri desktop apps.

### I2: Path Traversal Mitigation

The image cache path is constructed as `{base_dir}/{steam_app_id}/{filename}`. `steam_app_id` originates from user input (profile settings) and from API responses.

**Recommended mitigation pattern:**

```rust
fn safe_image_cache_path(
    base_dir: &Path,
    app_id: &str,
    filename: &str,
) -> Result<PathBuf, ImageDownloadError> {
    // app_id must be a pure decimal integer — no slashes, no dots
    if !app_id.chars().all(|c| c.is_ascii_digit()) || app_id.is_empty() {
        return Err(ImageDownloadError::InvalidAppId);
    }
    // filename must be a safe basename — no path separators
    let fname = Path::new(filename);
    if fname.components().count() != 1 {
        return Err(ImageDownloadError::InvalidFilename);
    }
    // Resolve canonical base, then assert prefix
    let canonical_base = std::fs::canonicalize(base_dir)?;
    let joined = canonical_base.join(app_id).join(filename);
    // joined is not yet canonical (subdirs may not exist); strip to parent
    let parent = joined.parent().ok_or(ImageDownloadError::InvalidPath)?;
    std::fs::create_dir_all(parent)?;
    let canonical_parent = std::fs::canonicalize(parent)?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(ImageDownloadError::PathEscaped);
    }
    Ok(joined)
}
```

This pattern is consistent with the existing `validate_name()` in `toml_store.rs` which gates all profile filesystem operations.

**Confidence**: High — path traversal via cache directory construction is a well-documented vulnerability class. The `canonicalize` + prefix-assert pattern is the standard Rust defense.

---

## API Key Management

### K1: SteamGridDB API Key in Plaintext settings.toml

**Current state**: The API key will live at `settings.toml` under `AppSettingsData`. This is the same mechanism used for all other user preferences. On Linux, `settings.toml` is likely at `~/.config/crosshook/settings.toml` or `~/.local/share/crosshook/settings.toml`. It is not encrypted.

**Risk factors**:

- Dotfiles backup/sync services (Mackup, chezmoi, bare git repos) frequently include `~/.config/**` in their scope, accidentally committing API keys
- The SteamGridDB key grants read access to the API on behalf of the user's account — it can be used to scrape data or inflate usage metrics
- SteamGridDB does not currently charge for API usage, so the financial impact is low; the primary risk is key revocation inconvenience

**Migration path toward OS keyring** (recommended for a future phase, not a blocker):

The `keyring` crate (v3.6+ with `linux-native-sync-persistent` feature) provides cross-desktop Linux access to the Secret Service API (GNOME Keyring, KWallet). Usage:

```rust
use keyring::Entry;

fn store_api_key(key: &str) -> keyring::Result<()> {
    let entry = Entry::new("crosshook", "steamgriddb-api-key")?;
    entry.set_password(key)
}

fn load_api_key() -> keyring::Result<String> {
    let entry = Entry::new("crosshook", "steamgriddb-api-key")?;
    entry.get_password()
}
```

**Recommended initial implementation** (acceptable for v1):

- Store key in `settings.toml` with a `# Note: this key is stored in plaintext` comment
- In the Settings UI, display a warning adjacent to the API key input: "This key is stored in plain text. Do not put it in version-controlled dotfiles."
- Do not log the API key value anywhere (Tauri tracing, frontend console)

**Future phase**: Migrate to `keyring` crate with `settings.toml` as fallback for environments where Secret Service is unavailable (headless, minimal DEs). This follows the same pattern Flatpak apps use for secrets management on Linux.

**Confidence**: Medium — OS keyring availability is inconsistent on Linux (not all DEs run a Secret Service daemon). Plaintext with UX warning is a pragmatic initial approach.

---

## External API Trust

### Steam Store API (store.steampowered.com)

**Authentication**: None (public endpoint, no API key). HTTPS-only.
**Response fields at risk**: `name`, `short_description`, `header_image` URL.
**Injection risk**: Low, provided the frontend renders metadata fields as JSX text nodes. Any call to `dangerouslySetInnerHTML` with API-sourced data is a hard NO.
**URL validation**: `header_image` is a URL that will be downloaded. This URL must be validated: it must start with `https://cdn.cloudflare.steamstatic.com/` or `https://steamcdn-a.akamaihd.net/`. Any other domain in the `header_image` field from an API response should be rejected — this prevents a compromised API endpoint from redirecting downloads to attacker-controlled servers.

**Confidence**: High — Steam CDN URL pattern is stable and well-documented.

### SteamGridDB API (api.steamgriddb.com)

**Authentication**: Bearer token (user's API key in Authorization header). HTTPS-only.
**Response fields at risk**: `url` field pointing to image download location. Should be hosted on `img.steamgriddb.com` (Amazon S3 CDN).
**URL validation**: The `url` field returned by the API should be validated before download: it must begin with `https://cdn2.steamgriddb.com/` or `https://img.steamgriddb.com/`. Reject any `url` that points to a different domain.
**API key leakage via logs**: Do not log the Authorization header or the API key value in any tracing span. Use `#[tracing::instrument(skip(api_key))]` or equivalent to prevent accidental key inclusion in log output.

**Confidence**: Medium — SteamGridDB is community-operated. The URL pattern is consistent in practice but not contractually guaranteed to remain stable.

### MITM / HTTPS enforcement

Both Steam Store API and SteamGridDB API are HTTPS-only. The `reqwest` client (already used in `protondb/client.rs`) validates TLS certificates by default via `rustls` or the system TLS stack. Do not call `.danger_accept_invalid_certs(true)` on the `ClientBuilder`. HTTPS pinning is not required for this threat model (single-user desktop, not a financial application).

SteamGridDB images are served from S3 CDN over HTTPS. Steam CDN uses `cdn.cloudflare.steamstatic.com` / Akamai. Both validate correctly. No additional TLS configuration is needed beyond the reqwest defaults.

**Confidence**: High — reqwest TLS defaults are sound. MITM on HTTPS requires a CA compromise, which is out of scope for this threat model.

---

## Cache Security

### I5 / I6: Cache Integrity and Size Controls

**Checksum verification**: The `game_image_cache` SQLite table should store a SHA-256 checksum (hex string) of each downloaded image at insert time. On serve (before returning the file path to the frontend), verify the on-disk file's checksum matches the stored value. A mismatch indicates either filesystem corruption or external modification of the cache file.

```rust
use sha2::{Sha256, Digest};

fn compute_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
```

**Cache poisoning threat model**: Because the cache directory is under the user's own `~/.local/share/crosshook/`, the only party who can write to it is the user themselves or a process running as the same user. Cache poisoning by an external actor requires prior code execution — at which point the attacker has already won. Checksum verification is therefore a **defense against filesystem corruption and accidental overwrite**, not a strong security control.

**Disk exhaustion**: Without a cap, fetching cover art for large game libraries could accumulate GBs. Recommended controls:

- Per-entry TTL: 30 days (`expires_at` in `game_image_cache`)
- `last_accessed_at` column updated on cache hit — enables LRU eviction
- Configurable max total cache size (default 500 MB); evict LRU entries when exceeded
- "Clear image cache" action in Settings (analogous to browser cache clearing)

**Stale cache**: The existing `external_cache_entries` TTL mechanism from ProtonDB should be replicated in `game_image_cache`. Stale entries serve the wrong art if a game's cover is updated — no security risk, but a correctness issue.

**Confidence**: Medium — disk exhaustion is real on constrained devices (Steam Deck default ~512 GB storage with game installs). The 500 MB default is conservative and can be user-configurable.

---

## Webview Image Rendering

### C1 / C2: Asset Protocol and CSP Configuration

To render locally cached images in the Tauri webview, `tauri.conf.json` must be updated. **This is a configuration change with a security impact** — it expands the set of local filesystem paths the webview can read.

**Required configuration additions:**

```json
"security": {
  "csp": {
    "default-src": "'self' ipc: http://ipc.localhost",
    "img-src": "'self' asset: http://asset.localhost"
  },
  "assetProtocol": {
    "enable": true,
    "scope": [
      "$LOCALDATA/cache/images/**"
    ]
  }
}
```

**Security constraints**:

- The `scope` must be as narrow as possible. `$LOCALDATA` resolves to `~/.local/share/crosshook/` on Linux. The pattern `$LOCALDATA/cache/images/**` limits webview read access to the image cache subdirectory only.
- Do not use `"**/.local/share/**/*"` or `"$HOME/**"` — these grant the webview read access to the entire home directory.
- `img-src` must not include `data:` or `blob:` unless specifically needed. Data URIs bypass the asset protocol scope.
- Note that `'self'` in Tauri's CSP context refers to bundled app assets, not user files. The `asset:` scheme is the mechanism for accessing user-filesystem files.

**Rendering pattern (frontend):**

```typescript
import { convertFileSrc } from '@tauri-apps/api/core';

// Convert local path to asset:// URL safe for use in <img src>
const assetUrl = convertFileSrc('/home/user/.local/share/crosshook/cache/images/440/header.jpg');
// -> "asset://localhost/%2Fhome%2Fuser%2F.local%2Fshare%2Fcrosshook%2Fcache%2Fimages%2F440%2Fheader.jpg"
```

The path returned to the frontend from the IPC command should be absolute. The frontend converts it to an asset URL using `convertFileSrc`. Never pass raw `file://` URLs — the webview CSP does not include `file:` in `img-src`, which is intentional.

**Confidence**: High — asset protocol + `convertFileSrc` is the documented Tauri v2 pattern for displaying local files. The scope constraint is documented behavior.

---

## State Management Security (Pass 1, Preserved)

**Current model**: Profile state is centralized in `useProfile` hook (exposed via `ProfileContext`). The `dirty` flag is set when `updateProfile` is called and cleared on save or profile switch. Local-only UI state (env var rows) is buffered in `CustomEnvironmentVariablesSection` and flushed to context via `onUpdateProfile` on every change.

**Tab navigation risk**: If the restructuring introduces sub-tabs that conditionally render form sections, React will unmount components that go out of view. `CustomEnvironmentVariablesSection` holds local `rows` state synchronized from `customEnvVars` via a `useEffect`. If this component unmounts mid-edit (tab switch before blur), the `applyRows` call on the last keystroke may not have fired, and a partially entered row will be lost on unmount.

**Recommendation (W1 mitigation)**:

- Prefer `display: none` / CSS visibility toggling over conditional rendering so components stay mounted and state is preserved.
- If unmount is unavoidable, add a `useEffect` cleanup in `CustomEnvironmentVariablesSection` that calls `applyRows(rows)` on unmount to flush current draft state to context.
- The `dirty` indicator in `ProfileActions` already covers the top-level save state.

**Cross-profile data leakage**: Not a concern. Profile state is always loaded fresh from the backend on profile selection.

---

## Input Validation (Pass 1, Preserved and Extended)

**Profile names**: Fully validated at `crosshook-core::profile::validate_name()`. Path traversal via profile name is not possible.

**ProtonDB-sourced environment variables**: Backend sanitization in `aggregation.rs:safe_env_var_suggestions()` (lines 254–311) applies key allowlist, value character restrictions, and reserved key removal before crossing the IPC boundary. Sound and defense-in-depth adequate.

**Steam App ID (new)**: `steam_app_id` values used in image cache path construction must be validated as pure decimal integers in the Rust backend before any filesystem operation. Do not rely on profile-level validation; validate at the image download call site.

**Metadata fields from external APIs (new)**: `name`, `short_description`, and similar fields sourced from Steam Store API or SteamGridDB must be treated as untrusted strings. Render them as JSX text nodes. Do not interpolate into `dangerouslySetInnerHTML` or `innerHTML` anywhere in the metadata display components.

---

## Tauri IPC Security

**Current capability surface** (`capabilities/default.json`):

```json
{
  "permissions": [
    "core:default",
    "dialog:default",
    { "identifier": "shell:allow-open", "allow": [{ "url": "https://www.protondb.com/**" }] }
  ]
}
```

**Required changes for #52**:

1. Enable `assetProtocol` with narrow scope (see C1 above) — in `tauri.conf.json`, not `capabilities/default.json`
2. Add `https://www.steamgriddb.com/**` to `shell:allow-open` allow list if an "Open on SteamGridDB" link is included (advisory C3)
3. No new `#[tauri::command]` handlers should expand the `default.json` permission surface beyond `core:default`

**New IPC commands** (anticipated for #52): `fetch_game_metadata`, `fetch_game_cover_art` (or similar). These should follow the established pattern: `snake_case` names, Serde types, `State<'_, MetadataStore>` injection. The handlers must run all image validation and path construction in the Rust backend — do not accept raw URLs or file paths from the frontend for download/storage operations.

---

## Dependency Security

No new runtime dependencies are introduced by issue #52. The Steam Store API and SteamGridDB API are accessed via the existing `reqwest` HTTP client already used by the ProtonDB lookup. Image processing uses Rust's standard `std::fs` for file I/O. No third-party image decoding crates are required — images are served as-is to the Tauri webview, which handles rendering via the system's native image codecs.

| Dependency   | Version  | Known Issues | Risk Level | Notes                                          |
| ------------ | -------- | ------------ | ---------- | ---------------------------------------------- |
| `reqwest`    | existing | None active  | Low        | Already in use for ProtonDB; no new dependency |
| `serde_json` | existing | None active  | Low        | Already in use for metadata parsing            |
| `rusqlite`   | existing | None active  | Low        | Already in use for MetadataStore               |

---

## Secure Coding Guidelines

These apply to the implementation of both the UI restructuring and the #52 metadata / cover art feature:

1. **SVG rejection at the backend**: Validate image MIME type by magic bytes (using the `infer` crate) before writing to disk. Allowlist: `image/jpeg`, `image/png`, `image/webp`. Reject SVG unconditionally.

2. **Path construction with prefix assertion**: Build cache file paths using `canonicalize` + `starts_with` assertion. Validate `steam_app_id` as a pure decimal integer before use in path construction.

3. **API-sourced text is untrusted**: Never pass metadata strings to `dangerouslySetInnerHTML`. Use JSX text nodes for all metadata display.

4. **Image size cap before write**: Accumulate bytes up to 5 MB; abort and clean up partial writes if exceeded.

5. **No API key logging**: Do not log or include the SteamGridDB API key in any tracing span or frontend console output. Use `skip(api_key)` annotations where necessary.

6. **Asset protocol scope**: Limit `assetProtocol.scope` to `$LOCALDATA/cache/images/**`. Do not use broader patterns.

7. **URL domain validation for image downloads**: Validate that Steam CDN URLs start with `https://cdn.cloudflare.steamstatic.com/` and SteamGridDB image URLs start with `https://cdn2.steamgriddb.com/` or `https://img.steamgriddb.com/` before download.

8. **Component mounting strategy** (from pass 1): Use CSS-based show/hide for tab content rather than conditional rendering where components hold local draft state.

9. **Do not introduce `dangerouslySetInnerHTML`**: No scenario in this feature requires raw HTML injection.

10. **Preserve `RESERVED_CUSTOM_ENV_KEYS` mirror contract**: If `CustomEnvironmentVariablesSection` is moved or refactored, the client-side reserved key list must remain synchronized with `crosshook-core/src/launch/request.rs`.

---

## Trade-off Recommendations

| Trade-off                                       | Recommendation                                                                                                                                                                       |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| SVG rejection vs. allowing SVGs via `<img>` tag | Reject SVG at download time. Relying on `<img>`-based sandboxing is a fragile defense — webview rendering behavior can change across Chromium versions. Simpler to never store SVGs. |
| OS keyring vs. plaintext API key                | Plaintext in `settings.toml` with a UX warning is acceptable for v1. Plan OS keyring migration (`keyring` crate) for a follow-up phase.                                              |
| `asset://` vs. `file://` for image rendering    | Use `asset://` (via `convertFileSrc`). `file://` is not in the CSP `img-src`, and `asset://` with a narrow scope is the documented Tauri v2 pattern.                                 |
| Broad vs. narrow `assetProtocol.scope`          | Narrow scope (`$LOCALDATA/cache/images/**`) is non-negotiable. A broad scope like `"**/*"` would allow the webview to read any user file via a crafted `asset://` URL.               |
| Checksum verification on serve                  | Include in initial implementation. It is low cost (one SHA-256 computation per cache hit) and provides defense against filesystem corruption.                                        |
| Cache size limit                                | Default 500 MB, user-configurable. Enforce by LRU eviction on the `last_accessed_at` column.                                                                                         |
| CSS show/hide vs. unmount for tabs              | Prefer CSS show/hide to preserve local component state. Accept slightly higher DOM size for safety.                                                                                  |

---

## Open Questions

1. **Image naming in cache**: Will cached images use a fixed name (`header.jpg`, `grid.png`) or a content-addressed name (SHA-256 hash)? Content-addressed naming eliminates filename collision and simplifies cache invalidation but requires the frontend to request file names from the backend rather than constructing them.
2. **SteamGridDB URL format contractual stability**: The image CDN domain (`cdn2.steamgriddb.com`) is inferred from API responses. If SteamGridDB changes CDN providers, the domain allowlist needs updating. Consider whether to pin to a strict allowlist or to validate that the URL scheme is `https` and the hostname ends in `.steamgriddb.com`.
3. **Env var draft flushing on unmount** (from pass 1): If sub-tabs unmount components, auto-flush draft state via `useEffect` cleanup. Auto-flush is simpler and safer than a warning dialog.
4. **OS keyring availability**: Does CrossHook have a target minimum environment (e.g. GNOME, KDE, any desktop)? This affects whether `keyring` crate with Secret Service can be the primary storage path in a future phase.

---

## Sources

- [Tauri v2 Content Security Policy](https://v2.tauri.app/security/csp/) — CSP configuration, `img-src` directive for asset protocol
- [Tauri v2 Asset Protocol Discussion](https://github.com/tauri-apps/tauri/discussions/11498) — scope configuration for hidden directories, `convertFileSrc` usage
- [Tauri v2 Command Scopes](https://v2.tauri.app/security/scope/) — granular scope configuration for capabilities
- [infer crate](https://github.com/bojand/infer) — magic-byte MIME type detection for Rust
- [RUSTSEC-2023-0018: remove_dir_all TOCTOU](https://rustsec.org/advisories/RUSTSEC-2023-0018) — TOCTOU race condition reference for Rust filesystem operations
- [SVG XSS attack surface — Fortinet](https://www.fortinet.com/blog/threat-research/scalable-vector-graphics-attack-surface-anatomy) — SVG `<script>`, `foreignObject`, XXE in browser context
- [keyring crate](https://docs.rs/keyring) — Cross-platform OS keyring / Secret Service integration for Rust
- [secret-service crate](https://docs.rs/secret-service/latest/secret_service/) — Direct GNOME Keyring / KWallet DBus API for Rust
- [Path traversal guide — StackHawk](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/) — `canonicalize` + prefix-assert pattern
- [API Key Leaks — PayloadsAllTheThings](https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/API%20Key%20Leaks) — API key exposure vectors
- [Secure API file uploads with magic numbers — Transloadit](https://transloadit.com/devtips/secure-api-file-uploads-with-magic-numbers/) — defense-in-depth for file type validation
- [Flatpak secrets management — Opensource.com](https://opensource.com/article/19/11/secrets-management-flatpak-applications) — OS keyring patterns for Linux desktop apps
- Codebase: `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — established pattern for external HTTP with reqwest, timeout, caching
- Codebase: `src/crosshook-native/src-tauri/capabilities/default.json` — current capability surface
- Codebase: `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs:497-521` — `validate_name()` path traversal prevention
- Codebase: `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx:6-53` — env var key validation and reserved key protection
- Codebase: `src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs:254-311` — backend sanitization of external API data
