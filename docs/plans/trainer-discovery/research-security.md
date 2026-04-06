# Security Research: Trainer Discovery

## Executive Summary

Trainer discovery introduces three new attack surfaces to CrossHook: (1) external trainer source URLs stored and rendered in the UI, (2) community tap metadata indexing external trainer source metadata, and (3) search query processing against a SQLite FTS index. The existing codebase already has strong foundations — SHA-256 verification at launch, tap URL validation in `community/taps.rs`, parameterized SQLite queries via `rusqlite`, git subprocess environment isolation (`GIT_CONFIG_NOSYSTEM`, `GIT_TERMINAL_PROMPT=0`), a 524,288-byte cache payload cap in `external_cache_entries`, and Tauri's CSP enforcement. No CRITICAL blockers exist for the link-only MVP model. Five WARNING-level issues require pre-ship mitigations; legal risk is the most significant and requires a deliberate design choice before proceeding.

---

## Findings by Severity

| #   | Finding                                                                                                                     | Severity | Area                |
| --- | --------------------------------------------------------------------------------------------------------------------------- | -------- | ------------------- |
| S1  | Linking to trainer download sources carries DMCA trafficking liability risk under §1201(a)(2)                               | WARNING  | Legal               |
| S2  | Community tap trainer source metadata has no integrity verification beyond git                                              | WARNING  | Tap Security        |
| S3  | External cache entries (`external_cache_entries`) lack response integrity validation — cache poisoning via MITM is possible | WARNING  | Data Integrity      |
| S4  | Search query input must be sanitized before FTS5 query construction                                                         | WARNING  | Input Validation    |
| S5  | Discovery source URLs rendered in WebKitGTK frontend need explicit sanitization before `innerHTML` or `href` use            | WARNING  | XSS / Frontend      |
| S6  | New HTTP fetch code for trainer source metadata indexes must enforce HTTPS-only                                             | ADVISORY | Transport Security  |
| S7  | Community tap TOML/JSON field-length bounds must be applied to trainer source metadata fields                               | ADVISORY | Input Validation    |
| S8  | `reqwest` and `rusqlite` crates should be audited with `cargo audit` before adding new trainer source fetch paths           | ADVISORY | Dependency Security |
| S9  | SHA-256 hashes published by community taps for trainers should feed the existing launch-time verification                   | ADVISORY | Integration         |
| S10 | Discovery results that link to external sites should display a trust indicator and external-navigation warning              | ADVISORY | UX / Security       |

---

## 1. Source URL Validation

### Finding S6 (ADVISORY): Enforce HTTPS-only for trainer source URLs

**Risk**: A tap maintainer publishes an `http://` trainer download URL. CrossHook renders and opens it in the system browser. A MITM on the same network sees a plain-text request to a trainer download site, revealing the user's intent.

**Confidence**: High — this is a well-established risk for any link-aggregator tool.

**Mitigation — URL Allowlist Approach**

The `url` crate (servo/rust-url) is already an indirect dependency of `reqwest`. Use it to enforce scheme and optionally a domain allowlist before storing any trainer source URL.

```rust
use url::Url;

/// Validate a trainer source URL before storing it in the index.
/// Returns Err with a user-visible message on failure.
pub fn validate_trainer_source_url(raw: &str) -> Result<Url, String> {
    let parsed = Url::parse(raw)
        .map_err(|e| format!("invalid trainer source URL: {e}"))?;

    if parsed.scheme() != "https" {
        return Err(format!(
            "trainer source URLs must use HTTPS; got scheme '{}'",
            parsed.scheme()
        ));
    }

    let host = parsed.host_str().unwrap_or("");
    if host.is_empty() {
        return Err("trainer source URL has no host".to_string());
    }

    Ok(parsed)
}
```

**Domain Allowlist Consideration**: An optional allowlist of trusted trainer source domains (e.g., FLiNG, WeMod, CheatHappens) could be maintained in a community tap configuration file — not hardcoded in CrossHook — to avoid CrossHook becoming a de-facto curator of trusted trainer sites. This design keeps the trust decision with the community tap maintainer.

**Phase 1 recommendation**: Enforce HTTPS-only. Domain allowlist is optional for Phase 1 but document it as a future hardening step.

