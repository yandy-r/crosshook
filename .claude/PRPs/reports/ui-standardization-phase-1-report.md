# Implementation Report: UI Standardization Phase 1 — Route Banner Baseline

## Summary

Implemented a shared `RouteBanner` plus centralized `routeMetadata` (nav labels, section eyebrow, title, summary, art). Wired the banner into all nine top-level routes, aligned `Sidebar` labels with `ROUTE_NAV_LABEL`, added `DiscoverArt` for the Discover route, and removed or demoted duplicate route-level intros (Profiles hero, Launch panel strip, Community/Compatibility/Discover/Settings in-card headers). Added route-banner styles in `theme.css`. Archived the plan to `.claude/PRPs/plans/completed/ui-standardization-phase-1.plan.md`.

## Assessment vs Reality

| Metric | Predicted (Plan) | Actual |
| --- | --- | --- |
| Complexity | Large | Large (touched all primary routes + shared layout) |
| Confidence | (not in plan) | High — `npm run build` and `cargo test -p crosshook-core` pass |
| Files Changed | 12–16 | 18 tracked files (+2 new components); plan file archived |

## Tasks Completed

| # | Task | Status | Notes |
| --- | --- | --- | --- |
| 1 | RouteBanner + route metadata | Complete | `RouteBanner.tsx`, `routeMetadata.ts`, `DiscoverArt` in `PageBanner.tsx` |
| 1.5 | Sidebar/banner parity | Complete | `Sidebar` uses `ROUTE_NAV_LABEL` from `routeMetadata` |
| 2 | Shared banner CSS | Complete | `.crosshook-route-banner*` in `theme.css` |
| 3 | Wire all top-level routes | Complete | Library, Profiles, Launch, Install, Community, Discover, Compatibility, Settings, Health |
| 4 | Remove duplicate intros | Complete | LaunchPanel, Profiles, Community/Compatibility/Discover panels, Settings in-card |
| 5 | Scroll / layout | Complete | No new scroll containers; banner is `flex-shrink: 0` |
| 6 | Copy / a11y | Complete | `h1` + `aria-labelledby` on banner; phase strip `aria-label` on Launch |

## Validation Results

