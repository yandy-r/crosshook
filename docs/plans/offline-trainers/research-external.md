# External API Research: Offline Trainers Feature

**Research date:** 2026-03-31
**Feature:** Offline-first trainer management for Steam Deck portable use
**Issue:** CrossHook #44

---

## Executive Summary

The offline-trainers feature has three distinct integration profiles:

1. **FLiNG trainers** are fully offline-capable by design: standalone Windows EXEs distributed as RAR/ZIP archives, no DRM, no phone-home, no internet requirement once downloaded. CrossHook can cache downloaded binaries locally and verify integrity with SHA-256. The primary technical challenge is scraping flingtrainer.com to discover download URLs, as no public API exists.

2. **Aurora (CheatHappens)** offline keys are **hardware-bound to Windows HWID and explicitly do not work on Steam Deck or Linux**. Offline key generation requires running Aurora on Windows, is gated behind a Lifetime PLUS membership, and produces a machine-specific key that expires after a set number of days. No API exists for programmatic key requests. CrossHook cannot automate Aurora offline key acquisition; the feature must be handled via a manual-instruction modal.

3. **WeMod** has no public API and prohibits spidering/crawling in its ToS. Its offline mode is a rolling session cache (10-14 days max, often 24 hours in practice) rather than a true offline key. Automation via the observed unofficial API endpoints (`https://api.wemod.com/v3/...`) would violate ToS. WeMod on Linux/Steam Deck runs entirely through Proton (no native app), making offline key replication even more complex. CrossHook should treat WeMod offline as a user-managed manual concern and show a clear limitation notice.

4. **Community tap offline caching** is the most tractable item: `taps.rs` already invokes `git` via `std::process::Command`; wrapping the fetch call to treat network failures as graceful degradation is sufficient.

The recommended technology stack for implementation:

- **SHA-256 hashing:** `sha2` 0.11.x (RustCrypto, pure Rust, hardware-accelerated) — only new dependency
- **Connectivity detection:** `std::net::TcpStream` probe (stdlib only, no extra crate)
- **Git tap sync (offline-safe):** `std::process::Command` invoking system `git` (already used in `taps.rs`) — no `git2` crate
- **Cache metadata persistence:** existing `rusqlite` already in CrossHook; extend the SQLite schema with a `trainer_hashes` table

---

## Primary APIs

### 1. WeMod (Wand) API

**Status:** Unofficial / no public documentation. Observed via reverse engineering community projects.

**Confidence:** Medium — endpoints confirmed by community implementations but not by WeMod officially.

**Auth flow:**

```
POST https://api.wemod.com/auth/token
Content-Type: application/x-www-form-urlencoded

client_id=infinity&gdpr_consent_given=0&grant_type=password
  &password=<password>&username=<email>
```

Response: `{ "access_token": "...", "refresh_token": "..." }`

Refresh:

```
grant_type=refresh_token&refresh_token=<token>
```

**Game catalog endpoint (public CDN, no auth required):**

```
GET https://storage-cdn.wemod.com/catalog.json
```

Response: JSON array of game records with fields including `TitleId`, `GameId`, `Name`, `Slug`.

**Trainer blueprint endpoint:**

```
GET https://api.wemod.com/v3/games/{GameId}/trainer?gameVersions=&locale=en-US&v=2
Authorization: Bearer <access_token>
User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) ...
```

Response: JSON with cheat definitions and hotkey data.

**Offline mode behavior:**

| Version  | Offline Session Length                   |
| -------- | ---------------------------------------- |
| WeMod <6 | No offline support                       |
| WeMod 6+ | ~10-14 days, often ~24 hours in practice |

- Session is invalidated if the app is fully closed and PC restarted while offline.
- Only games/trainers previously accessed online are available offline.
- No offline key system exists — it is purely session-cache based.

**ToS restrictions:**

WeMod's ToS explicitly prohibits spidering and crawling (confirmed via ToS;DR analysis). The license grant is "personal use only" and prohibits derivative works or reverse engineering. Using `api.wemod.com` endpoints programmatically without explicit WeMod authorization violates these terms.

