# Documentation Research: game-details-modal

## Architecture Docs

- `AGENTS.md`: canonical architecture/rules, route layout contracts, and scroll-container requirements.
- `.cursorrules`: same normative guidance mirrored for Cursor workflows.
- `CONTRIBUTING.md`: architecture boundaries (`crosshook-core` vs thin IPC) and workflow/testing conventions.
- `docs/plans/game-details-modal/feature-spec.md`: feature-level architecture and file-level implementation map.
- `docs/plans/game-details-modal/research-technical.md`: technical constraints and integration details for this feature.
- `docs/plans/game-details-modal/research-recommendations.md`: implementation strategy and incremental rollout guidance.

## API Docs

- `src/crosshook-native/src-tauri/src/lib.rs`: authoritative command registration list.
- `src/crosshook-native/src-tauri/src/commands/profile.rs`: profile summary/load command contracts.
- `src/crosshook-native/src-tauri/src/commands/game_metadata.rs`: game metadata and art command contracts.
- `src/crosshook-native/src-tauri/src/commands/protondb.rs`: ProtonDB command surface.
- `docs/plans/game-details-modal/research-external.md`: external service mapping and what is already integrated.

## Development Guides

- `README.md`: dev/build entry points and high-level feature docs links.
- `docs/internal-docs/local-build-publish.md`: local build/dev scripts and packaging guidance.
- `docs/research/additional-features/implementation-guide.md`: persistence and schema-aware planning guidance.

## README Files

- `README.md`: project overview, setup entry points, and docs index links.

## Must-Read Documents

- `docs/plans/game-details-modal/feature-spec.md`: You _must_ read this when implementing modal scope and acceptance behavior.
- `docs/plans/game-details-modal/research-recommendations.md`: You _must_ read this when choosing modal wiring and rollout sequence.
- `AGENTS.md`: You _must_ read this when touching frontend layout/scroll behavior or Tauri command surfaces.
- `docs/plans/game-details-modal/research-technical.md`: You _must_ read this for concrete integration points and constraints.
- `docs/plans/game-details-modal/research-ux.md`: You _must_ read this for accessibility, interaction, and device UX expectations.

## Documentation Gaps

- `docs/plans/game-details-modal/` lacked `shared.md` and `parallel-plan.md` before this workflow run.
- There is no single generated IPC reference document; command contracts are distributed across Rust command files.
- Modal implementation patterns are spread across components and CSS rather than a dedicated modal implementation guide.