### Finding S5 (WARNING): URL rendering in WebKitGTK frontend

**Risk**: Trainer source URLs from community tap metadata are user-controlled text. If rendered via `innerHTML` or inserted into `href` attributes without sanitization, a `javascript:` URI or crafted HTML could execute in WebKitGTK's rendering context. While Tauri's CSP mitigates script injection, `javascript:` URIs in `href` bypass CSP in some WebKit versions.

**Confidence**: High — Tauri's own security docs flag this as a known risk for data rendered from external sources.

**Mitigation**:

1. Always open external URLs via Tauri's `open` plugin (which invokes `xdg-open`) — never via `window.location.href` or `<a>` navigation within the WebView.
2. In React components, bind `href` only to validated `https://` URLs. Reject and display an error for any URL that does not parse to HTTPS.
3. Never use `dangerouslySetInnerHTML` with tap-sourced metadata. Use React's text interpolation (`{value}`) which escapes by default.

```tsx
// GOOD — href bound to validated URL, opens externally
import { open } from '@tauri-apps/plugin-shell';

function TrainerSourceLink({ url }: { url: string }) {
  const isHttps = url.startsWith('https://');
  if (!isHttps) return <span>Invalid source URL</span>;
  return <button onClick={() => open(url)}>Open Download Page</button>;
}

// BAD — never do this
<a href={trainerSource.url}>Download</a>;
```

---

## 2. Community Tap Security

### Finding S2 (WARNING): No integrity verification beyond git for trainer source metadata

**Risk**: Community taps are user-added git repositories. Anyone can create a tap and publish trainer source metadata pointing to malicious download sites, or inject crafted field values targeting CrossHook's indexer. The existing guards in `community/taps.rs` — `validate_tap_url()` (allows only `https://`, `ssh://git@`, and `git@` schemes; rejects `file://`, `git://`, and bare paths), `validate_branch_name()`, and `is_valid_git_sha()` — prevent command injection into the git subprocess and protect the _subscription_ URL. They do not verify the _content_ that a tap publishes inside its repository.

Additionally, git subprocess isolation is already in place via `git_security_env_pairs()` in `taps.rs`: `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, and `GIT_TERMINAL_PROMPT=0` prevent system git config, global hooks, and credential prompts from being inherited. These protections cover the tap sync operation itself — not the content ingested from tap metadata files.

A further gap: git repos can be force-pushed. An unpinned tap's content can change between syncs without any indication in CrossHook.

**Confidence**: High — this is the standard community-curated content trust problem.

**Attack vectors**:

- Malicious tap publishes `download_url: "javascript:evil()"` or `download_url: "file:///etc/passwd"`.
- Tap publishes trainer source metadata with oversized strings targeting a SQLite buffer or UI rendering issue.
- Tap maintainer account is compromised and silently replaces legitimate URLs with phishing links via force-push.

**Mitigations**:

1. **Content validation at index time**: Apply the same A6 field-length guards already used in `metadata/community_index.rs` to all new trainer source metadata fields. Enforce a maximum of 2048 bytes for URLs, 512 bytes for display names, 128 bytes for version strings.

2. **URL scheme validation at index time**: Call `validate_trainer_source_url()` (see S6) on every `download_url` field before inserting into SQLite. The validation mirrors `validate_tap_url()` but is stricter — HTTPS-only, no SSH schemes, and must parse as a valid URL with a non-empty host. Store `NULL` for invalid URLs and log a warning — do not abort the entire tap sync on one bad URL.

3. **Pinned commit support**: The existing `pinned_commit` field in `CommunityTapSubscription` is already implemented, validated via `is_valid_git_sha()` (7–64 hex characters). Encourage tap consumers to pin to a commit SHA after vetting, providing point-in-time integrity that survives force-pushes.

4. **No signature verification for Phase 1**: Git commit signing (GPG/SSH) verification is not currently implemented for tap sync. Adding it would require significant infrastructure. Defer to Phase 2 with documented justification: "Tap content is validated on ingest; users are responsible for vetting taps they add."

**Gap**: CrossHook currently has no mechanism to revoke or flag a compromised tap's content retroactively. Document this as a known limitation in the feature spec.

### TOML/JSON Metadata Injection

Trainer source metadata published in taps will be parsed by `serde` (`toml` or `serde_json`). These parsers do not have known injection vulnerabilities beyond ordinary Rust type deserialization. However:

- A field typed `String` in the Rust model can receive arbitrarily long input. Apply explicit length truncation at the boundary.
- A field typed as an optional URL can receive a `file://` URI. Validate at the struct boundary after deserialization, not in the TOML schema.

