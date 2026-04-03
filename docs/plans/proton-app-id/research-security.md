# Security Research: proton-app-id

## Executive Summary

The proton-app-id feature introduces an optional `steam_app_id` field on `RuntimeSection` (for `proton_run` profiles) and a tri-art system (cover, portrait, background) with both auto-download from Steam CDN / SteamGridDB and user-uploaded custom art. The existing `game_images` subsystem (`client.rs`, `import.rs`, `steamgriddb.rs`) already implements a mature security baseline: magic-byte validation, 5 MB hard cap, decimal-only app_id enforcement, and `canonicalize`-based path traversal prevention. There are **no CRITICAL hard stops** that would block this feature from being designed and implemented. There are several **WARNINGs** that must be addressed before shipping.

The most impactful new risk surface introduced by this feature is:

1. **SteamGridDB URL redirect following** — the image download step hits a URL returned by the SteamGridDB API, which CrossHook cannot control. Reqwest follows redirects to arbitrary hosts by default; this requires a domain-allow-list redirect policy.
2. **`settings_load` IPC leaks the raw API key to the frontend** — `AppSettingsData` serializes `steamgriddb_api_key` as a plain string. Any component that calls `settings_load` and re-displays or logs the response can expose the key.
3. **Pixel-flood / decompression bomb** — the 5 MB byte-size cap does NOT prevent a maliciously crafted PNG from having tiny compressed size but 500 MP decoded dimensions (e.g., 50 000 × 10 000 px). If the `image` crate is ever used for decoding/re-encoding, this must be constrained.
4. **Custom art upload path is read from the local filesystem before validation** — the current `import_custom_cover_art` reads the entire file into RAM before validating, which is fine for the 5 MB cap but must stay that way as the feature evolves.
5. **Community profile import does not strip `custom_cover_art_path`** — the `sanitize_profile_for_community_export` function clears machine-specific paths but `game.custom_cover_art_path` is not explicitly cleared, making it a path disclosure vector if non-empty.

---

## Findings by Severity

### CRITICAL — Hard Stops

None. All existing pre-ship conditions are WARNINGs or ADVISORYs.

---

### WARNING — Must Address

| ID   | Area                      | Finding                                                                                                                                                                                                                                                                                                                                             | Severity | Mitigation Path                                                                                                                                                                                           |
| ---- | ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S-01 | Network Security          | `reqwest` default redirect policy follows redirects to any host. SteamGridDB API returns image URLs that CrossHook fetches; a compromised or malicious response could redirect to `file://`, `http://127.0.0.1`, or internal hosts                                                                                                                  | WARNING  | Apply a custom redirect policy allowing only `https://` redirects to the confirmed allow-list: `cdn.cloudflare.steamstatic.com`, `steamcdn-a.akamaihd.net`, `www.steamgriddb.com`, `cdn2.steamgriddb.com` |
| S-02 | Credential Handling       | `settings_load` IPC command returns raw `AppSettingsData` including `steamgriddb_api_key: Option<String>`. Any frontend component that calls `settings_load` receives the plaintext key                                                                                                                                                             | WARNING  | Return a sanitized DTO from `settings_load` that replaces the key with `has_api_key: bool`; accept the key only in `settings_save`                                                                        |
| S-03 | File System Security      | `sanitize_profile_for_community_export` does not explicitly clear `game.custom_cover_art_path`. A user with a non-empty custom art path who exports to a community profile may disclose a local filesystem path. May be resolved by architecture if custom art paths move to `local_override` and `portable_profile()` clears that section entirely | WARNING  | Confirm `portable_profile()` clears `local_override`; if not, add explicit `clear()` calls in `sanitize_profile_for_community_export`                                                                     |
| S-04 | Image Processing Security | No maximum decoded dimension constraint. A malicious JPEG or PNG can pass the 5 MB byte-size check while expanding to multi-gigapixel dimensions when decoded. Affects any future code that decodes downloaded images                                                                                                                               | WARNING  | If any code path decodes image bytes (e.g., for thumbnailing, re-encoding, or dimension inspection), enforce a maximum dimension (e.g., 8 192 × 8 192 px) before full decode                              |
| S-05 | Input Validation          | `fetch_game_cover_art` IPC accepts `image_type` as `Option<String>` with a `match` fallback to `Cover`. Passing an unrecognized type silently selects `Cover`. This is safe but surprising behavior for callers                                                                                                                                     | WARNING  | Return a typed error for unrecognized `image_type` values rather than silently defaulting; document accepted values in the IPC contract                                                                   |
| S-06 | Network Security          | `reqwest` client is built without HTTPS enforcement. The CDN URLs are all HTTPS constants in the code, but a redirect from SteamGridDB to a plain HTTP URL would be followed without complaint                                                                                                                                                      | WARNING  | Combine with S-01 mitigation: the custom redirect policy should also reject redirects to `http://` targets (allow `https://` only)                                                                        |

