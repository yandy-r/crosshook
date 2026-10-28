# YAN-835 security research

Source inspection only; no builds, tests, network requests, or source edits. Paths below are relative to `src/crosshook-native/`. Recommendations are proposed behavior, not claims about exercised tests.

A focused read-only `code-reviewer` independently confirmed the index symlink/cycle exposure (medium), prepare/import asymmetry (low; UX/policy inconsistency rather than an actual-import bypass), and non-cancellable Git lifecycle gap (medium). It found no critical/high issue in those three reviewed boundaries. This is not a full-repository security audit; no source fixes were made in this research assignment.

## 1. Existing containment and path checks

### Community prepare/import: checks differ today

- `src-tauri/src/commands/community.rs:112-123`: actual import constructs `PathBuf`, runs `validate_import_path_in_workspace` on a blocking task, then passes the **original** path to `ProfileWriter`.
- `community.rs:194-207`: actual import uses `path.canonicalize()` and, for every subscribed workspace, `workspace.local_path.canonicalize().map(|root| canonical.starts_with(&root)).unwrap_or(false)`.
- `community.rs:209-216`: rejects with `"is not inside a known tap workspace"` if no root matches. Missing/unresolvable candidate is an error; missing/unresolvable roots fail closed. Existing intermediate/final symlinks are resolved. This is a component-wise `Path` prefix comparison, not a string prefix comparison.
- `community.rs:145-169`: prepare canonicalizes, requires `canonical.is_file()`, and requires a case-insensitive `.json` extension.
- `community.rs:171-183`: prepare explicitly permits local file-picker imports. It computes `_is_workspace_path` with the same canonical-prefix expression, but **does not enforce the result**. Therefore an outside JSON can preview successfully and then fail import. The extension's ASCII case-insensitivity is unrelated to path containment.
- `crates/crosshook-app/src/services/profiles/imports.rs:39-52`: writer contract says `"The caller has already checked that path is allowed"`; `:123-127` directly calls the core importer. Containment currently belongs only to the Tauri actual-import adapter, not the reusable writer/core importer.
- `crates/crosshook-core/src/profile/exchange/import.rs:38-45`: preview directly calls `fs::read_to_string(json_path)`. No containment, file-kind, symlink, or size check. Actual core import calls preview (`:15`) and then `ProfileStore::save` (`:26-27`).

**Consequence:** move one real authorization check into the service, apply it to both prepare and import, and use the returned resolved path rather than discarding it. The spec's “every import path” should mean tap-contained community imports. If local-file import is intentionally retained, define a distinct explicitly authorized local root/capability for both prepare and commit; never silently use `candidate.parent()` as its own authority.

### Tap workspace and index paths

- `crates/crosshook-core/src/community/taps/store.rs:44-54,142-144`: normalized subscription becomes `base_path.join(subscription.directory_name())`; there is no canonical containment or symlink check on the resulting workspace.
- `community/taps/types.rs:25-40` and `community/taps/utils.rs:26-40`: directory name is a lowercased ASCII-alphanumeric/dash slug of URL plus optional branch; no slash/`..` survives. That prevents lexical path injection, but different URLs/branches can have the same slug; pinned commit is not part of it. Dedupe by exact URL/branch/pin does not prevent workspace aliasing.
- `community/taps/store.rs:97-125`: creates base directory, then trusts `workspace.local_path.exists()` to choose destructive fetch/reset/clean versus clone. A pre-existing workspace symlink is not rejected. Validate workspace under the trusted store base **before** Git or indexing, and reject a symlink workspace root (do not canonicalize an attacker-chosen root into a new authority).
- `community/index.rs:124-133`: `path.is_dir()` follows directory symlinks; only dot-prefixed directories are skipped, then recursion continues. No containment or cycle guard.
- `community/index.rs:138-149,176-181`: reads trainer-source/profile JSON via `fs::read_to_string`, including symlinked files; no canonical containment or byte-size bound.
- `community/index.rs:161-168,197-200`: `.strip_prefix(root).unwrap_or_else(...)` / `.unwrap_or(path.as_path())` derive display-relative paths, **not security checks**. They never reject escape.
- `community/taps/utils.rs:5-23`: recursive size walker also has no containment/cycle policy; it is not an authorization primitive.