**TARmageddon (CVE-2025-62518) note**: This vulnerability in `async-tar`/`tokio-tar` involves tar archive path traversal. CrossHook uses the `tar` crate (`0.4`) synchronously. If trainer source metadata is ever distributed as a tarball, verify that tar extraction uses `entry.unpack_in()` (bounds-checked) and never `entry.unpack()`. For Phase 1 (link-only model), this is not applicable — tap content is standard git, not tar archives.

---

## 3. Input Validation

### Finding S4 (WARNING): FTS5 query construction must sanitize user search input

**Risk**: SQLite FTS5 has its own query syntax. User-supplied search terms containing `MATCH`, `AND`, `OR`, `NOT`, `*`, `"`, or parentheses can be interpreted as FTS query operators rather than literal text, potentially causing unexpected query behavior or crashes.

**Confidence**: High — FTS5 query injection is a known class of SQLite vulnerability when user input is passed directly to `fts_table MATCH ?`.

**Mitigation**: Escape or strip FTS5 query syntax characters from user input before passing to `MATCH`. Two safe approaches:

```rust
/// Strip FTS5 special characters for a plain substring search.
/// Use this when you want literal matching, not FTS5 operator support.
pub fn sanitize_fts5_query(input: &str) -> String {
    // FTS5 special chars: " * ^ ( ) : .
    // Approach: wrap each token in double quotes for phrase matching
    input
        .split_whitespace()
        .map(|token| format!("\"{}\"", token.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}
```

Alternatively, use SQLite `LIKE`-based search on the `community_profiles` table columns directly (as recommended in `research-practices.md`) which avoids FTS5 query syntax entirely for Phase 1.

**The `rusqlite` parameterization guarantee**: Using `params![user_input]` in `rusqlite` prevents SQL injection into the surrounding `SELECT` statement — the `?` placeholder is treated as a string literal in the SQL parser. However, the _value_ is still passed to SQLite's FTS5 `MATCH` operator which performs its own parsing. Both layers of protection are needed.

**Additional input validation rules**:

- Game name search queries: trim whitespace, max 256 characters, reject non-UTF8.
- Trainer name search queries: same constraints.
- Cache keys for `external_cache_entries`: derive from the URL only — never include user-supplied text in cache keys. If a cache key is derived from any external string (e.g., a trainer source index URL), run it through `slugify()` (already implemented in `taps.rs` — strips to alphanumeric + hyphens) to prevent path-traversal characters from appearing in cache key derivations used in any file-system context.

---

## 4. Data Integrity

### Finding S3 (WARNING): `external_cache_entries` lacks response integrity validation

**Risk**: HTTP responses cached in `external_cache_entries` are stored as raw bytes. If an attacker performs a MITM attack on the network (or a captive portal serves fake responses), the cached trainer source metadata is poisoned and persisted until TTL expiry. CrossHook uses `rustls-tls` in `reqwest` (confirmed in `crosshook-core/Cargo.toml`: `features = ["json", "rustls-tls"]`), which provides strong TLS validation. However, server-side compromise or a compromised CDN would not be caught by TLS alone.

**Confidence**: Medium — TLS with `rustls` significantly reduces the MITM surface, but server-side cache poisoning remains.

**Existing defenses already in place**:

- `MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB) is enforced in `metadata/models.rs` and applied in `cache_store.rs`: payloads exceeding this limit are stored as `NULL` `payload_json` with a warning log. This caps memory and disk impact from oversized responses.
- `evict_expired_cache_entries` in `cache_store.rs` handles TTL-based eviction.

**Mitigations**:

1. **TLS is the primary defense** — `rustls-tls` with no custom root certificate injection means certificate pinning is not feasible, but the standard WebPKI validation prevents MITM from commodity network attackers.

2. **TTL enforcement**: Apply short TTLs (e.g., 6–24 hours) for trainer source metadata entries. The existing TTL eviction infrastructure handles expiry automatically.

3. **Content-type validation**: After fetching, verify the `Content-Type` header is `application/json` before parsing. Reject HTML responses (captive portals often return HTML) with a logged warning rather than caching them. This is a new check — not currently applied to ProtonDB cache entries — and should be added for all discovery fetches.

4. **Response size limits at the HTTP layer**: The 524,288-byte cap applies at the _store_ layer. For trainer source index fetches, apply an additional limit at the HTTP layer before reading the full body — use `reqwest`'s response streaming with an early abort above 1 MB. This prevents memory exhaustion before the store-layer cap is reached.

5. **Hash verification for trainer source index files (Phase 2)**: If community taps publish a SHA-256 of their `trainer-sources.json` alongside it, CrossHook can verify integrity after download. This requires tap maintainer cooperation and is deferred to Phase 2.

### Stale/Expired Data

The `CachedFallback` status in `CommunityTapSyncStatus` means old tap data may be displayed when network is unavailable. Trainer source URLs from stale cache entries may point to deleted or replaced pages. This is expected behavior (offline-first design) and does not represent a security vulnerability — but the UI should surface the stale status to users (see S10).

---

## 5. Legal Considerations

### Finding S1 (WARNING): DMCA §1201(a)(2) trafficking risk from linking to trainer download sources

**Risk**: The DMCA's anti-trafficking provision (17 U.S.C. §1201(a)(2)) prohibits providing tools that help others circumvent technological protection measures. Courts have found that linking to circumvention tools can trigger this provision, particularly when coupled with promotional conduct. The 2024 _Bungie v. AimJunkies_ ruling ($4.3M arbitration award) and ongoing enforcement trends make this a material risk for any tool that facilitates discovery and download of trainer software.

**Key legal facts**:

- Trainers that bypass anti-cheat systems (e.g., EasyAntiCheat, BattlEye) clearly implicate §1201.
- Trainers for single-player offline games operate in a legal gray area — personal use may be exempt under the 2024 triennial exemption, but §1201(a)(2) **anti-trafficking provisions are not covered by the exemption**.
- DMCA safe harbor (Section 512) protects platforms from _third-party copyright infringement_ but does **not** cover active linking or promotion of circumvention tools under §1201.
- The exemption for personal use does not extend to tools that _distribute or facilitate access to_ circumvention tools.

**Confidence**: Medium-High — legal analysis based on published case law and U.S. Copyright Office rulings. Not a substitute for legal counsel.

**How similar tools handle this**:

- **Nexus Mods**: Operates as a hosting platform with DMCA takedown compliance. Mods (not trainers) are a different legal category. Nexus does not host trainers.
- **ProtonDB**: Aggregates compatibility _reports_ — user-generated text — not links to executable downloads. Fundamentally different liability profile.
- **FLiNG Trainers, WeMod**: These services host trainers directly and accept the legal risk themselves.

**Mitigations CrossHook must implement**:

1. **Scope discovery to single-player/offline trainers only**: The feature spec should explicitly exclude trainers for games with active online anti-cheat. This reduces (but does not eliminate) §1201 risk. Display a UI warning that trainer discovery is intended for single-player use only.

2. **No curation, no endorsement**: CrossHook must make clear it is a passive index of community-curated tap data, not an endorsement of any trainer source. This mirrors how search engines disclaim liability for search results.

3. **Display a legal disclaimer**: Before first use of the discovery feature, display a one-time acknowledgment that the user is responsible for compliance with applicable laws, and that CrossHook does not host, distribute, or endorse trainers.

4. **No automated download or execution**: The feature must remain link-only. CrossHook must never fetch, cache, or execute trainer executables as part of discovery. This is already the stated design intent and must be enforced in code.

5. **Consult legal counsel**: Before shipping this feature publicly, obtain qualified legal advice specific to CrossHook's jurisdiction and user base. This research is informational, not legal advice.

6. **Consider a geo-restriction or opt-in flag**: If legal risk is judged unacceptable, make the discovery feature an explicit opt-in (`discovery_enabled: false` default in settings) with a disclosure at opt-in time.

---

## Dependency Security

### Finding S8 (ADVISORY): Audit existing and new crates before trainer source fetch path

**Current `crosshook-core` dependencies relevant to discovery**:

| Crate        | Version              | Risk Surface                             |
| ------------ | -------------------- | ---------------------------------------- |
| `rusqlite`   | 0.39.0               | SQLite query execution for FTS and cache |
| `reqwest`    | 0.12                 | HTTP fetch for trainer source metadata   |
| `sha2`       | 0.11.0               | Hash verification                        |
| `toml`       | 1.1.0                | Tap metadata parsing                     |
| `serde_json` | 1                    | JSON trainer source index parsing        |
| `url`        | indirect via reqwest | URL validation                           |

**No new crates are required for Phase 1** if the link-only + community tap model is used (as recommended in `research-practices.md`). The existing `rusqlite` + `reqwest` + `sha2` stack covers all needs.

**If FTS5 is added** (new `community_profiles_fts` virtual table via migration): FTS5 is built into SQLite bundled by `rusqlite` — no new crate is needed.

**Recommended pre-ship actions**:

```bash
# Run from src/crosshook-native/
cargo audit
cargo deny check
cargo tree -d  # Check for duplicate/conflicting dependency versions
```

**RustSec advisories to watch**:

- `rusqlite`: No active critical advisories as of April 2025. Bundle SQLite is kept current with `rusqlite` releases.
- `reqwest 0.12`: Uses `hyper 1.x`. Check for HTTP smuggling advisories (RUSTSEC-2024-0042 affected hyper 0.14).
- `toml`: No known injection vulnerabilities; parsing is type-safe via `serde`.

**Supply chain risk for community taps**: Taps are git repositories. A compromised tap maintainer account could push malicious `trainer-sources.json` content. This is mitigated by content validation at index time (see S2) and user responsibility for vetting taps they add.

---

## 7. Integration with Existing Security

### Finding S9 (ADVISORY): Community tap SHA-256 fields should integrate with launch-time verification

**Existing infrastructure**: `offline/hash.rs` implements `verify_and_cache_trainer_hash()`, which stat-checks the file, compares size and mtime against the `trainer_hash_cache` SQLite table (schema v13), and re-hashes only when stale. The `community_schema.rs` `CommunityProfileMetadata` struct already has `trainer_sha256: Option<String>`. The launch-time hash advisory mechanism (`launch/trainer_hash.rs`) uses this field. The `normalize_sha256_hex()` utility is already available for canonicalizing any SHA-256 string from community metadata.

**Discovery integration path**: When a community tap's `trainer-sources.json` publishes an expected SHA-256 for a trainer download, that hash should be:

1. Stored in the discovery index (`trainer_sources` table, `expected_sha256` column).
2. Surfaced in the discovery UI as a "verified hash" indicator — not enforced at link-open time (since the user hasn't downloaded yet).
3. After the user downloads the trainer and adds it as a profile, the existing `verify_and_cache_trainer_hash` mechanism handles the on-disk verification.

This creates an end-to-end integrity chain: tap publisher asserts expected hash → discovery surfaces it → launch verifier enforces it.

**Phase 1 scope**: Store and display the hash if present. Enforcement remains at launch time via existing code. No new verification at the discovery layer.

---

## 8. Existing Security Infrastructure — What Already Applies

The following existing mechanisms in the codebase already provide defense-in-depth for trainer discovery without requiring new code:

| Mechanism                                                                                                           | Location                                       | Applies to Discovery                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `validate_tap_url()` — scheme allowlist (`https://`, `ssh://git@`, `git@`); rejects `file://`, `git://`, bare paths | `community/taps.rs:485`                        | Protects tap _subscription_ URLs; trainer source `download_url` fields need an equivalent (HTTPS-only, stricter) validator                                              |
| `slugify()` — strips non-alphanumeric to hyphens                                                                    | `community/taps.rs:533`                        | Use for cache key derivation from external strings; reuse `slugify_profile_name()` in `install/service.rs:289` for any file-path components derived from discovery data |
| `is_valid_git_sha()` — validates 7–64 hex chars                                                                     | `community/taps.rs`                            | Already validates `pinned_commit`; extend to validate any SHA-256 fields from discovery metadata (64 hex chars specifically)                                            |
| `MAX_CACHE_PAYLOAD_BYTES = 524_288`                                                                                 | `metadata/models.rs:152`                       | Caps `external_cache_entries` payload size; automatically applied by `put_cache_entry` in `cache_store.rs`                                                              |
| Git env isolation: `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`                  | `community/taps.rs:495–499`                    | Applies to all git subprocess calls during tap sync; no changes needed                                                                                                  |
| `verify_and_cache_trainer_hash()` + `trainer_hash_cache` table                                                      | `offline/hash.rs`, `metadata/offline_store.rs` | Hash verification at launch time; discovery surfaces expected hashes, enforcement is at launch                                                                          |
| `network isolation (unshare --net)` per-profile                                                                     | Launch infrastructure                          | Available for trainer process execution; not applicable at the discovery (link-only) layer                                                                              |