---

### ADVISORY — Best Practices

| ID   | Area                      | Finding                                                                                                                                                                                                                                                                                                   | Severity | Mitigation Path                                                                                                                                                                                                |
| ---- | ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S-07 | Image Processing Security | The `infer` crate (v0.16) has no known CVEs as of April 2026. Magic-byte checks are a solid first line of defense but can be spoofed by prepending valid magic bytes before malicious content (polyglot files). The 5 MB cap limits the blast radius                                                      | ADVISORY | Track `infer` in `cargo audit`. Polyglot risk is low for JPEG/PNG/WebP in a local desktop app; no action required now                                                                                          |
| S-08 | Dependency Security       | `reqwest` is configured with `rustls-tls` and no `native-tls`, ensuring consistent TLS behavior. No known CVEs as of April 2026. `cargo audit` should be added to CI                                                                                                                                      | ADVISORY | Add `cargo audit` to the CI pipeline; pin a minimum `reqwest` version in `Cargo.toml`                                                                                                                          |
| S-09 | Dependency Security       | `webp` crate (RUSTSEC-2024-0443) had a memory-exposure bug when encoding. CrossHook uses the `infer` crate for detection only and does not decode or encode WebP using the `webp` crate; this advisory does not apply directly. Verify no transitive dependency pulls in the vulnerable `webp` version    | ADVISORY | Run `cargo tree -d` to confirm no transitive `webp` dependency; add to `cargo audit` CI                                                                                                                        |
| S-10 | File System Security      | `import_custom_cover_art` reads the full file into RAM before validation. This is safe with the 5 MB guard in `validate_image_bytes`. If the size cap is ever raised, the read should be chunked or streaming                                                                                             | ADVISORY | Keep the current read-then-validate pattern; document that raising the cap requires moving to streaming read                                                                                                   |
| S-11 | File System Security      | `safe_image_cache_path` uses `std::fs::canonicalize` which resolves symlinks. A symlink in the cache directory pointing outside the base cannot escape the prefix check post-canonicalization. However, the TOCTOU window between `canonicalize` and `create_dir_all` + `write` exists                    | ADVISORY | The TOCTOU risk is low for a single-user local app. Document the invariant. No action required for initial ship                                                                                                |
| S-12 | Network Security          | SteamGridDB HTTP 401/403 (key invalid/expired) surfaces as `GameImageError::Network` with silent stale-cache fallback — same as a transient 429. Users whose key has expired may not discover it. Required: on 401/403 fall back to Steam CDN (not stale cache) and surface a distinguishable error state | WARNING  | Add `GameImageError::AuthFailure` variant; on 401/403 skip stale-cache fallback, fall back to Steam CDN, and return a discriminated error so the UI can show "API key invalid or expired — update in Settings" |
| S-13 | IPC / Data Handling       | `RuntimeSection` currently has no `steam_app_id` field. When added as `Option<String>`, must be validated at profile-save time (decimal digits only, max 12 chars) — not only at image-fetch time. A non-numeric value should be rejected before reaching the download pipeline                           | WARNING  | Add a `validate_steam_app_id` helper enforcing `^[0-9]{1,12}$`; call at profile-save time; reuse the existing numeric guard from `download_and_cache_image`                                                    |
| S-14 | IPC / Data Handling       | Profile DTOs crossing the IPC boundary should not include raw filesystem paths (`custom_cover_art_path`) in user-visible contexts (error messages, logs, tooltips)                                                                                                                                        | ADVISORY | Already enforced in the existing codebase; document convention explicitly for new IPC commands added in this feature                                                                                           |
| S-15 | Credential Handling       | `steamgriddb_api_key` is stored as plaintext in `~/.config/crosshook/settings.toml`. On a shared or compromised machine the key is readable by anyone with file access                                                                                                                                    | ADVISORY | The key rotates every 90 days; for a single-user local app, plaintext TOML config is acceptable. Consider noting in docs that users on shared systems should use filesystem permissions (`chmod 600`)          |

