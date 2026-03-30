# Security Research: `config-history-rollback`

Date: 2026-03-30  
Scope: Rust (`crosshook-core`) + SQLite metadata store + Tauri command surface

## Executive Summary

`config-history-rollback` should be implemented as a tamper-evident, bounded, and auditable subsystem. The highest risk is accepting poisoned rollback payloads or applying rollback data to the wrong profile state. The safest architecture is:

1. Store snapshot history in SQLite (not user-controlled paths).
2. Make snapshot records tamper-evident (content hash + chain link; optional HMAC).
3. Keep rollback operations append-only auditable.
4. Enforce strict per-profile and global size/count limits to prevent resource exhaustion.
5. Reuse and strengthen existing profile/path validation boundaries in `crosshook-core`.

## Existing Baseline Controls In CrossHook

- Profile names are already constrained by `validate_name()` (rejects absolute paths, separators, traversal-like names, Windows reserved chars) in `crates/crosshook-core/src/profile/toml_store.rs`.
- Metadata DB already enforces secure file/dir permissions (`0700` dir, `0600` file), symlink check, WAL mode, foreign keys, and `PRAGMA quick_check` in `crates/crosshook-core/src/metadata/db.rs`.
- Metadata layer already uses bounded payload patterns:
  - `MAX_DIAGNOSTIC_JSON_BYTES = 4096`
  - `MAX_CACHE_PAYLOAD_BYTES = 524288`
  - `MAX_VERSION_SNAPSHOTS_PER_PROFILE = 20`

These are strong patterns to copy for config history.

## Threat Model (Feature-Specific)

- Untrusted input can arrive at rollback command boundaries (frontend IPC inputs, imported data, manipulated local files).
- Local attacker (or malware) can attempt DB/file tampering at rest.
- Compromised/buggy frontend can issue abusive rollback/history requests.
- Large history payloads can exhaust disk, memory, or CPU.

## Findings And Recommendations

## 1) Tamper/Poisoning Risks For Snapshots And Rollback Payloads

### CRITICAL: Enforce snapshot authenticity and applicability before rollback

**Risk**: A crafted or modified snapshot could apply attacker-controlled values or rollback the wrong profile state (CWE-345, CWE-502 class concerns for untrusted structured data).  
**Confidence**: High (multiple authoritative sources align on authenticity verification and untrusted data handling).

Actionable implementation:

- Add a `profile_config_snapshots` table with:
  - `snapshot_id` (UUID)
  - `profile_id` (FK to stable profile identity, not profile filename)
  - `snapshot_kind` (`full` or `diff`)
  - `payload_json` (or compressed blob)
  - `payload_sha256`
  - `prev_snapshot_sha256` (hash-chain link)
  - `base_profile_content_hash` (from current profile sync hashing model)
  - `created_at`
- On create:
  - Serialize payload deterministically.
  - Compute `payload_sha256`.
  - Store `prev_snapshot_sha256` as current head hash for that profile.
- On rollback (must fail closed):
  - Verify `payload_sha256` matches stored payload.
  - Verify `profile_id` matches target profile.
  - Verify chain continuity from current head or selected snapshot ancestry.
  - Verify `base_profile_content_hash` compatibility (or run explicit merge/conflict path).
  - Abort on any mismatch with a structured error.
- Optional hardening:
  - Store `hmac_sha256(payload)` with key from OS keyring/secret store for tamper evidence beyond plain hashes.

Rust/SQLite note:

- Perform verification and apply in a single transaction (`BEGIN IMMEDIATE` pattern) to avoid TOCTOU between verify and write.

## 2) Path Traversal / Profile Name Validation Interactions

### WARNING: Keep history addressing independent from filenames and canonicalize after resolution

**Risk**: Traversal can re-enter through secondary code paths (export/import/history file storage), especially if rollback uses profile names or file paths directly (CWE-22, CWE-180).  
**Confidence**: High (CWE and OWASP guidance are explicit on canonicalization and allowlist strategy).

Actionable implementation:

- Primary key for rollback targeting should be `profile_id`, with profile name only as display metadata.
- If filesystem storage is ever used for snapshot blobs:
  - Never derive filenames from profile names.
  - Use opaque IDs (`snapshot_id.bin`), fixed extension, and server-side path construction only.
  - Canonicalize resolved path and enforce `starts_with(base_dir)` after canonicalization.
  - Reject symlink targets for snapshot files (mirror existing DB symlink defense).
- Keep `validate_name()` as gate for UI-facing profile naming but do not treat it as sole traversal defense.

Rust/Tauri note:

- Treat every Tauri command argument as untrusted (even from first-party frontend); validate in command handler and in core service.

## 3) Secure Handling Of Sensitive Profile Fields In History

### WARNING: Minimize and classify retained data; avoid raw sensitive history blobs in logs

**Risk**: Snapshot history can silently become a sensitive-data archive (local paths, tokens/env vars if added later, operational metadata), increasing disclosure impact.  
**Confidence**: High (OWASP logging and cryptographic storage guidance strongly support minimization and masking).

Actionable implementation:

- Define a sensitivity classification for profile fields now:
  - `public`: non-sensitive toggles.
  - `sensitive-local`: filesystem paths, launch arguments, environment values.
  - `secret` (future-proof): credentials/tokens if ever introduced.
- Store history as structured diffs with field-level policy:
  - `public`: full value allowed.
  - `sensitive-local`: store hashed or redacted display value for UI/audit; keep full value only if required for rollback correctness.
  - `secret`: do not persist in history unless encrypted-at-rest with separate key management.
- Log policy:
  - Never log raw snapshot payloads.
  - Log only snapshot IDs, profile IDs, byte lengths, hash prefixes, result codes.

SQLite note:

- If full sensitive rollback values must be stored, prefer app-level encryption for those columns/fields; DB file permissions alone are not equivalent to cryptographic protection.

## 4) Integrity Checks And Auditability For Rollback Actions

### CRITICAL: Implement append-only rollback audit trail with verifiable before/after state

**Risk**: Without immutable audit records, malicious or accidental rollbacks are hard to detect and impossible to investigate confidently.  
**Confidence**: High (OWASP logging guidance and Tauri trust-boundary model support explicit audit events).

Actionable implementation:

- Add `profile_config_rollback_audit` table:
  - `event_id`, `profile_id`, `snapshot_id`, `requested_by` (`ui`, `command`, `startup-recovery`), `requested_at`
  - `pre_apply_profile_hash`, `post_apply_profile_hash`
  - `result` (`success`, `rejected_integrity`, `rejected_conflict`, `error`)
  - `reason_code`, `error_class`
- Write audit event inside the same transaction as rollback apply.
- Record both successful and rejected rollback attempts.
- Restrict destructive maintenance:
  - No hard delete/update on audit rows in normal code paths.
  - If cleanup is required, make it explicit retention policy with its own audit row.
- Include `trace_id/operation_id` from Tauri command boundary to Rust logs for correlation.

Tauri note:

- Keep rollback command exposed only to intended windows via capabilities; avoid accidental remote/webview expansion.

## 5) Denial-Of-Service Risks Via Unbounded History/Diffs

### CRITICAL: Enforce hard count/size/rate limits at API + DB layers

**Risk**: Unlimited snapshots/diffs can exhaust disk, CPU, and memory (CWE-400/CWE-770).  
**Confidence**: High (CWE and SQLite docs are explicit; SQLite supports runtime and DB size controls).

Actionable implementation:

- New explicit limits (mirror existing metadata constants pattern), for example:
  - `MAX_CONFIG_SNAPSHOTS_PER_PROFILE` (e.g., 50)
  - `MAX_CONFIG_SNAPSHOT_BYTES` (e.g., 64 KiB compressed)
  - `MAX_CONFIG_DIFF_OPS` (e.g., 256 operations)
  - `MAX_ROLLBACK_REQUESTS_PER_MINUTE_PER_PROFILE` (e.g., 6)
- Enforce limits in this order:
  1. Tauri command validation (reject early).
  2. Core service validation (authoritative).
  3. SQLite constraints/checks where possible.
