# Documentation Research: protondb-lookup

## Relevant Documentation Files

- /AGENTS.md: repo-wide architecture constraints, especially “business logic in `crosshook-core`” and Tauri `snake_case` IPC naming
- /docs/getting-started/quickstart.md: current user-facing explanation of Steam App ID auto-populate, profile editing, and launch workflows
- /docs/features/steam-proton-trainer-launch.doc.md: deeper explanation of Steam metadata, launch optimizations, and Steam launch options
- /docs/research/additional-features/deep-research-report.md: original research source that identified ProtonDB lookup as issue `#53`
- /docs/research/additional-features/implementation-guide.md: backlog ordering and dependency note showing `#53` after version correlation `#41`
- /docs/plans/protondb-lookup/feature-spec.md: synthesized feature-research output for this task

## Documentation Coverage Notes

- `quickstart.md` already teaches users about Steam App ID and auto-populate, so it is the natural place to document how ProtonDB lookup is triggered.
- `steam-proton-trainer-launch.doc.md` already documents launch optimizations, Steam app launch, and Steam launch options; any “copy/apply suggestion” behavior should be described there as part of the same user mental model.
- The research docs are important because they establish why issue `#53` is lower priority than diagnostics/version work and why it should reuse existing version/metadata infrastructure rather than invent a new subsystem.

## Must-Read Topics For Implementation

- Steam App ID derivation and editor placement
- existing metadata cache reuse
- existing launch-options/custom-env flows for copy/apply recommendations
- distinction between community compatibility metadata and live ProtonDB tiers

## Missing Documentation Gaps

- There is no current documentation for an external web-service integration pattern in `crosshook-core`.
- There is no user-facing explanation yet for cache freshness or stale advisory metadata in the profile editor.
- There is no existing doc section that explains how a remote advisory feature should interact with `launch.custom_env_vars`.
