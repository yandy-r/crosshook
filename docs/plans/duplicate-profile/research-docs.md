# Documentation Research: duplicate-profile

## Architecture Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/duplicate-profile/feature-spec.md`: **Primary source of truth** for the duplicate-profile design. Defines the full implementation across backend (ProfileStore::duplicate), Tauri IPC (profile_duplicate command), and frontend (useProfile hook + ProfileActions button). Includes architecture diagram, data models, name generation algorithm, phased implementation plan, and risk assessment.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/duplicate-profile/research-technical.md`: Detailed technical architecture covering component diagram, data flow, API design, core library design, name generation/collision algorithm, and frontend integration. Specifies all 8 files to modify with exact change descriptions. Confirms no new files are needed.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/duplicate-profile/research-recommendations.md`: Implementation approach comparison (Option A: ProfileStore::duplicate in core, Option B: compose in Tauri command layer, Option C: filesystem copy, Option D: atomic write). Feature spec chose Option A; recommendations doc originally favored Option B. Both agree on name generation, conflict detection, and UI placement.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md`: Authoritative project architecture reference. Defines the workspace separation pattern ("crosshook-core contains all business logic; crosshook-cli and src-tauri are thin consumers"), file structure, code conventions, commit/changelog hygiene, and GitHub workflow requirements.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md`: Mirrors CLAUDE.md content; useful as a standalone project orientation for agents unfamiliar with the codebase.