| Level | Status | Notes |
| --- | --- | --- |
| Static Analysis | Pass | `cd src/crosshook-native && npm run build` |
| Unit Tests | N/A | No frontend test harness (per plan) |
| Rust tests | Pass | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` |
| Build | Pass | Vite production build |
| Integration | N/A | Manual route sweep recommended |
| Edge Cases | Not automated | Checklist in plan remains for human verification |

## Files Changed

| File | Action |
| --- | --- |
| `src/crosshook-native/src/components/layout/RouteBanner.tsx` | CREATED |
| `src/crosshook-native/src/components/layout/routeMetadata.ts` | CREATED |
| `src/crosshook-native/src/components/layout/PageBanner.tsx` | UPDATED — added `DiscoverArt`; later rewrote all 9 `*Art` exports as 64×64 icons (polish pass — see below) |
| `src/crosshook-native/src/components/layout/Sidebar.tsx` | UPDATED — `ROUTE_NAV_LABEL` |
| `src/crosshook-native/src/components/pages/*.tsx` | UPDATED — nine pages |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx` | UPDATED — added `RouteBanner`; later promoted `__content` wrapper to `.crosshook-card` (polish pass) |
| `src/crosshook-native/src/components/LaunchPanel.tsx` | UPDATED |
| `src/crosshook-native/src/components/SettingsPanel.tsx` | UPDATED |
| `src/crosshook-native/src/components/CommunityBrowser.tsx` | UPDATED |
| `src/crosshook-native/src/components/CompatibilityViewer.tsx` | UPDATED |
| `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx` | UPDATED |
| `src/crosshook-native/src/styles/theme.css` | UPDATED — initial route-banner styles; polish pass added radial halo background, simplified icon rules to mirror sidebar brand-art exactly, tightened `@media (max-height: 820px)` block, added `.crosshook-library-page__content` to the `route-card-scroll` panel-children selector |
| `src/crosshook-native/src/styles/library.css` | UPDATED (polish pass) — dropped the custom `padding`/`max-width` overrides on `.crosshook-library-page__content`; now relies on `.crosshook-card` for inner padding and panel boundary |
| `.claude/PRPs/plans/completed/ui-standardization-phase-1.plan.md` | ARCHIVED (moved from `plans/`) |

## Deviations from Plan

- **SettingsPanel**: Removed `PanelRouteDecor` / `SettingsArt` from the settings card in addition to demoting the heading stack, so route-level art is not duplicated beside `RouteBanner` (aligned with Community/Compatibility after their decor removal).

## Issues Encountered

Two visual regressions surfaced after the initial implementation and were resolved in a polish pass (see "Post-implementation polish" below). Both stemmed from the initial implementation reusing existing scaffolding without checking that it was correctly shaped for the new banner contract.

- **Banner icons nearly invisible.** First pass reused the existing `*Art` exports (`viewBox="0 0 200 120"` decorative backdrop strips) inside a 74×74 square icon slot, applied a `drop-shadow` filter that was clipped by the panel's `overflow: hidden`, and never reproduced the sidebar brand's actual glow technique (a radial halo painted on the panel background, not a filter on the icon).
- **Library content visually narrower than the banner with extra spacing.** LibraryPage was the only route that wrapped its scroll content in a custom `__content` div with `padding: var(--crosshook-page-padding)` (32px) plus a redundant `max-width` cap, instead of the standard `.crosshook-card` wrapper every other panel uses (`.crosshook-card` provides 28px padding via `--crosshook-card-padding`). The toolbar/grid sat inset 32px from the route edges with no visible card boundary connecting them to the banner above.

Pre-existing `Cargo.lock` change in the working tree was not introduced by this UI work; review before commit.

## Post-implementation polish

A follow-up pass corrected the two regressions above with surgical, scope-bounded fixes. No new files; no scope expansion; no plan amendments.

### Fix 1 — RouteBanner icon visibility (mirror sidebar brand exactly)

- **Root cause:** three independent bugs combined to make the icons look broken — wrong SVG aspect ratio (200×120 letterbox squashed into 74×74), drop-shadow filter clipped by `overflow: hidden`, no background halo to lift the icon visually. The sidebar brand achieves its glow via a `radial-gradient` on the panel background (`sidebar.css:25-27`), not via a filter on the icon.
- **Fix:**
  - `PageBanner.tsx` — rewrote all 9 `*Art` exports (`LibraryArt`, `ProfilesArt`, `LaunchArt`, `InstallArt`, `CommunityArt`, `DiscoverArt`, `CompatibilityArt`, `SettingsArt`, `HealthDashboardArt`) as true 64×64 square icons with sidebar-grade strokes (`strokeWidth: 1.5`) and the same opacity vocabulary as the sidebar crosshair (outer rings 0.3–0.4, inner accents 0.15–0.25, center fill ~0.25). Each route keeps a unique illustration.
  - `theme.css` `.crosshook-route-banner.crosshook-panel` — added the sidebar's exact two-layer background stack (`radial-gradient(ellipse 80% 100% at 95% 50%, rgba(0, 120, 212, 0.08), transparent 60%)` over `linear-gradient(180deg, rgba(18, 23, 42, 0.96), rgba(12, 17, 32, 0.96))`).
  - `theme.css` `.crosshook-route-banner__icon` — replaced the filter+oversize-icon block to mirror `.crosshook-sidebar__brand-art` exactly: `56×56`, `opacity: 0.6`, no `drop-shadow`, no dead `stroke-width` override; inner svg uses `width/height: 100%`. Compact `@media (max-height: 820px)` drops the icon to `44×44`.

### Fix 2 — LibraryPage width/spacing parity with banner

- **Root cause:** every other route panel renders as `<section className="crosshook-card crosshook-XXX-panel">`. The `.crosshook-card` class provides `28px` standard padding, a visible card boundary, and via the `theme.css` panel-children selector gets `min-height: 100%; align-content: start; box-sizing: border-box`. LibraryPage rolled its own `__content` wrapper with `padding: var(--crosshook-page-padding)` (32px), a redundant `max-width: var(--crosshook-content-width)` cap (the page-scroll-shell already caps at 1440px), and no card boundary.
- **Fix (three minimal edits):**
  - `LibraryPage.tsx:118` — added `crosshook-card` to the wrapper className: `<div className="crosshook-card crosshook-library-page__content">`.
  - `library.css:6-9` — dropped the `padding` and `max-width` overrides; the wrapper now inherits padding/border/background from `.crosshook-card`.
  - `theme.css:522-525` — added `.crosshook-route-card-scroll > .crosshook-library-page__content` to the panel-children selector list so it inherits `min-height: 100%; align-content: start; box-sizing: border-box` like every other route panel.
- **Result:** Library is now structurally and stylistically identical to every other page — `.crosshook-card` panel as the direct child of `route-card-scroll`, content sits at the same 28px inset as other routes, visible card boundary connects the panel to the banner above.

### Polish-pass validation

| Level | Status | Notes |
| --- | --- | --- |
| Static Analysis | Pass | `cd src/crosshook-native && npm run build` (clean; only the pre-existing chunk-size advisory) |
| Rust tests | Pass | `cargo test -p crosshook-core` — 718 unit + 3 integration tests pass |
| Manual visual | Pending | Sweep all 9 routes; confirm icons read clearly with the warm blue halo, Library content spans the same width as other pages |

## Tests Written

| Test File | Tests | Coverage |
| --- | --- | --- |
| N/A | N/A | No frontend unit tests configured |

## Next Steps

- [ ] Code review (`/code-review`)
- [ ] Open PR (`/prp-pr`) — e.g. `Closes #163` / `#160` as appropriate
- [ ] Manual: navigate all sidebar routes, confirm single banner, scroll behavior, Health padding
