# Profile Rename -- Documentation Research

Profile rename has the most comprehensive pre-implementation documentation of any CrossHook feature: a complete feature spec, five focused research reports, and strong precedent from the duplicate-profile feature's documentation. This document catalogs every relevant document an implementer should read, organized by priority.

## Must-Read Documents (Priority Order)

### 1. Feature Specification (Implementation Blueprint)

- **`docs/plans/profile-rename/feature-spec.md`**: The single source of truth. Contains executive summary, architecture overview with data flow diagram, API design (enhanced `ProfileStore::rename()` and `profile_rename` Tauri command), files-to-modify table (~6 files, ~75 lines), UX design (Rename button + modal dialog), phased task breakdown (3 phases), risk assessment, and 4 decisions needing resolution. **Read this first.**

### 2. Project Guidelines

- **`CLAUDE.md`** (project root): Defines workspace separation ("crosshook-core contains all business logic"), code conventions (Rust `snake_case`, React `PascalCase`), commit message requirements (conventional commits for git-cliff changelog), PR template usage, label taxonomy, and build commands. **Must read before writing any code.**

### 3. Technical Specification

- **`docs/plans/profile-rename/research-technical.md`**: Detailed architecture design with component diagram, data model analysis (profile name = filename, NOT stored in TOML), enhanced API contracts with exact Rust/TypeScript signatures, system constraints (atomicity, case sensitivity, concurrent access), complete files-to-modify and files-NOT-to-modify tables with rationale, 4 technical decisions with options analysis, and cross-team synthesis of UX vs technical recommendations.

### 4. Business Logic & Requirements

- **`docs/plans/profile-rename/research-business.md`**: 4 user stories with acceptance criteria, 7 business rules, 7 edge cases, primary workflow (rename via name field edit), error recovery flows, domain model (entities and state transitions), existing codebase integration status (backend ~90% done, frontend needs implementation), and success criteria checklist.

### 5. UX Research

- **`docs/plans/profile-rename/research-ux.md`**: Competitive analysis of 5 launchers (Steam, Bottles, Lutris, Heroic, VS Code), 3 alternative workflow designs with pros/cons, industry standards for rename operations (PatternFly, Carbon, NNGroup), accessibility requirements (ARIA, keyboard, gamepad), Steam Deck considerations (virtual keyboard, dialog sizing, touch targets), validation timing patterns, error display standards, and prioritized recommendations (P0/P1/P2).

### 6. External API & Library Research

- **`docs/plans/profile-rename/research-external.md`**: Evaluation of `std::fs::rename` (recommended), `renamore` (rejected), `atomic-write-file` (wrong tool), `tempfile::persist_noclobber` (wrong use case). Tauri v2 File System Plugin analysis (not recommended for backend ops). Integration patterns: direct rename with pre-check, write-then-delete for launchers, cascade update pattern, Tauri IPC result pattern. 6 constraints/gotchas documented with mitigations.

### 7. Recommendations & Risk Assessment

- **`docs/plans/profile-rename/research-recommendations.md`**: Implementation approach (atomic `fs::rename` + cascading), technology choices rationale, 3-phase plan with task breakdown, quick wins list, complete existing code inventory, competitive landscape summary, risk matrix (6 technical risks with likelihood/impact/mitigation), integration challenges, and 3 alternative approaches compared.

## Comparable Feature Documentation (Precedent)

The duplicate-profile feature was the most recent similar feature and provides the best implementation precedent:

- **`docs/plans/duplicate-profile/shared.md`**: Shared context document showing the three-layer pattern (core -> command -> hook) that rename must follow. Lists relevant files, patterns, and docs for duplicate. **Critical for understanding the implementation pattern.**
- **`docs/features/profile-duplication.doc.md`**: User-facing feature documentation. Shows the expected documentation format for when rename ships. Covers workflow, name generation, edge cases, gamepad usage, troubleshooting.
- **`docs/features/profile-duplication.arch.md`**: Architectural analysis with mermaid diagrams (data flow sequence, name generation flowchart, layer architecture graph), safety mechanisms, test coverage table. **The rename feature doc should follow this format.**
- **`docs/api/profile-duplicate.md`**: API reference for `profile_duplicate` command. Shows the exact format for documenting Tauri IPC commands including Rust signatures, TypeScript invocation, parameters, response types, error tables, side effects, and frontend integration. **The `profile_rename` API doc should follow this template.**
- **`docs/plans/duplicate-profile/parallel-plan.md`**: The implementation plan used for duplicate. Shows how tasks were decomposed into parallel batches.

## Configuration Files

These files contain settings relevant to the implementation:

