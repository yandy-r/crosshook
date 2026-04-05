# Security Research: ML-Assisted Configuration

Feature: ProtonDB-derived ML configuration suggestions (Issue #77)

**Reviewed against actual implementation** in
`src/crosshook-native/crates/crosshook-core/src/protondb/` (aggregation.rs, client.rs, tests.rs).

---

## Executive Summary

The existing ProtonDB module has solid security foundations (format-validated env keys, shell metacharacter rejection, parameterized SQL, request timeouts). The critical gap is S2: the env var key blocklist does not cover Linux process injection vectors (`LD_PRELOAD`, `PATH`, `HOME`). This must be fixed before any "apply suggestion to profile" flow ships. All other findings are WARNING or ADVISORY with straightforward mitigations.

## Findings by Severity

| ID  | Area                         | Finding                                                                       | Severity | Status               |
| --- | ---------------------------- | ----------------------------------------------------------------------------- | -------- | -------------------- |
| S1  | Input Validation / Injection | `is_safe_env_key` uses pattern-match allowlist but is not name-allowlisted    | WARNING  | Partially mitigated  |
| S2  | Input Validation / Injection | `RESERVED_ENV_KEYS` blocklist missing LD_PRELOAD, LD_LIBRARY_PATH, PATH, HOME | CRITICAL | Not mitigated        |
| S3  | Input Validation / Injection | `is_safe_env_value` does not reject newline (`\n`, `\r`)                      | WARNING  | Partial gap          |
| S4  | Input Validation / Injection | "Copy-only" raw launch strings stored and surfaced without sanitization       | WARNING  | Needs review         |
| S5  | Input Validation / Injection | XSS via unsanitized report text rendered in React frontend                    | WARNING  | Needs frontend audit |
| S6  | Infrastructure               | No HTTP response size limit before `response.json()` deserialization          | WARNING  | Not mitigated        |
| S7  | Infrastructure               | Cache TTL is 6 hours — stale poisoned responses persist through TTL window    | ADVISORY | Acceptable           |
| S8  | Dependency Security          | reqwest/hyper RUSTSEC-2024-0042 — pin to hyper >= 0.14.26 or reqwest >= 0.12  | WARNING  | Needs verification   |
| S9  | Dependency Security          | cargo-audit not confirmed in CI                                               | ADVISORY | Needs verification   |

---

## What the Existing Implementation Gets Right

Before detailing gaps, the existing code has solid foundations worth preserving:

- **`is_safe_env_key`** (aggregation.rs:300–308): Requires uppercase ASCII or underscore start, and only uppercase/digit/underscore characters throughout. This correctly rejects lowercase keys, hyphenated keys (`bad-name`), and multi-word shell constructs.
- **`is_safe_env_value`** (aggregation.rs:310–322): Rejects null bytes, all whitespace, and the shell metacharacters `$ ; " ' \ \` | & < > ( ) %`. This is a solid set.
- **`%command%` stripping** (aggregation.rs:261–265): `safe_env_var_suggestions` splits on `%command%` and only parses the prefix. This correctly prevents Steam-style command injection patterns.
- **`RESERVED_ENV_KEYS`** blocklist: Blocks `WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, and the `STEAM_COMPAT_` prefix family.
- **User-Agent set** (client.rs:182): `CrossHook/{version}` — correct and already in place.
- **6-second request timeout** (client.rs:25): Prevents hung connections.
- **Parameterized SQLite queries** (client.rs:379–380): `params![cache_key]` — no SQL injection risk.
- **Error containment**: `ProtonDbError` variants don't leak internal paths to callers; `tracing::warn!` used for internal logging only.
- **Test coverage**: tests.rs covers the critical env var parsing paths including `WINEPREFIX` rejection, `BAD-NAME` rejection, and `%command%` prefix isolation.

---

## S2 — Missing Dangerous Linux Env Var Blocklist (CRITICAL)

### The Gap

`RESERVED_ENV_KEYS` only blocks Wine/Steam-specific path vars. It does not block Linux-level process injection vectors.

`is_safe_env_key` allows any key that matches `[A-Z_][A-Z0-9_]*`. This pattern accepts:

```
LD_PRELOAD         → shared library injection → code execution in subprocess
LD_LIBRARY_PATH    → library search path hijacking
LD_AUDIT           → audit library injection
LD_DEBUG           → information disclosure
PATH               → binary hijacking in subprocess
HOME               → shell startup file redirection
ZDOTDIR            → zsh startup file redirection
SHELL              → interpreter override
NODE_OPTIONS       → arbitrary Node.js code execution
PYTHONPATH         → Python module hijacking
```

A ProtonDB report with `launch_options: "LD_PRELOAD=/home/user/libevil.so %command%"` would currently pass `is_safe_env_key` and `is_safe_env_value`, not appear in `RESERVED_ENV_KEYS`, and be returned as a valid `ProtonDbEnvVarSuggestion`.

### Impact

If a user accepts this suggestion and it is applied at game launch time, the malicious `.so` executes in the Wine/Proton process context. This is a well-documented Linux privilege escalation / code execution vector.

### Mitigation

Extend `RESERVED_ENV_KEYS` (or a parallel `BLOCKED_ENV_KEY_PREFIXES` constant) to cover dynamic linker and shell startup vars:

```rust
const RESERVED_ENV_KEYS: &[&str] = &[
    // Existing — Wine/Steam path control
    "WINEPREFIX",
    "STEAM_COMPAT_DATA_PATH",
    "STEAM_COMPAT_CLIENT_INSTALL_PATH",
    // Dynamic linker injection (Linux-specific, high severity)
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "LD_DEBUG",
    "LD_ORIGIN_PATH",
    "LD_PROFILE",
    // Process/shell environment hijacking
    "PATH",
    "HOME",
    "ZDOTDIR",
    "SHELL",
    "ENV",
    "BASH_ENV",
    // Interpreter-specific code execution
    "NODE_OPTIONS",
    "PYTHONPATH",
    "RUBYLIB",
    "PERL5LIB",
];

const BLOCKED_ENV_KEY_PREFIXES: &[&str] = &[
    "STEAM_COMPAT_",
    "LD_",       // catch any future LD_* additions
];
```

Update the guard in `safe_env_var_suggestions`:

```rust
if RESERVED_ENV_KEYS.contains(&normalized_key)
    || BLOCKED_ENV_KEY_PREFIXES.iter().any(|p| normalized_key.starts_with(p))
{
    continue;
}
```

**Confidence**: High — LD_PRELOAD privilege escalation is a documented and widely exploited Linux technique.

---

## S1 — No Positive Name Allowlist (WARNING)

### The Gap

The existing approach is blocklist-based for dangerous keys, pattern-based for syntax. A positive allowlist approach would be more robust: only keys that are known to be safe are suggested, regardless of whether they happen to pass the pattern check.

This matters because the PROTON*\* and DXVK*\* namespaces are large, and some less-documented variables have side effects. For example, `PROTON_EXTRA_ARGS` appends raw strings as command arguments — if this variable passed validation, its value (even after `is_safe_env_value` filtering) could be used in unintended ways depending on how the launch infrastructure handles it.

### Assessment

This is a WARNING rather than CRITICAL because:

1. The existing `is_safe_env_key` + `is_safe_env_value` + `RESERVED_ENV_KEYS` combination provides meaningful defense.
2. A positive allowlist is a deeper security posture but requires maintaining a curated list.
3. S2 (LD_PRELOAD family) is the most urgent gap; fixing S2 significantly reduces S1's practical risk surface.

### Mitigation Options

**Option A (Recommended short-term)**: Fix S2 first. The pattern-based approach with an expanded blocklist is pragmatically sound for the Proton/DXVK ecosystem where new variables emerge regularly.

**Option B (Recommended long-term)**: Add a positive allowlist for known-safe keys and treat unknown keys as copy-only suggestions (displayed but not auto-applicable as env vars). This is architecturally cleaner and future-proof.

---

## S3 — `is_safe_env_value` Missing Newline Characters (WARNING)

### The Gap

`is_safe_env_value` correctly rejects whitespace via `ch.is_whitespace()`. However, it is worth verifying that `\n` and `\r` specifically are covered, since these can terminate environment variable values in some contexts and create log injection opportunities.

In Rust, `'\n'.is_whitespace()` returns `true`, so newlines are already blocked. This is a **confirmed non-issue** on closer analysis.

The `normalize_text` function already strips null bytes (`replace('\0', "")`). The value validator also rejects null via `value.contains('\0')`.

**Status: No gap — existing code handles this correctly.**

---

## S4 — "Copy-Only" Launch Strings Surfaced Without Sanitization (WARNING)

### The Gap

When `safe_env_var_suggestions` finds no parseable env vars, or when `launch_string_needs_copy_only` returns true, the raw `launch_options` string is stored verbatim as a `ProtonDbLaunchOptionSuggestion.text` and presented to the user as a "Copy-only launch string."

This raw string:

- Has not been through `is_safe_env_value` (only env var values pass through that filter)
- May contain shell metacharacters (`$`, `\``,`|`, etc.)
- Is labeled "Copy-only" implying the user should paste it directly into Steam or CrossHook

