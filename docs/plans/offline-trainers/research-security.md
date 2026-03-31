# Security Research: offline-trainers

## Executive Summary

The offline-trainers feature introduces four distinct security-sensitive areas that require attention before shipping: (1) trainer executable integrity via SHA-256 hash caching, (2) offline activation key storage for Aurora/WeMod, (3) community tap offline cache integrity, and (4) FLiNG trainer execution risk under Proton. The existing codebase already applies strong input sanitization for community tap operations (URL scheme validation, branch name allowlisting, SHA hex validation for pinned commits) and field-length bounds on community manifest data (`metadata/community_index.rs`), which is a good foundation.

The primary gaps are: no encryption or OS keyring integration for sensitive offline keys, no constant-time hash comparison in `metadata/version_store.rs:203` (confirmed `!=` operator on hex strings), SQLite database permissions defaulting to world-readable (0644), the `trainer.offline_activated` flag must not travel in portable TOML, and the inherent risk of executing unverified Windows binaries in Proton with no runtime sandbox.

No CRITICAL findings were discovered that block the feature entirely. Five WARNING items must be addressed before ship. Several ADVISORY best practices can be deferred.

_Updated 2026-03-31 with findings from business analysis cross-reference and direct code review of `metadata/version_store.rs` and `metadata/community_index.rs`._

---

## Findings by Severity

### CRITICAL — Hard Stops

None identified. The feature can proceed with the WARNING items addressed.

---

### WARNING — Must Address

#### W-1: Offline Activation Keys Stored in Plaintext

**Affected area**: Aurora/WeMod offline keys stored in `~/.config/crosshook/` (TOML or SQLite)

Aurora offline keys are hardware-bound tokens that expire in 14 days. WeMod credentials function similarly. If either is written to disk in plaintext inside the TOML profile or the SQLite metadata database, any process running as the same user can read them. On shared-user systems or systems with compromised non-root accounts, this directly exposes subscription credentials.

**Mitigation**: Store offline keys via the OS secret service (Linux `keyring` crate backed by the D-Bus Secret Service API / KWallet / GNOME Keyring), not in TOML or SQLite. The `keyring` crate (v3+) supports Linux via Secret Service and has stable bindings. If keyring is unavailable (Steam Deck gaming session without a desktop keyring daemon), fall back to AES-256-GCM encryption using a per-installation key derived via PBKDF2-SHA256 from a machine-unique identifier (e.g., `/etc/machine-id`), stored at `~/.config/crosshook/secrets.db` with `chmod 600`. Document the fallback behavior clearly. **Do not** store raw keys in TOML profiles that users share or export.

**Confidence**: High — Aurora offline keys are documented as hardware-bound tokens that expire; local plaintext exposure is a clear credential theft vector.

---

#### W-2: SQLite Database File Created with World-Readable Permissions (0644)

**Affected area**: `~/.config/crosshook/metadata.db` (and WAL/SHM sidecar files)

SQLite creates database files with mode `0644` by default, meaning any local user can read the database contents. This matters most if offline keys or activation tokens are stored in the database, but also exposes launch history, profile names, and any locally cached community tap data to other users on the same machine.

**Mitigation**: After creating the SQLite database file (in `metadata/db.rs`), immediately call `fs::set_permissions` with mode `0600`. Ensure the WAL (`-wal`) and shared-memory (`-shm`) sidecar files receive the same treatment. Apply the same restriction to any `secrets.db` file created for the W-1 mitigation.

```rust
use std::os::unix::fs::PermissionsExt;
fs::set_permissions(&db_path, fs::Permissions::from_mode(0o600))?;
```

**Confidence**: High — SQLite documentation and Linux security guides confirm 0644 default; the fix is a single call.

---

#### W-3: Hash Comparison Uses `!=` Operator (Timing Oracle)

**Affected area**: `metadata/version_store.rs:203` — `compute_correlation_status()`

