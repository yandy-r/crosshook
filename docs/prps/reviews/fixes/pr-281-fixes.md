# Fix Report: pr-281-review

**Source**: `docs/prps/reviews/pr-281-review.md`
**Applied**: 2026-04-17
**Mode**: Parallel sub-agents (1 batch, max width 6)
**Severity threshold**: MEDIUM

## Summary

- **Total findings in source**: 27
- **Already processed before this run**:
  - Fixed: 9
  - Failed: 0
- **Eligible this run**: 11
- **Applied this run**:
  - Fixed: 10
  - Failed: 1
- **Skipped this run**:
  - Below severity threshold: 7
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                                             | Line | Status | Notes                                                                                   |
| ---- | -------- | -------------------------------------------------------------------------------- | ---- | ------ | --------------------------------------------------------------------------------------- |
| F010 | MEDIUM   | `docs/architecture/adr-0003-proton-download-manager.md`                          | 67   | Fixed  | ADR now documents Proton-EM as install-capable and `ChecksumKind::None` as a live path. |
| F011 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs`             | 90   | Fixed  | Added `cancelled` error kind across Rust + TS IPC types.                                |
| F012 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs`             | 407  | Fixed  | Link validation now rejects only archive links that escape the install root.            |
| F013 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs`             | 674  | Fixed  | Unsafe derived archive filenames now fail before temp-path creation.                    |
| F014 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/progress.rs`            | 39   | Fixed  | Broadcast buffer cap is now explicit and documented as intentional lossy back-pressure. |
| F015 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/providers/boxtron.rs`   | 79   | Fixed  | Removed redundant catalog-only wrapper helpers in Boxtron and Luxtorpeda.               |
| F016 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/providers/ge_proton.rs` | 59   | Fixed  | Installable providers now share a single GitHub fetch-and-parse helper.                 |
| F017 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/providers/mod.rs`       | 253  | Fixed  | `take(max)` now limits successfully parsed versions, not attempted releases.            |
| F018 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/uninstall.rs`           | 32   | Failed | Suggested `/home`/`/root` denial changes module semantics and misclassifies user paths. |
| F019 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/protonup/uninstall.rs`           | 147  | Fixed  | Added an exact `SystemPathRefused` regression test for explicit Steam system roots.     |
| F020 | MEDIUM   | `src/crosshook-native/src/components/proton-manager/ProtonManagerPanel.tsx`      | 106  | Fixed  | Replaced `window.confirm()` with inline accessible confirmation UI.                     |

## Files Changed

- `docs/architecture/adr-0003-proton-download-manager.md` (F010)
- `src/crosshook-native/crates/crosshook-core/src/protonup/install.rs` (F011, F012, F013)
- `src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs` (F011)
- `src/crosshook-native/crates/crosshook-core/src/protonup/progress.rs` (F014)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/mod.rs` (F016, F017)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/ge_proton.rs` (F016)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/proton_cachyos.rs` (F016)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/proton_em.rs` (F016)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/boxtron.rs` (F015)
- `src/crosshook-native/crates/crosshook-core/src/protonup/providers/luxtorpeda.rs` (F015)
- `src/crosshook-native/crates/crosshook-core/src/protonup/uninstall.rs` (F019; F018 attempted and rejected)
- `src/crosshook-native/src/components/proton-manager/ProtonManagerPanel.tsx` (F020)
- `src/crosshook-native/src/styles/proton-manager.css` (F020)
- `src/crosshook-native/src/types/protonup.ts` (F011)

## Failed Fixes

### F018 — `src/crosshook-native/crates/crosshook-core/src/protonup/uninstall.rs:32`

**Severity**: MEDIUM  
**Category**: Security  
**Description**: The suggested fix pushes broad `/home` / `/root` paths through `SystemPathRefused` instead of `PathOutsideKnownRoots`.

**Suggested fix (from review)**: Add `"/home"` and `"/root"` to `SYSTEM_PREFIX_DENYLIST` so broad home-directory paths produce `UninstallError::SystemPathRefused`.

**Blocker**: That change breaks the documented uninstall contract by misclassifying normal user-owned home paths as system paths, and an earlier implementation also blocked valid native Steam uninstall roots under `$HOME/.local/share/Steam/compatibilitytools.d`.

**Recommendation**: Keep the current `PathOutsideKnownRoots` behavior for unmatched home paths unless the product/API contract is deliberately changed. If explicit reclassification is still desired, update the module docs, user-facing copy, and tests first, then revisit the design in a dedicated follow-up.

## Validation Results

| Check               | Result |
| ------------------- | ------ |
| Type check          | Pass   |
| Frontend type check | Pass   |
| Tests               | Pass   |

## Next Steps

- Re-run `$code-review 281` to verify the remaining open findings and confirm the fixed ones stay resolved.
- Decide whether `F018` should remain a documentation-level disagreement or become a separate design issue.
- Run `$git-workflow` when you want to commit the fix batch.