**Recommendation:** the indexer must check every directory and manifest before reading, not rely on later import checks. Simplest safe traversal policy is skip all symlink entries (`DirEntry::file_type`/`symlink_metadata`) and allow only ordinary directories/files. If in-root symlinks are supported instead, apply containment and track canonical visited directories to prevent cycles. Never fall back to raw paths after failed containment.

### Export paths and profile destination names

- `src-tauri/src/commands/community.rs:96-102`: output path is accepted verbatim and passed to `export_community_profile`; only profile name is trimmed.
- `profile/exchange/export.rs:16-23`: loads profile through `ProfileStore`, sanitizes the shareable profile, then writes to supplied path.
- `profile/exchange/utils.rs:241-247,271-275`: `create_dir_all(parent)` and `fs::write(output_path, json)` follow intermediate/final symlinks, create parents, and overwrite existing files. No workspace/export-root policy exists.
- `profile/toml_store/utils.rs:57-90`: destination profile-name validation rejects empty/`.`/`..`, controls, absolute names, `/`, `\\`, `:`, and reserved path characters. Keep it through `ProfileWriter`; source containment is not a replacement for destination validation.
- `profile/exchange/utils.rs:17-54`: export strips machine-local paths, unknown extras and launch hooks. `:57-84` disables imported pre/post/DLL hooks; retain this untrusted-manifest protection.

**Recommendation:** arbitrary user-selected export destinations are existing functionality, not tap imports. Do not force all exports under taps. If a constrained export root is added, it must be a separately trusted chosen root and use the missing-candidate rules below. Do not claim `write_atomic` is a containment fix: `core/fs_util.rs:9-21,45-54` deliberately follows final symlinks.

### Lutris and external sources

- `src-tauri/src/commands/lutris.rs:16-21`: optional directory is passed directly to preview. `profile/lutris_import/import.rs:19-23` accepts any existing directory or discovers a default root.
- `profile/lutris_import/paths.rs:18-29`: default roots are known config/data directories selected only by `.is_dir()`.
- `paths.rs:33-81`: scans `<root>/games`, flat and one nested level; `.is_file()`/`.is_dir()` follow symlinks, without canonical containment. Entry cap is 10,000 (`:6,45,68`).
- `paths.rs:89-106`: opens `<root>/pga.db` read-only and parameterizes its row limit; no symlink containment.
- `profile/lutris_import/parse.rs:57-80`: 512 KiB metadata-size check before YAML read; no containment, and metadata/read are separate operations.
- `profile/lutris_import/import.rs:52-61`: reads scanned YAML. `:105-148`: apply consumes caller-provided mapped entries and writes validated unique profile names; **does not reopen `entry.source_path`**. It is provenance, not a second file import.
- No `canonicalize`/`starts_with`/`strip_prefix`/`ParentDir` containment checks occur in these Lutris modules. Recommend treating the selected/discovered Lutris root as trusted authority, checking `games/` candidates and `pga.db` under that root before reads. Do not restrict game executable/prefix paths to the Lutris root: legitimate installed games live elsewhere.
- Discovery external sources are URLs, not filesystem imports; their validation is described in §4.

## 2. Shared `ensure_within_workspace` contract

### Placement and error shape

Recommend `crosshook-core::fs_util::ensure_within_workspace(root: &Path, candidate: &Path) -> Result<PathBuf, WorkspacePathError>`, not app-only. The core indexer and Lutris scanner need the same primitive; core cannot depend on app. App services choose trusted roots and convert errors to `AppError`. No toolkit types or settings lookup in this helper.

Suggested typed core variants: `InvalidRoot`, `Traversal`, `OutsideWorkspace`, and `Io { operation, source }` (including not-found, permission denied, loop and not-directory). Keep raw paths out of `Display`; app maps traversal/outside to `Validation { field: "path" }`, absent source to `NotFound`, other I/O to `Io`. Existing app error categories/sanitization live at `crates/crosshook-app/src/error.rs:5-24,27-55,99-102`. Keep candidate-is-file/JSON/YAML and size checks with the operation, not the general containment helper.