The function `compute_correlation_status` compares trainer hashes with `current_trainer_hash != snapshot_trainer_hash` (line 203), a standard string equality check that short-circuits on the first differing byte. This is the live comparison path for determining `TrainerChanged` vs. `Matched` correlation status — the exact comparison a timing oracle would target.

Additionally, `hash_trainer_file` (line 215) reads the entire trainer binary into memory with `std::fs::read` before hashing. For large trainers (100MB+) this could cause OOM conditions on memory-constrained devices like Steam Deck (16GB shared RAM). Streaming hashing via `sha2::Sha256::update` in chunks is safer.

**Mitigation**: Replace the `!=` comparison in `compute_correlation_status` with a constant-time comparison. Since the hashes are hex strings (not raw bytes), compare via a byte-level constant-time check:

```rust
use subtle::ConstantTimeEq;

let trainer_changed = match (current_trainer_hash, snapshot_trainer_hash) {
    (Some(a), Some(b)) => a.as_bytes().ct_eq(b.as_bytes()).unwrap_u8() == 0,
    (None, None) => false,
    _ => true,
};
```

For `hash_trainer_file`, switch to chunked streaming to avoid loading large binaries fully into memory:

```rust
use std::io::{BufReader, Read};
let file = std::fs::File::open(path).ok()?;
let mut reader = BufReader::new(file);
let mut hasher = Sha256::new();
let mut buf = [0u8; 65536];
loop {
    let n = reader.read(&mut buf).ok()?;
    if n == 0 { break; }
    hasher.update(&buf[..n]);
}
```

**Confidence**: High — the `!=` operator is confirmed in `metadata/version_store.rs:203`; timing-attack class is well-documented; the subtle crate is the standard Rust mitigation.

---

#### W-4: FLiNG Trainer Executables Are Inherently Untrusted Windows Binaries

**Affected area**: Trainer execution via Proton

