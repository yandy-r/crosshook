# Documentation Research: proton-optimizations

## Overview

The implementation context for `proton-optimizations` is split between the just-created feature planning docs, the repo’s user-facing launch guides, and a small set of inline comments in the Proton launch code that define non-obvious invariants. The most important local sources are the new `feature-spec.md` and its supporting research files, followed by the existing launch workflow docs and repo-level instructions in `AGENTS.md`.

## Relevant Documentation Files

- /AGENTS.md: Repo-level architecture summary, file map, build/test commands, launch-method definitions, and workflow rules that directly constrain implementation planning.
- /README.md: High-level product positioning, launch-mode descriptions, install-flow summary, and current user-facing copy that will need updating once launch optimizations ship.
- /docs/getting-started/quickstart.md: Current profile creation, `proton_run`, save-flow, and export guidance; important because launch optimizations will extend the saved profile model.
- /docs/features/steam-proton-trainer-launch.doc.md: Most detailed local explanation of `steam_applaunch`, `proton_run`, auto-populate, launcher export, and current launch flow semantics.
- /docs/plans/proton-optimizations/feature-spec.md: Primary source of truth for the feature contract, required scope (`proton_run`), option catalog shape, autosave boundary, affected files, and phased implementation plan.
- /docs/plans/proton-optimizations/research-technical.md: Implementation-oriented architecture notes for typed persistence, section-only autosave, Rust launch resolution, and file-level impact.
- /docs/plans/proton-optimizations/research-business.md: Product rules for profile-scoped persistence, applicability, autosave boundaries, and install-review behavior.
- /docs/plans/proton-optimizations/research-ux.md: UI placement, grouping, tooltip, accessibility, and launch-preview requirements for the new panel.
- /docs/plans/proton-optimizations/research-recommendations.md: Prioritized v1 versus advanced option set, phased scope, and risk mitigation guidance.
- /docs/plans/proton-optimizations/research-external.md: Condensed upstream/community source map for Proton vars, MangoHud, GameMode, and advanced community-documented flags.
- /.github/pull_request_template.md: Build verification checklist and conditional review prompts relevant because this feature spans launch logic, profile persistence, and UI.
- /tasks/lessons.md: Existing repo-specific pitfalls, including controller/input handling, Proton path assumptions, and launch-environment gotchas that can affect this feature.

## Code Comments and Inline Documentation

- /src/crosshook-native/crates/crosshook-core/src/launch/env.rs: Documents that `WINE_ENV_VARS_TO_CLEAR` must stay in sync with the shell helper unset lists; critical if launch optimizations add or preserve Proton/WINE variables.
- /src/crosshook-native/runtime-helpers/steam-launch-helper.sh: Inline `Keep in sync` comment mirrors the Rust env-clearing contract and shows how Steam-mode helper scripts currently sanitize Proton/WINE state.
- /src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh: Repeats the same sync invariant for detached trainer launching, which matters if the feature later expands beyond `proton_run`.
- /src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: Function names and comments around `env_clear()`, `apply_host_environment()`, and `apply_runtime_proton_environment()` explain the existing direct-Proton execution model this feature will extend.
- /src/crosshook-native/src/types/install.ts: `Keep in sync` note on frontend/backend validation messaging is a good reminder of the project’s expectation that cross-boundary contracts stay explicit and aligned.

## External Documentation References

- <https://github.com/ValveSoftware/Proton>: Authoritative reference for Proton runtime environment variables and launch-time behavior.
- <https://github.com/flightlessmango/MangoHud>: Canonical wrapper usage and caveats for overlay integration.
- <https://github.com/FeralInteractive/gamemode>: Canonical `gamemoderun` wrapper behavior and host-side performance profile semantics.
- <https://wiki.cachyos.org/configuration/gaming/>: Practical reference for the user-requested Linux gaming options, wrapper ordering, and advanced community-documented flags.
- <https://github.com/GloriousEggroll/proton-ge-custom>: Useful when evaluating community/fork-specific options such as `SteamDeck=1`, HDR, Wayland, or upscaler upgrade flags that are not Valve-official.

## Must-Read Documentation

- /docs/plans/proton-optimizations/feature-spec.md: Required before implementing the feature contract, scope, task phases, and option model.
- /docs/plans/proton-optimizations/research-technical.md: Required before changing profile types, Tauri commands, or Rust launch resolution.
- /docs/plans/proton-optimizations/research-ux.md: Required before building the panel layout, status feedback, or per-option info tooltips.
- /docs/plans/proton-optimizations/research-business.md: Required before deciding autosave behavior, runner applicability, or install-review handling.
- /docs/features/steam-proton-trainer-launch.doc.md: Required before changing launch behavior because it explains the current `proton_run` and Steam/Proton workflow semantics.
- /docs/getting-started/quickstart.md: Required before updating user-visible workflow copy or profile/save guidance.
- /README.md: Required before updating top-level feature descriptions or launch-mode language.
- /AGENTS.md: Required before implementation planning because it defines the repo architecture, build/test commands, and project workflow constraints.