**Key design note**: The `network isolation (unshare --net)` protection applies to launched trainer processes — not to the CrossHook app itself. Discovery fetches run in-process with normal network access (gated by `reqwest` + `rustls`). This is correct behavior for a link aggregator and does not need to change.

---

## 9. Secure Coding Guidelines

The following patterns are required for all new trainer discovery code:

### SQL Query Safety

All SQLite queries in `discovery/search.rs` and `metadata/discovery_index.rs` must use parameterized queries via `rusqlite::params![]`. String interpolation into SQL is forbidden.

```rust
// CORRECT
conn.query_row(
    "SELECT * FROM community_profiles WHERE game_name LIKE ?1",
    rusqlite::params![format!("%{}%", sanitized_query)],
    |row| { ... }
)?;

// NEVER
let sql = format!("SELECT * FROM community_profiles WHERE game_name LIKE '%{}%'", user_input);
conn.query_row(&sql, [], |row| { ... })?;
```

Note: Do NOT enable `load_extension` in the `rusqlite` connection used for discovery — the rusqlite docs warn that enabling extension loading allows SQL queries to escalate injection into code execution during the `load_extension_enable` window.

### URL Storage and Rendering

```rust
// At index time — validate before storing
let validated_url = validate_trainer_source_url(&raw_url)
    .map_err(|e| tracing::warn!("skipping invalid trainer source URL: {e}"))?;
let url_str = validated_url.as_str().to_string();

// In SQLite — store as TEXT
// In frontend — open only via Tauri shell plugin, never via href navigation
```