---

## Input Validation

### Steam App ID

**Confidence**: High (reviewed `client.rs:155` and `safe_image_cache_path:99–101` directly)

`download_and_cache_image` validates `app_id` with:

```rust
if app_id.is_empty() || !app_id.chars().all(|c| c.is_ascii_digit()) {
    return Err(format!("invalid app_id {:?}: must be a non-empty decimal integer", app_id));
}
```

`safe_image_cache_path` applies the same check before constructing any filesystem path. These guards are in place and tested.

**Gap for proton-app-id**: The new `steam_app_id` field on `RuntimeSection` does not yet exist. When it is added, the field must be validated at profile-save time (S-13), not only at image-fetch time. A TOML containing a non-numeric `steam_app_id` should be rejected at parse/save, not silently ignored until the fetch pipeline rejects it.

### Image Type String

`fetch_game_cover_art` accepts `image_type: Option<String>` and silently defaults unrecognized values to `Cover` (S-05). This should be changed to return a typed error for unrecognized types before shipping — it makes the IPC contract explicit and prevents silent behavior surprises as more art types are added.

### API Response Validation

The SteamGridDB response is deserialized via `serde` into a typed struct (`SteamGridDbResponse`). Unknown fields are silently ignored (serde default behavior), which is acceptable. The `success: bool` and `data: Option<Vec<SteamGridDbItem>>` check is in place at `steamgriddb.rs:63`. No injection risk from the JSON itself since the only field consumed is `item.url` which is then passed to a separate HTTP GET — see S-01 for the redirect risk this creates.

---

## Image Processing Security

### Byte-Size Cap

**Confidence**: High (reviewed `client.rs:17–67` directly)

The 5 MB cap (`MAX_IMAGE_BYTES = 5 * 1024 * 1024`) is enforced at two levels:

1. **Content-Length header check** (`read_limited_response:443–447`): rejects responses claiming to be larger than 5 MB before downloading.
2. **Streaming chunk accumulation** (`read_limited_response:449–456`): aborts accumulation once bytes exceed 5 MB, regardless of what Content-Length said.
3. **Magic-byte validation** (`validate_image_bytes:55–68`): checks the final byte slice against the same cap after download.

This is defense-in-depth and correct.

### Pixel Flood / Decompression Bomb (S-04)

**Confidence**: Medium

A PNG file can be ~200 KB on disk (well within 5 MB) but declare dimensions of 50 000 × 50 000 pixels with highly compressible data. When decoded by any image library, this expands to ~10 GB of uncompressed pixel data, exhausting memory.

**Applicability to CrossHook**: The current implementation **does not decode image bytes** — it validates by magic bytes and stores/serves the raw file. The `image` crate is **not currently a dependency** in `crosshook-core/Cargo.toml`. Therefore this attack does not apply to the current code.

**Risk flag**: If a future implementation step adds dimension inspection (e.g., to generate thumbnails, crop to aspect ratio, or validate resolution for the tri-art display system), the `image` crate would be introduced and this attack becomes relevant. Before adding any decode step:

