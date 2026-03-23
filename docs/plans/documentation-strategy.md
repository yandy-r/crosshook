# Documentation Strategy

## Audit Summary

- `README.md` is already large and currently mixes release info, build info, platform quick starts, troubleshooting, and customization details.
- The new Steam / Proton feature exists in `docs/features/steam-proton-trainer-launch.doc.md`, but it is too narrow to act as the main user guide.
- There is no clear getting-started guide with a navigation-first table of contents for supported environments.
- The repo has no dedicated docs index or quickstart area for end users.

## Identified Gaps

1. A concise root README that points users to the right detailed guides.
2. A detailed quickstart guide covering Linux / Steam Deck, macOS / Whisky, and external launcher export.
3. A clearer Steam / Proton feature guide with workflows, limits, and troubleshooting guidance.
4. Better cross-linking between README and detailed docs.

## Prioritized Workstreams

1. README Updates
   - shorten the root README
   - add a table of contents
   - link to detailed docs instead of embedding every walkthrough inline

2. Feature Docs
   - expand the Steam / Proton guide
   - create a full quickstart guide for supported environments
   - link the quickstart and feature guide together

## Scope

- Mode: `update`
- Target files:
  - `README.md`
  - `docs/features/steam-proton-trainer-launch.doc.md`
  - `docs/getting-started/quickstart.md`

## Deferred

- Screenshots and annotated UI callouts
- Architecture docs
- API docs
- Code comment pass