- DB growth controls:
  - Set/verify `PRAGMA max_page_count` for metadata DB budget.
  - Keep prune-on-insert behavior (already used in version snapshots) for snapshot retention.
- Safe failure:
  - On limit breach return deterministic typed error and do not partially apply rollback.

## Severity Matrix

| Area                                                      | Severity | Why                                                          |
| --------------------------------------------------------- | -------- | ------------------------------------------------------------ |
| Snapshot authenticity + applicability checks              | CRITICAL | Direct config compromise if poisoned payload is accepted     |
| Rollback audit trail (append-only + before/after hash)    | CRITICAL | No trustworthy forensic trail otherwise                      |
| History/diff count-size-rate limits                       | CRITICAL | Straightforward local or IPC-driven DoS path                 |
| Path traversal/canonicalization at history boundaries     | WARNING  | Existing name checks help, but secondary paths remain risky  |
| Sensitive data minimization and redaction policy          | WARNING  | History can become high-value local data store               |
| Optional at-rest encryption for sensitive history fields  | ADVISORY | Depends on threat model and whether secrets enter profiles   |
| Optional HMAC key management for stronger tamper evidence | ADVISORY | Stronger guarantee than hash-only, with operational overhead |

## Suggested Implementation Sequence (Security-First)

1. Schema: snapshot + rollback audit tables with bounded columns and indexes.
2. Core write path: deterministic serialization + hash chain + retention pruning.
3. Core rollback path: full integrity validation + transactional apply + audit emit.
4. API guards: Tauri command validation (length/count/rate) + capability scoping review.
5. Privacy hardening: field classification, redaction rules, and log scrubbing.
6. Abuse tests: malformed payloads, oversized diffs, replayed snapshots, chain break, rapid rollback spam.

## Verification Checklist For Implementation PR

- [ ] Rollback refuses tampered payload (hash mismatch test).
- [ ] Rollback refuses cross-profile snapshot ID.
- [ ] Rollback refuses chain discontinuity/replay when policy requires linear ancestry.
- [ ] Snapshot retention pruning keeps exact configured max rows.
- [ ] Oversize payload and diff-op count are rejected before DB write.
- [ ] Audit row exists for success and for rejected rollback attempts.
- [ ] No raw snapshot payload appears in logs.
- [ ] Tauri capability/command exposure reviewed for rollback commands.

## Sources

1. SQLite limits and anti-DoS guidance: <https://sqlite.org/limits.html> (last updated 2026-03-11)
2. SQLite runtime limit API: <https://www.sqlite.org/c3ref/limit.html>
3. SQLite `max_page_count` pragma: <https://www.sqlite.org/pragma.html#pragma_max_page_count>
4. Tauri security model and trust boundaries: <https://v2.tauri.app/security/>
5. Tauri capabilities model: <https://v2.tauri.app/security/capabilities/>
6. Tauri command scopes: <https://v2.tauri.app/security/scope/>
7. CWE-22 (Path Traversal): <https://cwe.mitre.org/data/definitions/22.html>
8. CWE-180 (Validate Before Canonicalize): <https://cwe.mitre.org/data/definitions/180.html>
9. CWE-345 (Insufficient Verification of Data Authenticity): <https://cwe.mitre.org/data/definitions/345.html>
10. CWE-400 (Uncontrolled Resource Consumption): <https://cwe.mitre.org/data/definitions/400.html>
11. CWE-770 (Allocation Without Limits/Throttling): <https://cwe.mitre.org/data/definitions/770.html>
12. OWASP Logging Cheat Sheet (audit/integrity/sensitive-data logging): <https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html>
13. OWASP Input Validation Cheat Sheet: <https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html>
14. OWASP Cryptographic Storage Cheat Sheet: <https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html>

## Freshness Notes

- Fast-changing area: Tauri v2 security docs are current and should be rechecked at implementation time for capability/command defaults.
- Stable references: CWE entries and OWASP cheat sheets are appropriate for design controls but should be paired with current crate/library APIs during coding.
