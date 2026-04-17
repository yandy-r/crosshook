# PR Review #281 — feat(protonup): native Proton download manager with AppImage/Flatpak parity

**Reviewed**: 2026-04-17
**Mode**: PR
**Author**: yandy-r
**Branch**: `feat/native-proton-manager` → `main`
**Head SHA**: `2f1d812a12b615b955baff9bc778a0d25c66fd90`
**Closes**: #274
**Decision**: REQUEST CHANGES

## Summary

Ambitious, well-scoped replacement for ProtonUp-Qt orchestration — clean `ProtonReleaseProvider` trait, streaming install orchestrator with cancel tokens, Flatpak `:ro` resolver, SQLite v22/v23 schema work, and matching TS/React UI. Validation is green (1058 cargo tests, clippy `-D warnings`, rustfmt, biome, tsc, host-gateway). However, the native download-and-extract path — which is supply-chain-adjacent — ships with one CRITICAL symlink-redirect vulnerability in the `compatibilitytools.d` install-root check and three HIGH hardening gaps around URL origin allowlisting and unbounded response bodies. Merge blocked until F001–F004 and F007 are addressed; the remaining findings are strong improvements but not strict blockers.

## Findings

### CRITICAL

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs:147` — Symlink-based install-root redirect. `validate_install_destination` checks that the path contains a `compatibilitytools.d` segment **before** calling `path.canonicalize()`. If an attacker (or a misconfigured user install) places a symlink at a `compatibilitytools.d` component that resolves outside the Steam tree (e.g., `~/.local/share/Steam/compatibilitytools.d` → `/`), the pre-canonicalize check passes and the returned canonical `dest_dir` is handed directly to `entry.unpack_in(dest_dir)` at install.rs:432, extracting into the symlink target. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: After `path.canonicalize()` at install.rs:147, re-run the `has_compat_segment` check on the canonical result and return `Err(InstallError::InvalidPath(...))` if the resolved path no longer contains a `compatibilitytools.d` component. Add a unit test that creates a tempdir symlink at a `compatibilitytools.d/` path pointing to a sibling directory and asserts the install is refused.

### HIGH

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs:304` — Unbounded response body for checksum fetches (`fetch_sha512_sidecar` at :304 and `fetch_sha256_manifest` at :341). Both call `.text().await` with no byte cap. Legitimate sidecars are ~200 B and manifests a few KiB; a hostile or mis-served CDN response would be buffered into RAM for the full 10 s timeout window. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Gate on `response.content_length()` with a hard ceiling (e.g., 1 MiB) before calling `.text()`, or stream via `bytes_stream()` with a running byte counter that errors past 64 KiB.
- **[F003]** `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs:681` — No URL origin allowlist before download. `download_url` and `checksum_url` originate from the `ProtonUpAvailableVersion` catalog entry (persisted in the SQLite cache from upstream GitHub responses). The `protonup_install_version_async` handler at src-tauri/.../protonup.rs:244 checks for client-vs-server URL mismatch (anti-frontend-tamper), but no code enforces that the host is `github.com` / `objects.githubusercontent.com`. A poisoned cache row or compromised upstream response could point the downloader at an attacker-controlled host. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Add a `validate_release_url(url: &str) -> Result<(), InstallError>` helper in `install.rs` that asserts `https` scheme and host ∈ {`github.com`, `api.github.com`, `objects.githubusercontent.com`}. Call it for `download_url` just after resolution at install.rs:663 and for `checksum_url` before `fetch_sha512_sidecar` / `fetch_sha256_manifest`. Apply the same check in provider-constructed checksum URLs (e.g., boxtron.rs:286).
- **[F004]** `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs:782` — `Sha256Manifest` missing-`checksum_url` path calls `best_effort_cleanup` inside the `ok_or_else` closure but never emits `Phase::Failed`. Every other error branch in the checksum dispatch (lines 750, 763, 775, 797, 811) emits a terminal `Phase::Failed` event. When a `Sha256Manifest` provider ships a version with no `checksum_url`, the frontend progress bar gets stuck permanently in `Verifying` with no terminal event. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Resolve the `ok_or_else` result into a local, emit `Phase::Failed` via `em` before the `?`, then propagate: `let manifest_url = match version_info.checksum_url.as_deref() { Some(u) => u, None => { best_effort_cleanup(&temp_path, None); if let Some(em) = em { em.emit(Phase::Failed, 0, None, Some(err.to_string())); } return Err(InstallError::ChecksumMissing(...)); } };`.
- **[F005]** `src/crosshook-native/crates/crosshook-core/src/protonup/providers/boxtron.rs:69` (and `ge_proton.rs:65`, `proton_cachyos.rs:63`, `proton_em.rs:68`, `luxtorpeda.rs:69`) — Unbounded `response.json::<Vec<GhRelease>>().await` for catalog fetch. The 10 s `protonup_http_client` timeout limits duration but not byte volume; a trickle attack can still buffer a large payload. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Add a shared `fetch_github_releases(client, url)` helper in `providers/mod.rs` that rejects responses with `content_length > MAX_CATALOG_BYTES` (e.g., 10 MiB) or streams via `bytes_stream()` with a running counter. Wire all five providers through it (also addresses F016).
- **[F006]** `src/crosshook-native/crates/crosshook-core/src/protonup/providers/mod.rs:94` — `registry(include_prereleases: bool)` accepts a parameter it immediately drops with `let _ = include_prereleases`, and `describe_providers()` at :141 calls `registry(false)` — a silent default. The prerelease flag belongs on `fetch()` (where it actually filters), not on registry enumeration, and keeping it here is misleading API surface. [maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Remove `include_prereleases` from the `registry()` and `find_provider_by_id()` signatures and from every call site. Keep the flag only on the `fetch()` path where it is actually consumed.
- **[F007]** `src/crosshook-native/src/components/proton-manager/InstallProgressBar.tsx:91` — The Cancel button (rendered in the non-terminal branch) has no `aria-label`, while the Dismiss button at :82 correctly sets `aria-label="Dismiss install status"`. Screen-reader users tabbing to an active install will hit an unlabeled interactive control. [a11y]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Add `aria-label={\`Cancel install ${opId.slice(0, 8)}\`}`(or a stable`"Cancel Proton install"`) to the Cancel`<button>`, mirroring the Dismiss button pattern.
- **[F008]** `src/crosshook-native/src/hooks/useProtonManager.ts:186` — `console.warn` left in production code inside the per-provider catalog fan-out catch block. Fires on every provider catalog failure in the shipped binary. Per CLAUDE.md, `console.log`/`console.warn` should not be in shipped frontend code. [pattern]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Remove the `console.warn` and propagate the per-provider failure into a `perProviderErrors` map (or the existing `allError` state) so the UI can surface it. If silent fallback is desired, move the diagnostic to `tracing::warn!` on the Rust side.
- **[F009]** `src/crosshook-native/src/types/protonup.ts:3` — `ProtonUpProvider` is a narrow literal union (`'ge-proton' | 'proton-cachyos' | 'proton-em'`) while `providers/mod.rs` may emit `'boxtron'` or `'luxtorpeda'` under the `experimental-providers` cargo feature. The unsafe cast `(selectedProviderId ?? 'ge-proton') as ProtonUpProvider` at `src/hooks/useProtonManager.ts:119` suppresses the type error instead of handling the widening at runtime. [type-safety]
  - **Status**: Fixed
  - **Category**: Type Safety
  - **Suggested fix**: Widen `ProtonUpProvider` to `string` (and document the canonical IDs) or explicitly add `'boxtron' | 'luxtorpeda'` to the union. Remove the `as ProtonUpProvider` assertion at `useProtonManager.ts:119`; if a narrowing is needed, validate the string against a runtime Set of known IDs.

### MEDIUM

- **[F010]** `docs/architecture/adr-0003-proton-download-manager.md:67` — The ADR treats Proton-EM as a "future" browse-only provider with `supports_install() → false`, but the shipped `proton_em.rs:38` sets `supports_install() → true` with `ChecksumKind::None`. Leaves the `ChecksumKind::None` install pathway undocumented in the authoritative design record. [maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Update ADR-0003 § _Provider trait_ to reflect that Proton-EM ships install-capable with `ChecksumKind::None`, and add a note that `None` is a first-class (flagged) install path rather than a deferred case.
- **[F011]** `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs:90` — `InstallError::Cancelled` maps to `ProtonUpInstallErrorKind::Unknown` in `to_result()`. Today this is reached only through the legacy `install_version` wrapper (which discards the error variant), but any future caller that inspects `error_kind` will misclassify cancellations. [completeness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add a `Cancelled` variant to `ProtonUpInstallErrorKind` on both the Rust enum and the TS union in `src/types/protonup.ts`, and route `InstallError::Cancelled` to it.
- **[F012]** `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs:407` — `tar::Archive::unpack_in` (crate 0.4.x) enforces the destination-prefix invariant on regular entries, but does not sanitize symlink _targets_ during extraction. An archive containing `GE-ProtonX/escape -> /etc` followed by `GE-ProtonX/escape/payload` can effect a symlink-chain escape outside `dest_dir`. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Either (a) iterate `archive.entries_with_seek()?` first and reject any `Symlink`/`Hardlink` whose `link_name()` resolves (via `dest_dir.join(link).canonicalize()`) outside `dest_dir`, or (b) disable symlink/hardlink extraction for the Proton tarballs (none of GE-Proton / CachyOS / EM require them for operation). Document the choice in a module-level comment.
- **[F013]** `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs:674` — `archive_filename` derived from the last `/`-segment of `download_url` is not validated for path separators or `..` before being interpolated into `format!(".tmp.{archive_filename}")`. Not directly exploitable (destination is canonicalized under `compatibilitytools.d`), but a hardening gap that depends on F001 being fixed. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: After deriving `archive_filename`, reject any value that contains `/`, `\\`, NUL, or `..` components: `if archive_filename.is_empty() || archive_filename.contains('/') || archive_filename.contains("..") { return Err(InstallError::InvalidPath(...)); }`.
- **[F014]** `src/crosshook-native/crates/crosshook-core/src/protonup/progress.rs:39` — The 64-slot broadcast channel silently drops progress messages under back-pressure (`let _ = tx.send(...)`). The 256 KiB `EMIT_INTERVAL_BYTES` throttle in the download phase is fine; the non-download phases have no rate limit and, combined with the Tauri event pump at `src-tauri/.../commands/protonup.rs:273`, can flood the webview during fast verify/extract transitions. [performance]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Either (a) document the 64-slot cap as the intentional rate limit, or (b) add a minimum-interval coalescer in the pump task (e.g., 50 ms) so the webview receives at most 20 progress events/second regardless of phase. Verify with a stress-test that an 80 Mbps download does not emit more than 10 events/s.
- **[F015]** `src/crosshook-native/crates/crosshook-core/src/protonup/providers/boxtron.rs:79` (and `luxtorpeda.rs:79`) — Both catalog-only providers define an identical `releases_to_versions` function whose body is a single call to `build_versions_from_releases`. The abstraction adds zero value. [maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Inline the three-line call and delete the local `releases_to_versions` functions in both files.
- **[F016]** `src/crosshook-native/crates/crosshook-core/src/protonup/providers/ge_proton.rs:59` (and `proton_cachyos.rs`, `proton_em.rs`) — All three installable providers have identical `fetch()` bodies (same headers, same `error_for_status`, same deserialize-then-`parse_releases`). The only differences are the URL constant and, in one case, a feature flag. The trait exists to deduplicate this. [maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Extract `async fn fetch_github_releases(client: &reqwest::Client, url: &str) -> Result<Vec<GhRelease>, ProviderError>` as `pub(super)` in `providers/mod.rs` and have the five providers call it, keeping only `parse_releases(...)` in the provider body. (This also gives F005 a single choke point for size limits.)
- **[F017]** `src/crosshook-native/crates/crosshook-core/src/protonup/providers/mod.rs:253` — `parse_releases` applies `take(max)` before `filter_map(gh_release_to_version)`. If the first `max` GitHub releases are drafts or lack a supported tarball asset, `filter_map` drops them and the final vector is short. Example: a catalog with `max = 30` draft/no-tarball entries followed by 30 valid releases returns 0 entries instead of 30. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Reorder to `.filter(...).filter_map(|r| gh_release_to_version(r, provider_id)).take(max).collect()` so `take` caps successfully parsed versions rather than releases attempted.
- **[F018]** `src/crosshook-native/crates/crosshook-core/src/protonup/uninstall.rs:32` — `SYSTEM_PREFIX_DENYLIST` covers `/usr`, `/opt`, `/snap`, and the Flatpak runtime tree, but not `/home` or `/root`. The `PathOutsideKnownRoots` check at step 3 still prevents deletion of arbitrary home paths, so the refusal fires via the _wrong_ gate. Defense-in-depth matters here because this module is the last line before `fs::remove_dir_all`. [security]
  - **Status**: Failed
  - **Category**: Security
  - **Suggested fix**: Add `"/home"` and `"/root"` to `SYSTEM_PREFIX_DENYLIST` so broad home-directory paths produce `UninstallError::SystemPathRefused` (the stated property) rather than relying on step 3.
- **[F019]** `src/crosshook-native/crates/crosshook-core/src/protonup/uninstall.rs:147` — The system-path refusal test accepts either `SystemPathRefused` or `PathOutsideKnownRoots`. There is no isolated test that the canonical `STEAM_SYSTEM_ROOTS` entries are refused specifically via `SystemPathRefused`. A regression that removed the `STEAM_SYSTEM_ROOTS` list would still pass. [completeness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add a test that constructs a tempdir whose canonical path matches one of the `STEAM_SYSTEM_ROOTS` entries (via a bind-style tempdir or by calling `plan_uninstall_core` with a pre-canonicalized path) and asserts `UninstallError::SystemPathRefused` exactly.
- **[F020]** `src/crosshook-native/src/components/proton-manager/ProtonManagerPanel.tsx:106` — Uninstall confirmation uses `window.confirm()`, a synchronous native dialog that bypasses the app's styling, accessibility baseline, and keyboard-trap conventions. [maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Replace with the project's modal/dialog component (consistent with how other destructive actions confirm) or an inline `aria-live`-backed confirmation pattern; preserve the existing "type the version to confirm" affordance if one exists elsewhere in the app.

### LOW

- **[F021]** `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:4` — `run_migrations` snapshots `user_version` once at start; a mid-run failure leaves the DB at the last committed version. Pre-existing pattern, not introduced by this PR. [completeness]
  - **Status**: Failed
  - **Category**: Completeness
  - **Suggested fix**: No action required for this PR. If future migrations add irreversible operations, consider re-reading `user_version` after each migration or wrapping the full sequence in a single transaction.
- **[F022]** `src/crosshook-native/crates/crosshook-core/src/metadata/proton_catalog_store.rs:1` — No explicit composite index on `(provider_id, fetched_at)` on `proton_release_catalog`. With 3 providers × ~30 rows each, performance is fine today, but a composite index makes the `ORDER BY fetched_at DESC` query planner-friendly as the cache ages. [performance]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Add `CREATE INDEX IF NOT EXISTS idx_proton_release_catalog_provider_fetched ON proton_release_catalog(provider_id, fetched_at DESC)` to the v22 migration (or a follow-up v24 if v22 is considered frozen).
- **[F023]** `src/crosshook-native/crates/crosshook-core/src/protonup/catalog.rs:53` — Benign `OnceLock` TOCTOU in `protonup_http_client`: two concurrent first-callers can both construct a `reqwest::Client`; the losing one is dropped. Equivalent clients means no correctness hazard. [security]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Use `OnceLock::get_or_try_init` (Rust ≥ 1.80) or `tokio::sync::OnceCell` to make initialization strictly atomic. Cosmetic.
- **[F024]** `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs:564` — `extract_archive` is offloaded to `spawn_blocking` but has no cancellation check inside the tar loop. For multi-GB archives, the user waits for extraction to complete before cancel takes effect. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Document as a known limitation in a module-level comment. A full fix requires a streaming async tar extractor; consider a periodic `cancel.is_cancelled()` check between archive entries by manually iterating `archive.entries()?` instead of calling `unpack_in` (this also enables F012's per-entry symlink validation).
- **[F025]** `src/crosshook-native/src-tauri/src/commands/protonup.rs:18` — `const DEFAULT_PROVIDER_ID: &str = "ge-proton";` is declared between two `use` blocks rather than after all `use` items. Minor layout nit. [pattern]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Move the constant below all `use` statements, above the first struct/function definition.
- **[F026]** `src/crosshook-native/src/components/proton-manager/VersionRow.tsx:38` — Status pill (`Installed` / `Available`) conveys state through both text and class-based color. Fine today, but confirm the `--installed` / `--available` colors meet WCAG AA contrast against `var(--crosshook-color-surface)` in both light and dark themes. [a11y]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Run a contrast check (or rely on the existing theme-token contrast audit if one exists) and document the results; no runtime change required if ratios pass.
- **[F027]** `src/crosshook-native/src/hooks/useProtonManager.ts:85` — `ALL_MODE_SENTINEL = null` is a zero-abstraction constant; it does not add beyond an inline comment. [maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Either inline `null` with a brief comment at each use site, or promote the constant to a discriminated-union type (`type ProviderSelection = { kind: 'all' } | { kind: 'specific'; id: string }`). As-is it adds a name without carrying semantics.

## Validation Results

| Check        | Result                                       |
| ------------ | -------------------------------------------- |
| Type check   | Pass (`tsc --noEmit`)                        |
| Lint         | Pass (rustfmt, `clippy -D warnings`, biome)  |
| Tests        | Pass (`cargo test -p crosshook-core` — 1058) |
| Build        | Skipped (CI `build-native.sh --binary-only`) |
| Host-gateway | Pass (`scripts/check-host-gateway.sh`)       |

## Files Reviewed

- `AGENTS.md` (Modified)
- `CHANGELOG.md` (Modified)
- `CLAUDE.md` (Modified)
- `docs/architecture/adr-0003-proton-download-manager.md` (Added)
- `src/crosshook-native/Cargo.lock` (Modified)
- `src/crosshook-native/crates/crosshook-core/Cargo.toml` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/proton_catalog_store.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/protonup/install_root.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/protonup/progress.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/boxtron.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/ge_proton.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/luxtorpeda.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/mod.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/proton_cachyos.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/proton_em.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/protonup/uninstall.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/Cargo.toml` (Modified)
- `src/crosshook-native/src-tauri/src/commands/protonup.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/settings.rs` (Modified)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/App.tsx` (Modified)
- `src/crosshook-native/src/components/SettingsPanel.tsx` (Modified)
- `src/crosshook-native/src/components/icons/SidebarIcons.tsx` (Modified)
- `src/crosshook-native/src/components/layout/ContentArea.tsx` (Modified)
- `src/crosshook-native/src/components/layout/PageBanner.tsx` (Modified)
- `src/crosshook-native/src/components/layout/Sidebar.tsx` (Modified)
- `src/crosshook-native/src/components/layout/routeMetadata.ts` (Modified)
- `src/crosshook-native/src/components/pages/ProtonManagerPage.tsx` (Added)
- `src/crosshook-native/src/components/proton-manager/InstallProgressBar.tsx` (Added)
- `src/crosshook-native/src/components/proton-manager/InstallRootBadge.tsx` (Added)
- `src/crosshook-native/src/components/proton-manager/ProtonManagerPanel.tsx` (Added)
- `src/crosshook-native/src/components/proton-manager/ProviderPicker.tsx` (Added)
- `src/crosshook-native/src/components/proton-manager/VersionRow.tsx` (Added)
- `src/crosshook-native/src/hooks/useProtonInstallProgress.ts` (Added)
- `src/crosshook-native/src/hooks/useProtonInstalls.ts` (Modified)
- `src/crosshook-native/src/hooks/useProtonManager.ts` (Added)
- `src/crosshook-native/src/hooks/useProtonUp.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/protonup.ts` (Modified)
- `src/crosshook-native/src/lib/protonup/classifyInstall.ts` (Added)
- `src/crosshook-native/src/lib/protonup/format.ts` (Added)
- `src/crosshook-native/src/styles/proton-manager.css` (Added)
- `src/crosshook-native/src/types/protonup.ts` (Modified)
- `src/crosshook-native/src/types/settings.ts` (Modified)