```rust
// SAFE pattern using the image crate
use image::io::Reader as ImageReader;
let reader = ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format()?;
let (width, height) = reader.into_dimensions()?;  // Reads dimensions WITHOUT full decode
const MAX_DIMENSION: u32 = 8_192;
if width > MAX_DIMENSION || height > MAX_DIMENSION {
    return Err(GameImageError::DimensionsTooLarge);
}
```

The `into_dimensions()` method parses only the image header without decoding pixel data, avoiding the memory expansion.

### Magic-Byte Detection vs. Polyglot Files (S-07)

The `infer` crate identifies formats by magic-byte prefix matching. A polyglot file that starts with valid JPEG magic bytes (`FF D8 FF`) but contains embedded scripts or executable payloads in later bytes would pass the format check. For a local desktop app that serves images via `asset://` (not executing them), this risk is negligible — the WebView renders the file as an image, not as a script. SVG (which can contain `<script>` tags) is rejected because it has no magic bytes.

---

## Network Security

### HTTPS Enforcement

**Confidence**: High

All hardcoded CDN and SteamGridDB API URLs use `https://`. The `reqwest` client is built with `rustls-tls` and no `native-tls`. No plain HTTP downloads are initiated by the existing code.

**Gap**: The `reqwest` client does not have an explicit HTTPS-only redirect policy. If a redirect from SteamGridDB points to `http://`, reqwest will follow it (S-06).

### Redirect Policy — SSRF Risk (S-01, S-06)

**Confidence**: High

The reqwest client in `client.rs:33–43` is built with `timeout` and `user_agent` but **no custom redirect policy**. The default policy follows up to 10 redirects to any host. The SteamGridDB API returns `item.url` values (the actual image CDN URLs) in its JSON response. If the SteamGridDB API response is compromised, tampered with, or returns an unexpected redirect URL, reqwest would follow it to:

- Internal addresses (`http://127.0.0.1`, `http://192.168.x.x`) — SSRF risk
- `file://` URIs — local file access (reqwest rejects `file://` on all platforms by default since it is an HTTP client, so this vector is mitigated)
- HTTP downgrade — credential/content exposure in transit

**Confirmed allow-list** (from api-researcher): `cdn.cloudflare.steamstatic.com`, `steamcdn-a.akamaihd.net`, `www.steamgriddb.com`, `cdn2.steamgriddb.com`. HTTPS only.

**Required mitigation before shipping (S-01 + S-06)**:

```rust
const REDIRECT_ALLOWED_HOSTS: &[&str] = &[
    "cdn.cloudflare.steamstatic.com",
    "steamcdn-a.akamaihd.net",
    "www.steamgriddb.com",
    "cdn2.steamgriddb.com",
];

let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
    .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
    .redirect(reqwest::redirect::Policy::custom(|attempt| {
        let url = attempt.url();
        let allowed = url.scheme() == "https"
            && url.host_str().is_some_and(|h| {
                REDIRECT_ALLOWED_HOSTS.iter().any(|&allowed| h == allowed)
            });
        if allowed {
            attempt.follow()
        } else {
            attempt.error(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("redirect to disallowed host: {url}"),
            ))
        }
    }))
    .build()
```

### Certificate Validation

**Confidence**: High

`reqwest` with `rustls-tls` validates TLS certificates against the OS/webpki trust store by default. No `danger_accept_invalid_certs` or `danger_accept_invalid_hostnames` calls are present in the codebase. This is correct.

### Response Size Limits

**Confidence**: High (reviewed `read_limited_response` directly)

The streaming chunk accumulation enforces 5 MB before the full body is buffered. The SteamGridDB JSON metadata response (not the image) is read via `.json()` which buffers the entire response without a separate size limit. For the API endpoint (`/api/v2/grids/steam/{app_id}`), the JSON is small (a list of image records with URLs and metadata) and unlikely to cause issues, but adding a size cap to the JSON response would be best practice (S-12/ADVISORY).

