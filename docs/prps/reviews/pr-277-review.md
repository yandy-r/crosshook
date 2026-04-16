# PR Review #277 — feat(onboarding): sqlite-backed host readiness catalog

**Reviewed**: 2026-04-16T14:15:41-04:00
**Mode**: PR
**Author**: yandy-r
**Branch**: feat/flatpak-host-rediness -> main
**Decision**: REQUEST CHANGES

## Summary

The readiness catalog and SQLite-backed onboarding work are directionally good, and the validation stack passes on the PR head. The blocking issue is that the new distro-aware guidance path can emit blank install commands for SteamOS and gaming-immutable hosts, and there are two additional persistence/UX gaps around dismissal handling and browser-mode parity.

## Findings

### CRITICAL

- None.

### HIGH

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs:141` — `build_umu_install_advice()` accepts catalog rows whose `command` is empty, and the new default catalog does that for `SteamOS` and `GamingImmutable`. When `umu-run` is actually missing on those hosts, onboarding emits `umu_install_guidance.install_command = ""` and the review UI renders a blank `Copy command` action instead of actionable install guidance.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Ignore empty-command catalog rows in `build_umu_install_advice()` and fall back to a usable docs-only or `Unknown` guidance path before constructing `UmuInstallGuidance`.

### MEDIUM

- **[F002]** `src/crosshook-native/src-tauri/src/commands/onboarding.rs:121` — `dismiss_readiness_nag()` returns `Ok(())` when `MetadataStore` is unavailable, but there is no fallback persistence path for host-tool dismissals. In the supported SQLite-disabled mode, the UI clears the reminder locally and then the reminder reappears on the next readiness run because nothing was actually saved.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Either persist a degraded fallback dismissal state in `settings.toml`, or return an error so the frontend does not pretend the dismissal succeeded.

- **[F003]** `src/crosshook-native/src/components/ReadinessChecklist.tsx:193` — the new host-tool guidance UI only exposes `Install help` when `install_guidance.command` is non-empty. Several catalog rows intentionally rely on `alternatives` and `docs_url` with an empty command for SteamOS and immutable hosts, so those missing-tool states currently render with no expandable help, docs link, or dismiss action at all.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Treat non-empty `alternatives` or `docs_url` as sufficient to show the guidance block, and only hide the `Copy command` button when the command itself is empty.

### LOW

- **[F004]** `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts:166` — the new `dismiss_readiness_nag` browser-dev mock is a no-op, so rerunning generalized readiness resurrects dismissed host-tool reminders immediately. That makes browser-only UI verification diverge from the real backend for the new feature.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Track dismissed tool IDs in the mock store and clear matching `install_guidance` entries in `buildMockReadinessResult()` before returning.

## Validation Results

| Check      | Result |
| ---------- | ------ |
| Type check | Pass   |
| Lint       | Pass   |
| Tests      | Pass   |
| Build      | Pass   |

## Files Reviewed

- `.cursorrules` (Modified)
- `AGENTS.md` (Modified)
- `CLAUDE.md` (Modified)
- `docs/prps/plans/completed/umu-migration-phase-4-auto-default.plan.md` (Modified)
- `src/crosshook-native/assets/default_host_readiness_catalog.toml` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/readiness_catalog_store.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/readiness_dismissal_store.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/readiness_snapshot_store.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/catalog.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/onboarding.rs` (Modified)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/components/LaunchPanel.tsx` (Modified)
- `src/crosshook-native/src/components/OnboardingWizard.tsx` (Modified)
- `src/crosshook-native/src/components/PinnedProfilesStrip.tsx` (Modified)
- `src/crosshook-native/src/components/ReadinessChecklist.tsx` (Modified)
- `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx` (Modified)
- `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx` (Modified)
- `src/crosshook-native/src/hooks/useOnboarding.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/wrapHandler.ts` (Modified)
- `src/crosshook-native/src/styles/preview.css` (Modified)
- `src/crosshook-native/src/types/onboarding.ts` (Modified)
- `src/crosshook-native/src/utils/hostReadinessTooltips.ts` (Added)