## API/IPC Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/duplicate-profile/research-external.md`: External patterns research covering Tauri v2 IPC command design, TOML file operations, filesystem safety (conflict detection approaches), name generation algorithms (OS convention survey), and similar desktop app implementations (VS Code, Figma, Lutris).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs`: Existing Tauri profile command handlers (profile*list, profile_load, profile_save, profile_delete, profile_rename, profile_import_legacy, profile_save_launch_optimizations). The new `profile_duplicate` command must follow the same pattern: receive `State<'*, ProfileStore>`, delegate to core, `map_err(map_error)`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: Tauri setup file where all commands are registered in the `invoke_handler`. The new `profile_duplicate` command must be added here.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: ProfileStore implementation with load/save/list/rename/delete/profile_path methods. Key facts: `save()` has no existence guard (silently overwrites), `rename()` also overwrites via `fs::rename`, `validate_name()` rejects empty/path-traversal/reserved chars. The `duplicate()` method, `generate_unique_copy_name()`, `strip_copy_suffix()`, and `DuplicateProfileResult` struct go here.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`: Module root that re-exports public types. `DuplicateProfileResult` must be added to the `pub use toml_store::{...}` line.
- [Tauri v2 IPC Documentation](https://v2.tauri.app/develop/calling-rust/): External reference for command pattern, argument serialization, and error handling conventions.
- [Rust std::fs::OpenOptions](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html): Reference for atomic `create_new` if stricter safety is ever needed (not required for MVP).

## Development Guides

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/duplicate-profile/research-ux.md`: Comprehensive UX research covering competitive analysis (Lutris, VS Code, JetBrains, Figma, Bottles, Firefox, macOS Finder, Windows Explorer), terminology analysis ("Duplicate" preferred over "Copy"/"Clone"), keyboard shortcut conventions (Ctrl+D), gamepad/Steam Deck considerations, error handling UX, and feedback patterns. Directly informs UI placement, button styling, loading states, and post-duplicate focus behavior.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/duplicate-profile/research-business.md`: Business requirements with 12 rules, user stories, domain model, state transitions, error recovery table, and existing codebase integration analysis. Documents the critical rule that duplicate must NEVER silently overwrite (unlike save/rename which do).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/getting-started/quickstart.md`: End-user guide covering profile creation, TOML format, launch modes, and save semantics. Useful for understanding the user mental model that duplicate must fit into.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md`: Feature guide documenting launch methods, auto-discovery, launcher export, and console view. Relevant for understanding launcher association semantics (duplicated profiles do NOT inherit exported launchers).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/research/additional-features/implementation-guide.md`: Positions #56 as a quick win (hours-level effort) that unblocks #50 (optimization presets). Confirms the implementation scope: one Tauri command composing load+save with conflict check.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/research/additional-features/deep-research-report.md`: Rates profile duplicate/clone at 3/8 complexity, Low risk, Medium value, Ready status. Identifies the missing `profile_duplicate` command as the only gap.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/tasks/lessons.md`: Implementation lessons from prior work. Key relevant entries: gamepad handlers must skip editable controls (affects Ctrl+D shortcut), Tauri plugin permissions must be verified in capabilities JSON, and when refactoring UI, explicitly audit every feature surface to avoid losing render sites.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/pull_request_template.md`: PR template requiring: issue linkage (`Closes #56`), type-of-change checkboxes, build verification checklist, and conditional checks for profile/ and UI component changes.

## Type Definitions

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts`: Frontend TypeScript types including `GameProfile`, `LaunchMethod`, `TrainerLoadingMode`, and `ProfileData` (legacy). The new `DuplicateProfileResult` interface (`{ name: string; profile: GameProfile }`) must be added here.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: Canonical Rust `GameProfile` struct with all sections (game, trainer, injection, steam, runtime, launch). Derives `Clone`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`, `Default`. The `Clone` derive is critical -- it enables `.clone()` for the full deep copy in `duplicate()`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/index.ts`: TypeScript re-exports. May need to export `DuplicateProfileResult` if other modules consume it.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/launcher.ts`: Launcher lifecycle types (info, delete, rename results). Reference for how other result types are structured in the type system.

## README Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/README.md`: High-level product overview. Does not need updating for the duplicate feature since it does not enumerate individual profile actions.

## Must-Read Documents (Prioritized for Implementers)

### Required Reading (read before writing any code)

1. **`docs/plans/duplicate-profile/feature-spec.md`** -- The agreed feature contract. Defines the architecture, data models, API design, name generation algorithm, files to modify, phasing, and risk assessment. This is the implementation blueprint.
2. **`src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`** -- The file receiving the most changes. Understand `save()` (no existence guard), `load()`, `list()`, `rename()`, `validate_name()`, and the existing test patterns before adding `duplicate()`.
3. **`src/crosshook-native/crates/crosshook-core/src/profile/models.rs`** -- Understand `GameProfile` struct and its `Clone` derive. Confirms that `.clone()` produces a full deep copy with no manual field handling.
4. **`src/crosshook-native/src-tauri/src/commands/profile.rs`** -- Understand the existing command pattern (`State<'_, ProfileStore>`, `map_err(map_error)`, `Result<T, String>`) to match it for `profile_duplicate`.
5. **`src/crosshook-native/src/hooks/useProfile.ts`** -- Understand `refreshProfiles()`, `loadProfile()`, `hydrateProfile()`, the `saving`/`deleting` state pattern, and the `UseProfileResult` interface that `duplicateProfile` must extend.

### Strongly Recommended (read for context and edge cases)

6. **`docs/plans/duplicate-profile/research-technical.md`** -- Detailed technical specifications including test cases, frontend integration patterns, and all 5 technical decisions with rationale.
7. **`docs/plans/duplicate-profile/research-business.md`** -- 12 business rules, edge case table, and the critical no-overwrite constraint.
8. **`docs/plans/duplicate-profile/research-ux.md`** -- UI placement, button styling, loading states, gamepad behavior, terminology, and competitive analysis that informed the design.
9. **`CLAUDE.md`** -- Project conventions, workspace separation pattern, commit message requirements, and label taxonomy.
10. **`tasks/lessons.md`** -- Prior implementation lessons, especially around gamepad handler exclusions for editable controls.

### Nice-to-Have (background context)

11. **`docs/plans/duplicate-profile/research-external.md`** -- External patterns for Tauri IPC, TOML operations, filesystem safety, and name generation algorithms.
12. **`docs/plans/duplicate-profile/research-recommendations.md`** -- Alternative approach comparison and risk assessment.
13. **`docs/research/additional-features/implementation-guide.md`** -- Positions #56 in the broader feature roadmap; confirms quick-win status.
14. **`docs/getting-started/quickstart.md`** -- User-facing profile workflow context.
15. **`docs/features/steam-proton-trainer-launch.doc.md`** -- Launcher export semantics (duplicated profiles do NOT inherit launchers).

## Documentation Gaps

1. **No `ProfileContext.tsx` documentation**: The feature spec references `useProfileContext()` and `ProfileContext.tsx` as the context wrapper that provides `duplicateProfile` to consumers, but there is no dedicated documentation for this component's interface or how it wraps `useProfile`. Implementers must read the source directly.
2. **No `ProfileActions.tsx` prop documentation**: The component's current props interface is not documented outside the source code. The feature spec defines the new `canDuplicate`/`onDuplicate` props, but the existing prop shape must be discovered from the source.
3. **No `ProfilesPage.tsx` architecture documentation**: The page component that wires context to actions is not documented. Its role in the duplicate flow (providing `canDuplicate` guard logic and wiring `onDuplicate` callback) must be understood from source code.
4. **Disagreement between feature-spec and recommendations on implementation approach**: The feature spec (Section "Recommendations") places `duplicate()` in `ProfileStore` (crosshook-core), while `research-recommendations.md` originally favored composing in the Tauri command layer. The feature spec takes precedence as the final design decision, but implementers should be aware of this history.
5. **No CLI duplication documentation**: The feature is scoped to Tauri UI only (#56), but the `ProfileStore::duplicate()` method is designed to be CLI-reusable. No CLI integration documentation exists yet.
6. **No automated frontend test coverage**: The project has no frontend test framework configured. All duplicate-feature UI verification is manual. The testing plan in the feature spec acknowledges this.