---

## File System Security

### Safe Path Construction

**Confidence**: High (reviewed `safe_image_cache_path:94–119` directly)

The `safe_image_cache_path` function:

1. Validates `app_id` as pure decimal — rejects `../etc`, `440/extra`, `440`
2. Validates `filename` as a single path component — rejects `../../evil.jpg`
3. Canonicalizes the base directory
4. Creates the subdirectory with `create_dir_all`
5. Canonicalizes the parent after creation
6. Asserts the canonicalized parent starts with the canonicalized base

This is correct defense-in-depth path traversal prevention. Passing tests confirm all attack vectors.

### Symlink Attack on DB File

**Confidence**: High (referenced `db.rs` review in prior research)

The metadata DB is opened with symlink detection before opening. This is already implemented and correct.

### Custom Art Import

**Confidence**: High (reviewed `import.rs:32–65` directly)

`import_custom_cover_art`:

1. Reads from a user-specified `source_path` (validated by Tauri dialog — the user picks the file via the native OS file picker)
2. Validates magic bytes + size before writing
3. Writes to `$LOCALDATA/crosshook/media/covers/<hash[..16]>.ext`
4. Uses content-addressed naming (SHA-256 prefix) — idempotent, no filename collision

**Remaining concern (S-03 / Community Export)**: The `source_path` argument for `import_custom_cover_art` comes from the Tauri dialog plugin (`dialog:default`). Tauri's `open()` dialog adds the selected path to the filesystem scope for the session, but the asset protocol scope in `tauri.conf.json` already includes `$LOCALDATA/crosshook/media/**`. After import, the file is at a content-addressed path inside the media directory, which is in scope.

The original `source_path` (e.g., `/home/user/Pictures/game.jpg`) is not stored anywhere after import — only the destination path in the media directory is returned and stored. This is correct.

### `media/**` vs `cache/images/**` Scope

`tauri.conf.json` (line 27) already includes `$LOCALDATA/crosshook/media/**` in the asset protocol scope and `default.json` (line 20) includes it in `fs:allow-read-file`. The media directory scope is correctly in place for the custom art use case.

---

## Dependency Security

### `reqwest` (v0.12, `rustls-tls`)

**Confidence**: High

- TLS: `rustls-tls` feature, no `native-tls` dependency — consistent cross-platform TLS with a pure-Rust stack
- No known CVEs against v0.12.x as of April 2026
- Redirect policy: default (10 hops, any host) — must be replaced per S-01/S-06

### `infer` (v0.16)

**Confidence**: High

- No CVEs or RustSec advisories found as of April 2026
- Uses pure-Rust byte matching with no unsafe blocks in the public API
- Limitation: `exe` and `dll` files share the same magic bytes and cannot be distinguished — not relevant for image validation

### `webp` crate (RUSTSEC-2024-0443)

**Confidence**: High

- CrossHook does NOT depend on the `webp` crate directly
- `infer` only identifies WebP by magic bytes (`52 49 46 46 ... 57 45 42 50`) — no decode
- Verify with `cargo tree -d` that no transitive dependency pulls in a vulnerable `webp` version

### `image` crate

**Confidence**: High

- **Not currently in `crosshook-core/Cargo.toml`** — the `image` crate is not a dependency
- If added for future dimension inspection or thumbnail generation, apply the pixel-flood protection pattern described in the Image Processing section

### `sha2` (v0.11)

**Confidence**: High

- Part of the RustCrypto project, regularly audited
- No known CVEs

---

## Secure Coding Guidelines

### 1. Steam App ID Validation (new `RuntimeSection` field)

```rust
// In RuntimeSection validator or profile save path:
pub fn validate_steam_app_id(app_id: &str) -> Result<(), String> {
    if app_id.is_empty() {
        return Ok(()); // Optional field; empty means "not set"
    }
    // Max 12 digits: Steam app IDs are u32 (max 10 digits); 12 is a generous safe cap
    if app_id.len() > 12 || !app_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "steam_app_id must be a decimal integer (max 12 digits), got: {app_id:?}"
        ));
    }
    Ok(())
}
```