### Exact semantics

1. `root` must be an existing directory, selected by the application, not inferred from candidate input. Canonicalize it once per operation; failure is fatal. A trusted configured store-base symlink can resolve to its actual location, but validate/reject untrusted workspace-root symlinks before treating a tap as this authority.
2. Inspect **original candidate components before canonicalization**. Reject any `Component::ParentDir`, including `nested/../valid.json` even if the final target is inside; this makes the AC's `..` rejection explicit. Do not percent-decode paths, expand `~`/variables, replace backslashes, or perform Unicode/case rewriting. Reject empty candidate; decide separately whether root itself is a valid operation target (never a profile file).
3. Relative candidate resolves from the canonical root, never process CWD. For absolute input, require a component-wise lexical prefix of either the absolute supplied root spelling or its canonical spelling, strip that prefix, and resolve the remaining components from the canonical root. Reject other absolute paths even if an outside alias points back in. This permits a trusted configured root symlink without accepting arbitrary aliases. Compare `Path`/`OsStr` components, never `display()`/`to_string_lossy()`/string prefixes. Canonicalizing the complete existing target and checking `Path::starts_with(&canonical_root)` is the final containment check, not a substitute for the component walk.
4. Walk the relative suffix from the held canonical root. Use `symlink_metadata` to distinguish a genuinely absent component from an existing dangling link. Canonicalize each existing prefix, check its resolved location remains contained, and require intermediates to be directories. Only an actual `NotFound` for an absent component permits collecting the remaining `Normal` components as a lexical suffix; append that suffix to the canonical existing ancestor and check containment again. Never treat permission errors, `ELOOP`, `ENOTDIR`, or an unresolved/dangling symlink as an absent safe suffix. This handles absent final files and absent nested directories; it does not create them. The root's own ancestors are trusted setup, not candidate prefixes to reject for being above the root.
5. Existing final symlink: canonicalize and accept only if its resolved target stays inside; dangling final symlink: reject (even if its text appears in-root). Existing intermediate symlink: resolve and check at that point; reject if it leaves root, even if a later link would return. For tap indexing, prefer the stricter no-symlink traversal policy above.
6. Return the resolved `PathBuf`; callers read/write that resolved path, not original input. Community reads must still require an existing regular manifest, so the generic missing-suffix allowance does **not** authorize importing a missing file. Recheck when committing, not only at preview time.

### Linux case and Unicode: concrete interpretation

- On ordinary case-sensitive ext4, `/x/Taps` and `/x/taps` are different directories. With root `/x/Taps`, reject an absolute candidate in `/x/taps`; do **not** lowercase paths to make them match. Also reject `/x/Taps-extra/...` through component comparison.
- `/x/caf\u00e9` (NFC) and `/x/cafe\u0301` (NFD) are different byte paths on ordinary ext4. If the NFC directory is authority, a candidate under the NFD sibling must fail. The same applies to visually confusable Unicode letters: appearance does not grant authority.
- Recommend canonical **component-wise `OsStr` byte comparisons**, not NFC-normalizing both paths. Normalizing then comparing could incorrectly authorize a different on-disk directory. Do not use lossy UTF-8 conversion. Blanket rejection of non-NFC filenames is **not required for containment** and would unnecessarily reject legitimate Linux filenames; a valid decomposed filename _inside the real root_ is still contained. If product explicitly chooses NFC-only import names, reject non-NFC relative components rather than rewriting them, and add `unicode-normalization` deliberately (not currently in core/app manifests). It is an additional filename policy, not a symlink or containment fix.
- Tests should create two actual sibling directories with different case and NFC/NFD spellings, put a real valid JSON in the outside sibling, and assert rejection. Do not test merely a nonexistent wrong-spelling path: that only proves not-found behavior. State the fixture filesystem assumption; casefold-enabled ext4 or normalization-insensitive mounts do not share ordinary ext4 name semantics.

