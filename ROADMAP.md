# CrossHook Roadmap

Living priority map for what to build next. Updated from `main` commit history,
GitHub releases, open issues, and recent PR state (**2026-06-05**).

**How to use this file**

- Treat **P0** as the current sprint unless blocked.
- Link implementation PRs with `Closes #...` or `Part of #...` per
  [`.github/pull_request_template.md`](.github/pull_request_template.md).
- When a phase ships, check off or close the matching issue and update this file
  in the same PR or a follow-up docs commit.
- Canonical implementation detail lives in PRDs under `docs/prps/prds/`; this
  file is the executive view.

---

## Snapshot

| Area                          | Status                                                                                                                                                                                                                              |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Latest release**            | [v0.3.1](https://github.com/yandy-r/crosshook/releases/tag/v0.3.1) (2026-06-05) — includes Unified Desktop, Hero Detail Consolidation, hook runtime execution, production mock-sentinel fix, and Tauri / TypeScript dependency bump |
| **Unified Desktop Redesign**  | **Shipped** — responsive shell, Hero Detail mode, command palette, context rail, status bar, route reworks, responsive smoke, polish, and design-token docs                                                                         |
| **Hero Detail Consolidation** | **Shipped** — profile editing, launch tab, hook editor, overview deep-links, route removal, legacy page deletion, smoke rewrite, and cleanup all landed                                                                             |
| **Open issues**               | 18 (see [Open issue inventory](#open-issue-inventory))                                                                                                                                                                              |
| **Open PRs**                  | 0                                                                                                                                                                                                                                   |

---

## Recently shipped

### v0.3.1 (2026-06-05)

Release focused on production hardening and dependency freshness after the large
Hero Detail / Unified Desktop release batch.

| Commit / PR                                                                                                               | Summary                                                     |
| ------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| [`25024c8`](https://github.com/yandy-r/crosshook/commit/25024c80)                                                         | `chore(release): prepare v0.3.1`                            |
| [`3f56204`](https://github.com/yandy-r/crosshook/commit/3f56204d) / [#500](https://github.com/yandy-r/crosshook/pull/500) | Upgrade native dependencies for Tauri 2.11 and TypeScript 6 |

### v0.3.0 (2026-06-05)

Release captured the completed Unified Desktop and Hero Detail Consolidation
work, plus the launch-hook runtime and production-bundle fix.

| Commit / PR                                                                                                               | Summary                                                   |
| ------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| [`6a41595`](https://github.com/yandy-r/crosshook/commit/6a41595a)                                                         | `fix(ui): strip dev-mock sentinel from production bundle` |
| [`094bbd2`](https://github.com/yandy-r/crosshook/commit/094bbd2c)                                                         | `chore(release): prepare v0.3.0`                          |
| [`1385b2a`](https://github.com/yandy-r/crosshook/commit/1385b2aa) / [#499](https://github.com/yandy-r/crosshook/pull/499) | Execute profile pre/post launch hooks at runtime          |
| [`a2e48ac`](https://github.com/yandy-r/crosshook/commit/a2e48ac0) / [#498](https://github.com/yandy-r/crosshook/pull/498) | Stabilize profile auto-load after refresh                 |
| [`5cbbaae`](https://github.com/yandy-r/crosshook/commit/5cbbaaeb)                                                         | Unify shell column chrome and library inspector layout    |

---

## Recently completed PRs

These PRs landed after [v0.2.11](https://github.com/yandy-r/crosshook/releases/tag/v0.2.11)
and are now included in `v0.3.0` / `v0.3.1`.

### Hero Detail Consolidation

| Phase | Issue / PR                                                                                                                                                               | Delivered                                                                       |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| 1     | [#466](https://github.com/yandy-r/crosshook/issues/466) / [#480](https://github.com/yandy-r/crosshook/pull/480)                                                          | Extended Hero Detail panel contract                                             |
| 2     | [#467](https://github.com/yandy-r/crosshook/issues/467) / [#481](https://github.com/yandy-r/crosshook/pull/481)                                                          | Sidebar cleanup, Favorites filter, Currently Playing filter                     |
| 3     | [#468](https://github.com/yandy-r/crosshook/issues/468) / [#483](https://github.com/yandy-r/crosshook/pull/483)                                                          | `LaunchHook` schema and breadcrumb navigation                                   |
| 4     | [#469](https://github.com/yandy-r/crosshook/issues/469) / [#485](https://github.com/yandy-r/crosshook/pull/485)                                                          | Hero Detail Profiles tab editor                                                 |
| 5     | [#470](https://github.com/yandy-r/crosshook/issues/470) / [#488](https://github.com/yandy-r/crosshook/pull/488)                                                          | Hero Detail Launch tab and highlighted command block                            |
| 5b    | [#486](https://github.com/yandy-r/crosshook/issues/486) / [#489](https://github.com/yandy-r/crosshook/pull/489)                                                          | Launch/profile parity before route removal                                      |
| 5c    | [#487](https://github.com/yandy-r/crosshook/issues/487) / [#490](https://github.com/yandy-r/crosshook/pull/490)                                                          | Create-profile wizard and creation flow                                         |
| 5d    | [#491](https://github.com/yandy-r/crosshook/issues/491) / [#492](https://github.com/yandy-r/crosshook/pull/492)                                                          | Library-level add-game entry point and empty-library creation path              |
| 6     | [#471](https://github.com/yandy-r/crosshook/issues/471) / [#493](https://github.com/yandy-r/crosshook/pull/493)                                                          | Live pre/post launch hook editor                                                |
| 7     | [#472](https://github.com/yandy-r/crosshook/issues/472) / [#494](https://github.com/yandy-r/crosshook/pull/494)                                                          | Overview panel deep-links into Hero Detail tabs                                 |
| 8-9   | [#473](https://github.com/yandy-r/crosshook/issues/473), [#474](https://github.com/yandy-r/crosshook/issues/474) / [#495](https://github.com/yandy-r/crosshook/pull/495) | Removed legacy `profiles` / `launch` routes and rewired navigation              |
| 10-11 | [#475](https://github.com/yandy-r/crosshook/issues/475), [#476](https://github.com/yandy-r/crosshook/issues/476) / [#496](https://github.com/yandy-r/crosshook/pull/496) | Deleted legacy pages and rewrote smoke coverage for Hero Detail flows           |
| 12    | [#477](https://github.com/yandy-r/crosshook/issues/477) / [#497](https://github.com/yandy-r/crosshook/pull/497)                                                          | Design-token docs, command-preview tokens, release copy, and dead-asset cleanup |
| +     | [#482](https://github.com/yandy-r/crosshook/issues/482) / [#499](https://github.com/yandy-r/crosshook/pull/499)                                                          | Runtime execution for the hook schema introduced during consolidation           |
| Bug   | [#484](https://github.com/yandy-r/crosshook/issues/484) / [#498](https://github.com/yandy-r/crosshook/pull/498)                                                          | Fixed flaky profile auto-load behavior surfaced during route-removal cleanup    |

**Current product state:** Library / Hero Detail is the single per-game
workspace. Standalone `/profiles` and `/launch` routes and pages are removed.
Trainer tab full editing remains deferred under [#479](https://github.com/yandy-r/crosshook/issues/479).

### Unified Desktop Redesign

| Phase | Issue / PR                                                                                                      | Delivered                                                   |
| ----- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| 1     | [#440](https://github.com/yandy-r/crosshook/issues/440) / [#453](https://github.com/yandy-r/crosshook/pull/453) | `useBreakpoint`, layout unlock, `AppShell` extraction       |
| 2     | [#441](https://github.com/yandy-r/crosshook/issues/441) / [#454](https://github.com/yandy-r/crosshook/pull/454) | Steel-blue palette and legacy-palette sentinel              |
| 3     | [#442](https://github.com/yandy-r/crosshook/issues/442) / [#455](https://github.com/yandy-r/crosshook/pull/455) | Sidebar variants and formalized Collections section         |
| 4     | [#443](https://github.com/yandy-r/crosshook/issues/443) / [#457](https://github.com/yandy-r/crosshook/pull/457) | Library cards and inspector rail                            |
| 5     | [#444](https://github.com/yandy-r/crosshook/issues/444) / [#458](https://github.com/yandy-r/crosshook/pull/458) | In-shell Hero Detail mode                                   |
| 6     | [#445](https://github.com/yandy-r/crosshook/issues/445) / [#459](https://github.com/yandy-r/crosshook/pull/459) | Global command palette                                      |
| 7     | [#446](https://github.com/yandy-r/crosshook/issues/446) / [#460](https://github.com/yandy-r/crosshook/pull/460) | Ultrawide context rail pane                                 |
| 8     | [#447](https://github.com/yandy-r/crosshook/issues/447) / [#461](https://github.com/yandy-r/crosshook/pull/461) | Responsive console status bar                               |
| 9     | [#448](https://github.com/yandy-r/crosshook/issues/448) / [#462](https://github.com/yandy-r/crosshook/pull/462) | Dashboard route rework                                      |
| 10    | [#449](https://github.com/yandy-r/crosshook/issues/449) / [#463](https://github.com/yandy-r/crosshook/pull/463) | Install, Settings, Community, and Discover route rework     |
| 11    | [#450](https://github.com/yandy-r/crosshook/issues/450) / [#464](https://github.com/yandy-r/crosshook/pull/464) | Profiles and Launch route rework before later consolidation |
| 12    | [#451](https://github.com/yandy-r/crosshook/issues/451) / [#424](https://github.com/yandy-r/crosshook/pull/424) | Responsive Playwright smoke and route sweep expansion       |
| 13    | [#452](https://github.com/yandy-r/crosshook/issues/452) / [#465](https://github.com/yandy-r/crosshook/pull/465) | Polish, accessibility, and design-token docs                |

### Platform, CI, and maintainability since v0.2.11

| PR                                                                                                                                                                  | Delivered                                                                 |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| [#412](https://github.com/yandy-r/crosshook/pull/412)                                                                                                               | Flatpak per-app isolation with first-run host migration                   |
| [#396](https://github.com/yandy-r/crosshook/pull/396)                                                                                                               | Trainer watchdog cleanup parity with game launches                        |
| [#394](https://github.com/yandy-r/crosshook/pull/394)                                                                                                               | Conventional Commit PR-title autofix normalization                        |
| [#351](https://github.com/yandy-r/crosshook/pull/351), [#353](https://github.com/yandy-r/crosshook/pull/353), [#354](https://github.com/yandy-r/crosshook/pull/354) | Frontend, coverage, Playwright, and Tauri E2E test foundation             |
| [#382](https://github.com/yandy-r/crosshook/pull/382)-[#410](https://github.com/yandy-r/crosshook/pull/410)                                                         | Broad module-splitting/refactor pass across core, CLI, frontend, settings |

---

## P0 — Post-release hygiene

Keep this small. `v0.3.1` is current, and there are no open PRs.

| Item                                                                   | Action                                                                                                      | Why now                                                       |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| Refresh or close [#78](https://github.com/yandy-r/crosshook/issues/78) | Reconcile the deep-research tracker with the current issue board; many checklist entries are already closed | It is the only open high-priority tracker and now reads stale |
| Verify release notes for `v0.3.0` / `v0.3.1`                           | Confirm the public release notes reflect Hero Detail, hook runtime, production-bundle fix, and dep upgrade  | These releases shipped a large amount of user-visible work    |
| Triage [#479](https://github.com/yandy-r/crosshook/issues/479)         | Decide whether the Trainer tab editor is the next feature slice or should remain explicitly deferred        | It is the direct follow-up to Hero Detail Consolidation       |

---

## P1 — Next product slices

| Issue                                                   | Summary                                                                      | Suggested next step                                                                  |
| ------------------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| [#479](https://github.com/yandy-r/crosshook/issues/479) | Hero Detail Trainer tab editor upgrade: loaded hooks, injection config, logs | Best next user-facing continuation if Hero Detail should become fully editable       |
| [#123](https://github.com/yandy-r/crosshook/issues/123) | Config history semantic diff, retention UI, and UX polish                    | Good fit if the next goal is reliability / explainability over another large UI pass |
| [#233](https://github.com/yandy-r/crosshook/issues/233) | Auto-resolve GAMEID via `umu-database` HTTP lookups with SQLite cache        | Good fit if launch setup friction is the next priority                               |

---

## P2 — Platform & distribution

Strategic work, but not blocking the current release train.

| Issue                                                   | Summary                                        | Notes                                                                                              |
| ------------------------------------------------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| [#210](https://github.com/yandy-r/crosshook/issues/210) | Flatpak Phase 4 — Flathub submission           | Depends on per-app isolation ([ADR-0004](docs/architecture/adr-0004-flatpak-per-app-isolation.md)) |
| [#206](https://github.com/yandy-r/crosshook/issues/206) | Submit CrossHook to Flathub                    | Child of Flatpak track                                                                             |
| [#69](https://github.com/yandy-r/crosshook/issues/69)   | Flatpak as secondary packaging format          | Substantial groundwork landed in [#412](https://github.com/yandy-r/crosshook/pull/412)             |
| [#71](https://github.com/yandy-r/crosshook/issues/71)   | Lutris profile import                          | Migration aid                                                                                      |
| [#76](https://github.com/yandy-r/crosshook/issues/76)   | macOS port investigation (GPTK 2)              | Out of core Linux scope                                                                            |
| [#249](https://github.com/yandy-r/crosshook/issues/249) | Custom Proton fork "tinkerers" UX              | UMU / advanced-user follow-up                                                                      |
| [#250](https://github.com/yandy-r/crosshook/issues/250) | Non-x86_64 architectures (umu container scope) | UMU / compatibility follow-up                                                                      |

---

## P3 — Deferred UI / architecture ideas

These remain intentionally out of the active board until there is a clear user
pull or a new PRD.

| Issue                                                   | Topic                                                 |
| ------------------------------------------------------- | ----------------------------------------------------- |
| [#426](https://github.com/yandy-r/crosshook/issues/426) | Alternate themes / theme switcher                     |
| [#427](https://github.com/yandy-r/crosshook/issues/427) | Persisted layout prefs: inspector width, cmdk recency |
| [#428](https://github.com/yandy-r/crosshook/issues/428) | URL routing / deep links                              |
| [#429](https://github.com/yandy-r/crosshook/issues/429) | New icon library                                      |
| [#430](https://github.com/yandy-r/crosshook/issues/430) | Replace `react-resizable-panels`                      |
| [#431](https://github.com/yandy-r/crosshook/issues/431) | Backend / Community marketplace scope                 |
| [#432](https://github.com/yandy-r/crosshook/issues/432) | n-zone gamepad-nav refactor (4+ zones)                |
| [#433](https://github.com/yandy-r/crosshook/issues/433) | Hero Detail Media tab                                 |

---

## Maintenance & blocked

| Issue                                                 | Summary                                                     | Status                                                                                                                                     |
| ----------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| [#26](https://github.com/yandy-r/crosshook/issues/26) | Track upstream fix for vulnerable glib in Tauri Linux stack | `status:blocked` — still on `glib 0.18.5` after Tauri 2.11.x; see [upstream tracking](#upstream-tracking-for-issue-26-glib-advisory) below |

### Upstream tracking for issue 26 (glib advisory)

CrossHook cannot bump `glib` to `>= 0.20.0` while the Linux stack resolves
`gtk 0.18.2` → `glib ^0.18`. The patch landed in
[gtk-rs/gtk-rs-core#1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343)
(**RUSTSEC-2024-0429** / **GHSA-wrw7-89jp-8q8g**); the remaining work is a
Tauri ecosystem migration to gtk4-rs / WebKitGTK6.

**Primary upstream tracking**

| Upstream issue                                                                    | Repo              | Role                                   |
| --------------------------------------------------------------------------------- | ----------------- | -------------------------------------- |
| [tauri#12563](https://github.com/tauri-apps/tauri/issues/12563)                   | tauri             | Upgrade `tauri` to gtk4-rs             |
| [wry#1474](https://github.com/tauri-apps/wry/issues/1474)                         | wry               | Upgrade `wry` to gtk4-rs + webkit6     |
| [tauri#12564](https://github.com/tauri-apps/tauri/issues/12564)                   | tauri             | gtk-rs outreach for glib upgrade       |
| [javascriptcore-rs#84](https://github.com/tauri-apps/javascriptcore-rs/issues/84) | javascriptcore-rs | Security advisory on `glib` dependency |
| [muda#259](https://github.com/tauri-apps/muda/issues/259)                         | muda              | Upgrade to gtk-4                       |

**Related runtime stack issues:** [tauri#12561](https://github.com/tauri-apps/tauri/issues/12561),
[tauri#12562](https://github.com/tauri-apps/tauri/issues/12562)

**WIP community PRs (not merged):** [wry#1530](https://github.com/tauri-apps/wry/pull/1530),
[tao#1104](https://github.com/tauri-apps/tao/pull/1104),
[muda#341](https://github.com/tauri-apps/muda/pull/341)

Re-evaluate [#26](https://github.com/yandy-r/crosshook/issues/26) when a Tauri
release ships with gtk4-rs / webkit6 and resolves `glib >= 0.20.0`. Latest local
check: 2026-06-05 (see issue comment).

**Strategic principle** (from [#78](https://github.com/yandy-r/crosshook/issues/78)):
invest in making the trainer-on-Linux workflow **reliable, diagnosable, and
shareable** — depth over breadth. The shipped Hero Detail work aligns with this;
the next feature should preserve that direction rather than broaden launcher
scope prematurely.

---

## Open issue inventory

All 18 open issues grouped by theme (2026-06-05).

### Active tracker / hygiene (1)

#78

### Hero Detail follow-up (1)

#479

### Unified Desktop deferred (8)

#426, #427, #428, #429, #430, #431, #432, #433

### Platform / build (6)

#26, #69, #71, #76, #206, #210

### UMU deferred (2)

#249, #250

### Other features (2)

#123, #233

---

## Key documents

| Document                                                                                                                             | Purpose                                                                  |
| ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| [`docs/prps/prds/unified-desktop-redesign.prd.md`](docs/prps/prds/unified-desktop-redesign.prd.md)                                   | Shipped shell redesign — phase table + decisions                         |
| [`docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`](docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md) | Shipped consolidation PRD — phases and route-removal rationale           |
| [`docs/internal-docs/design-tokens.md`](docs/internal-docs/design-tokens.md)                                                         | Token rules post Unified Desktop / Hero Detail polish                    |
| [`docs/research/additional-features/deep-research-report.md`](docs/research/additional-features/deep-research-report.md)             | Source for [#78](https://github.com/yandy-r/crosshook/issues/78) backlog |
| [`CHANGELOG.md`](CHANGELOG.md)                                                                                                       | Release history (git-cliff)                                              |
| [`AGENTS.md`](AGENTS.md)                                                                                                             | Agent/repo policy                                                        |

---

## Suggested next actions

1. Refresh or close [#78](https://github.com/yandy-r/crosshook/issues/78) so
   the only high-priority open tracker reflects the post-`v0.3.1` state.
2. Decide whether [#479](https://github.com/yandy-r/crosshook/issues/479) is the
   next product slice. If yes, write a focused PRP plan for the Trainer tab
   editor with explicit storage/runtime boundaries.
3. If the next cycle should be smaller, pull [#123](https://github.com/yandy-r/crosshook/issues/123)
   or [#233](https://github.com/yandy-r/crosshook/issues/233) instead of
   starting another route-scale UI project.
