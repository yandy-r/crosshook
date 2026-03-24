# Documentation Research: install-game

## Architecture Docs

- `/docs/features/steam-proton-trainer-launch.doc.md`: Current launch-method behavior, Proton workflow, discovery paths, launcher export, and console/log expectations.

## API Docs

- `/docs/features/steam-proton-trainer-launch.doc.md`: Closest thing to backend API documentation because it explains current launch/runtime contracts and required profile fields.
- `/docs/getting-started/quickstart.md`: User-facing input requirements for `steam_applaunch` and `proton_run`, including current profile TOML shape.

## Development Guides

- `/AGENTS.md`: Project rules for planning, task tracking, code organization, error handling, and testing.
- `/CLAUDE.md`: Mirrored project conventions and workflow context for the native rewrite.
- `/tasks/lessons.md`: Project-specific mistakes and guardrails, including the Proton dropdown/editable-path lesson that directly applies to install-game.

## README Files

- `/README.md`: High-level app purpose, launch modes, storage locations, and current feature set.

## Must-Read Documents

- `/docs/plans/install-game/feature-spec.md`: You _must_ read this when implementing install-game because it locks the product decisions and final planning assumptions.
- `/docs/features/steam-proton-trainer-launch.doc.md`: You _must_ read this when working on Proton runtime behavior, log expectations, and launch-mode semantics.
- `/docs/getting-started/quickstart.md`: You _must_ read this when adjusting user-visible setup flows, profile fields, and storage paths.
- `/AGENTS.md`: You _must_ read this when planning tasks, organizing files, and deciding test scope.
- `/tasks/lessons.md`: You _must_ read this when implementing the Proton selector and typed-input interactions so the new flow does not regress existing UX decisions.

## Documentation Gaps

- There is no existing documentation for a non-Steam install workflow inside CrossHook; all current docs assume the game is already installed and the user is creating or launching a profile afterward.
- There is no documented contract yet for a future install command surface, install result payload, or executable auto-discovery behavior.
- There is no existing doc that explains where CrossHook should store large standalone Proton prefixes; current user-facing docs only mention profile/settings storage.
- The launch docs cover log streaming for launch flows, but not how a long-running installer workflow should surface progress and final review states.