If a user pastes a copy-only suggestion into a field that gets shell-interpreted at launch time, the metacharacters become injection vectors.

### Assessment

The risk depends on what CrossHook does with copy-only strings downstream. If they are only displayed for user reference and never programmatically applied to any subprocess, this is informational. If there is any path where a copy-only string gets applied to a launch configuration, it needs sanitization.

### Mitigation

1. Confirm in the IPC layer and profile writer that `copy-only` launch strings are never automatically applied to subprocess arguments or env vars.
2. In the UI, label copy-only strings clearly as "for manual Steam use only" — not for CrossHook profiles.
3. Optionally, apply a lighter sanitization pass to copy-only strings (strip null bytes, truncate to max length) before storage, even if not the full `is_safe_env_value` filter.

---

## S5 — XSS via Report Text in React Frontend (WARNING)

### The Gap

`notes` (from `concluding_notes`), `source_label` (constructed from `proton_version`, `variant`, `notes.variant`, and `report.id`), and copy-only launch strings all flow through to the frontend as string data in `ProtonDbRecommendationGroup`.

The source_label construction (aggregation.rs:209–234) concatenates user-submitted text into strings like `"Custom Proton: {custom_variant}"` and `"Variant: {variant}"`. These strings are not HTML-sanitized.

If the React frontend renders any of these fields via `dangerouslySetInnerHTML`, attacker-controlled values in those fields execute in the WebView context.