**Docs:** [WeMod Terms of Service](https://www.wemod.com/terms) | [ToS;DR](https://tosdr.org/en/service/2354)

**Sources:** [wemod-deck AHK script](https://github.com/wemod-deck/wemod-deck/blob/main/wemod-deck.ahk) | [WeMod community forum](https://community.wemod.com/t/offline-usage/230390)

---

### 2. Aurora (CheatHappens) Offline Key System

**Status:** No public API. Manual in-app workflow only.

**Confidence:** High — confirmed via official Cheat Happens documentation and community discussions.

**Offline key mechanism:**

Aurora offline keys are generated within the Aurora Windows application (Settings > Offline Key tab). The key is:

- Hardware-bound to Windows HWID
- Requires Lifetime PLUS membership
- One key per machine (not per trainer)
- **Expires** after a set number of days for downloaded trainers (can be renewed)
- **Does NOT work on Steam Deck or Linux**: Aurora explicitly checks for Windows hardware identifiers and software keys that do not exist on SteamOS

**Key expiry behavior:**

The offline key itself does not expire based on time, but each trainer downloaded for offline use has a rolling expiry (exact duration not publicly documented). Renewal requires internet connectivity to re-download.

**Programmatic automation:** Impossible without violating Cheat Happens ToS and without running on Windows (HWID collection requires Windows registry keys).

**Docs:** [How to use Aurora Offline Key](https://cheathappens.zendesk.com/hc/en-us/articles/4451585703315-How-do-i-use-my-Offline-Key-in-Aurora) | [How to obtain offline key](https://cheathappens.zendesk.com/hc/en-us/articles/4408862962835-How-do-i-obtain-an-offline-key-for-my-trainers) | [Steam Deck limitations](https://www.cheathappens.com/show_board2.asp?headID=152001&titleID=77044)

---

### 3. FLiNG Trainer Distribution

**Status:** No public API. Website is WordPress-based. Downloads served through RAR/ZIP archives hosted on the site or mirrors.

**Confidence:** Medium — based on community documentation and open-source tool inspection.

**Distribution format:**

- Trainers packaged as `.rar` or `.zip` archives
- Archives contain: trainer `.exe` + readme (anti-cheat instructions)
- Standalone `.exe` extracted and executed directly
- Per-game-version specificity: trainer must match game version

**Phone-home / DRM behavior:**

FLiNG standalone trainers have **no phone-home requirement and no DRM**. They are single-file Windows executables that target specific game memory addresses. Standalone versions are explicitly differentiated from the WeMod-integrated versions (which do use WeMod's online infrastructure). FLiNG does offer a separate premium "FLiNG Cheat" service with a license model, but the standalone game trainers remain free and offline-capable.

**Website structure:**

- Main catalog: `https://flingtrainer.com/all-trainers/` (alphabetical)
- Per-trainer page: `https://flingtrainer.com/trainer/<slug>/`
- Archive (pre-2019): `https://archive.flingtrainer.com/`
- The site returns HTTP 403 to automated requests (bot detection active)

**Known scraping approach (from open-source tools):**

The FLiNG Trainer Collection desktop app (Melon-Studio/FLiNG-Trainer-Collection) uses a Crawler component that imports trainer data as JSON into a local SQLite database. Exact scraping mechanism is proprietary to that project but the pattern is: crawl the trainer pages to extract game name + download link, cache locally.

**Recommended CrossHook approach:** Do not scrape flingtrainer.com directly. Instead:

1. Let users provide the local path to a downloaded and extracted FLiNG trainer `.exe`
2. On first registration, compute and store SHA-256 hash
3. On subsequent launches, verify hash matches cached value

**Sources:** [flingtrainer.com](https://flingtrainer.com/) | [Tom's Hardware safety discussion](https://forums.tomshardware.com/threads/is-fling-trainers-safe.3838152/) | [Melon-Studio/FLiNG-Trainer-Collection](https://github.com/Melon-Studio/FLiNG-Trainer-Collection)

---

## Libraries and SDKs

### SHA-256 Hash Verification

**Crate:** `sha2` (part of RustCrypto)
**Version:** 0.11.0 (current as of research date)
**Maintenance:** Active — part of the well-maintained RustCrypto organization
**License:** MIT/Apache-2.0

```toml
[dependencies]
sha2 = "0.11"
```

**Usage pattern for file hashing:**

```rust
use sha2::{Sha256, Digest};
use std::fs::File;
use std::io::{BufReader, Read};

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
```

**Docs:** [docs.rs/sha2](https://docs.rs/sha2) | [RustCrypto/hashes](https://github.com/RustCrypto/hashes)

---

### Network Connectivity Detection

**Option A: Standard library only (recommended)**

```rust
use std::net::TcpStream;
use std::time::Duration;

fn is_online() -> bool {
    TcpStream::connect_timeout(
        &"8.8.8.8:53".parse().unwrap(),
        Duration::from_secs(3),
    ).is_ok()
}
```

No extra dependencies. Connects to Google DNS (port 53) as the probe target. The Rust forum recommends this approach as the most practical for production use.

**Option B: `online` crate**

```toml
[dependencies]
online = "3"
```

```rust
use online::check;
fn is_online() -> bool { check(None).is_ok() }
```

Uses Chrome/Firefox captive-portal domains as probe targets. Convenience wrapper around a similar TCP probe. Last release: 2019 era — **maintenance status unclear**, minimal downloads.

**Recommendation:** Use the stdlib TCP probe directly (Option A). It is dependency-free, controllable, and follows community best practices.

**Docs:** [lib.rs/crates/online](https://lib.rs/crates/online) | [Rust forum connectivity thread](https://users.rust-lang.org/t/how-to-check-for-internet-connection/89893)

---

### Git Operations for Community Tap Offline Cache

**Approach:** `std::process::Command` invoking the system `git` binary (already used in CrossHook)

**Note:** The `git2` crate (libgit2 bindings) is NOT used in CrossHook for tap sync. The existing implementation in `crates/crosshook-core/src/community/taps.rs` uses `std::process::Command::new("git")` for all git operations. This is the correct approach to preserve — it avoids a C FFI dependency with its own CVE surface and is consistent with the existing codebase.

For offline-safe tap sync, the pattern is to wrap the existing `git fetch` subprocess call and treat non-zero exit codes / network errors as a graceful degradation rather than a hard failure:

```rust
use std::process::Command;

pub enum SyncStatus { Updated, UsedCache }

pub fn try_sync_tap(tap_dir: &Path) -> anyhow::Result<SyncStatus> {
    let output = Command::new("git")
        .arg("fetch")
        .arg("--depth=1")
        .arg("origin")
        .current_dir(tap_dir)
        .output()?;

    if output.status.success() {
        // Also fast-forward: git merge FETCH_HEAD
        let _ = Command::new("git")
            .args(["merge", "--ff-only", "FETCH_HEAD"])
            .current_dir(tap_dir)
            .output();
        Ok(SyncStatus::Updated)
    } else {
        // Network unavailable or unreachable — use last-synced local state
        log::warn!(
            "Tap sync skipped (offline?): {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(SyncStatus::UsedCache)
    }
}
```

Key points:

- No new dependency — reuses existing `std::process::Command` pattern from `taps.rs`
- When offline, non-zero exit from `git fetch` is caught and returns `SyncStatus::UsedCache`
- The local git clone already contains the last-synced state and remains fully readable
- Initial tap clone can use `--depth=1` to minimize storage

---

### Hash Cache Persistence

CrossHook already uses `rusqlite` for SQLite metadata. The trainer hash cache fits naturally into the existing metadata layer.

**Proposed schema addition:**

```sql
CREATE TABLE IF NOT EXISTS trainer_hashes (
    id          INTEGER PRIMARY KEY,
    profile_id  TEXT NOT NULL,
    path        TEXT NOT NULL,
    sha256      TEXT NOT NULL,
    verified_at INTEGER NOT NULL,  -- Unix timestamp
    UNIQUE(profile_id, path)
);
```

**Verification pattern:**

```rust
fn verify_trainer_offline(
    conn: &Connection,
    profile_id: &str,
    trainer_path: &Path,
) -> anyhow::Result<OfflineStatus> {
    let cached = conn.query_row(
        "SELECT sha256 FROM trainer_hashes WHERE profile_id=?1 AND path=?2",
        params![profile_id, trainer_path.to_str()],
        |row| row.get::<_, String>(0),
    ).optional()?;

    match cached {
        None => Ok(OfflineStatus::NoCachedHash),
        Some(expected_hash) => {
            let actual = sha256_file(trainer_path)?;
            if actual == expected_hash {
                Ok(OfflineStatus::Verified)
            } else {
                Ok(OfflineStatus::HashMismatch { expected: expected_hash, actual })
            }
        }
    }
}
```

**Docs:** [docs.rs/rusqlite](https://docs.rs/rusqlite) | [rusqlite GitHub](https://github.com/rusqlite/rusqlite)

---

### HTTP Retry and Offline Fallback (for online-mode fetch paths)

**Crate:** `reqwest` (already used in CrossHook via `tauri`)

For tap sync and any future trainer catalog fetches, wrap network calls with timeout + error classification:

```rust
use reqwest::Client;
use std::time::Duration;

async fn fetch_with_offline_fallback<T: serde::de::DeserializeOwned>(
    client: &Client,
    url: &str,
    cached: Option<T>,
) -> anyhow::Result<T> {
    let result = client
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .await;

    match result {
        Ok(resp) => Ok(resp.json::<T>().await?),
        Err(e) if e.is_timeout() || e.is_connect() => {
            cached.ok_or_else(|| anyhow::anyhow!("offline and no cached data"))
        }
        Err(e) => Err(e.into()),
    }
}
```

**Docs:** [docs.rs/reqwest](https://docs.rs/reqwest)

---

## Integration Patterns

### Offline-First State Machine

The recommended pattern for the CrossHook profile pre-flight check:

```
ONLINE mode:
  1. Check connectivity (TcpStream probe)
  2. Attempt tap sync (git fetch, gracefully degrade on error)
  3. Verify trainer path exists + compute/compare hash
  4. Cache hash if new or changed

OFFLINE mode:
  1. Skip connectivity probe (already known offline)
  2. Skip tap sync (use last-fetched local git state)
  3. Verify trainer path exists + compare against cached hash
  4. If no cached hash: warn user, allow launch with caution
  5. If hash mismatch: warn user (binary may have been replaced), ask confirmation
  6. Show offline indicator in UI for network-dependent features
```

### Graceful Degradation Pattern

| Feature            | Online                              | Offline                                          |
| ------------------ | ----------------------------------- | ------------------------------------------------ |
| Profile load       | Normal                              | Normal (TOML is local)                           |
| Trainer launch     | Normal                              | Normal (EXE is local)                            |
| FLiNG hash verify  | Compute + cache                     | Compare cached hash                              |
| Community tap sync | git fetch + update                  | Use local last-sync state                        |
| ProtonDB lookup    | HTTP fetch                          | Show "unavailable offline"                       |
| WeMod launch       | Normal (user manages WeMod session) | User warned: WeMod may expire                    |
| Aurora launch      | Normal                              | User warned: offline keys not supported on Linux |

### Community Tap Offline Cache Architecture

The existing tap structure (local git clone in `~/.config/crosshook/taps/<name>/`) already supports offline operation after initial clone. The enhancement needed is:

1. Record the last successful fetch timestamp per tap in SQLite
2. On launch, if fetch fails, check staleness: warn if >N days since last sync
3. Community profiles from the local clone remain fully readable without network

---

## Constraints and Gotchas

### WeMod

- **No public API**: All observed endpoints are reverse-engineered and violate ToS if used programmatically.
- **No offline key**: WeMod's "offline mode" is a session cache, not a key-based system. It cannot be replicated or extended by CrossHook.
- **Session expiry is unpredictable**: ~10-14 days per official statements but users report 24-hour practical expiry.
- **Linux constraint**: WeMod runs under Proton on Steam Deck. The rolling session cache still applies, but session invalidation behavior under Proton Wine prefix differs from native Windows.
- **Recommendation**: CrossHook should display a clear notice that WeMod offline mode is user-managed, time-limited, and may not work reliably on Steam Deck.

### Aurora (CheatHappens)

- **Offline keys are Windows-only**: The HWID collection uses Windows registry keys that do not exist on SteamOS. Cheat Happens documentation explicitly confirms this.
- **Lifetime PLUS required**: Offline keys are gated behind a ~$40 one-time membership. Free/annual subscribers cannot use offline keys.
- **No API**: Key generation is entirely in-app and manual.
- **Trainer expiry**: Offline trainers expire (exact duration undocumented) and require internet connectivity to renew.
- **Recommendation**: CrossHook should show an info modal explaining Aurora offline key limitations on Steam Deck, with a link to the Cheat Happens Steam Deck tool guide.

### FLiNG

- **Bot detection**: `flingtrainer.com` returns 403 to automated requests. Direct scraping for trainer discovery is blocked.
- **No stable download URL pattern**: FLiNG trainer download links appear to go through the website's download flow rather than a stable CDN URL pattern.
- **Version-pinned executables**: Each trainer targets a specific game version. Users must manually re-download when games update.
- **Recommendation**: CrossHook should not auto-download FLiNG trainers. Instead, support local path registration with hash caching for integrity verification.

### SHA-256 Hash Caching

- Hashing large executables on every launch is expensive. Cache the hash at registration time and only re-hash if the file modification timestamp changes (`mtime` check before SHA-256).
- The hash cache should be invalidated when the user explicitly updates a trainer.

### git2 Network Features

- The `https` feature must be explicitly enabled in `git2` Cargo features for remote fetch to work. Without it, only local operations are available.
- When a fetch fails due to connectivity, `git2::Error` does not provide a clean `is_offline()` predicate. Match on `git2::ErrorCode` or check if the error message contains network-related strings.

---

## Code Examples

### Complete Trainer Hash Registration and Verification

```rust
use sha2::{Sha256, Digest};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use rusqlite::{Connection, OptionalExtension, params};

pub fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn register_trainer_hash(
    conn: &Connection,
    profile_id: &str,
    path: &Path,
) -> anyhow::Result<String> {
    let hash = sha256_file(path)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO trainer_hashes (profile_id, path, sha256, verified_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![profile_id, path.to_str(), &hash, now],
    )?;
    Ok(hash)
}

pub enum HashVerifyResult {
    Verified,
    Mismatch { expected: String, actual: String },
    NoCache,
}

pub fn verify_trainer_hash(
    conn: &Connection,
    profile_id: &str,
    path: &Path,
) -> anyhow::Result<HashVerifyResult> {
    let cached = conn.query_row(
        "SELECT sha256 FROM trainer_hashes WHERE profile_id=?1 AND path=?2",
        params![profile_id, path.to_str()],
        |row| row.get::<_, String>(0),
    ).optional()?;
    match cached {
        None => Ok(HashVerifyResult::NoCache),
        Some(expected) => {
            let actual = sha256_file(path)?;
            if actual == expected {
                Ok(HashVerifyResult::Verified)
            } else {
                Ok(HashVerifyResult::Mismatch { expected, actual })
            }
        }
    }
}
```

### Offline Connectivity Probe

```rust
use std::net::TcpStream;
use std::time::Duration;

pub fn is_online() -> bool {
    TcpStream::connect_timeout(
        &"8.8.8.8:53".parse().unwrap(),
        Duration::from_secs(3),
    ).is_ok()
}
```

### Offline-Safe Tap Fetch

```rust
use git2::{Repository, FetchOptions};

pub fn try_sync_tap(repo_path: &Path) -> anyhow::Result<SyncStatus> {
    let repo = Repository::open(repo_path)?;
    let mut remote = repo.find_remote("origin")?;
    let mut opts = FetchOptions::new();
    match remote.fetch(&["main"], Some(&mut opts), None) {
        Ok(_) => Ok(SyncStatus::Updated),
        Err(e) => {
            // Network errors: treat as offline, use cached state
            log::warn!("Tap sync skipped (offline or unreachable): {}", e);
            Ok(SyncStatus::UsedCache)
        }
    }
}

pub enum SyncStatus { Updated, UsedCache }
```

---

## Open Questions

1. **FLiNG trainer discovery UX**: If CrossHook does not scrape flingtrainer.com, should it provide a built-in browser/link to the FLiNG site for manual download, or require users to paste a path? The former is friendlier; the latter is safer from a ToS perspective.

2. **WeMod session cache staleness detection**: Can CrossHook detect that a WeMod session has expired (e.g., by probing a WeMod auth endpoint without violating ToS) and warn the user proactively before going offline?

3. **Hash re-verification frequency**: Should hash verification happen on every launch (slowest, most secure) or only when file mtime changes (fast, reasonable security)? The mtime approach has known TOCTOU edge cases but is acceptable for the threat model here (corruption detection, not adversarial).

4. **Aurora on Steam Deck workaround**: Cheat Happens provides a paid Steam Deck Tool ($10) that automates Aurora setup via Proton but cannot provide offline keys. Should CrossHook's UI link to this tool as a supported workflow for Aurora users on Steam Deck?

5. **git2 HTTPS feature gate**: CrossHook's existing community tap sync presumably uses HTTPS remotes. If the `https` feature is already enabled in `git2`, offline-safe fetch just needs error handling. If not, this needs to be enabled in `Cargo.toml`.

---

## Sources

- [WeMod Terms of Service](https://www.wemod.com/terms)
- [WeMod ToS;DR analysis](https://tosdr.org/en/service/2354)
- [WeMod offline usage community thread](https://community.wemod.com/t/offline-usage/230390)
- [WeMod offline mode community thread](https://community.wemod.com/t/wemod-offline-mode/84100)
- [wemod-deck AHK script (unofficial API reverse engineering)](https://github.com/wemod-deck/wemod-deck/blob/main/wemod-deck.ahk)
- [DeckCheatz WeMod launcher](https://github.com/DeckCheatz/wemod-launcher)
- [CheatHappens Aurora offline key support doc](https://cheathappens.zendesk.com/hc/en-us/articles/4451585703315-How-do-i-use-my-Offline-Key-in-Aurora)
- [CheatHappens offline key request guide](https://cheathappens.zendesk.com/hc/en-us/articles/4408862962835-How-do-i-obtain-an-offline-key-for-my-trainers)
- [Aurora Steam Deck offline key limitation discussion](https://www.cheathappens.com/show_board2.asp?headID=152001&titleID=77044)
- [CheatHappens Steam Deck tool](https://www.cheathappens.com/steamdecktool.asp)
- [FLiNG Trainer main site](https://flingtrainer.com/)
- [FLiNG safety analysis - Tom's Hardware](https://forums.tomshardware.com/threads/is-fling-trainer-safe.3838152/)
- [Melon-Studio FLiNG Trainer Collection (open-source downloader)](https://github.com/Melon-Studio/FLiNG-Trainer-Collection)
- [sha2 crate docs](https://docs.rs/sha2)
- [RustCrypto/hashes GitHub](https://github.com/RustCrypto/hashes)
- [online crate on lib.rs](https://lib.rs/crates/online)
- [Rust forum: checking internet connectivity](https://users.rust-lang.org/t/how-to-check-for-internet-connection/89893)
- [git2 crate docs](https://docs.rs/git2/latest/git2/)
- [git2-rs GitHub (rust-lang)](https://github.com/rust-lang/git2-rs)
- [rusqlite GitHub](https://github.com/rusqlite/rusqlite)
- [reqwest GitHub](https://github.com/seanmonstar/reqwest)
- [Game-Cheats-Manager (multi-source trainer manager)](https://github.com/dyang886/Game-Cheats-Manager)

---

## Search Queries Executed

1. `WeMod API offline mode activation endpoint authentication 2024 2025`
2. `FLiNG trainer offline standalone Windows executable phone home DRM behavior`
3. `Rust crates network connectivity detection offline-first patterns crate 2024`
4. `WeMod Aurora trainer offline key API endpoint documentation developer`
5. `Aurora CheatHappens offline key hardware binding HWID implementation technical`
6. `Rust sha2 sha256 file hash verification crate offline integrity check`
7. `git2 rust crate offline git operations local repository cache 2024`
8. `FLiNG trainer website distribution download format zip exe 2024 2025`
9. `WeMod reverse engineer API network traffic offline authentication header`
10. `WeMod unofficial reverse engineered community github endpoints JSON`
11. `FLiNG trainer download direct link scraping HTML structure flingtrainer.com 2024`
12. `WeMod terms of service automation API access third party integration restriction`
13. `offline-first desktop app pattern cache invalidation graceful degradation Rust Tauri 2024`
14. `reqwest Rust HTTP client offline fallback retry pattern cache 2024`
15. `git offline clone shallow fetch community tap caching strategy`
16. `serde_json toml Rust offline cache metadata file hash storage pattern`
17. `Rust sled SQLite rusqlite offline cache trainer metadata local storage pattern 2024`
18. `WeMod Linux Proton Steam Deck support trainer launcher community project`
19. `FLiNG trainer standalone exe no internet DRM anti-cheat free download direct`
20. `Aurora CheatHappens Steam Deck offline use SteamOS Proton HWID Linux guide 2024`
21. `game-cheats-manager FLiNG trainer download mechanism API scraping method github`
22. `"storage-cdn.wemod.com/catalog.json" trainer game catalog structure format`
23. `WeMod terms automating scraping disallow programmatic access reverse engineering violation`