FLiNG trainers are standalone Windows `.exe` files distributed via third-party sites. Malware analysis (ANY.RUN, Hybrid Analysis, BleepingComputer) confirms that malicious executables impersonating FLiNG trainers actively harvest browser credentials, Discord tokens, and crypto-wallet data, and exfiltrate over plaintext HTTP. Even legitimate trainers use memory injection techniques that trigger AV/EDR scanners. The existing network isolation via `unshare --net` (issue #62) partially mitigates exfiltration, but does not prevent local credential harvesting on the host.

**Mitigation**:

1. Make hash verification of trainer executables mandatory before launch — a cached hash mismatch must block execution, not just warn.
2. Ensure network isolation (`unshare --net`) is on by default for `proton_run` trainer launches.
3. Display an explicit trust dialog the first time any trainer is launched from a given hash, making clear the binary is untrusted.
4. Consider adding a filesystem namespace restriction (`--ro-bind` via bubblewrap/bwrap) to limit what the trainer process can read on the host filesystem; this is deeper hardening but feasible for the `proton_run` code path.

**Confidence**: High — malware analysis reports from ANY.RUN and Hybrid Analysis directly confirm the threat; the FLiNG ecosystem is a known malware distribution surface.

---

### ADVISORY — Best Practices

---

#### W-5: `trainer.offline_activated` Flag Must Not Be Stored in Portable TOML

**Affected area**: Profile TOML storage — `TrainerSection` in `profile/models.rs`

The business analysis confirms a `trainer.offline_activated` flag is planned to record whether a user has activated offline mode for a trainer. If this flag is stored in the TOML profile (which participates in `portable_profile()` export and community tap sharing), it becomes portable: a community-shared profile could arrive pre-marked as "offline activated," falsely implying activation on the recipient's device.

Aurora offline activation is per-device and per-account. A flag that travels with a community profile or via profile export is a misleading trust assertion — and could allow a user to bypass the activation check on a new machine by importing a profile with the flag set.

**Mitigation**: Store `offline_activated` state exclusively in SQLite (machine-local metadata layer), not in the TOML profile. The existing `storage_profile()` / `portable_profile()` distinction in `profile/models.rs` already handles this boundary — offline activation state belongs in the same category as `local_override` paths: machine-specific, never portable. If the flag is needed at runtime, look it up from SQLite keyed on `profile_id`.

**Confidence**: High — the business-analyzer explicitly identified this; the profile model's portability boundary is well-defined in the existing code.

---

### ADVISORY — Best Practices (A-)

#### A-1: Cache Directory Permissions Should Be Restrictive (0700)

**Affected area**: Community tap workspace at `~/.local/share/crosshook/community/taps/`

Currently no explicit directory permission is set on `base_path` after `fs::create_dir_all`. Default umask for most Linux users is `0022`, making directories `0755` (world-executable, readable). While the directory contains only Git repositories of community profiles (not secrets), exposing them to local users leaks which community taps a user subscribes to.

**Mitigation**: After `create_dir_all`, set directory permissions to `0700` using `set_permissions`. Low effort, good hygiene.

**Confidence**: High — trivial fix; standard local-data directory hardening.

---

#### A-2: Hash Cache Entries Have No Expiry or Invalidation on File Modification

**Affected area**: Hash cache for trainer executables

If a trainer file is replaced after its hash has been cached, the cache entry will remain valid until manually cleared. An attacker who replaces the trainer binary after a verified hash is stored will not trigger re-verification unless the cache is invalidated on file metadata changes (mtime/size).

**Mitigation**: Store `mtime`, `size`, and `inode` alongside the cached hash. On verification, compare file metadata first; if it changed since caching, recompute and re-verify against user expectations. This does not need full inotify; a point-in-time stat check at launch time is sufficient.

**Confidence**: Medium — TOCTOU between hash computation and file use is a real attack vector; metadata-based invalidation is widely used (ccache, cargo, etc.).

---

#### A-3: ~~Community Tap Index Files Deserialized Without Schema Validation~~ — RESOLVED

**Status**: Already implemented. `metadata/community_index.rs` contains explicit length bounds constants (commented as "A6 advisory security finding"):

- `MAX_GAME_NAME_BYTES = 512`
- `MAX_DESCRIPTION_BYTES = 4_096`
- `MAX_PLATFORM_TAGS_BYTES = 2_048`
- `MAX_TRAINER_NAME_BYTES = 512`
- `MAX_AUTHOR_BYTES = 512`
- `MAX_VERSION_BYTES = 256`

These are applied before inserting tap data into SQLite. No action required — this advisory is resolved by the existing implementation.

**Confidence**: High — confirmed by reading `metadata/community_index.rs:9-15`.

---

#### A-4: Custom Environment Variables in Profiles Not Validated Against an Allowlist

**Affected area**: `launch/env.rs`, `LaunchSection.custom_env_vars` in profile models

Profiles allow arbitrary `custom_env_vars` entries. A community-provided profile could inject environment variables like `LD_PRELOAD=/path/to/malicious.so` or `PATH=/attacker-controlled/path:...`. While `LD_PRELOAD` is cleared in `WINE_ENV_VARS_TO_CLEAR` for the WINE path, it may pass through in other launch modes or via community profile import.

**Mitigation**: When importing a community profile via a tap, validate `custom_env_vars` against a blocklist (at minimum: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `PATH`). Display a warning in the UI when a community profile sets `custom_env_vars`. For local user profiles, this is a user-trust decision, but community profiles should be treated with more suspicion.

**Confidence**: Medium — the `LD_PRELOAD` clearing exists specifically because this is a known vector; extending that discipline to community-imported `custom_env_vars` is consistent.

---

#### A-5b: Offline Mode Must Suppress All Outbound Connections

**Affected area**: App startup / background tasks — tap sync, ProtonDB queries, Steam API calls

The business analysis establishes a user expectation: when offline mode is active, CrossHook must not make any outbound network calls. This has both privacy implications (metered/monitored connections) and correctness implications (calls that fail loudly while offline degrade UX).

**Mitigation**: Add an `offline_mode: bool` flag to the app settings layer. When true: (a) skip tap sync on startup, (b) skip ProtonDB and Steam API queries, (c) skip any version check calls. Community tap data and cached metadata should be served from local SQLite/Git workspaces only. This is also the correct behavior for the Steam Deck in airplane mode.

**Confidence**: High — explicit business requirement; straightforward to implement as a settings gate.

---

#### A-5: No `cargo audit` Step in CI

**Affected area**: Dependency supply chain

The repository uses `sha2 = "0.11.0"` (RustCrypto, widely audited), `rusqlite = "0.39.0"` (has two historical advisories from 2020 and 2021, both in old versions), `toml = "1.1.0"`, and `serde`. No automated advisory scanning is visible in the CI workflow. The sha2 crate from RustCrypto has no known current advisories and is one of the safest hash implementations available.

**Mitigation**: Add `cargo audit` (or the GitHub advisory action `rustsec/audit-check`) to the CI pipeline. This catches future advisories against current dependencies automatically. Low effort, high signal.

**Confidence**: High — rustsec.org confirms sha2 and current rusqlite have no active advisories; this is a preventive measure for future advisories.

---

#### A-6: Git Binary Invoked via `std::process::Command` — No GIT_CONFIG_NOSYSTEM

**Affected area**: `community/taps.rs` — `git_command()` helper

The `git_command()` helper sets only `GIT_HTTP_LOW_SPEED_LIMIT` and `GIT_HTTP_LOW_SPEED_TIME`. A user's global `~/.gitconfig` or system `/etc/gitconfig` can define hooks (`core.hooksPath`), alternate object stores, or protocol overrides that affect how git processes tap URLs. A malicious `~/.gitconfig` entry (e.g., `url.file:///etc/passwd.insteadOf = https://github.com`) could redirect a valid HTTPS tap URL to a local file path, bypassing the URL validation.

**Mitigation**: Add `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null` (or an empty controlled config), and `GIT_TERMINAL_PROMPT=0` to the environment of every `git_command()` invocation. This prevents external git configuration from influencing tap operations.

```rust
fn git_command() -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_HTTP_LOW_SPEED_LIMIT", GIT_HTTP_LOW_SPEED_LIMIT)
        .env("GIT_HTTP_LOW_SPEED_TIME", GIT_HTTP_LOW_SPEED_TIME)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}
```

**Confidence**: Medium — the URL scheme validation in `validate_tap_url()` is a strong defense, but `url.<scheme>.insteadOf` in gitconfig can redirect validated URLs; layered defense is justified.

---

## Authentication and Authorization

**FLiNG trainers**: No authentication required or possible. FLiNG trainers are standalone executables. The only trust mechanism available is hash verification (SHA-256 of the binary before execution).

**Aurora offline keys**: Hardware-bound tokens issued by CheatHappens that expire after 14 days. They cannot be renewed offline — they must be fetched during an online session. The offline feature implies: (a) the key was fetched online and cached, (b) the feature enforces the expiry window, and (c) the key is stored securely per W-1.

**WeMod**: WeMod requires active subscription for mod activation. Offline mode caches session state. Activation tokens must be treated as secrets — same storage requirements as Aurora keys (W-1). WeMod does not require credentials for FLiNG-style standalone trainers.

**Community taps**: No per-user authentication — taps are public Git repositories. Integrity comes from commit pinning (supported and validated in `taps.rs`) and the URL scheme allowlist. Offline mode for taps means serving from the locally cloned workspace without re-fetching; no additional auth concerns arise, but the workspace integrity should be verified at startup via `rev-parse HEAD` check against the pinned commit.

---

## Data Protection

| Data                       | Current State                                           | Risk                                 | Recommendation            |
| -------------------------- | ------------------------------------------------------- | ------------------------------------ | ------------------------- |
| Aurora/WeMod offline keys  | Unknown — feature is new                                | HIGH if plaintext                    | OS keyring (W-1)          |
| Trainer SHA-256 hash cache | SQLite `external_cache_entries`                         | Medium — world-readable DB           | chmod 600 on DB (W-2)     |
| Community tap workspaces   | Git repos at `~/.local/share/crosshook/community/taps/` | Low — public data                    | chmod 700 dir (A-1)       |
| TOML profiles              | `~/.config/crosshook/*.toml`                            | Low — no secrets currently           | Don't add secrets to TOML |
| SQLite metadata            | `~/.config/crosshook/metadata.db`                       | Medium — launch history, health data | chmod 600 (W-2)           |

The `storage_profile()` / `portable_profile()` distinction in `profile/models.rs` is well-designed — machine-specific paths are moved to `local_override` and stripped for sharing. Extend this discipline to ensure offline keys are never written to the `GameProfile` TOML storage format.

---

## Dependency Security

### Current Dependencies Relevant to This Feature

| Crate         | Version | Status | Notes                                                                      |
| ------------- | ------- | ------ | -------------------------------------------------------------------------- |
| `sha2`        | 0.11.0  | Clean  | RustCrypto, no active advisories; widely audited                           |
| `rusqlite`    | 0.39.0  | Clean  | Two historical advisories (2020, 2021) in old versions; 0.39 is unaffected |
| `toml`        | 1.1.0   | Clean  | No known advisories                                                        |
| `serde`       | 1.x     | Clean  | No known advisories                                                        |
| `directories` | 6.0.0   | Clean  | Used for base path resolution                                              |

### New Dependencies for Offline-Trainers Feature

| Recommended            | Purpose                                        | Risk Level                                        |
| ---------------------- | ---------------------------------------------- | ------------------------------------------------- |
| `keyring` (v3+)        | OS keyring for offline key storage             | Low — maintained, widely used, cross-platform     |
| `subtle`               | Constant-time hash comparison                  | Very Low — RustCrypto project, minimal dependency |
| _(optional)_ `aes-gcm` | Fallback key encryption if keyring unavailable | Low — RustCrypto project, audited                 |
| _(optional)_ `pbkdf2`  | Key derivation for fallback encryption         | Low — RustCrypto project                          |

**Do not add**: `git2` (libgit2 bindings) — the current approach of invoking the `git` CLI is safer because it avoids bundling a C library with its own CVE surface. The existing `git_command()` approach with controlled arguments is adequate.

**Run `cargo audit` before shipping** to confirm no active advisories against these dependencies.

---

## Input Validation

### Existing Controls (Well-Implemented)

The existing validation in `community/taps.rs` is a strong baseline:

- `validate_tap_url()`: Accepts only `https://`, `ssh://git@`, and `git@` schemes. Rejects `file://`, `git://`, and bare paths. (Blocks local file access and unauthenticated git protocol.)
- `validate_branch_name()`: Allowlist of `[a-zA-Z0-9/._-]`, max 200 chars, must not start with `-`. (Prevents flag injection into git CLI arguments.)
- `is_valid_git_sha()`: Hex-only, 7–64 chars. (Prevents command injection via pinned commit strings.)

These controls are tested in the existing test suite.

### Gaps for Offline-Trainers

1. **Trainer file paths**: When resolving the trainer path for hash verification or execution, always canonicalize using `std::fs::canonicalize` and verify the result is within an expected directory. A profile value of `../../../usr/bin/passwd` as the trainer path should be rejected.

2. **Hash cache key values**: The `cache_key` field in `external_cache_entries` should be validated as a non-empty string with bounded length (<=512 chars) and no null bytes before insert.

3. **Community profile manifest fields**: Strings like `game_name`, `trainer_name` used in display or file operations should be bounded (<=256 chars) and stripped of null bytes and control characters.

4. **Offline key field values**: Aurora/WeMod keys arriving from their APIs should be validated as printable ASCII with a maximum length before storage.

---

## Infrastructure Security

### File System Layout

```
~/.config/crosshook/
  *.toml              (mode 0600 recommended — currently unknown)
  metadata.db         (mode 0644 by default — MUST be 0600, see W-2)
  metadata.db-wal     (mode 0644 by default — MUST be 0600)
  metadata.db-shm     (mode 0644 by default — MUST be 0600)
  secrets.db          (new, for encrypted offline keys — MUST be 0600)

~/.local/share/crosshook/community/taps/  (mode 0755 by default — should be 0700, see A-1)
  <tap-slug>/         (Git workspace)
```

### Network Isolation Default

Issue #62 proposes `unshare --net` for trainer processes. For offline mode this is especially important because the trainer has no legitimate reason to make outbound network calls. **Recommend making network isolation the default-on setting** for `proton_run` trainer launches, with an explicit user opt-out (not opt-in). Creating a network namespace requires `CAP_SYS_ADMIN` or user namespace support (`CLONE_NEWUSER + CLONE_NEWNET`) — the latter is available without root on most modern Linux kernels and Steam Deck.

### Offline Key Expiry Enforcement

Aurora offline keys expire in 14 days. The application must enforce this expiry locally even without network access — do not allow a user to override the expiry window. Store the expiry timestamp alongside the encrypted key and check it at launch time.

---

## Secure Coding Guidelines

These guidelines apply specifically to implementing the offline-trainers feature:

1. **Never write activation keys to TOML files.** TOML profiles may be shared, exported, or imported via community taps. Secrets belong in the OS keyring or the encrypted `secrets.db`.

2. **Always use constant-time comparison for hash verification.** Use `subtle::ConstantTimeEq` — never `==` on hash byte slices.

3. **Verify file hashes immediately before execution.** Do not cache verification state in memory across session boundaries; re-verify at every launch.

4. **Canonicalize all user-provided file paths before use.** Check that the resolved path starts with an expected base directory. Fail closed — if canonicalization fails (file does not exist), report an error rather than proceeding.

5. **Run git with a controlled environment.** Add `GIT_CONFIG_NOSYSTEM=1` and `GIT_CONFIG_GLOBAL=/dev/null` to prevent user gitconfig from interfering with tap operations (A-6).

6. **Set restrictive permissions on new files.** Use `0600` for database files and `0700` for data directories. Do this immediately after creation, not lazily.

7. **Show trust information in the UI before executing trainers.** Display the SHA-256 fingerprint and its source (locally cached vs. freshly computed) in the launch UI so users can make informed decisions about unverified binaries.

8. **Block community profile import of sensitive env vars.** When importing profiles from community taps, reject or warn on `LD_PRELOAD`, `LD_LIBRARY_PATH`, and `PATH` in `custom_env_vars`.

---

## Trade-off Recommendations

| Trade-off                                   | Recommended Position                                                                                                                                                              |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Keyring vs. plaintext key storage           | Keyring (with encrypted fallback for Steam Deck sessions). Not plaintext.                                                                                                         |
| Network isolation on by default vs. opt-in  | Default-on for trainer launches in offline mode; opt-out with explicit user acknowledgment                                                                                        |
| Hash verification blocking vs. advisory     | Blocking — a hash mismatch must halt execution. Advisory-only is insufficient for untrusted binaries.                                                                             |
| git CLI vs. git2 (libgit2 bindings)         | Keep git CLI. libgit2 adds a C dependency with its own CVE history and no measurable benefit here.                                                                                |
| SQLite encryption (SQLCipher) vs. chmod 600 | chmod 600 is sufficient for profile data and hash cache. SQLCipher adds significant complexity and is only needed if keys are stored in SQLite (which should be avoided per W-1). |
| Tauri IPC path validation                   | Validate all file paths received via IPC before passing to the Rust backend — Tauri's isolation pattern can intercept and check paths in the WebView boundary.                    |

---

## Open Questions

1. **Aurora/WeMod key format**: What format do Aurora offline keys take? (JWT? Opaque token? HMAC?) The validation approach for W-1 depends on key structure. This needs coordination with the api-researcher.

2. **Steam Deck keyring availability**: On a Steam Deck in Game Mode (no desktop session), is the D-Bus Secret Service daemon running? If not, the fallback encrypted-storage path becomes the primary path. The UX implications of "keyring unavailable" need to be defined with the ux-researcher.

3. **Hash verification UI flow**: When a trainer hash is not yet cached (first launch), what does the user see? There must be an explicit confirmation step, not a silent proceed. The exact UX flow needs definition.

4. **Offline key renewal UX**: Aurora keys expire in 14 days. What happens when the user is offline at expiry? The user must be informed clearly — not silently blocked — that they need to reconnect to renew.

5. **Community tap offline mode**: If the network is unavailable at startup, does the app serve community taps from the local Git workspace without attempting `git fetch`? The sync flow needs a clear offline-first code path that does not fail on network errors.

6. **Gamescope and bubblewrap interaction**: If bubblewrap (`bwrap`) sandboxing is added for trainer processes, does it interact correctly with gamescope on Steam Deck? This needs testing.

7. **`offline_activated` flag data model**: Where exactly in the SQLite schema should offline activation state live? A new column on `profile_sync` or a dedicated `offline_activation` table? The decision affects whether activation state survives profile deletion and re-import.

8. **WeMod ToS on offline session caching**: Does WeMod's terms of service permit caching session tokens for offline use? This is a compliance question the business-analyzer flagged — needs legal/ToS review before implementing WeMod offline key storage.

---

## Sources

- [RustCrypto/hashes — sha2 crate](https://github.com/RustCrypto/hashes)
- [sha2 — crates.io](https://crates.io/crates/sha2)
- [keyring — crates.io](https://crates.io/crates/keyring/4.0.0-beta.1)
- [keyring-rs — GitHub](https://github.com/hwchen/keyring-rs)
- [RustSec Advisory Database](https://rustsec.org/advisories/)
- [rusqlite advisories — RustSec](https://rustsec.org/packages/rusqlite.html)
- [Tauri v2 Security](https://v2.tauri.app/security/)
- [Tauri Isolation Pattern](https://v2.tauri.app/concept/inter-process-communication/isolation/)
- [subtle crate — constant-time comparison](https://docs.rs/subtle)
- [ANY.RUN — FLiNG malware analysis](https://any.run/report/29549ebe10d1b69dbe3b38decd9d0399099c42da9152def3a2f9a86decbd1bfd/0a9e0750-6d33-405f-a301-685437201f07)
- [Hybrid Analysis — FLiNGTrainer_setup.exe](https://hybrid-analysis.com/sample/b39e77db437997cb38b388cec461bd7c240a80573828ee9604a1fee7c228468a/5f096877ce24b31b7925fec3)
- [BleepingComputer — FLiNG@3DMGAME Trojan](https://www.bleepingcomputer.com/forums/t/471837/fling3dmgame-trojan/)
- [WeMod Community — Offline Mode](https://community.wemod.com/t/offline-mode/94241)
- [Aurora offline keys — CheatHappens forums](https://www.cheathappens.com/show_board2.asp?headID=151570&titleID=77043&onPage=2)
- [SQLite security best practices](https://dev.to/stephenc222/basic-security-practices-for-sqlite-safeguarding-your-data-23lh)
- [Linux network namespace isolation](https://sigma-star.at/blog/2023/05/sandbox-netns/)
- [OWASP Path Traversal](https://owasp.org/www-community/attacks/Path_Traversal)
- [Rust path traversal prevention — StackHawk](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/)
- [Timing attack prevention — Paragon Initiative](https://paragonie.com/blog/2015/11/preventing-timing-attacks-on-string-comparison-with-double-hmac-strategy)
- [Sandboxing Linux with namespaces](https://nixhacker.com/sandboxing-and-program-isolation-in-linux-using-many-approaches/)
