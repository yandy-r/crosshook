# Fix Report: pr-281-review

**Source**: `docs/prps/reviews/pr-281-review.md`
**Applied**: 2026-04-17
**Mode**: Parallel sub-agents (1 batch, max width 6)
**Severity threshold**: LOW

## Summary

- **Total findings in source**: 27
- **Already processed before this run**:
  - Fixed: 19
  - Failed: 1
- **Eligible this run**: 7
- **Applied this run**:
  - Fixed: 6
  - Failed: 1
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                                              | Line | Status | Notes                                                                                 |
| ---- | -------- | --------------------------------------------------------------------------------- | ---- | ------ | ------------------------------------------------------------------------------------- |
| F021 | LOW      | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`           | 4    | Failed | Suggested fix explicitly says no action is required for this PR.                      |
| F022 | LOW      | `src/crosshook-native/crates/crosshook-core/src/metadata/proton_catalog_store.rs` | 1    | Fixed  | Already satisfied in current code; v22 migration already creates the composite index. |
| F023 | LOW      | `src/crosshook-native/crates/crosshook-core/src/protonup/catalog.rs`              | 53   | Fixed  | Replaced manual singleton init with async `tokio::sync::OnceCell`.                    |
| F024 | LOW      | `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs`              | 564  | Fixed  | Added module-level note documenting extraction cancellation limits.                   |
| F025 | LOW      | `src/crosshook-native/src-tauri/src/commands/protonup.rs`                         | 18   | Fixed  | Moved `DEFAULT_PROVIDER_ID` below all `use` items.                                    |
| F026 | LOW      | `src/crosshook-native/src/components/proton-manager/VersionRow.tsx`               | 38   | Fixed  | Documented WCAG AA audit results; no visual change required.                          |
| F027 | LOW      | `src/crosshook-native/src/hooks/useProtonManager.ts`                              | 85   | Fixed  | Removed the `ALL_MODE_SENTINEL` constant and inlined `null` with comments.            |

## Files Changed

- `src/crosshook-native/crates/crosshook-core/src/protonup/catalog.rs` (Fixed F023)
- `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs` (Fixed F024)
- `src/crosshook-native/src-tauri/src/commands/protonup.rs` (Fixed F025)
- `src/crosshook-native/src/hooks/useProtonManager.ts` (Fixed F027)
- `src/crosshook-native/src/styles/proton-manager.css` (Fixed F026)

## Failed Fixes

### F021 — `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:4`

**Severity**: LOW
**Category**: Completeness
**Description**: `run_migrations` snapshots `user_version` once at start; a mid-run failure leaves the DB at the last committed version. Pre-existing pattern, not introduced by this PR.
**Suggested fix (from review)**: No action required for this PR. If future migrations add irreversible operations, consider re-reading `user_version` after each migration or wrapping the full sequence in a single transaction.
**Blocker**: The suggested fix is explicitly non-actionable for this PR, so changing migration behavior here would go beyond the finding's own stated scope.
**Recommendation**: Leave `run_migrations` unchanged in this PR. If irreversible migrations are added later, open a dedicated follow-up to revisit transaction boundaries or version re-reads.

## Validation Results

| Check      | Result                                                                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Type check | Fail (`npx tsc --noEmit -p src/crosshook-native/tsconfig.json` reports existing errors in `src/crosshook-native/src/lib/plugin-stubs/fs.ts`) |
| Tests      | Pass (`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`)                                                        |

## Post-change Review

No findings were reported by the required code-review pass on the local diff.

Residual validation gap:

- There is no focused test that exercises concurrent first-call initialization of `protonup_http_client()`.
- Frontend-only edits were not covered by browser or component tests because this repo does not ship a frontend test framework.

## Next Steps

- Re-run `$code-review 281` to confirm the remaining review state from the updated artifact
- Address `F021` only if you want to take on migration behavior as a separate follow-up
- Investigate the existing TypeScript errors in `src/crosshook-native/src/lib/plugin-stubs/fs.ts`
- Run `$git-workflow` when you are ready to commit the fixes