### Mitigation

1. Audit all React components that render `ProtonDbRecommendationGroup` fields. Confirm they use plain-text interpolation (`{value}`) not `dangerouslySetInnerHTML`.
2. In Rust, the `normalize_text` function (aggregation.rs:248–250) currently only trims and strips null bytes. Consider adding HTML entity stripping (rejecting `<` and `>`) as a defense-in-depth measure for text that will be displayed in a web context.

---

## S6 — No HTTP Response Size Limit (WARNING)

### The Gap

In `client.rs`, both `fetch_summary` and the report feed fetch call `response.json::<T>()` directly without checking response size first. `reqwest`'s `.json()` will buffer the entire response body in memory before deserializing.

A malformed or intentionally large response (e.g., from a network interception or a ProtonDB endpoint returning an error page) could allocate unbounded memory.

```rust
// client.rs:209 — no size guard
response
    .error_for_status()
    ...
    .json::<ProtonDbSummaryResponse>()
    .await
    .map_err(ProtonDbError::Network)
```

### Mitigation

Add a `bytes()` step with a size cap before deserialization:

```rust
const MAX_RESPONSE_BYTES: usize = 1_048_576; // 1 MB

async fn fetch_json_bounded<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, ProtonDbError> {
    let bytes = response
        .bytes()
        .await
        .map_err(ProtonDbError::Network)?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(ProtonDbError::Network(/* reqwest::Error */ todo!()));
    }
    serde_json::from_slice(&bytes).map_err(|e| ProtonDbError::Network(/* wrap */ todo!()))
}
```

Alternatively, add a `ProtonDbError::ResponseTooLarge` variant and use `bytes_stream()` with a byte counter. The exact approach is an implementation detail, but the size guard is required.

**Confidence**: High — standard HTTP client hardening, not hypothetical.

---

## S7 — Cache TTL and Stale Data (ADVISORY)

### Assessment

The existing TTL is 6 hours (`CACHE_TTL_HOURS = 6`). This is reasonable. Stale-on-failure fallback (`allow_expired = true`) correctly serves degraded data rather than crashing.

The `expires_at` is set at write time based on `fetched_at + 6h` and enforced in the SQL query (line 370: `expires_at > ?2`). This is correct.

**Status: No action required. The TTL implementation is sound.**

---

## Dependency Security

### S8 — reqwest/hyper Version (WARNING)

### The Gap

RUSTSEC-2024-0042 affects `hyper < 0.14.26`. The project already uses `reqwest` — the transitive `hyper` version depends on which `reqwest` release is pinned.

### Mitigation

Run `cargo tree -p hyper` to confirm the resolved `hyper` version. Ensure it is `>= 0.14.26` (or `>= 1.0` if using `reqwest >= 0.12`). Add `cargo audit` to CI to catch future advisories automatically.

---

## S9 — cargo-audit in CI (ADVISORY)

Standard hygiene for any Rust project adding dependencies. Integrate as a CI step:

```yaml
- name: Security audit
  run: cargo audit
```

---

## Secure Coding Guidelines Specific to This Feature

### 1. Expand RESERVED_ENV_KEYS (S2 — CRITICAL, fix before ship)

See S2 section above. Minimum additions: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `PATH`, `HOME`, `ZDOTDIR`, `SHELL`, `NODE_OPTIONS`, `PYTHONPATH`. Use a prefix block for `LD_` as future-proofing.

### 2. Frontend Rendering Rule

