# Fix Report: pr-281-review

**Source**: docs/prps/reviews/pr-281-review.md
**Applied**: 2026-04-17
**Mode**: Parallel sub-agents — 2 batches, max width 3
**Severity threshold**: HIGH (default — fixes CRITICAL + HIGH)

## Summary

- **Total findings in source**: 27
- **Eligible this run**: 9 (F001 CRITICAL + F002–F009 HIGH)
- **Applied this run**:
  - Fixed: 9
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 18 (F010–F020 MEDIUM, F021–F027 LOW) — remain `Status: Open`
  - No suggested fix: 0
  - Missing file: 0

## Batches

- **Batch 1** (3 parallel sub-agents, different file sets)
  - Agent A — `install.rs`: F001, F002, F003, F004 (4 sequential fixes, same file)
  - Agent B — `InstallProgressBar.tsx`: F007
  - Agent C — `useProtonManager.ts` + `types/protonup.ts`: F008, F009
- **Inter-batch validation gate**: cargo test (1063 pass), cargo clippy -D warnings (clean), tsc --noEmit (clean), biome check (clean)
- **Batch 2** (1 sub-agent, providers module + catalog.rs + install.rs call site)
  - Agent D — providers (5 files) + `providers/mod.rs` + `catalog.rs` + `install.rs:619`: F005, F006

## Fixes Applied

| ID   | Severity | File(s)                                                                                                | Status | Notes                                                                                                                                                                                                                                                                                                                       |
| ---- | -------- | ------------------------------------------------------------------------------------------------------ | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F001 | CRITICAL | `crosshook-core/src/protonup/install.rs`                                                               | Fixed  | `validate_install_destination` now re-runs the `compatibilitytools.d` segment check AFTER `path.canonicalize()` and returns `InstallError::InvalidPath` if the resolved path escapes the compat tree. New test `rejects_symlink_redirect_escaping_compat_dir` exercises the symlink-redirect scenario via a tempdir.        |
| F002 | HIGH     | `crosshook-core/src/protonup/install.rs`                                                               | Fixed  | Added `const MAX_CHECKSUM_BYTES: u64 = 1024 * 1024`. Both `fetch_sha512_sidecar` and `fetch_sha256_manifest` now gate on `response.content_length() > MAX_CHECKSUM_BYTES` before `.text().await`, returning `InstallError::ChecksumFailed`.                                                                                 |
| F003 | HIGH     | `crosshook-core/src/protonup/install.rs`                                                               | Fixed  | New `InstallError::UntrustedUrl(String)` variant + `validate_release_url(url)` helper (https + host allowlist: `github.com`, `api.github.com`, `objects.githubusercontent.com`, `github-releases.githubusercontent.com`). Called for `download_url`, sha512 `checksum_url`, and sha256 `manifest_url`. 4 new unit tests.    |
| F004 | HIGH     | `crosshook-core/src/protonup/install.rs`                                                               | Fixed  | Replaced `ok_or_else` closure in `ChecksumKind::Sha256Manifest` missing-`checksum_url` branch with explicit `match`; on `None`, calls `best_effort_cleanup`, emits `Phase::Failed` via `em`, returns `Err(InstallError::ChecksumMissing(...))` — matches pattern used by the other error branches.                          |
| F005 | HIGH     | `crosshook-core/src/protonup/providers/{mod,ge_proton,proton_cachyos,proton_em,boxtron,luxtorpeda}.rs` | Fixed  | New `pub(super) async fn fetch_github_releases(client, url)` in `providers/mod.rs` with `const MAX_CATALOG_BYTES: u64 = 10 * 1024 * 1024` byte cap (returns `ProviderError::Parse` on overrun). All 5 provider `fetch()` methods wired through the helper.                                                                  |
| F006 | HIGH     | `crosshook-core/src/protonup/providers/mod.rs`, `catalog.rs`, `install.rs`                             | Fixed  | Removed `include_prereleases: bool` from `registry()` signature + all call sites (install.rs, catalog.rs × 2, `describe_providers`, 2 test sites). `include_prereleases` is now threaded only where actually consumed (`provider.fetch(client, include_prereleases)`). Removed now-unused param from `persist_catalog`.     |
| F007 | HIGH     | `src/components/proton-manager/InstallProgressBar.tsx`                                                 | Fixed  | Added `aria-label={`Cancel install ${opId.slice(0, 8)}`}` to the Cancel `<button>`, mirroring the Dismiss button pattern.                                                                                                                                                                                                   |
| F008 | HIGH     | `src/hooks/useProtonManager.ts`                                                                        | Fixed  | Removed `console.warn` from per-provider catalog fan-out catch; catch param renamed to `_err`. Settled-promise pattern already propagates failure gracefully.                                                                                                                                                               |
| F009 | HIGH     | `src/types/protonup.ts`, `src/hooks/useProtonManager.ts`                                               | Fixed  | Widened `ProtonUpProvider` union with `'boxtron' \| 'luxtorpeda'`. Added `KNOWN_PROVIDER_IDS: Set<ProtonUpProvider>` + `asProvider(id)` runtime narrower in `useProtonManager.ts`; replaced `(selectedProviderId ?? 'ge-proton') as ProtonUpProvider` with `asProvider(selectedProviderId)`. No downstream consumers broke. |

## Validation Results (final)

| Check                   | Result                                                             |
| ----------------------- | ------------------------------------------------------------------ |
| `cargo test`            | Pass (1063 passed, 0 failed — up from 1058 baseline; +5 new tests) |
| `cargo clippy`          | Pass (`-D warnings` clean)                                         |
| `tsc --noEmit`          | Pass                                                               |
| `biome check`           | Pass (258 files)                                                   |
| `check-host-gateway.sh` | Pass (no direct host-tool bypasses)                                |

## Remaining Open Findings (below severity threshold)

### MEDIUM (11) — `Status: Open` in source review

F010, F011, F012, F013, F014, F015, F016, F017, F018, F019, F020

Notable security-adjacent items intentionally deferred:

- **F012** (install.rs tar symlink targets) — defense-in-depth against archive-time symlink escape; F001 already closes the most obvious vector (pre-extract destination canonicalization).
- **F013** (archive_filename sanitization) — hardening gap; defused by F001 canonicalization.
- **F018** (uninstall.rs denylist gap — `/home`, `/root`) — the `PathOutsideKnownRoots` check still refuses, but via the "wrong gate".

### LOW (7) — `Status: Open` in source review

F021, F022, F023, F024, F025, F026, F027

## Next Steps

1. **Re-review** to verify fixes resolved the findings:
   - `/ycc:code-review 281`
2. **Commit** the fixes:
   - `/ycc:git-workflow`
3. (Optional) **Follow-up pass** on MEDIUM findings:
   - `/ycc:review-fix 281 --parallel --severity MEDIUM`