### Race boundary

Canonicalize-then-open is a **point-in-time check**, not race-free authorization: another actor can exchange a parent symlink after checking. Passing canonical output closes obvious original-link reuse, not all parent replacement races. Serialize sync/import/index against the same managed tap workspace so the app does not race its own reset/clean. If hostile concurrent filesystem writers are in the threat model, open through a held root directory FD using Linux `openat2` (`RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS`, plus `RESOLVE_NO_SYMLINKS` for no-link policy), then parse from that opened handle; missing-file writes need FD-relative create/rename too. A two-path validator alone cannot promise that stronger guarantee. Do not describe sequential containment tests as proving race resistance.

### Required tests for implementation owner

- Existing nested relative/absolute file accepted; root itself rejected by manifest validation.
- Raw `..` (relative, absolute, and path ending inside) rejected; outside absolute/sibling-prefix rejected.
- Intermediate/final outside symlinks rejected; dangling links and loops fail closed; valid in-root links tested if supported.
- Real case/NFC/NFD/confusable sibling escapes rejected without lowercasing/normalization; valid Unicode nested path accepted under byte-based policy.
- Missing final/nested path returns a contained resolved candidate only for creation use; missing import rejected; permission/not-directory errors never fall back.
- Prepare and commit apply identical authority; indexer/Lutris scanner reject outside links before file reads; workspace-root link rejected before Git reset/clean.

## 3. Git invocation and cancellation

### What is already protected

- `community/taps/git.rs:14-35`: Git gets low-speed HTTP cutoff (1 KiB/s for 30s), `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`. Under Flatpak it uses `host_std_command_with_env("git", &env, &BTreeMap::new())`; outside Flatpak `std::process::Command::new("git").envs(&env)`.
- Commands use `.arg`/`.args`, not a shell. The diagnostic `format!` strings are not executed (`git.rs:38-64,148-173`).
- `taps/validation.rs:40-64`: URL allowlist accepts `https://`, `ssh://git@`, SCP-style `git@`, and absolute local paths; rejects other schemes/relative paths and all whitespace after trimming. Thus a URL beginning `-` is already rejected before sync. It is a prefix check, **not a full URL/SCP parser**.
- `validation.rs:15-28,66-74`: branch max 200 bytes, ASCII alphanumeric plus `/._-`, not leading `-`; empty values become default branch. This prevents flag injection but is not complete Git ref validation (`..`, trailing dot, repeated separators remain possible). Invalid refs should be validation errors rather than fall through to cached success; use a pure valid-ref check or `git check-ref-format --branch` behind the gateway.
- `git.rs:111-121`: pinned checkout validates hex-only SHA, length 7–64. Preserve it; preferably validate pin at add/resolve, not only after clone/fetch.
- `git.rs:70-94`: fetch uses `origin`, `--`, then branch; reset targets fixed `FETCH_HEAD`; clean flags fixed `-fdx`.

### Remaining hardening considerations

- Clone has no `--` between options and repository/destination (`git.rs:40-46`). Current URL allowlist blocks leading-dash input; retain that validation and add an explicit `--` before repository for defense in depth. Never relax validation merely because arguments are not a shell.
- Validate additions before settings persistence: `commands/community.rs:75-82` only pushes/dedupes; normalization happens later at workspace resolution. Dedupe normalized identities and address slug collisions so unrelated repositories cannot share a directory subject to reset/clean.
- Disable repository hooks for managed sync commands (`-c core.hooksPath=/dev/null`) and constrain protocols to intentionally supported HTTPS/SSH/local paths. Local taps are intentional developer functionality, not network URLs. Prefix validation alone does not establish host trust or forbid private-network Git endpoints; do not silently add a public-host-only policy that breaks local fixtures.
- Flatpak std gateway uses `--clear-env` and forwards the five provided values (`platform/gateway.rs:374-389`); native `.envs` **inherits** the rest of the application environment (`:410-416`). System/global Git config suppression does not remove `GIT_CONFIG_COUNT`, `GIT_DIR`, `GIT_WORK_TREE`, `GIT_SSH_COMMAND`, alternate config/home variables, nor local `.git/config`. Treat inherited environment/local workspace as local trust, not remote tap data. For deterministic sync, explicitly strip command-affecting Git variables while preserving deliberate SSH transport requirements; do not claim the current five variables fully sandbox Git. Never put credentials/raw credential-bearing URLs in job progress/errors.