All ProtonDB-derived text (notes, source labels, launch option strings) must use React plain-text interpolation. Never `dangerouslySetInnerHTML`. This applies to every component that renders `ProtonDbRecommendationGroup` fields.

### 3. Response Size Guard

Add the size cap to both `fetch_summary` and the report feed fetch before calling `.json()`. 1 MB is a reasonable ceiling for ProtonDB responses.

### 4. Copy-Only String Disposition

Clarify and document in code whether copy-only launch strings are: (a) display-only for user reference, or (b) applicable to CrossHook profile fields. If (b), they need the same validation pass as env var values, or a separate sanitization step.

### 5. Re-Validate at Application Time

When a user accepts a suggestion and it is written to a TOML profile, re-run `is_safe_env_key` and `is_safe_env_value` (and the reserved key check including the expanded S2 list) at write time. Do not trust cached suggestion data as pre-validated.

### 6. Test Coverage for S2 Gap

Add tests for the newly blocked keys:

```rust
#[test]
fn ld_preload_is_rejected_as_env_suggestion() {
    let groups = normalize_report_feed(feed(vec![ProtonDbReportEntry {
        id: "injection-attempt".to_string(),
        timestamp: 1,
        responses: ProtonDbReportResponses {
            launch_options: "LD_PRELOAD=/evil.so PROTON_LOG=1 %command%".to_string(),
            ..ProtonDbReportResponses::default()
        },
    }]));
    // Should produce env vars with only PROTON_LOG=1; LD_PRELOAD must be absent
    let env_group = groups.iter().find(|g| g.title == "Suggested environment variables");
    assert!(env_group.is_some());
    let vars = &env_group.unwrap().env_vars;
    assert!(vars.iter().all(|v| v.key != "LD_PRELOAD"));
    assert!(vars.iter().any(|v| v.key == "PROTON_LOG"));
}
```

---

## Trade-off Recommendations

| Decision                             | Recommendation                                                                               | Rationale                                                           |
| ------------------------------------ | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| Blocklist vs. positive allowlist     | Expand blocklist now (fix S2); evaluate positive allowlist for v2 of this feature            | Blocklist is lower friction for a fast-moving env var ecosystem     |
| Copy-only strings: apply vs. display | Display-only, clearly labeled. Never auto-apply.                                             | Eliminates downstream injection risk without removing user value    |
| Response size cap implementation     | Add a `MAX_RESPONSE_BYTES` constant and `bytes()` guard before `.json()` in both fetch paths | Minimal code change, high protection value                          |
| Re-validation at TOML write time     | Yes — defense-in-depth; cache data should not be treated as pre-sanitized                    | Guards against cache tampering and future parser regressions        |
| cargo-audit in CI                    | Add to CI pipeline; low cost, catches transitive dep CVEs automatically                      | Ecosystem standard, protects entire workspace not just this feature |

---

## Open Questions

1. **What does the profile writer do with accepted env var suggestions?** Does it call `is_safe_env_key` / `is_safe_env_value` again, or trust the suggestion struct? Re-validation at write time is required.
2. **Are copy-only launch strings ever programmatically applied?** If yes, they need full sanitization. If display-only, document that invariant in code.
3. **Which React components render ProtonDB text fields?** Need frontend audit for XSS surface (S5).
4. **What is the current resolved `hyper` version?** Run `cargo tree -p hyper` to verify RUSTSEC-2024-0042 status.

---

## Sources

- [OWASP OS Command Injection Defense Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/OS_Command_Injection_Defense_Cheat_Sheet.html)
- [Rust std::process::Command — env_clear()](https://doc.rust-lang.org/std/process/struct.Command.html)
- [Unusual LD_PRELOAD/LD_LIBRARY_PATH Detection — Elastic Security](https://www.elastic.co/guide/en/security/8.19/unusual-ld-preload-ld-library-path-command-line-arguments.html)
- [Linux Privilege Escalation using LD_PRELOAD — HackingArticles](https://www.hackingarticles.in/linux-privilege-escalation-using-ld_preload/)
- [Shell startup env injection bypass (RCE class) — GHSA-xgf2-vxv2-rrmg](https://github.com/openclaw/openclaw/security/advisories/GHSA-xgf2-vxv2-rrmg)
- [RustSec Advisory Database — RUSTSEC-2024-0042](https://rustsec.org/advisories/)
- [Tauri v2 Security Documentation](https://v2.tauri.app/security/)
- [ProtonDB Data — bdefore/protondb-data](https://github.com/bdefore/protondb-data)
- [Two Malicious Rust Crates — Socket.dev](https://socket.dev/blog/two-malicious-rust-crates-impersonate-popular-logger-to-steal-wallet-keys)