### Field Length Enforcement

Apply the same pattern as `community_index.rs` A6 guards:

```rust
const MAX_URL_BYTES: usize = 2048;
const MAX_DISPLAY_NAME_BYTES: usize = 512;
const MAX_VERSION_BYTES: usize = 128;

fn clamp_str(s: &str, max_bytes: usize) -> &str {
    let end = s.char_indices()
        .scan(0usize, |acc, (i, c)| {
            *acc += c.len_utf8();
            Some((i, *acc))
        })
        .take_while(|(_, acc)| *acc <= max_bytes)
        .last()
        .map(|(i, _)| i + 1)
        .unwrap_or(0);
    &s[..end]
}
```

### Error Messages

IPC command handlers must not return raw SQLite error strings to the frontend. Map errors with `map_err(|e| e.to_string())` at the command boundary (existing convention), but ensure internal paths and SQL fragments are not included in `Display` implementations of discovery errors.

### Frontend Trust Boundaries

```tsx
// All trainer source data from IPC is UNTRUSTED. Apply:
// 1. No dangerouslySetInnerHTML with tap-sourced content
// 2. URL displayed as text; open via Tauri shell plugin
// 3. Game name / trainer name: render as {text}, never as HTML
```

---

## 10. Trade-off Recommendations

| Decision                                 | Recommended Choice                                   | Rationale                                                                        |
| ---------------------------------------- | ---------------------------------------------------- | -------------------------------------------------------------------------------- |
| Link-only vs. download-and-execute       | Link-only (no download in CrossHook)                 | Eliminates most DMCA trafficking risk surface; consistent with stated design     |
| Domain allowlist for trainer source URLs | Optional, maintained by tap, not CrossHook           | CrossHook should not be the arbiter of trusted trainer sites                     |
| FTS5 vs. LIKE search for Phase 1         | LIKE on indexed columns                              | Lower attack surface; no FTS5 query injection risk; sufficient for Phase 1       |
| SHA-256 integration                      | Display from tap metadata; enforce at launch         | Consistent with existing architecture; no new enforcement path needed            |
| Legal disclaimer                         | Mandatory one-time consent before enabling discovery | Reduces CrossHook's endorsement liability; informs users                         |
| `cargo audit` in CI                      | Add to release workflow before shipping discovery    | Zero-cost safety net                                                             |
| Community tap signature verification     | Defer to Phase 2                                     | High infrastructure cost; content validation at ingest is sufficient for Phase 1 |