- **`src/crosshook-native/Cargo.toml`**: Workspace root defining members (`crosshook-core`, `crosshook-cli`, `src-tauri`), version `0.2.2`. No changes needed.
- **`src/crosshook-native/crates/crosshook-core/Cargo.toml`**: Core library dependencies -- `serde`, `toml 0.8`, `directories 5`, `tracing 0.1`, `tempfile 3` (dev). No new dependencies needed for rename.
- **`src/crosshook-native/src-tauri/tauri.conf.json`**: Tauri config -- AppImage target, 1280x800 dark theme window. No changes needed. Confirms Steam Deck display dimensions for dialog sizing.
- **`src/crosshook-native/package.json`**: Frontend dependencies -- React 18, `@tauri-apps/api ^2.0.0`, `@radix-ui/react-select`, `@radix-ui/react-tabs`, `react-resizable-panels`. No new dependencies needed.

## GitHub Workflow Templates

- **`.github/pull_request_template.md`**: PR template with Summary, Changes, Type of Change checkboxes, Testing section with build checklist. **Profile changes trigger**: "If touching `crates/crosshook-core/src/profile/`: Verified profile save/load/import" and "If touching `src/components/` or `src/hooks/`: Verified UI renders correctly".
- **`.github/ISSUE_TEMPLATE/feature_request.yml`**: Feature request form template. Profile rename likely already has a linked issue.
- **`.github/workflows/release.yml`**: CI release workflow. Build verification runs `./scripts/build-native.sh` and `cargo test -p crosshook-core`.

## Development Guides

- **`docs/getting-started/quickstart.md`**: First-time setup and basic usage guide. Shows how profiles are created and managed from the user's perspective. Useful for understanding the user mental model.
- **`docs/features/steam-proton-trainer-launch.doc.md`**: Feature guide for the core launch workflow. Useful background for understanding how profiles relate to game launching, but not directly needed for rename.

## Other Feature Plans (For Pattern Reference)

These feature plans demonstrate the repo's planning conventions:

- **`docs/plans/profile-modal/`**: Profile review modal -- 10 research files, parallel-plan, shared context. Most recent completed feature plan.
- **`docs/plans/update-game/`**: Update game panel -- same research structure. Shows research-to-implementation pipeline.
- **`docs/plans/proton-optimizations/`**: Proton optimizations -- full plan set. Good example of phased backend work.

## External Documentation References

Referenced in the feature research:

- [std::fs::rename (Rust)](https://doc.rust-lang.org/std/fs/fn.rename.html) -- Core rename primitive
- [rename(2) man page](https://man7.org/linux/man-pages/man2/rename.2.html) -- POSIX atomicity guarantees
- [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/) -- IPC command patterns
- [Tauri v2 Calling Frontend](https://v2.tauri.app/develop/calling-frontend/) -- Event system (not needed initially)
- [Tauri v2 File System Plugin](https://v2.tauri.app/plugin/file-system/) -- Evaluated, not recommended for backend ops
- [PatternFly Inline Edit Guidelines](https://www.patternfly.org/components/inline-edit/design-guidelines/) -- Modal vs inline UX
- [Carbon Edit Pattern](https://carbondesignsystem.com/community/patterns/edit-pattern/) -- Edit/rename UX patterns
- [NNGroup: Confirmation Dialogs](https://www.nngroup.com/articles/confirmation-dialog/) -- Undo vs confirm for reversible actions
- [Steamworks: Steam Deck Recommendations](https://partner.steamgames.com/doc/steamdeck/recommendations) -- Virtual keyboard, gamepad input
- [VS Code Profiles](https://code.visualstudio.com/docs/configure/profiles) -- Gold standard for profile management UX

## Documentation Gaps

1. **No `shared.md` for profile-rename yet**: The duplicate-profile feature had `docs/plans/duplicate-profile/shared.md` as a synthesized context document. Profile rename needs one before implementation begins (combining architecture, patterns, integrations, and docs into a single reference).
2. **No `parallel-plan.md` for profile-rename yet**: Implementation task decomposition hasn't been created yet. The feature-spec has a "Task Breakdown Preview" but not a formal parallel plan.
3. **No existing API doc for `profile_rename`**: Unlike `profile_duplicate` which has `docs/api/profile-duplicate.md`, the rename command has no API reference doc yet. Should be created post-implementation following the duplicate template.
4. **No user-facing feature doc for rename**: Will need `docs/features/profile-rename.doc.md` and optionally `docs/features/profile-rename.arch.md` post-implementation, following the duplicate feature's documentation pattern.
5. **No `analysis-code.md`, `analysis-context.md`, or `analysis-tasks.md`**: These intermediate analysis artifacts exist for other features but haven't been generated for profile-rename yet. They're created during the `shared-context` -> `parallel-plan` workflow.
