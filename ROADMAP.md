# CrossHook Roadmap

Living priority map for what to build next. Updated from `main` commit history,
Forgejo/GitHub releases, open issues on Forgejo (`origin`), and recent PR state
(**2026-06-17**).

**Issue tracker:** Active work is tracked on
[Forgejo](https://git.home.rfamily.dev/yandy/crosshook/issues). Migrated issues
retain the original GitHub number in the body (`Migrated from GitHub issue #…`).
This roadmap uses **Forgejo numbers** for open work; historical shipped PRs below
still link to GitHub for archaeology.

**How to use this file**

- Treat **Do next** as the current sprint unless blocked.
- Link implementation PRs with `Closes #…` or `Part of #…` per
  [`.forgejo/pull_request_template.md`](.forgejo/pull_request_template.md).
- When a phase ships, check off or close the matching issue and update this file
  in the same PR or a follow-up docs commit.
- Canonical implementation detail lives in PRDs under `docs/prps/prds/`; this
  file is the executive view.

---

## Do next

Prioritized actions for the current cycle. Work top-to-bottom; skip only when
blocked.

| #   | Action                                                                                                                                                                                                                                                                                                                                     | Why now                                                                                                                                                         |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Cut the next release from `main`** — run `./scripts/prepare-release.sh`, validate changelog sections, smoke native + Flatpak build, tag to Forgejo `origin` and GitHub `github`. Ships launch command-arguments UI ([#504](https://github.com/yandy-r/crosshook/pull/504)), skip-launcher fix, and dual Forgejo/GitHub release publish.  | [v0.5.0](https://github.com/yandy-r/crosshook/releases/tag/v0.5.0) landed Flatpak-only distribution; `main` has unreleased launch UX and CI work.               |
| 2   | **Refresh [#3](https://git.home.rfamily.dev/yandy/crosshook/issues/3)** — reconcile the deep-research tracker (GitHub #78) with the 16-issue Forgejo board: check off Flatpak (#69 → shipped v0.5.0), drop macOS (#76, not on board), update schema inventory (v24), and triage remaining P2/P3 items (#63 trainer hash, #73 collections). | Only `priority:high` / `status:in-progress` tracker; body still cites schema v13 and open Flatpak/Flatpak-adjacent checklist rows.                              |
| 3   | **Start [#4](https://git.home.rfamily.dev/yandy/crosshook/issues/4)** — config history semantic diff, retention UI, and UX polish (GitHub #123). Write a focused PRP plan with storage boundaries before coding.                                                                                                                           | **Promoted next feature** — reliability / explainability over breadth; builds on shipped config-revisions MVP (#46); aligns with the strategic principle in #3. |
| 4   | **Groom Flatpak submission track** — review [#6](https://git.home.rfamily.dev/yandy/crosshook/issues/6) / [#5](https://git.home.rfamily.dev/yandy/crosshook/issues/5) (GitHub #210 / #206) against Phase 4 isolation ([#412](https://github.com/yandy-r/crosshook/pull/412)).                                                              | Per-app isolation shipped; Flathub is the next distribution milestone after the release train clears.                                                           |

**Strategic principle** (from [#3](https://git.home.rfamily.dev/yandy/crosshook/issues/3)): invest in making the
trainer-on-Linux workflow **reliable, diagnosable, and shareable** — depth over
breadth. Hero Detail consolidation, the trainer tab editor, and GAMEID auto-resolve
all align; the next feature should preserve that direction rather than broaden
launcher scope prematurely.

---

## Snapshot

| Area                          | Status                                                                                                                                                                                                               |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Latest release**            | [v0.5.0](https://github.com/yandy-r/crosshook/releases/tag/v0.5.0) (2026-06-12) — Flatpak-only distribution, effective Steam path for Proton, Hero Detail toolbar declutter                                          |
| **On `main`, unreleased**     | Launch command-arguments sub-tab ([#504](https://github.com/yandy-r/crosshook/pull/504)); skip-launcher argv fix; Forgejo collaboration CI + dual Forgejo/GitHub release publish                                     |
| **Unified Desktop Redesign**  | **Shipped** (v0.3.0) — responsive shell, Hero Detail mode, command palette, context rail, status bar, route reworks                                                                                                  |
| **Hero Detail Consolidation** | **Shipped** (v0.3.0) — profile/launch/hook editing in Hero Detail; legacy `/profiles` and `/launch` routes removed; trainer tab editor completed on `main` ([#479](https://github.com/yandy-r/crosshook/issues/479)) |
| **Open issues (Forgejo)**     | 16 on `origin` — migrated from GitHub 2026-06-17 (see [Open issue inventory](#open-issue-inventory))                                                                                                                 |
| **Open PRs**                  | 0                                                                                                                                                                                                                    |

---

## Recently landed on `main` (unreleased)

Work merged after [v0.5.0](https://github.com/yandy-r/crosshook/releases/tag/v0.5.0); target the next release tag.

| Commit / PR                                                                                                                | Issue | Summary                                                      |
| -------------------------------------------------------------------------------------------------------------------------- | ----- | ------------------------------------------------------------ |
| [`44361884`](https://github.com/yandy-r/crosshook/commit/44361884) / [#504](https://github.com/yandy-r/crosshook/pull/504) | -     | Launch command-arguments sub-tab and catalog                 |
| [`15c394a9`](https://github.com/yandy-r/crosshook/commit/15c394a9)                                                         | -     | Fix skip-launcher argv token; add `nolauncher` catalog entry |
| [`c4eb0fdf`](https://github.com/yandy-r/crosshook/commit/c4eb0fdf)                                                         | -     | Move collaboration CI to Forgejo (`.forgejo/workflows/`)     |
| [`ec5e02ba`](https://github.com/yandy-r/crosshook/commit/ec5e02ba)                                                         | -     | Publish releases to Forgejo and GitHub                       |

### Shipped in v0.5.0 (2026-06-12)

| Commit / PR                                                                                                               | Issue                                                 | Summary                                                        |
| ------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- | -------------------------------------------------------------- |
| [`940182e`](https://github.com/yandy-r/crosshook/commit/940182e1)                                                         | -                                                     | Use effective Steam path when building Proton profile requests |
| [`3fff5f0`](https://github.com/yandy-r/crosshook/commit/3fff5f0c) / [#503](https://github.com/yandy-r/crosshook/pull/503) | [#69](https://github.com/yandy-r/crosshook/issues/69) | Remove AppImage distribution and make Flatpak the release path |
| [`0d44239`](https://github.com/yandy-r/crosshook/commit/0d442396)                                                         | -                                                     | Declutter Hero Detail profile and launch action toolbars       |

---

## Recently shipped releases

### v0.4.1 (2026-06-08)

Release focused on a Steam launch regression after v0.4.0.

| Commit / PR                                                       | Summary                                       |
| ----------------------------------------------------------------- | --------------------------------------------- |
| [`ba9de12`](https://github.com/yandy-r/crosshook/commit/ba9de128) | `chore(release): prepare v0.4.1`              |
| [`2d662a6`](https://github.com/yandy-r/crosshook/commit/2d662a64) | `fix(launch): allow Steam hook optimizations` |

### v0.4.0 (2026-06-07)

Release captured the trainer tab editor, umu GAMEID lookup resolver, and roadmap
updates after v0.3.1.

| Commit / PR                                                                                                               | Summary                                                                             |
| ------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| [`56468b1`](https://github.com/yandy-r/crosshook/commit/56468b14)                                                         | `chore(release): prepare v0.4.0`                                                    |
| [`6474d50`](https://github.com/yandy-r/crosshook/commit/6474d50d) / [#502](https://github.com/yandy-r/crosshook/pull/502) | Auto-resolve GAMEID via umu-database HTTP lookups with SQLite cache (schema v24)    |
| [`985e5bf`](https://github.com/yandy-r/crosshook/commit/985e5bf9) / [#501](https://github.com/yandy-r/crosshook/pull/501) | Hero Detail Trainer tab — loaded hooks editor, injection config, injection log tail |
| [`b40069c`](https://github.com/yandy-r/crosshook/commit/b40069c4)                                                         | `docs(roadmap): move Do next to top and refresh priorities`                         |

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
and are included in `v0.3.0` / `v0.3.1` or on `main` awaiting the next tag.

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
| +     | [#479](https://github.com/yandy-r/crosshook/issues/479) / [#501](https://github.com/yandy-r/crosshook/pull/501)                                                          | Trainer tab editor — loaded hooks, injection config, log tail                   |
| Bug   | [#484](https://github.com/yandy-r/crosshook/issues/484) / [#498](https://github.com/yandy-r/crosshook/pull/498)                                                          | Fixed flaky profile auto-load behavior surfaced during route-removal cleanup    |

**Current product state:** Library / Hero Detail is the single per-game
workspace. Standalone `/profiles` and `/launch` routes and pages are removed.
Trainer tab editing is complete on `main`.

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
| [#503](https://github.com/yandy-r/crosshook/pull/503)                                                                                                               | Flatpak-only release distribution; AppImage release surface removed       |
| [#502](https://github.com/yandy-r/crosshook/pull/502)                                                                                                               | umu GAMEID lookup resolver with SQLite cache (schema v24)                 |

---

## P1 — Next product slices

| Issue                                                                 | Summary                                                   | Notes                                                                             |
| --------------------------------------------------------------------- | --------------------------------------------------------- | --------------------------------------------------------------------------------- |
| [#4](https://git.home.rfamily.dev/yandy/crosshook/issues/4) (GH #123) | Config history semantic diff, retention UI, and UX polish | **Promoted next feature** — reliability / explainability over another UI overhaul |
| [#2](https://git.home.rfamily.dev/yandy/crosshook/issues/2) (GH #71)  | Lutris profile import                                     | Migration aid; good when onboarding friction is the priority                      |

---

## P2 — Platform & distribution

Strategic work, not blocking the current release train.

| Issue                                                                 | Summary                                        | Notes                                                                                              |
| --------------------------------------------------------------------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| [#6](https://git.home.rfamily.dev/yandy/crosshook/issues/6) (GH #210) | Flatpak Phase 4 — Flathub submission           | Depends on per-app isolation ([ADR-0004](docs/architecture/adr-0004-flatpak-per-app-isolation.md)) |
| [#5](https://git.home.rfamily.dev/yandy/crosshook/issues/5) (GH #206) | Submit CrossHook to Flathub                    | Child of Flatpak track                                                                             |
| [#7](https://git.home.rfamily.dev/yandy/crosshook/issues/7) (GH #249) | Custom Proton fork "tinkerers" UX              | UMU / advanced-user follow-up; `deferred`                                                          |
| [#8](https://git.home.rfamily.dev/yandy/crosshook/issues/8) (GH #250) | Non-x86_64 architectures (umu container scope) | UMU / compatibility follow-up; `deferred`                                                          |

---

## P3 — Deferred UI / architecture ideas

These remain intentionally out of the active board until there is a clear user
pull or a new PRD.

| Issue                                                                   | Topic                                                 |
| ----------------------------------------------------------------------- | ----------------------------------------------------- |
| [#9](https://git.home.rfamily.dev/yandy/crosshook/issues/9) (GH #426)   | Alternate themes / theme switcher                     |
| [#10](https://git.home.rfamily.dev/yandy/crosshook/issues/10) (GH #427) | Persisted layout prefs: inspector width, cmdk recency |
| [#11](https://git.home.rfamily.dev/yandy/crosshook/issues/11) (GH #428) | URL routing / deep links                              |
| [#12](https://git.home.rfamily.dev/yandy/crosshook/issues/12) (GH #429) | New icon library                                      |
| [#13](https://git.home.rfamily.dev/yandy/crosshook/issues/13) (GH #430) | Replace `react-resizable-panels`                      |
| [#14](https://git.home.rfamily.dev/yandy/crosshook/issues/14) (GH #431) | Backend / Community marketplace scope                 |
| [#15](https://git.home.rfamily.dev/yandy/crosshook/issues/15) (GH #432) | n-zone gamepad-nav refactor (4+ zones)                |
| [#16](https://git.home.rfamily.dev/yandy/crosshook/issues/16) (GH #433) | Hero Detail Media tab                                 |

---

## Maintenance & blocked

| Issue                                                                | Summary                                                     | Status                                                                                                                                    |
| -------------------------------------------------------------------- | ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| [#1](https://git.home.rfamily.dev/yandy/crosshook/issues/1) (GH #26) | Track upstream fix for vulnerable glib in Tauri Linux stack | `status:blocked` — still on `glib 0.18.5` after Tauri 2.11.x; see [upstream tracking](#upstream-tracking-for-issue-1-glib-advisory) below |

### Upstream tracking for issue 1 (glib advisory)

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

Re-evaluate [#1](https://git.home.rfamily.dev/yandy/crosshook/issues/1) when a Tauri
release ships with gtk4-rs / webkit6 and resolves `glib >= 0.20.0`. Latest local
check: 2026-06-07 (see issue comment).

---

## Open issue inventory

All 16 open issues on Forgejo (`origin`), migrated from GitHub 2026-06-17.
Flatpak packaging (GitHub #69) shipped in v0.5.0 and is no longer open.

### Active tracker / hygiene (1)

#3 — deep-research roadmap tracker (`priority:high`, `status:in-progress`; GitHub #78)

### P1 product slices (2)

#4 — config history enhancements (GitHub #123); #2 — Lutris profile import (GitHub #71)

### Platform / distribution (3)

#1 — glib upstream advisory (`status:blocked`, GitHub #26); #5 — Flathub submit (GitHub #206); #6 — Flatpak Phase 4 track (GitHub #210)

### UMU deferred (2)

#7 — custom Proton fork UX (GitHub #249); #8 — non-x86_64 umu scope (GitHub #250)

### Unified Desktop deferred (8)

#9–#16 — themes, layout prefs, URL routing, icons, resizable panels, community marketplace, gamepad-nav, Media tab (GitHub #426–#433)

---

## Key documents

| Document                                                                                                                             | Purpose                                                                        |
| ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| [`docs/prps/prds/unified-desktop-redesign.prd.md`](docs/prps/prds/unified-desktop-redesign.prd.md)                                   | Shipped shell redesign — phase table + decisions                               |
| [`docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`](docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md) | Shipped consolidation PRD — phases and route-removal rationale                 |
| [`docs/internal-docs/design-tokens.md`](docs/internal-docs/design-tokens.md)                                                         | Token rules post Unified Desktop / Hero Detail polish                          |
| [`docs/research/additional-features/deep-research-report.md`](docs/research/additional-features/deep-research-report.md)             | Source for [#3](https://git.home.rfamily.dev/yandy/crosshook/issues/3) backlog |
| [`CHANGELOG.md`](CHANGELOG.md)                                                                                                       | Release history (git-cliff)                                                    |
| [`AGENTS.md`](AGENTS.md)                                                                                                             | Agent/repo policy                                                              |