---

## 11. Open Questions

1. **Legal jurisdiction**: Is CrossHook's primary user base in a jurisdiction where anti-trafficking provisions of the DMCA apply? EU/UK users may face different (or no) equivalent provisions. Does the team want legal counsel to assess this before shipping?

2. **Anti-cheat scope filtering**: Should CrossHook automatically filter out trainer discovery results for games known to have online anti-cheat (e.g., via a ProtonDB-sourced or community-curated anti-cheat list)? This is a significant scope question that affects both legal risk and user experience.

3. **Tap vetting process**: Does the CrossHook project intend to publish an "official" curated tap, or is the trust model entirely user-managed? An official tap would require the project to accept the legal and reputational risk of the trainer source URLs it includes.

4. **Revocation mechanism**: If a community tap is found to be publishing malicious trainer source URLs, how does CrossHook notify users and remove the tap's data from their local index? There is currently no revocation mechanism.

5. **Cross-referencing trainer SHA-256 against known-bad hashes**: Could the community tap ecosystem be used to publish a blocklist of SHA-256 hashes for known malicious trainer binaries? This would turn the hash verification infrastructure into a security feature rather than just an integrity check.

---

## 12. Sources

- [U.S. Copyright Office: DMCA §1201](https://www.copyright.gov/dmca/)
- [EFF: DMCA §1201 Anti-Circumvention](https://www.eff.org/issues/dmca)
- [Vondranlegal: Cheat Software and the DMCA](https://www.vondranlegal.com/cheat-software-free-speech-and-the-dmca-understanding-17-u-s-c-1201)
- [Copyright and Cheating in Video Games – Plagiarism Today 2024](https://www.plagiarismtoday.com/2024/05/30/copyright-and-cheating-in-video-games/)
- [Section 1201 Ninth Triennial Rulemaking 2024 – U.S. Copyright Office](https://www.copyright.gov/1201/2024/2024_Section_1201_Registers_Recommendation.pdf)
- [Tauri v2 Content Security Policy](https://v2.tauri.app/security/csp/)
- [Tauri v2 Security Overview](https://v2.tauri.app/security/)
- [Beyond Electron: Attacking Desktop App Frameworks – Bishop Fox](https://bishopfox.com/blog/beyond-electron-attacking-alternative-desktop-application-frameworks)
- [rusqlite Params Documentation](https://docs.rs/rusqlite/latest/rusqlite/trait.Params.html)
- [rusqlite: Is SQL injection prevented? #820](https://github.com/rusqlite/rusqlite/issues/820)
- [Rust SQL Injection Guide – StackHawk](https://www.stackhawk.com/blog/rust-sql-injection-guide-examples-and-prevention/)
- [servo/rust-url – URL Parser for Rust](https://github.com/servo/rust-url)
- [RustSec Advisory Database](https://rustsec.org/)
- [cargo-audit on crates.io](https://crates.io/crates/cargo-audit)
- [TARmageddon CVE-2025-62518 – Edera](https://edera.dev/stories/tarmageddon)
- [GitHub Supply Chain Attack Advisory 2024 – Barracuda](https://blog.barracuda.com/2024/04/05/cybersecurity-threat-advisory-github-supply-chain-attack)
- [OWASP Cache Poisoning](https://owasp.org/www-community/attacks/Cache_Poisoning)
- [Web Cache Poisoning – PortSwigger](https://portswigger.net/web-security/web-cache-poisoning)
- [Rust Path Traversal Guide – StackHawk](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/)
- [Nexus Mods Terms of Service](https://help.nexusmods.com/article/18-terms-of-service)
- [Website External Links Legal Liability](https://www.internetlegalattorney.com/website-external-links-framing-liability-guide/)
- [Rust Security Best Practices 2025 – Corgea](https://corgea.com/Learn/rust-security-best-practices-2025)