Reuse `download_and_cache_image`'s existing guard at fetch time — do not duplicate the logic there. The profile-save validator is the earlier enforcement point.

### 2. API Key — Settings IPC (S-02)

```rust
// WRONG — leaks API key to frontend:
#[tauri::command]
pub fn settings_load(store: State<'_, SettingsStore>) -> Result<AppSettingsData, String> {
    store.load().map_err(...)  // AppSettingsData includes steamgriddb_api_key: Option<String>
}

// CORRECT — return a sanitized DTO:
#[derive(Serialize)]
pub struct AppSettingsView {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    pub offline_mode: bool,
    pub has_steamgriddb_api_key: bool,  // bool, not the raw key
}

#[tauri::command]
pub fn settings_load(store: State<'_, SettingsStore>) -> Result<AppSettingsView, String> {
    let data = store.load().map_err(...)?;
    Ok(AppSettingsView {
        auto_load_last_profile: data.auto_load_last_profile,
        last_used_profile: data.last_used_profile,
        community_taps: data.community_taps,
        onboarding_completed: data.onboarding_completed,
        offline_mode: data.offline_mode,
        has_steamgriddb_api_key: data.steamgriddb_api_key.as_deref().filter(|k| !k.trim().is_empty()).is_some(),
    })
}

// settings_save continues to accept the full AppSettingsData (including the key)
// so the user can update the key from the settings UI.
```

### 3. Redirect Policy (S-01, S-06)

Add a domain-allow-list redirect policy to the HTTP client singleton in `game_images/client.rs`. The allowed domains for image downloads are:

- `cdn.cloudflare.steamstatic.com`
- `steamcdn-a.akamaihd.net` (legacy Steam CDN)
- `img.steamgriddb.com` (SteamGridDB image CDN)
- `steamgriddb.com` (API host)

Only `https://` redirects should be followed.

### 4. Community Export — Clear `custom_cover_art_path` (S-03)

```rust
fn sanitize_profile_for_community_export(profile: &GameProfile) -> GameProfile {
    let mut out = profile.portable_profile();
    out.game.custom_cover_art_path.clear();  // ADD THIS — path is machine-specific
    out.injection.dll_paths.clear();
    out.steam.launcher.icon_path.clear();
    out.runtime.proton_path.clear();
    out.runtime.working_directory.clear();
    out
}
```

### 5. `image_type` IPC Parameter (S-05)

```rust
// CURRENT — silent default:
let image_type = match image_type.as_deref().unwrap_or("cover") {
    "hero" => GameImageType::Hero,
    "capsule" => GameImageType::Capsule,
    "portrait" => GameImageType::Portrait,
    _ => GameImageType::Cover,  // silently maps unknown → Cover
};

// BETTER — fail on unknown type:
let image_type = match image_type.as_deref().unwrap_or("cover") {
    "cover" => GameImageType::Cover,
    "hero" => GameImageType::Hero,
    "capsule" => GameImageType::Capsule,
    "portrait" => GameImageType::Portrait,
    other => return Err(format!("unknown image_type: {other:?}; accepted: cover, hero, capsule, portrait")),
};
```

### 6. Error Message Hygiene

Do not expose raw `source_path` values in IPC error responses. The `import_custom_cover_art` command currently returns error strings that include the raw path (e.g., `"source file does not exist: /home/user/pictures/game.jpg"`). This is acceptable because the user picked the file themselves and the path is their own input — but it should not be logged or forwarded to telemetry.

---

## Trade-off Recommendations

### API Key Exposure: Full DTO vs. Sanitized View

The current `settings_load` returns `AppSettingsData` directly, which includes the raw `steamgriddb_api_key`. This is the highest-priority WARNING to address before shipping. Two options:

- **Option A (Recommended)**: Return a separate `AppSettingsView` DTO with `has_steamgriddb_api_key: bool`. The settings UI shows "API key configured" or "No key set" without ever sending the raw value to the frontend.
- **Option B**: Keep the current `AppSettingsData` but mark `steamgriddb_api_key` with `#[serde(skip_serializing)]` and add a separate `set_steamgriddb_api_key` command. The frontend cannot read the key back at all.

Option A is preferred because it is explicit and matches the UI requirement (show whether a key is set, allow updating it).

### Redirect Policy: Domain Allow-List vs. Disable Redirects

- **Option A (Recommended)**: Domain allow-list with HTTPS-only policy. Allows SteamGridDB CDN redirects (which do happen in practice — SteamGridDB may serve from multiple CDN backends) while blocking SSRF vectors.
- **Option B**: Disable all redirects (`Policy::none()`). Simplest, most secure, but may break SteamGridDB image delivery if they use CDN redirects.

Option A is the better user experience and is still secure against SSRF.

### Pixel Flood: Enforce Now vs. Defer Until `image` Crate Added

Currently the `image` crate is not a dependency, so pixel flood is not a current risk. The recommendation is to **document the constraint now** and enforce it in the same PR that first adds any decode step. This avoids a future engineer adding `image` without realizing the dimension check is missing.

---

## Open Questions

1. **SteamGridDB image CDN domain**: What is the actual CDN domain for SteamGridDB image responses (not the API host)? The redirect policy needs the exact domain. Confirm by inspecting a live API response (`img.steamgriddb.com` is the expected CDN but should be verified by api-researcher).

2. **`settings_load` frontend consumers**: Which frontend components currently call `settings_load`? Before changing the return type, all callers must be audited to ensure they do not depend on receiving the raw key. This is a required pre-ship task for S-02.

3. **`portable_profile` and `effective_profile`**: The `sanitize_profile_for_community_export` fix (S-03) assumes `portable_profile()` does not already clear `custom_cover_art_path`. Verify that this method's contract before adding the explicit clear.

4. **Rate limiting visibility**: When SteamGridDB returns HTTP 429, the current code logs a warning and returns a stale cache fallback (or `None`). Should the frontend distinguish "key invalid" from "rate limited" from "image not found in SteamGridDB"? This affects UX design but has security implications (user may diagnose a compromised key as a rate-limit issue).

5. **`background` art type**: The feature spec mentions a "background" art type (cover, portrait, background). The existing `GameImageType` enum has `Cover`, `Hero`, `Capsule`, `Portrait`. The new `background` type may map to `Hero`. Confirm the mapping with api-researcher before implementing `fetch_game_cover_art` IPC changes to avoid returning `Cover` images for `background` requests due to the silent-default issue (S-05).

---

## Sources

- [Tauri v2 Security Overview](https://v2.tauri.app/security/)
- [Tauri v2 Dialog Plugin — Scope Notes](https://v2.tauri.app/plugin/dialog/)
- [Tauri v2 Capabilities Reference](https://v2.tauri.app/reference/acl/capability/)
- [reqwest Redirect Policy Docs](https://docs.rs/reqwest/latest/reqwest/redirect/struct.Policy.html)
- [RUSTSEC-2024-0443: webp crate memory exposure](https://rustsec.org/advisories/RUSTSEC-2024-0443.html)
- [RustSec Advisory Database](https://rustsec.org/advisories/)
- [infer crate — magic byte detection](https://crates.io/crates/infer)
- [Pixel flood attack — HackerOne disclosure](https://hackerone.com/reports/390)
- [PNG bomb protection patterns](https://github.com/ptrofimov/png-bomb-protection)
- [SSRF via open redirect](https://medium.com/@cyberseccafe/ssrf-with-filter-bypass-via-open-redirection-9949b6ed8eb9)
- [SteamGridDB API documentation](https://www.steamgriddb.com/api/v1)
- [Rust Cargo zip-bomb DoS — JFrog](https://research.jfrog.com/vulnerabilities/rust-cargo-zip-bomb-dos/)