### Actual cancellation, not just a cancelled waiter

- Current `commands/community.rs:220-226` calls synchronous `sync_many`. All Git operations wait with `.output()` (`taps/git.rs:48,126-130,153-157`). Moving this unchanged to `spawn_blocking` stops caller-thread blocking but **does not cancel Git**.
- `crates/crosshook-app/src/services/mod.rs:69-80,91-105`: `run_job` explicitly documents that cancellation returns `Cancelled` while its blocking closure finishes in the background. It cannot satisfy sync cancellation while network Git continues or index/backfill side effects still run.
- `jobs/registry.rs:71-77,219-228`: cancellation is cooperative; registry cancels a token and never drops the running future. `JobContext::cancel_token()` is at `:403-406`.
- `jobs/process.rs:51-76,84-152`: reuse `supervise_child` with a child launched using `process_group(0)`. It sends group SIGTERM, waits default 5s grace (`:17-18`), escalates to SIGKILL for living group members, and reaps the direct child. Drop guard kills the group (`:155-173`). It does not track descendants that deliberately leave their group (`:57-61`).
- Flatpak caveat is explicit: ordinary host builders kill only the proxy, leaving host work alive. Build cancellable Git through `platform::host_command_with_env_and_directory_lifecycle` with the Git env (`gateway.rs:77-122`), which adds `--watch-bus`. Do not simply convert the old std command to Tokio and assume this was preserved.
- Use an async `JobRegistry::start` sync pipeline: each Git child belongs to its group, stdout/stderr are drained concurrently (bounded retained diagnostics), `supervise_child(..., ctx.cancel_token(), DEFAULT_KILL_GRACE)` waits; check cancellation before each next Git/index/metadata phase. Cancellation must propagate as `Cancelled`, **never be converted into `CachedFallback`** by `taps/store.rs:110-118`. No index or successful last-sync update after a cancelled process. Serialize operations touching one physical workspace, without a broad UI-exclusive key that creates unrelated request conflicts.
- Launch is **not** a process-job kill template: `services/launch/spawner.rs:1-6` says plain `child.wait()` and no kill/process-group API; lifecycle belongs to session/watchdog logic. Installer likewise is not supervised: `commands/install.rs:29-36` uses blocking core install; `core/install/service.rs:64-70` spawns and waits. Prefix dependency mutations do use the lifecycle gateway (`core/prefix_deps/runner.rs:39-49`). Reuse the explicit jobs supervisor, not an imagined existing launch/install cancellation path.

## 4. Discovery URLs and ProtonDB cache

### Discovery

- `discovery/models.rs:155-189`: validates source ID (1–64 ASCII alphanumeric/`_-`), display name (nonblank, ≤128 bytes), parsed HTTPS base URL with host, and known `wordpress_rss` source type. No ban on URL credentials, local/private host, arbitrary port, query, or fragment. `commands/discovery.rs:87` invokes it only on add.
- `discovery/client/search.rs:36-37,148-179` loads enabled persisted sources and fetches without calling that validator; hand-edited TOML bypasses add-time validation. Revalidate at the network boundary as well as add. `fetch.rs:18-23` parses and query-encodes game name correctly; it does not re-enforce HTTPS.
- `fetch.rs:111-115` sets a 10s timeout and user-agent, but no explicit redirect/destination policy. **[INFERENCE]** Library-default redirects/proxy behavior therefore remains in force. HTTPS-with-host alone is not SSRF prevention: DNS/private destinations and redirected destinations need policy if public-internet-only is desired. This is a user-configurable desktop client, not an unauthenticated server proxy; severity depends on whether hostile settings/IPC are within threat model. Avoid overstating it as remote server SSRF.
- Keep existing 4-source concurrency, 50 cached-item and 1 MiB response constants (`client/mod.rs:21-26`). `fetch.rs:63-83` checks Content-Length then checks `.text()` after allocation: chunked/misreported oversized responses can still allocate beyond the cap. Stream with a byte limit if hardening resource consumption; the cache's 512 KiB persistence cap does not bound network allocation.
- RSS links are arbitrary nonempty strings (`parse.rs:39-49`), copied to `source_url` (`response.rs:59-64`); tap trainer-source entries likewise contain raw URLs (`models.rs:18-28`). UI calls `shellOpen` (`src/components/ExternalResultsSection.tsx:9-12`, `src/components/discovery/DiscoveryResultCard.tsx:74-76`); stub forwards to Tauri plugin/browser without its own scheme check (`src/lib/plugin-stubs/shell.ts:10-18`). Recommend HTTP(S)-only source links before exposing/opening, and retain text rendering (not raw HTML). This evidence does not establish an exploitable native opener bypass; plugin permissions were not reviewed.
- Cache keys use source ID and normalized query, not base URL (`client/cache.rs:8-12`). Removing/re-adding an ID with a different endpoint may show old provider data. Preserve schema but invalidate that source's cached rows or verify source URL before reuse; do not treat cache as trusted executable content.

### ProtonDB

- `protondb/client.rs:19-25,132-146` uses fixed HTTPS endpoint bases, six-hour TTL, six-second timeout. `models.rs:11-22` “normalization” only trims/nonempty-checks app ID; summary URL interpolation happens **before** later `i64` parsing (`client.rs:134,239-241`). Recommend bounded ASCII decimal positive Steam ID parsing before URL/cache-key construction. Current input cannot directly replace the fixed authority, but path/query-like ID text should not be accepted.
- `client.rs:85-129,396-433` preserves fresh-cache then live then stale fallback. Cache JSON decode failure is ignored safely; cached snapshot app ID is only filled when empty, not checked for mismatch (`:424-427`). Preserve stale/offline markers; verify/reconstruct identity/source URL on cache reads rather than trusting arbitrary cached presentation fields.
- Cache SQL is parameterized (`client.rs:362-390`; metadata writes `metadata/cache_store.rs:52-81`), no SQL interpolation of user values. `MAX_CACHE_PAYLOAD_BYTES=524288` (`metadata/models.rs:231`); oversized writes store NULL payload (`cache_store.rs:36-46`) and must continue to degrade without crashing.
- Summary/count/feed `.json()` reads have no response-size bound (`client.rs:206-225,284-289`); persistence limits apply only after parsing. Fixed provider lowers attacker control, but timeout alone is not a memory bound. Do not claim cache-size enforcement protects network parsing.
- Suggestions remain advisory; acceptance already goes through `ProfileWriter` (`commands/protondb.rs:72-80`). Keep that write boundary and do not auto-apply fetched/cached commands or environment values during lookup/suggestions.

## 5. Plan priority and persistence

1. Required for YAN-835: shared path authority primitive, consistent prepare/commit policy, guarded index/Lutris reads, containment tests, real supervised Git cancellation with Flatpak lifecycle, unchanged ProfileWriter authority and cache/stale semantics.
2. Explicit decisions to capture: local-file community imports; byte-based versus additional NFC-only filename policy; allowed internal symlinks; protection against hostile concurrent filesystem mutation; per-workspace sync serialization.
3. Other observed risks (URL private-host/redirect policy, strict numeric ProtonDB IDs, network byte limits, slug/cache identity collisions) need an explicit scope/security decision, not silent unrelated redesign during a service move. They are not evidence of new regressions introduced by the move.

No new persisted data is needed for containment/cancellation. Keep taps/external subscriptions in TOML, existing community/cache rows in SQLite, job tokens/progress/locks runtime-only; no migration. Offline indexed lists and cached ProtonDB stay usable with stale markers. Sync cancellation is visible as cancelled, not false successful cached synchronization. Error messages remain sanitized; detailed local paths/Git diagnostics stay out of IPC/job logs unless explicitly sanitized.
