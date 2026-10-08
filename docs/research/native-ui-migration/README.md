# CrossHook native UI migration: research and plan (Tauri/React/WebKitGTK → iced)

_Research date: 2026-10-07. Repository HEAD: `2e784de` on a clean `main`. Workspace version: `0.6.0`. Produced by a multi-agent research workflow; the Linear project “crosshook -- Native UI migration” tracks the plan._

_Facts re-checked against the tree for this revision:_

- The metadata schema is **v27** in code (`metadata/migrations/v26_v27.rs` adds `prefix_version_restore_journal`). CLAUDE.md still says v26.
- `packaging/flatpak/dev.crosshook.CrossHook.metainfo.xml` lists only `<release version="0.2.9">`.
- `.github/workflows/release.yml` builds one `.flatpak` bundle in `ghcr.io/flathub-infra/flatpak-github-actions:gnome-50`.
- `scripts/check-host-gateway.sh` `SCAN_DIRS` covers only `crosshook-core/src` and `src-tauri/src`. Its `PLATFORM_RS` still points at the removed `platform.rs`; the code is now `platform/`.
- `src-tauri/capabilities/default.json` allows `shell:allow-open` only for `https://www.protondb.com/**` and `https://www.steamgriddb.com/**`.

---

## 1. Executive summary

**Recommendation: replace the Tauri v2 + React/WebKitGTK frontend with a native Rust UI built on iced.** Pin a specific iced `0.15-dev` git revision and use the `tokio` executor. The migration runs as a strangler:

1. A UI-agnostic `crosshook-app` service crate, plus an optional pure `crosshook-view-model` crate, is extracted first.
2. The Tauri "Classic" UI and the new native UI then run side by side on top of it, under the same Flatpak app-id, until parity is proven.

**Judge tally: iced 2, gpui 1.** No override was applied.

| Lens             | Winner | Confidence |
| ---------------- | ------ | ---------- |
| Delivery risk    | iced   | 0.57       |
| Maintainability  | iced   | 0.66       |
| UX / performance | gpui   | 0.60       |

- **gpui is ahead** on widget coverage (GPUI Kit), focus and actions, virtualization, and upstream AccessKit.
- **iced is ahead on the risks a solo developer cannot control:**
  - **Supply chain:** iced has a stable MIT crates.io release. gpui is frozen at 0.2.2 and in practice ships through the single-maintainer `gpui-pre`.
  - **Upstream governance:** Zed has said it deprioritises non-Zed users.
  - **Executor fit:** `crosshook-core` needs a tokio reactor.
  - **Software-render fallback:** iced has tiny-skia; gpui has only llvmpipe.
- **iced's gaps can be built in app-owned code** on its public Widget/Operation APIs: focus traversal, a virtual list, modals/toasts/tabs, and accessibility (via the pop-os fork or #1849).
- **gpui's gaps cannot be closed without forking a large monorepo:** the release channel, renderer fallback, and Game Mode/XIM bugs.

**The decision is not final until M0 measures it.** M0 builds the same slice twice, once in iced and once in gpui. The slice is tested on a hardware matrix and includes AccessKit/Orca and IME probes. The outcome is recorded in **ADR-0002** against **numeric go/no-go thresholds** (§6). The ADR has three outcomes:

- **GO-iced**;
- **GO-gpui**;
- **NO-GO**: stay on tuned Tauri and keep extracting `crosshook-app`.

No toolkit-specific M1 work starts before the ADR is merged.

**Relief before the rewrite.** **v0.6.1 "Tuned Tauri + data-compat guards"** ships on **2026-11-11**. It contains:

- the I/O-bound sync commands moved off the GTK main thread;
- removal of blur, infinite animations and `useScrollEnhance`;
- a console cap;
- the SQLite schema forward guard (M1-10) and unknown-TOML-key preservation (M1-11);
- the `release/0.6` branch policy.

v0.6.1 gives users relief straight away, gives a fair baseline B, and protects anyone who later downgrades from an alpha.

**Release path:**

| Version       | Date       | What ships                                                                 |
| ------------- | ---------- | -------------------------------------------------------------------------- |
| 0.6.1         | 2026-11-11 | Tuned Tauri + data-compat guards                                           |
| 0.7.0-alpha.0 | 2026-12-02 | Internal, **untagged** spike build                                         |
| 0.7.0-alpha.1 | 2027-04-14 | Native preview launcher                                                    |
| 0.7.0-alpha.2 | 2027-06-16 | Native core loop                                                           |
| 0.7.0-alpha.3 | 2027-08-04 | Native configure & operate                                                 |
| 0.7.0-beta.1  | 2027-09-22 | Native UI feature complete                                                 |
| 0.7.0-beta.2  | 2027-10-20 | Native UI is the default on beta                                           |
| 0.7.0-rc.1    | 2027-11-17 | Release candidate                                                          |
| 0.7.0         | 2027-12-01 | Native default on stable; Classic kept for all of 0.7.x                    |
| 0.8.0-rc.1    | 2028-01-19 | Tauri removal candidate. Gated on the 4-week stable soak ending 2027-12-29 |
| 0.8.0         | 2028-02-02 | Native only                                                                |
| 0.8.1         | 2028-02-23 | `org.freedesktop.Platform//26.08`                                          |

**Timeline and capacity:**

- About **16.5 months** (2026-10-07 to 2028-02-23) for one developer working with AI agents.
- **481 points** in total, assuming about **10 points per week**.
- Holidays are excluded.
- Each release date can slip without hurting users, because Classic stays the default until beta.2 and the stable fallback until 0.8.0 (§12.2).

---

## 2. Current-state diagnosis

### 2.1 Size of what is being replaced (ui-inventory report)

| Bucket                                                        | Files | LOC    |
| ------------------------------------------------------------- | ----- | ------ |
| All `src/` (566 TS/TSX + 32 CSS)                              | 599   | 93,937 |
| Production UI logic (TS/TSX, excluding tests/mocks/CSS/types) | ~390  | 53,624 |
| Global CSS (`theme.css` alone is 5,866)                       | 27    | 13,168 |
| Tests (Vitest, jest-axe, utils)                               | 97    | 17,694 |
| Browser-dev mock layer (29 handler files)                     | 45    | 6,657  |
| `src-tauri` IPC layer (159 commands, 71 tests)                | –     | 12,381 |

The UI surface to port:

- 9 routes and about 60 screens and sections;
- **20 production dialogs**: the 21st, `BrowserDevPresetExplainerModal`, is dev-only and is dropped, not ported;
- 5 remaining `window.alert`/`confirm` calls, which need a native confirm primitive;
- 7 React context providers and 109 hook files;
- 147 distinct IPC commands and about 15 subscribed events;
- two 3-second polling loops: `check_game_running` in `useLaunchState`, and `list_running_profiles` in `useRunningProfiles`, which drives the "Currently Playing" badge;
- 46 `aria-live` regions, 568 aria attributes and 5 jest-axe suites;
- 18 skeleton loaders, 9 keyframes and 64 transitions, and 22 `backdrop-filter` blurs.

### 2.2 Root causes of jank, ranked (perf-diagnosis report)

1. **The software frame transport is forced on everyone.**
   - `WEBKIT_DISABLE_DMABUF_RENDERER=1` is set unconditionally in three places:
     - `src-tauri/src/lib.rs:74-79`;
     - the Flatpak manifest, `dev.crosshook.CrossHook.yml:56` (verified);
     - `scripts/dev-native.sh`.
   - WebKit then falls back to SharedMemory transport ([WebKit AcceleratedBackingStore.cpp](https://raw.githubusercontent.com/WebKit/WebKit/main/Source/WebKit/UIProcess/gtk/AcceleratedBackingStore.cpp); [Tauri Linux graphics guide](https://v2.tauri.app/develop/debug/linux-graphics/)).
   - The CSS is expensive on exactly that path: 22 blurs (including the base `.crosshook-panel`), 94 box-shadows, 90 gradients, and infinite animations on non-composited properties.
2. **117 of the 159 commands are synchronous and run on the GTK main thread** ([Tauri sync-command discussion](https://github.com/diffplug/dormouse/pull/321)).
   - `check_game_running` is polled every 3 s and can shell out to `flatpak-spawn --host ps`.
   - `batch_validate_profiles`, `community_sync` (git) and the readiness probes also block.
   - `profiles-changed` fans out to 5 listeners, so one favorite toggle triggers a full health scan.
3. **React re-render storms.**
   - The `ProfileContext` value changes identity on every render.
   - No component uses `React.memo`.
   - Gamepad focus state lives at the app root, and its `focusin` handler forces layout.
4. **`useScrollEnhance` hijacks wheel scrolling** and bypasses the async scrolling in WebKitGTK 2.52 ([WebKitGTK 2.52](https://webkitgtk.org/2026/03/18/webkitgtk-2.52-highlights.html); [WebKit bug 310814](https://bugs.webkit.org/show_bug.cgi?id=310814)).
5. **Perpetual polling and rAF loops.**
   - The running-profiles poll is mounted twice.
   - There is a gamepad rAF loop, plus a second rAF loop on the health dashboard.
6. **Unbounded lists.**
   - The launch log re-reads the whole file every 500 ms, which is O(n²) I/O.
   - `ConsoleView` appends without a cap.
   - Nothing is virtualized.
7. **Startup.**
   - A 1.1 MB JS bundle.
   - Synchronous `setup()` work.
   - Sleep-delayed startup events (350 ms, 500 ms and 2 s) that race the listeners.
8. **Footprint.** The `org.gnome.Platform//50` runtime is 1.1 GB, against 668.9 MB for `org.freedesktop.Platform//25.08`.

**Consequence for the rewrite.** Causes 1, 4, 7 and 8 disappear with a native toolkit. Causes 2, 3, 5 and 6 **do not**: they are app-architecture problems and would reproduce the jank under iced or gpui alike. The native UI must therefore enforce, and test in CI, four rules:

- No `crosshook-core` call inside `update` or `view`.
- Event-driven subscriptions instead of polling. Session-registry events replace both 3-second loops.
- Virtualized, bounded lists.
- Log lines batched once per frame.

Tuned Tauri (v0.6.1) fixes causes 1–6 inside Classic as far as WebKit allows. That makes baseline B the honest comparison for the native UI.

### 2.3 `src-tauri` is not thin (ipc-surface report)

About 3,500 lines of use-case orchestration live in `src-tauri`:

- the launch pipeline and its finalizer (version snapshot, known-good tagging, `cancel_linked_children`);
- profile mutation, revision and rollback;
- the prefix-dependency journal;
- process supervision for update and run-exe;
- health enrichment and bootstrap;
- the background-portal holder;
- runtime-helper lookup via Tauri `BaseDirectory::Resource` in `paths.rs` and `execution.rs`;
- security checks:
  - the ProtonUp catalog URL and checksum identity check;
  - community import workspace containment;
  - the storage-cleanup allow-list checked against a fresh scan.

`LaunchRequest` is built in four places: three TypeScript `buildLaunchRequest` copies and one in the CLI.

Several pieces of logic exist only in TypeScript:

- `profileNormalize`, `wizardValidation`, `installValidation` and `validateSteamAppId`;
- `derivePipeline*` and `hero-detail-model`;
- the 450-line launch dependency gate;
- the autosave debounce semantics: `useProfileLaunchAutosave` (673 lines) plus its effects (513 lines);
- `resolveLaunchMethod`;
- the custom-command tokenizer and highlighter.

All of this has to move into `crosshook-app`, `crosshook-core` or `crosshook-view-model`, with parity tests, before any native screen can reach parity (§7.3).

---

## 3. iced vs gpui: side-by-side (as of 2026-10-07)

| Dimension                   | iced                                                                                                                                                                                         | gpui                                                                                                                                                                                                                                                                                  | Edge                           |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| Latest official release     | 0.14.0, 2025-12-07, MIT ([crates.io](https://crates.io/crates/iced), [Phoronix](https://www.phoronix.com/news/Iced-0.14-Rust-GUI-LIbrary)). Master is 0.15-dev with MSRV 1.93.               | `gpui` 0.2.2, 2025-10-22, which predates the wgpu renderer ([crates.io API](https://crates.io/api/v1/crates/gpui)). In practice it ships through `gpui-pre` 0.3.8: weekly, one maintainer.                                                                                            | iced                           |
| Governance / bus factor     | Independent project; 88% of commits since 0.14 are by hecrj. COSMIC and Halloy maintain forks ([pop-os/iced](https://github.com/pop-os/iced), [halloy](https://github.com/squidowl/halloy)). | Zed-controlled. Dec 2025: work outside Zed's use case is "pushed off" ([HN quote of a Discord post](https://news.ycombinator.com/item?id=47003569); authorship unverified). The gpui-ce release pipeline is broken ([gpui-ce#278](https://github.com/gpui-ce/gpui-ce/issues/278)).    | iced                           |
| Churn                       | Breaking releases every 7–15 months. 0.15 changes `view` → `impl Widget` (#3495), overlay `Vec` (#3463), clipboard via Shell (#3238), and `Subscription::run_with_id` → `run_with`.          | DBFlux hit 54 compile errors on one minor bump ([dbflux#601](https://github.com/0xErwin1/dbflux/issues/601)). GPUI Kit shipped breaking 0.6 and 0.7 releases about a month apart.                                                                                                     | iced                           |
| Async / tokio fit           | First-class `tokio` executor feature; `Task::perform`, `Subscription::run_with`                                                                                                              | smol/async-task executor; the `gpui_tokio` bridge is not on crates.io                                                                                                                                                                                                                 | iced                           |
| Renderer / fallback         | wgpu with a software fallback: tiny-skia, or vello_cpu in #3306. Overrides via `ICED_BACKEND` and `WGPU_BACKEND`.                                                                            | wgpu since [zed#46758](https://github.com/zed-industries/zed/pull/46758). Fallback is llvmpipe only. The CPU renderer PR [#63936](https://github.com/zed-industries/zed/pull/63936) was closed 2026-10-07. GL crash: [zed#50996](https://github.com/zed-industries/zed/issues/50996). | iced                           |
| NVIDIA / Wayland bugs       | Open: Vulkan SIGSEGV #2572, washed-out colours #3401, Wayland freeze #3496 ([issues](https://github.com/iced-rs/iced/issues))                                                                | Flathub Zed NVIDIA bug ([dev.zed.Zed issues](https://github.com/flathub/dev.zed.Zed/issues), #382); driver 580 hang zed#35948                                                                                                                                                         | tie (iced has an escape hatch) |
| Keyboard focus / controller | Only text_input, text_editor and combo_box can take focus ([#489](https://github.com/iced-rs/iced/issues/489), open since 2020)                                                              | FocusHandle, tab stops, keymap actions dispatched along the focus path                                                                                                                                                                                                                | gpui                           |
| Accessibility               | None upstream; PR #1849 open since 2023, on the 0.15 milestone. The pop-os fork ships `a11y`.                                                                                                | AccessKit merged 2026-05-27 (zed#56065), AT-SPI on Linux                                                                                                                                                                                                                              | gpui                           |
| Virtualized lists           | No released list widget (branch `feature/list-widget-reloaded`, 2024-05). Long text is slow (#439). 0.14 adds culling in column/row.                                                         | `uniform_list`/`list` in core; GPUI Kit VirtualList and DataTable. gpui-fast reports per-frame CPU 3.31 → 0.19 ms ([gpui-fast](https://github.com/longbridge/gpui-fast)).                                                                                                             | gpui                           |
| Widget catalogue            | Core set plus `iced_aw` 0.14.1 (tabs, menu, number_input) ([iced_aw](https://github.com/iced-rs/iced_aw)); modal and toast exist only as examples                                            | GPUI Kit, 75+ components ([gpui-kit.com](https://gpui-kit.com/component))                                                                                                                                                                                                             | gpui                           |
| Layout / styling            | row, column, grid and responsive; closure and Catalog styles                                                                                                                                 | Taffy flex/grid behind a Tailwind-like API, closer to CSS                                                                                                                                                                                                                             | gpui                           |
| Text / IME                  | cosmic-text; IME since 0.14 (#2777); selectable text on master (#3497)                                                                                                                       | cosmic-text; XIM and text_input_v3; open XIM/fcitx5 bugs ([zed#60578](https://github.com/zed-industries/zed/issues/60578)) on the gamescope XWayland path                                                                                                                             | slight iced                    |
| HiDPI                       | winit `wp_fractional_scale_v1`                                                                                                                                                               | Own Wayland client; unverified                                                                                                                                                                                                                                                        | slight iced                    |
| Testing                     | `iced_test` Simulator, snapshot hashes, `.ice` scripts; `update` is pure Rust                                                                                                                | `TestAppContext` with a deterministic executor; headless windowing only since 2026-10-02 (zed#64969)                                                                                                                                                                                  | tie                            |
| Flatpak proof               | Halloy on Flathub on `org.freedesktop.Platform` ([flathub halloy](https://github.com/flathub/org.squidowl.halloy))                                                                           | Zed on Flathub ([dev.zed.Zed](https://flathub.org/apps/dev.zed.Zed))                                                                                                                                                                                                                  | tie                            |
| Licence                     | MIT. Note libcosmic is MPL-2.0 if adopted ([libcosmic](https://github.com/pop-os/libcosmic)).                                                                                                | Apache-2.0 (gpui, GPUI Kit)                                                                                                                                                                                                                                                           | tie                            |
| Docs                        | Thin book; 60+ examples ([iced-rs/book](https://github.com/iced-rs/book))                                                                                                                    | "Read the Zed source or ask on Discord" ([gpui.rs](https://www.gpui.rs/), [awesome-gpui](https://github.com/zed-industries/awesome-gpui))                                                                                                                                             | iced                           |
| Build / binary              | Measured: 43 s clean release; 21.5 MB stripped; 11.7 MB with fat LTO                                                                                                                         | About 12 MB claimed ([gpui-kit comparison](https://gpui-kit.com/docs/comparison)); heavy dependency tree                                                                                                                                                                              | iced                           |

### 3.1 Judge scores and rationale

| Lens             | iced     | gpui     | Winner (confidence) |
| ---------------- | -------- | -------- | ------------------- |
| Delivery risk    | 6        | 5        | iced (0.57)         |
| UX / performance | 6        | 7        | gpui (0.60)         |
| Maintainability  | 6.5      | 5        | iced (0.66)         |
| **Mean**         | **6.17** | **5.67** | **iced**            |

- **Delivery risk (iced, 0.57).** gpui gets to parity sooner: GPUI Kit maps almost one-to-one onto CrossHook's widget inventory, and it has upstream AccessKit and focus. But gpui has more tail risk:
  - there is no current official release;
  - `gpui-pre` and GPUI Kit both depend on one maintainer;
  - its churn is measured (DBFlux: 54 errors on one bump);
  - its software fallback is fragile.

  iced costs more up front but has a lower chance of a stall the project cannot recover from. Confidence is low because GPUI Kit could plausibly save more time than iced's stability does.

- **UX / performance (gpui, 0.60).** gpui wins on what dominates daily use:
  - D-pad navigation on the Deck;
  - frame times for large lists and the log;
  - polish and screen-reader support.

  iced wins on GPU and driver robustness and on software fallback. Neither toolkit has been benchmarked on CrossHook workloads, which is why M0 measures both.

- **Maintainability (iced, 0.66).** iced wins on:
  - tokio fit: `crosshook-core` uses tokio `rt`/`process`/`sync`/`time`, reqwest with streams, and zbus with tokio;
  - governance;
  - discrete release cadence.

  gpui wins on accessibility. Either way, the `crosshook-app`/`crosshook-view-model` layer limits the coupling to the toolkit.

**Dealbreakers recorded by the judges, and how M0 tests each one:**

| Dealbreaker                                                                    | M0 test                                            |
| ------------------------------------------------------------------------------ | -------------------------------------------------- |
| iced #2572 on the owner's RTX 5070 with no acceptable GL or tiny-skia fallback | M0-06, E1                                          |
| iced focus cannot be built without forking core widgets (#489)                 | M0-04 spatial-focus prototype                      |
| iced has no upstream AccessKit                                                 | M0-09 Orca probe on the pop-os fork / #1849 branch |
| gpui-pre or GPUI Kit stops publishing, or makes another 50+-error bump         | ADR risk register                                  |
| gpui has no software renderer and an open GL crash                             | M0-06 llvmpipe / no-Vulkan run                     |
| gpui XIM bugs on gamescope XWayland                                            | M0-09 Game Mode IME probe                          |
| `gpui_tokio` is not on crates.io                                               | M0-05 spike measures the vendoring cost            |

---

## 4. Recommendation and rationale

**Choose iced, pinned to a 0.15-dev git revision, subject to the ADR-0002 thresholds in §6.**

1. **A supply chain the project can survive.** iced has a stable MIT crate, and two production forks (COSMIC and Halloy, which is on Flathub) prove the "pin a revision, fork if needed" route. gpui has no current official crate, and one person (huacnlee) controls both the runtime channel and the widget kit.
2. **Executor fit.** iced runs natively on the tokio reactor that `crosshook-core` already needs. gpui would need a second runtime boundary and a vendored `gpui_tokio`.
3. **GPU robustness for gamers in Flatpak.** A mismatched `GL.nvidia` extension or a machine without Vulkan still gets a working window through tiny-skia, and the renderer can be overridden. gpui has no answer to that case.
4. **The gaps are app-owned work.** The focus layer, virtual list and log, modal stack, toasts, tabs, menus, split panes, masked input and selectable text are all built on public APIs inside `crosshook-ui`'s widget modules. M1 budgets them explicitly (§12).
5. **The UX lens favoured gpui.** The plan neutralises that with:
   - a focus layer and gilrs zones from the start of M1;
   - a culled log and list widget;
   - a perf checkpoint on real hardware at **every** pre-release (§9.4).

   If the M0 spike shows iced cannot reach the thresholds without forking core widgets, ADR-0002 picks gpui.

**Why 0.15-dev rather than 0.14.** 0.15 rewrites exactly the custom-widget signatures CrossHook would otherwise write twice: `impl Widget` views, overlay `Vec`, the removal of `Overlay::layout`, and clipboard via Shell. Halloy pins `0.15.0-dev` the same way.

**How the pin is maintained:**

- One scheduled **mid-programme rebase** of the pin, in M3.
- A move to the crates.io 0.15.x release when it ships, in its own sprint.
- iced APIs stay behind `crosshook-ui`'s widget layer so churn stays local.

**Accessibility strategy.** M0-09 must show that AccessKit roles, names and a live-region announcement reach Orca through either the pop-os `a11y` feature or the #1849 branch. Upstream iced has none of this today. If neither path works, ADR-0002 treats it as a gpui-favouring criterion (§6). From M1 onward:

- every interactive widget is built through the focus layer, so roles, labels and announcements live in one place;
- an AccessKit announcer replaces the 46 live regions;
- the M2-14 checkpoint runs keyboard and Orca checks;
- rc.1 is gated on Orca parity.

---

## 5. Risks and mitigations

| #   | Risk                                                                                        | L / I | Mitigation                                                                                                                                                                                                        | Issue                   |
| --- | ------------------------------------------------------------------------------------------- | ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| R1  | NVIDIA Vulkan SIGSEGV (#2572) or washed-out colours (#3401) on the RTX 5070                 | M / H | Reproduce in M0. Ship a renderer override from day one: `[ui] renderer` plus `ICED_BACKEND`/`WGPU_BACKEND`. Automatic fallback at startup.                                                                        | M0-06, M1-13, M5-03     |
| R2  | Wayland redraw freeze (#3496)                                                               | M / M | Pin a revision that includes the 2026-10-04 winit fix. Soak test. `fallback-x11` as a last resort.                                                                                                                | M1-13, M6-05            |
| R3  | The focus gap (#489) becomes a permanent tax                                                | M / H | Focusable wrappers plus a spatial-zone navigator, proven in M0 on button, checkbox, pick_list and modal. ADR picks gpui if this fails.                                                                            | M0-04, M1 focus layer   |
| R4  | No upstream a11y                                                                            | H / M | M0-09 AccessKit probe. A central announcer. Track #1849 and pop-os a11y. Orca checks at M2-14 and rc.1.                                                                                                           | M0-09, M2-14, M6-02     |
| R5  | An iced release lands mid-port                                                              | M / M | Pin a git revision. Mid-programme rebase in M3. Wrap iced APIs.                                                                                                                                                   | M1-13, M3               |
| R6  | Scope: about 54k LOC of UI logic plus 13k lines of CSS ported by one developer              | H / H | Explicit capacity of 10 points per week. Port in waves. React stays as the parity oracle. Per-area parity tests in M2, M3 and M4. Classic feature freeze from alpha.2. Dates slip, not scope (§12.2).             | M2–M4                   |
| R7  | Two UIs writing the same `settings.toml` and `metadata.db`                                  | M / H | Cross-binary flock instance lock, unknown-key preservation, atomic writes, schema forward guard with backup.                                                                                                      | M1-10, M1-11, M1-12     |
| R8  | Logic or security regressions while moving `src-tauri`                                      | M / H | Move the 71 tests along with the code. Security-preservation tests (§7.3). Tauri becomes thin delegations first, so React catches regressions. Browser-dev mocks are kept in sync under `check-mock-coverage.sh`. | M1 service moves, M1-08 |
| R9  | Steam Deck Game Mode quirks: gamescope XWayland, on-screen keyboard, duplicate pads         | M / M | `--device=input` from M1-26, so alphas can read pads. Steam Input de-duplication. Pickers instead of free text. Deck smoke at beta.1 and full matrix at beta.2.                                                   | M1-26, M4-12, M5-01     |
| R10 | Pre-release tags mis-handled by git-cliff or `release.yml`; an older build opens a newer DB | H / H | No spike tag. Pre-release pipeline before alpha.1. Schema guard and TOML preservation in 0.6.1.                                                                                                                   | M1-10, M1-11, M1-27     |
| R11 | iced bus factor (hecrj)                                                                     | L / H | MIT licence plus maintained forks; vendoring is possible.                                                                                                                                                         | ADR-0002                |
| R12 | Self-hosted OSTree beta repo: signing-key loss or hosting limits (e.g. GitHub Pages size)   | M / M | GPG key in the secrets store with an offline backup. Static deltas plus pruning to the last N commits. The GitHub Release bundle stays as a fallback channel.                                                     | M1-59                   |
| R13 | Native UI panic kills the app                                                               | M / M | Panic hook writes a crash log. Renderer info and native logs go into `export_diagnostics`. Classic launcher stays as the fallback.                                                                                | M1 crash hook           |
| R14 | Installed size roughly doubles while both GUIs ship                                         | H / L | Measured and gated: alpha.1 installed-size delta ≤ +30 MB; G9 at 0.8.1.                                                                                                                                           | M1-26, M7               |

---

## 6. M0 go/no-go gate: ADR-0002 thresholds

**The spike slice** is identical in both toolkits:

- a 500-card library with async cover art;
- a launch console replaying 5,000 lines/s for 60 s;
- the gamescope form;
- a modal with a focus trap;
- gilrs focus zones.

Each spike is built as a local Flatpak and run on E1, E2 and E3, plus an llvmpipe run with Vulkan unavailable.

**Hard thresholds.** A toolkit must pass all of these to be eligible.

| #   | Criterion     | Threshold                                                                                                                                                                                           |
| --- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H1  | Rendering     | E1 (RTX 5070, Wayland): 30-minute session with no crash and correct colours on at least one renderer. E2 Desktop and Game Mode: clean. E3: clean. No-Vulkan: renders, with library scroll ≥ 30 fps. |
| H2  | Focus         | 100% of the slice's interactive widgets reachable by D-pad and keyboard with a visible focus ring. Focus trapped in the modal and restored on close. **No fork of toolkit core widgets.**           |
| H3  | Scroll        | 500-card library: p95 ≤ 1 refresh interval (E1 8.3 ms at 120 Hz; E2 16.7 ms at 60 Hz). Dropped frames < 1%.                                                                                         |
| H4  | Console flood | 5k lines/s for 60 s: p95 frame ≤ 1 interval. Ring buffer ≤ 10k lines. RSS growth ≤ 15 MB.                                                                                                           |
| H5  | Idle          | CPU ≤ 0.5% over 60 s; UI wakeups ≤ 2/s; zero redraws while idle.                                                                                                                                    |

**Soft thresholds.** These are scored and weighed in the ADR.

| #   | Criterion                 | Threshold                                                                                                                           |
| --- | ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| S1  | Cold start to first frame | ≤ 300 ms on E1, ≤ 500 ms on E2                                                                                                      |
| S2  | Input latency             | Keystroke-to-photon ≤ 33 ms on E1; D-pad step ≤ 2 frames                                                                            |
| S3  | Accessibility (M0-09)     | Orca announces role and name for ≥ 90% of the slice widgets, plus one live-region announcement ("launch started") through AccessKit |
| S4  | IME (M0-09)               | fcitx5 or ibus CJK commit in a text input on E1 Wayland. Steam on-screen keyboard ASCII entry in Game Mode.                         |
| S5  | Effort                    | Custom widget code for the slice ≤ 1.5× the other toolkit's, in hours and LOC                                                       |
| S6  | Binary                    | Stripped release binary ≤ 30 MB                                                                                                     |

**Decision rule (ADR-0002):**

- **GO-iced:** iced passes H1–H5 and at least 4 of the 6 soft criteria, including S3 or a documented, dated path to it.
- **GO-gpui:** iced fails H1 or H2 and gpui passes H1–H5, including its no-Vulkan and Game Mode XIM runs.
- **NO-GO:** both toolkits fail a hard threshold.
  - Stay on tuned Tauri (the 0.6.x line).
  - Continue the toolkit-agnostic `crosshook-app` and `crosshook-view-model` work, which keeps its value.
  - Re-run the spike in 6 months, or sooner when iced 0.15 or a gpui software renderer ships.

ADR-0002 lives in `docs/architecture/`, so it is committed as `docs(architecture): …`, **not** `docs(internal)`. Raw results are committed under `docs/internal/bench/`.

---

## 7. Target architecture

### 7.1 Crate layout

```
src/crosshook-native/
  Cargo.toml                      # workspace; [profile.release] lto="fat", codegen-units=1, strip=true
  rust-toolchain.toml             # pinned toolchain ≥ iced MSRV (1.93); matches freedesktop rust-stable at 0.8.1
  deny.toml                       # cargo-deny: licences (MIT/Apache/BSD/Zlib/MPL-2.0 reviewed), advisories, bans
  crates/
    crosshook-core/               # domain, existing contracts unchanged, plus:
                                  #   LaunchRequest::from_profile (single builder for CLI, Classic, native)
                                  #   runtime_helpers::resolve: /app/resources → exe-relative → CARGO_MANIFEST_DIR (dev)
                                  #   platform::normalize_flatpak_host_path (/run/host, document-portal → real host path)
                                  #   metadata schema forward guard; TOML unknown-key preservation
                                  #   ported TS-only logic: profile normalize, install/wizard validation,
                                  #   validateSteamAppId, resolveLaunchMethod, command tokenizer
    crosshook-app/                # NEW, UI-agnostic (no iced/tauri deps)
      src/lib.rs                  #   App (Clone, Arc<Inner>) with service accessors
      src/bootstrap.rs            #   pre_thread_bootstrap() then App::bootstrap(opts) -> (App, StartupReport)
      src/runtime.rs              #   owns the tokio Runtime; App::spawn -> Send + 'static futures
      src/events.rs               #   AppEvent + broadcast EventBus (ProfilesChanged, SessionStarted/Ended, ...)
      src/jobs.rs                 #   JobId/JobHandle/JobEvent + JobRegistry (subscribe-before-start ordering)
      src/instance_lock.rs        #   flock + D-Bus activation (§10)
      src/log_tail.rs             #   offset-based incremental tailer, per-frame coalescing
      src/services/               #   launch, profiles, collections, community, export, health, onboarding,
                                  #   prefix_deps, protonup, protondb, storage, mods, settings, discovery,
                                  #   version, migration, steam, install, update, run_executable,
                                  #   diagnostics, catalog, game_metadata, offline, umu, background_portal
      src/error.rs                #   AppError { kind: Validation{field}|NotFound|Unavailable|Io|Conflict.., message }
      src/presenter/              #   display-path sanitising, view DTOs
    crosshook-view-model/         # NEW, pure state + reducers: form state, validation, autosave FSM, launch FSM,
                                  #   dependency gate, hero-detail model, derivePipeline (toolkit-agnostic, unit-tested)
    crosshook-ui/                 # NEW binary `crosshook-ui` (iced): shell/, routes/, screens/, bridge/, and
                                  #   widgets/ (theme tokens, focus layer, zones, modal stack, toasts, popover/menu,
                                  #   select, number input, tabs, split pane, textarea, masked input, selectable
                                  #   text, virtual list/table, log view, skeletons, motion layer)
    crosshook-cli/                # consumes the crosshook-app launch and profile services
  src-tauri/                      # Classic: delegations to crosshook-app in M1; deleted in M7
```

### 7.2 Rules (lint, review, CI)

- `crosshook-ui` never calls `crosshook-core` directly. It goes through `crosshook-app` via `Task::perform` or `Subscription`.
- No fixed-rate timers while idle.
- Every scrollable list over 200 items is virtualized.
- The log view is a ring buffer of at most 10k lines.
- Gateway scope:
  - `scripts/check-host-gateway.sh` `SCAN_DIRS` gains `crosshook-app`, `crosshook-view-model` and `crosshook-ui`.
  - `PLATFORM_RS` is fixed to `platform/`.
  - ADR-0001 is amended to cover the new crates. Portal calls (`ashpd`/`rfd` FileChooser, OpenURI) are **not** host-tool calls. Spawning `xdg-open` directly is banned in the UI crate; URLs go through OpenURI.
- A Rust URL allow-list in `crosshook-app` (protondb.com, steamgriddb.com) replaces the `shell:allow-open` scope in `capabilities/default.json`.

### 7.3 Contracts that must be specified and tested in M1

- **Bootstrap ordering.** `pre_thread_bootstrap()` runs single-threaded, before the tokio runtime or the iced event loop exist. It does the following, in order:
  1. the `unsafe set_var` XDG override;
  2. the Flatpak gate;
  3. first-run Flatpak migration;
  4. `migrate_legacy_tauri_app_id_xdg_directories`;
  5. `GDK_BACKEND` removal (Classic only).

  A test asserts the step order from a bootstrap trace, and asserts that `/proc/self/status` `Threads: 1` at entry.

- **Runtime helpers.** `runtime_helpers::resolve` replaces Tauri `BaseDirectory::Resource`. It is needed before `crosshook-ui` can launch anything.
- **Events instead of polling.**
  - Session-registry events on the bus replace `check_game_running` and `list_running_profiles` polling.
  - `JobHandle` delivers `run-executable` log and complete events in order, after subscription. This fixes the orphaned `run-executable-log` event and the race where "complete arrives before invoke resolves".
- **Security preservation tests** move with each service:
  - the ProtonUp catalog URL and checksum identity check;
  - community import workspace containment;
  - the storage-cleanup allow-list against a fresh scan;
  - the URL allow-list.
- **Profile save parity.**
  - The TS-only logic in §2.3 moves to core or the view-model, with tests that **native saves write byte-identical profile TOML** to Classic on golden fixtures.
  - In M2, Classic is routed through the core `LaunchRequest::from_profile` command, so the two UIs cannot build different launch requests during M2–M6.
- **Wayland identity.**
  - The iced window sets `application_id = "dev.crosshook.CrossHook"`.
  - Launchers are `dev.crosshook.CrossHook.desktop`, `dev.crosshook.CrossHook.NativePreview.desktop` and `dev.crosshook.CrossHook.Classic.desktop`.
  - All use `StartupWMClass=dev.crosshook.CrossHook` and `Icon=dev.crosshook.CrossHook`, so GNOME and KDE group them under one icon.
- **Crash handling.**
  - A panic hook writes a crash log into the existing log directory.
  - `export_diagnostics` includes native UI logs plus renderer info: adapter, backend, and fallback reason.

### 7.4 UI bindings

- One-off calls: `Task::perform(app.profiles().list_summaries(), Message::Loaded)`.
- Each launch session or job stream: `Subscription::run_with(session_id, …)`.
- Domain events: one subscription to `app.subscribe()`.

---

## 8. Coexistence / strangler strategy

1. **Shared layer first (M1, starting right after 0.6.1).**
   - Extract `crosshook-app` and `crosshook-view-model`.
   - Rewrite the 159 `#[tauri::command]` bodies as delegations.
   - Move the 71 tests.
   - Keep the 29 browser-dev mock handlers in sync under `check-mock-coverage.sh`.
   - React keeps working as the **parity oracle**.

   This step is useful whatever the ADR decides.

2. **One Flatpak app-id ships both binaries** from M1-26.
   - The second launcher is `dev.crosshook.CrossHook.NativePreview.desktop` (`Exec=crosshook-ui`).
   - The same `~/.var/app/...` tree means no data migration.
   - The manifest gains `--device=input` from M1-26, not M5, so the alphas can read controllers. Hosts with Flatpak older than 1.15.6 fall back to keyboard and Steam Input ([Flatpak 1.16](https://feaneron.com/2025/01/14/flatpak-1-16-is-out/), [device=input discussion](https://discourse.flathub.org/t/support-for-device-input/6645)).
   - No separate preview app-id: it would split the data, and Flathub rejects duplicate submissions ([Flathub requirements](https://docs.flathub.org/docs/for-app-authors/requirements)).
3. **Mutual exclusion.** The instance lock (§10) means Classic and native never run at the same time. `LaunchSessionRegistry` and the watchdogs are per-process.
4. **Port by domain, M2–M4.**
   - A route not yet ported shows an "Open in Classic" card, which releases the lock and execs Classic.
   - Hero Detail tabs owned by later milestones (Launch options, History, Compatibility) are placeholders until those milestones land.
   - Classic enters feature freeze at alpha.2 (fixes only).
5. **Switch the default at beta.2 (M5).**
   - The Flatpak `command:` becomes `crosshook-ui`.
   - `crosshook-native` becomes a **shim** that execs `crosshook-ui`, so exported launchers, Steam shortcuts and `flatpak run --command=crosshook-native` keep working (verified in M5-06).
   - The Tauri binary gets a Classic name and keeps `dev.crosshook.CrossHook.Classic.desktop`.
6. **Native default on stable (0.7.0, 2027-12-01).** Classic is kept for the whole 0.7.x line.
7. **Decommission (0.8.0, M7).** This starts only after the M6-09 4-week stable soak closes on 2027-12-29.
   - Remove `src-tauri`, React, Node, ts-rs, browser dev mode and the mocks.
   - Remove the Classic and NativePreview launchers.
   - Keep the `crosshook-native` shim.
   - Verify that `cargo tree` of the shipped binaries has no gtk3/glib, which closes #26 / YAN-755.
8. **Runtime switch (0.8.1).**
   - Move to `org.freedesktop.Platform//26.08` ([FreeDesktop SDK 26.08](https://www.phoronix.com/news/FreeDesktop-SDK-26.08)).
   - Build offline in the SDK from `cargo-sources.json` (flatpak-cargo-generator) with a `rust-stable` extension that satisfies the pinned MSRV.
   - The `release.yml` container changes from `flatpak-github-actions:gnome-50` to a freedesktop image.
   - GNOME 50 stays supported until GNOME 52 ([Flathub discourse](https://discourse.flathub.org/t/gnome-runtime-version-51-will-drop-gcr-3-and-libhandy-modules/12240); [runtimes](https://docs.flathub.org/docs/for-app-authors/runtimes)), so there is no rush.

**Maintaining stable 0.6.x during the rewrite.**

- `release/0.6` is branched at the v0.6.1 tag.
- Stable users get 0.6.x patch releases from that branch: backend security and data fixes, cherry-picked from `main`.
- Stable builds from `release/0.6` **never** contain `crosshook-ui`.
- `main` publishes only pre-releases until 0.7.0.

---

## 9. Benchmark suite

### 9.1 Terms

**Variants:**

- **A:** current Tauri (0.6.0).
- **B:** tuned Tauri (0.6.1).
- **C:** native.

**Environments:** defined canonically in [`docs/internal/bench/README.md`](../../internal/bench/README.md) — E1 developer laptop; E2-D Steam Deck Desktop Mode (KWin Wayland); E2-G same Deck, Game Mode (gamescope, XWayland, non-Steam shortcut); E3 Mesa AMD/Intel desktop; E3-sw software rendering (llvmpipe). Inventoried hardware/software values per environment: [`docs/internal/bench/environments.md`](../../internal/bench/environments.md).

**Tools:** metric-specific tools, units and failure rules are defined in [`docs/internal/bench/README.md`](../../internal/bench/README.md).

### 9.2 Gates G1–G10

| Gate             | Metric (canonical)                                               | Desktop (E1/E3)                        | Deck (E2-D/E2-G)                                                                                | Relative to A                                          |
| ---------------- | ---------------------------------------------------------------- | -------------------------------------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| G1               | Cold start to READY p95                                          | ≤ 500 ms                               | ≤ 900 ms                                                                                        | ≥ 2× faster                                            |
| G2               | Warm start to READY p95                                          | ≤ 250 ms                               | ≤ 400 ms                                                                                        | ≥ 2× faster                                            |
| G1 (first frame) | First frame painted                                              | ≤ 150 ms                               | ≤ 250 ms                                                                                        | ≥ 2× faster                                            |
| G3               | Idle PSS: empty library / 500 profiles after visiting all routes | ≤ 60 / ≤ 120 MB                        | same                                                                                            | ≥ 3× / ≥ 2× lower                                      |
| G3 (growth)      | 10-min memory growth                                             | < 5%                                   | < 5%                                                                                            | —                                                      |
| G7               | Idle CPU over 60 s; UI wakeups                                   | ≤ 0.2%; ≤ 2/s                          | ≤ 0.3%; ≤ 2/s                                                                                   | 0 fixed-rate timers                                    |
| G4               | Scroll p95 / p99 and frame pacing                                | ≤ 8.3 / 12.5 ms at 120 Hz              | ≤ 16.7 / 25 ms at 60 Hz                                                                         | Dropped < 1%; frames > 1.5× interval < 0.5%            |
| G5               | Input latency: keystroke-to-photon; D-pad step; route switch     | ≤ 25 ms; ≤ 1 frame; ≤ 50 ms            | ≤ 40 ms; ≤ 2 frames; ≤ 80 ms                                                                    | ≥ 2× lower                                             |
| G7 (Deck power)  | Deck idle power and GPU wake (fixed brightness)                  | –                                      | ≤ +0.3 W over the desktop-idle baseline; `gpu_busy` ≤ 1%; no redraw while idle except the caret | ≤ A                                                    |
| G8               | Console at 5k lines/s and health table (300 rows)                | p95 ≤ 1 interval; ≤ 10k lines; ≤ 15 MB | same                                                                                            | –                                                      |
| G10              | Size: Flatpak bundle / installed app / binary                    | ≤ 12 / ≤ 40 / ≤ 25 MB                  | same                                                                                            | Interim: installed delta ≤ +30 MB while both GUIs ship |
| G6               | UI-thread stalls > 16 ms from I/O or core                        | **0**                                  | **0**                                                                                           | Hard gate                                              |

Metric definitions, units and reported statistics are canonical in [`docs/internal/bench/README.md`](../../internal/bench/README.md); this table keeps the numeric thresholds only. IDs follow the canonical order and were remapped from the pre-canonical gate IDs used in earlier revisions (idle CPU G4→G7, scroll G5→G4, input G6→G5, Deck power unchanged (G7), size G9→G10, stalls G10→G6; first-frame moved from the G2 row to G1).

### 9.3 Baselines

- **A** is captured in M0-02.
- **B** is captured at the 0.6.1 exit for G1, G2, G3, G4 and G7 on E1/E2-D, with no regression against A allowed.
- **Both A and B are recaptured in M6-07** on the drivers and toolchain current at that time. Without that, the rc.1 comparison would drift by about a year.

### 9.4 Perf checkpoints at every pre-release

| Release         | Gate                                                                                                               |
| --------------- | ------------------------------------------------------------------------------------------------------------------ |
| 0.6.1           | B captured for G1, G2, G3, G4 and G7 on E1/E2-D; no regression against A                                           |
| alpha.0 (spike) | ADR-0002 hard and soft thresholds (§6)                                                                             |
| alpha.1         | G1, G2, G3, G4 and G7 within 2× of target on E1 (M1-52 real-hardware checkpoint); installed delta ≤ +30 MB         |
| alpha.2         | G1, G2, G3, G4 and G7 within 1.5× on E1 and E2-D, including the 500-profile library with cover art                 |
| alpha.3         | Same as alpha.2, over the configure surfaces                                                                       |
| beta.1          | Targets met on E1; within 1.25× on E2-D; covers console floods (G8), the health table and the 500+-profile library |
| beta.2          | Adds G5 input latency (gamepad) and G7 Deck idle power, within 1.25×                                               |
| rc.1            | G1–G10 pass on E1/E2-D/E2-G/E3 against the recaptured A and B                                                      |
| 0.8.1           | G10 final: bundle ≤ 12 MB, installed ≤ 40 MB                                                                       |

**Headless CI (M1-28)** runs under `cage` or `weston --backend=headless`. It guards G1, G3, G6 and G7 against regressions. It cannot measure GPU or compositor behaviour, which is why the real-hardware checkpoint (M1-52) repeats at every milestone. Raw CSV/JSON results are committed under `docs/internal/bench/`.

---

## 10. Persistence and storage boundary

| Datum                                                                                                                  | Class                           | Default / migration / editability                                                                                                                                                                                                                                                                          |
| ---------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[ui] library_view_mode` (`grid`/`list`)                                                                               | **TOML settings**               | Today in WebKit `localStorage` (`LibraryPage.tsx:32`). In M1, Classic reads `localStorage` once and writes the key to TOML, so the choice survives the port. Default `grid`. Editable in the Library toggle and in `settings.toml`.                                                                        |
| `[ui] scale` / `density`                                                                                               | **TOML settings**               | Default `auto`; covers gamescope at scale 1.0 and docked TVs. Settings → Appearance.                                                                                                                                                                                                                       |
| `[ui] renderer` (`auto`/`wgpu`/`gl`/`software`)                                                                        | **TOML settings**               | Default `auto`. Environment variables override it. Settings → Advanced, and shown in diagnostics.                                                                                                                                                                                                          |
| Accessibility: high contrast, motion (`auto`/`on`/`off`), controller mode (`auto`/`on`/`off`)                          | **TOML settings**               | Reuse the existing keys where present (verify in M1-11). The native UI also reads OS forced-colors and reduced-motion.                                                                                                                                                                                     |
| `[ui] window_width` / `window_height`                                                                                  | **TOML settings**               | Optional; absent means a centred default. Kept out of SQLite to avoid a schema bump.                                                                                                                                                                                                                       |
| Preferred UI                                                                                                           | **None (no key)**               | The choice is the launcher (.desktop) used, not a setting. No `preferred_ui` key.                                                                                                                                                                                                                          |
| Unknown keys from newer builds                                                                                         | **TOML settings**               | Preserved through `#[serde(flatten)] extra: toml::Table` from 0.6.1 on. Writes are atomic (temp file plus rename).                                                                                                                                                                                         |
| Launch history, health, version snapshots, readiness catalog, `profile_mods`, `prefix_version_restore_journal`, caches | **SQLite metadata**             | Schema is **v27** in code. CLAUDE.md says v26 and is fixed in M1. The programme plans **no** schema bump. Any v28+ bump needs an ADR, CLAUDE.md/AGENTS.md updates, and a downgrade-matrix entry.                                                                                                           |
| Schema forward guard                                                                                                   | **SQLite metadata** (behaviour) | If `user_version` is above the supported maximum: open read-only, refuse writes, show a banner. Before any migration, back up to `metadata.db.bak-v<N>`. Ships in 0.6.1.                                                                                                                                   |
| Session dismissals (rename toasts, Flatpak migration toast)                                                            | **Runtime-only**                | Today in `sessionStorage`; they stay per-session in memory.                                                                                                                                                                                                                                                |
| Instance lock                                                                                                          | **Runtime-only**                | `flock` on `$XDG_RUNTIME_DIR/app/dev.crosshook.CrossHook/instance.lock`, plus the D-Bus name `dev.crosshook.CrossHook` for activating the running window. The kernel releases a flock when the process dies, so a crash leaves **no stale lock**. A leftover file without a held lock is simply re-locked. |
| Log ring buffers, decoded texture LRU, job and session registries, gamepad state, portal handles                       | **Runtime-only**                | Never persisted.                                                                                                                                                                                                                                                                                           |

**Migration and backward compatibility**

- The app-id and data directories are unchanged and `app_id_migration` is kept, so no data migration is needed.
- **Downgrade matrix** (verified at rc.1 and in M7-11):

| Path                                 | Expected outcome                                                                                                                                                               |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 0.7.0 ↔ latest 0.6.x                 | Guarded by M1-10 and M1-11                                                                                                                                                     |
| 0.7.x → 0.6.0 or earlier (no guards) | Documented restore from `metadata.db.bak-v27` and `settings.toml.bak` written before the first native write. Release notes tell users to update to 0.6.1 before trying alphas. |
| 0.8.x → 0.7.0                        | Guards present; settings round-trip                                                                                                                                            |
| 0.8.1 → 0.8.0 runtime rollback       | `flatpak update --commit=<0.8.0 commit>`. Data is unaffected because app data is independent of the runtime.                                                                   |
| Any v28+ schema bump                 | Tested explicitly if one is introduced                                                                                                                                         |

**Offline**

- Unchanged: every UI path works offline.
- ProtonDB, SteamGridDB, community sync, the ProtonUp catalog and umu lookup keep their existing cached and stale fallbacks inside the services.

**Degraded behaviour**

| Condition                               | Behaviour                                                                          |
| --------------------------------------- | ---------------------------------------------------------------------------------- |
| `MetadataStore::disabled()`             | Fail-soft paths stay in the services; an "operational history unavailable" banner. |
| Newer schema detected                   | Read-only mode with a banner, no writes.                                           |
| No Vulkan, or NVIDIA extension mismatch | Automatic software renderer plus a one-time diagnostic toast.                      |
| No FileChooser portal                   | Inline error plus a typed-path field.                                              |
| No `/dev/input`                         | Keyboard and Steam Input navigation.                                               |
| Instance lock held                      | Activate the existing window over D-Bus, then exit.                                |

**User visibility.** Every new `[ui]` key is shown and editable in Settings and in `settings.toml`. The active renderer and fallback reason appear in Settings → Advanced and in diagnostic exports.

**Issue rule.** Persistence-touching issues (M1-10, M1-11, M1-12, the M2-01 view mode, M3-06 settings) carry the repo's **Storage boundary** and **Persistence & usability** subsections.

---

## 11. Release cycle

| Version       | Channel                        | Target     | Milestone | Native UI                                                                                     | Classic (Tauri)                                          |
| ------------- | ------------------------------ | ---------- | --------- | --------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| 0.6.1         | stable (GitHub Release bundle) | 2026-11-11 | M0        | –                                                                                             | Tuned; schema guard; TOML extras; `release/0.6` branched |
| 0.7.0-alpha.0 | spike, **internal, untagged**  | 2026-12-02 | M0        | iced and gpui spike slices                                                                    | Default                                                  |
| 0.7.0-alpha.1 | alpha (OSTree `beta` branch)   | 2027-04-14 | M1        | Preview: shell, read-only Library, console                                                    | Default                                                  |
| 0.7.0-alpha.2 | alpha                          | 2027-06-16 | M2        | Core loop: Library, Hero, profile editor, launch                                              | Default; feature freeze                                  |
| 0.7.0-alpha.3 | alpha                          | 2027-08-04 | M3        | Adds launch config, install/update/run, Proton, prefix deps, 13 Settings sections, onboarding | Default                                                  |
| 0.7.0-beta.1  | beta                           | 2027-09-22 | M4        | Feature complete: 9 routes, 20 dialogs                                                        | Default                                                  |
| 0.7.0-beta.2  | beta                           | 2027-10-20 | M5        | **Default on beta**; `command: crosshook-ui`                                                  | Classic launcher                                         |
| 0.7.0-rc.1    | rc                             | 2027-11-17 | M6        | G1–G10, a11y and data-compat gates pass                                                       | Classic launcher                                         |
| 0.7.0         | stable                         | 2027-12-01 | M6        | **Default on stable**                                                                         | Fallback for all of 0.7.x                                |
| 0.8.0-rc.1    | rc                             | 2028-01-19 | M7        | Only UI (still on GNOME 50)                                                                   | **Removed** (after the soak closes 2027-12-29)           |
| 0.8.0         | stable                         | 2028-02-02 | M7        | Only UI; `crosshook-native` shim kept                                                         | Removed; Node/React gone                                 |
| 0.8.1         | stable                         | 2028-02-23 | M7        | Only UI on `org.freedesktop.Platform//26.08`                                                  | –                                                        |

### 11.1 Release mechanics

- **Tags.**
  - The spike build gets **no tag and no release**. Its version string `0.7.0-alpha.0` would sort before alpha.1 if it were ever tagged by mistake; `spike.1` would have sorted after `alpha` and `beta`.
  - Pre-release tags `v0.7.0-{alpha,beta,rc}.N` become GitHub pre-releases: `prerelease: ${{ contains(github.ref_name, '-') }}`, with `make_latest` set to its negation.
- **git-cliff.** `ignore_tags = "-(alpha|beta|rc)\\.[0-9]+$"`, so pre-release commits roll into the stable section.
- **prepare-release.sh.**
  - In M1, add `crosshook-app`, `crosshook-view-model` and `crosshook-ui` to its version-match manifest list.
  - In M7, remove `src-tauri`.
- **Flatpak beta channel (M1-59).** `release.yml` today produces only a single-file bundle, so a channel has to be built:
  - a GPG-signed **OSTree repo**, e.g. on GitHub Pages, populated with `flatpak build-export` and `build-update-repo --generate-static-deltas`;
  - `beta` and `stable` branches;
  - a published `.flatpakrepo`.
  - Users install with `flatpak remote-add crosshook <url>/crosshook.flatpakrepo`, then `flatpak install crosshook dev.crosshook.CrossHook//beta`.
  - The GitHub Release bundle continues as a fallback.
  - Flathub beta comes only after Flathub acceptance, which the 0.8.1 source build makes practical ([Flathub maintenance](https://docs.flathub.org/docs/for-app-authors/maintenance)).
- **AppStream metainfo.**
  - Backfill `<release>` entries; the latest today is 0.2.9.
  - Add `type="development"` entries for alphas, betas and rcs.
  - Refresh screenshots and the description for the native UI in M6.
- **0.8.1 offline build (M7-05, M7-07).**
  - Vendor cargo sources with `flatpak-cargo-generator` into `cargo-sources.json`.
  - Use the `org.freedesktop.Sdk.Extension.rust-stable//26.08` extension, whose rustc must be ≥ the pinned MSRV.
  - Use a freedesktop CI container.
  - Pass the `/bin/bash` and host-gateway smoke tests.
- **CI cleanup in M7.** Remove or replace:
  - the `.github/workflows/lint-autofix.yml` (setup-node, npm, Biome) and `copilot-setup-steps.yml` (libwebkit2gtk, npm, Playwright) workflows;
  - `fixture-lint.yml` if it is TS-based;
  - the TS jobs in `lint.yml`;
  - the `lefthook.yml` Biome hooks;
  - `scripts/check-legacy-palette.sh` (replaced by a Rust theme-token check), `scripts/check-mock-coverage.sh`, and the ignores in `scripts/lib/modified-files.sh`;
  - the "Verify no mock code" step in `release.yml`.
- **Dev tooling in M1.**
  - `install-native-build-deps.sh` adds the xkbcommon, wayland, vulkan, fontconfig and xcb dev packages.
  - `dev-native.sh` gains a `crosshook-ui` mode.
  - `build-release-binary.sh` builds both binaries.
  - `lint.sh` runs clippy and fmt over the new crates.
- **Agent docs.**
  - Update in 0.6.1: remove the `useScrollEnhance` rule.
  - Update in M1: the `crosshook-app` architecture rule, ADR-0001 scope, schema v27, and the move from Forgejo to GitHub (commit `20a8da7`; templates now live in `.github/ISSUE_TEMPLATE/`).
  - M7-06 does only the final cleanup.

### 11.2 Entry and exit criteria (summary)

| Release    | Entry                                                                                                                                                                                  | Exit                                                                                                                                  |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 0.6.1      | M0-03, M0-08, M1-10, M1-11 and M0-11 merged; cargo test, Vitest and Playwright smoke green; CHANGELOG validated                                                                        | B captured on E1/E2 for G1–G5 with no regression; stable bundle published; `release/0.6` branched                                     |
| alpha.0    | iced and gpui slices built as local Flatpaks                                                                                                                                           | M0-06 matrix and M0-09 a11y/IME results recorded; ADR-0002 merged                                                                     |
| alpha.1    | M1 closed; src-tauri reduced to delegations with the mock check green; OSTree beta repo live; lock active in both UIs; pre-release pipeline and AppStream entry; G1–G5 within 2× on E1 | 1 week of dogfooding with no data-loss or lock bug; installed delta ≤ +30 MB; gamepad works in the Flatpak on the Deck (Desktop Mode) |
| alpha.2    | Native profile CRUD and launch; byte-identical saves; core-loop parity and the M2-14 a11y checkpoint; perf within 1.5× on E1/E2                                                        | 1 week launching real games through `proton_run` and `steam_applaunch`, on desktop and Deck with a gamepad                            |
| alpha.3    | M3 closed including the M3-13 parity tests; perf within 1.5×                                                                                                                           | Fresh-install onboarding end to end; settings round-trip without key loss into Classic and into `release/0.6`                         |
| beta.1     | M4-11 feature-complete gate; parity ≥ 95%; no "Open in Classic" placeholders; perf at target on E1 and within 1.25× on E2                                                              | 2 weeks with no P1; M4-12 Deck Desktop Mode smoke passes                                                                              |
| beta.2     | M5 closed; Deck Desktop and Game Mode matrix green; renderer fallback verified; command switched with the shim; G6/G7 within 1.25×                                                     | 2 weeks as the default with no P1 regression                                                                                          |
| rc.1       | M6-01..07 and M6-10 closed; G1–G10 on E1/E2/E3; Orca parity; downgrade matrix green; soak passes                                                                                       | 2 weeks with no P1/P2; release notes and AppStream validated                                                                          |
| 0.7.0      | rc.1 exit criteria met; `prepare-release.sh` green                                                                                                                                     | M6-09: 4 weeks with no forced rollback to Classic (closes 2027-12-29); M7-01 blocked until then                                       |
| 0.8.0-rc.1 | M6-09 closed; Tauri, React, Node, mocks and ts-rs removed; no gtk3/glib in shipped binaries; Rust-only CI green                                                                        | 1 week with no P1; 0.8 → 0.7 downgrade check passes                                                                                   |
| 0.8.0      | rc exit criteria met; launchers removed; docs updated                                                                                                                                  | 2 weeks stable                                                                                                                        |
| 0.8.1      | Freedesktop 26.08 offline SDK build; G9 met                                                                                                                                            | Data preserved on update; rollback via `--commit` verified                                                                            |

---

## 12. Milestone overview (dependency order)

| Milestone                                | Window                  | Goal                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Points                                                 |
| ---------------------------------------- | ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| **M0** Decision spike & tuned Tauri      | 2026-10-07 → 2026-12-16 | **Track 1** ships v0.6.1 on 2026-11-11: fixtures, baseline A, async conversion of the I/O-bound commands, removal of render costs and `useScrollEnhance`, the schema forward guard (M1-10), unknown-key preservation (M1-11), and the `release/0.6` policy. **Track 2**: iced and gpui spikes on the same slice, the AccessKit/Orca/IME probe, the E1/E2/E3 matrix, and ADR-0002 with numeric thresholds.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | 48 over 10 weeks                                       |
| **M1** Foundation                        | 2026-11-11 → 2027-04-14 | Toolkit-agnostic work right after 0.6.1: bootstrap ordering, event bus, job registry, `runtime_helpers::resolve`, launch and profile services, plus 6 domain-service moves with security-preservation tests; session events in place of polling; flock lock; src-tauri reduced to delegations with mock sync; gateway-check coverage, cargo-deny, the view-model crate, agent docs. After the ADR: `crosshook-ui` (Wayland identity, crash hook, theme, icons, focus layer, AccessKit announcer, overlays, confirm primitive, forms, virtual list/log/table, nav shell); `[ui]` settings, async bridge, image pipeline, portal dialogs with a Rust URL allow-list, path normalization, multi-zone gamepad; test harness, CI, dev scripts, Flatpak second binary with `--device=input`, OSTree beta repo, pre-release pipeline, AppStream, bench harness and the real-hardware checkpoint. Ships alpha.1. | 171 (~10/week, ~17.5 working weeks, holidays excluded) |
| **M2** Port: core loop                   | 2027-04-14 → 2027-06-16 | Library with favorites and an event-driven "Currently Playing" badge; context menu; inspector and context rail with the 7-day activity chart. Hero Detail shell and Trainer tab, with placeholders for later tabs. Profile editor, autosave FSM, rename/duplicate/delete, favorites, known-good, legacy import and exports. Launch FSM, console drawer, preview modal. Shared TS logic moves to core and the view-model with byte-identical TOML tests. Classic is routed through the core launch builder and the CLI reuses the services. Just-in-time widgets: split panes, textarea, masked input, selectable text, skeletons, motion layer, backdrop substitutes, high-contrast detection, locale formatting. Also page banners, controller prompts, Deck detection, skip link, back/forward history, and the a11y checkpoint. Ships alpha.2.                                                        | 81 over 9 weeks                                        |
| **M3** Port: configure & operate         | 2027-06-16 → 2027-08-04 | Launch sub-tabs; Gamescope, MangoHud and Optimizations panels plus the **trainer gamescope** panel; Hero Launch options. Install, Update, Run exe and Lutris import with installValidation. Prefix deps. Proton Manager with the uninstall-plan confirm. All 13 Settings sections, including prefix storage health, diagnostic export, umu controls and background protection. Onboarding with wizardValidation. Readiness nags and platform status banners. Parity tests, plus the mid-programme rebase of the iced pin. Ships alpha.3.                                                                                                                                                                                                                                                                                                                                                                 | 52 over 7 weeks                                        |
| **M4** Port: discover, health & organise | 2027-08-04 → 2027-09-22 | Community with import wizard and exports. Discover with the TrainerDiscovery modal. Compatibility with the ProtonDB card, ProtonVersionsPanel and overwrite confirm. Hero Compatibility and History tabs. Health with the offline trainer modal. Host tools. Collections with the launch-defaults editor. Mods. Launcher export with Settings → Manage launchers. Config history and trainer version tracking. Command palette, notifications and the migration toast. `iced_test` replacements for jest-axe and Playwright; feature-complete gate; Deck Desktop smoke. Ships beta.1.                                                                                                                                                                                                                                                                                                                    | 56 over 7 weeks                                        |
| **M5** Platform & distribution           | 2027-09-22 → 2027-10-20 | Steam Input de-duplication and hotplug. Full Deck validation in Desktop and Game Mode. Automatic renderer fallback. HiDPI, fonts and IME. Flatpak `command:` switched to `crosshook-ui` with the `crosshook-native` shim, and launchers/Steam shortcuts verified. Native becomes the default on beta, with a Classic launcher. Ships beta.2.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | 19 over 4 weeks (overlaps the beta.1 soak)             |
| **M6** Parity & hardening                | 2027-10-20 → 2027-12-29 | Baselines A and B recaptured. G1–G10 on E1/E2/E3. Accessibility parity. Compatibility and downgrade matrix. Parity audit of 9 routes and 20 dialogs. Soak, user docs, AppStream refresh. Ships rc.1 (2027-11-17) and 0.7.0 (2027-12-01). Closes when the M6-09 4-week soak ends on 2027-12-29.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | 29                                                     |
| **M7** Cutover & decommission            | 2027-12-29 → 2028-02-23 | Starts only after M6-09. Remove src-tauri and verify no gtk3/glib remains (closes #26 / YAN-755). Remove React, Node, ts-rs, browser dev mode and mocks. CI, hook and script cleanup. Remove the Classic and NativePreview launchers. Vendored cargo sources and the pinned rust-stable extension, then freedesktop 26.08 with an SDK build and a freedesktop CI container. Docs. Downgrade and runtime-rollback checks. Ships 0.8.0-rc.1, 0.8.0 and 0.8.1.                                                                                                                                                                                                                                                                                                                                                                                                                                              | 25                                                     |

**Total: 481 points.**

### 12.1 Cross-cutting dependency rules for the issue set

**Every port issue (M2–M4) is blocked by:**

- the async bridge (M1-20);
- the nav-shell route registration (M1-19);
- the service move for its own domain.

**The M1 domain-service moves are split by domain** (launch, profiles, plus six further moves), not one coarse issue, because nearly every M3/M4 issue depends on them. Specific blockers:

| Issue(s)                                 | Blocked by                                                     | Why                                  |
| ---------------------------------------- | -------------------------------------------------------------- | ------------------------------------ |
| M3-01, M3-02                             | Catalog service                                                | Catalog-backed panels                |
| M4-06                                    | Collections service                                            | Collections UI                       |
| M4-07                                    | Mods service                                                   | Mods UI                              |
| M3-05, M3-07, M4-01, M4-02, M4-03, M4-06 | Portal dialogs / URL / clipboard issue (M1-22)                 | File pickers, external links, copy   |
| M3-07 onboarding, M4-01 community import | Profile editor and launch-request issues (M2-04, M2-08, M1-01) | Both wizards create profiles         |
| Settings → Manage launchers              | Launcher export (M4)                                           | The section needs the export feature |
| M5-01 Deck validation                    | All port milestones and `--device=input` (M1-26)               | Validation covers the whole app      |

**Other ordering rules:**

- Settings → Manage launchers therefore ships in M4 with launcher export, not in M3.
- Hero Detail tabs owned by M3 and M4 are placeholders in M2.

**Issue labels follow the repo taxonomy:**

- `type:` (feature, refactor, build, migration, docs);
- `area:` (ui, launch, profiles, build, security, cli);
- `priority:`;
- `platform:` (steam-deck, linux).

### 12.2 Capacity, contingency and slip policy

- **Velocity.** About 10 points per week from one developer plus agents. M0 Track 2 shares capacity with the start of M1 from 2026-11-11 to 2026-12-16.
- **No hidden buffer.** Dates are computed from points ÷ velocity with holidays excluded, and nothing is padded inside them.
- **Slip rule.** If a milestone is more than 2 weeks behind at its midpoint, the release date moves and scope stays feature-complete, as CLAUDE.md requires. Users are not hurt, for three reasons:
  - Classic stays the default until beta.2 and remains the stable fallback through 0.7.x.
  - Stable users get 0.6.x patches from `release/0.6`.
  - The `crosshook-app` work already delivers value on its own.
- **NO-GO path.** If ADR-0002 is NO-GO, M1's toolkit-agnostic half continues and M1's UI half is suspended.

---

## 13. Repo-state notes found during research

- **Collaboration has moved to GitHub.** Commit `20a8da7` moved collaboration back to GitHub.
  - `.github/workflows/` contains `copilot-setup-steps`, `fixture-lint`, `lint-autofix`, `lint`, `pr-title-autofix`, `pr-title` and `release`.
  - Forgejo is archived under `.archive/forgejo/`, and the templates now live in `.github/ISSUE_TEMPLATE/`.
  - CLAUDE.md, AGENTS.md and the other agent-rule files were updated in the same commit.
- **Stale `platform.rs` references.** CLAUDE.md, AGENTS.md, ADR-0001 and `check-host-gateway.sh` (`PLATFORM_RS`) still reference `crosshook-core/src/platform.rs`. It is now the `platform/` module.
- **Schema version.** It is v27 in code (`v26_v27.rs`, `prefix_version_restore_journal`). AGENTS.md agrees; CLAUDE.md says 26.
- **Stale metainfo.** `<releases>` stops at 0.2.9.
- **Orphaned event.** `run-executable-log` has no listener. It is fixed by the `JobHandle` ordering contract (§7.3).

## 14. Sources

- **iced:** https://github.com/iced-rs/iced · https://crates.io/crates/iced · CHANGELOG · issues #439, #489, #1002, #1849, #2572, #3306, #3401, #3477, #3495, #3496, #3497 · https://github.com/iced-rs/book · https://github.com/iced-rs/iced_aw · https://github.com/pop-os/iced · https://github.com/pop-os/libcosmic · https://github.com/squidowl/halloy · https://github.com/flathub/org.squidowl.halloy · https://github.com/GyulyVGC/sniffnet · https://www.phoronix.com/news/Iced-0.14-Rust-GUI-LIbrary · https://phoronix.com/news/COSMIC-Epoch-1.8
- **gpui:** https://github.com/zed-industries/zed/tree/main/crates/gpui · https://www.gpui.rs/ · https://crates.io/api/v1/crates/gpui · https://github.com/zed-industries/zed/pull/46758 · https://news.ycombinator.com/item?id=47003569 · https://github.com/zed-industries/zed/pull/63936 · https://github.com/zed-industries/zed/issues/50996 · https://github.com/zed-industries/zed/issues/60578 · https://github.com/gpui-ce/gpui-ce/issues/278 · https://github.com/longbridge/gpui-kit · https://gpui-kit.com/component · https://gpui-kit.com/docs/comparison · https://github.com/longbridge/gpui-fast · https://github.com/0xErwin1/dbflux/issues/601 · https://flathub.org/apps/dev.zed.Zed · https://github.com/zed-industries/awesome-gpui
- **WebKit / Tauri:** https://v2.tauri.app/develop/debug/linux-graphics/ · https://webkitgtk.org/2025/11/26/webkitgtk-2.50.html · https://webkitgtk.org/2026/03/18/webkitgtk-2.52-highlights.html · https://bugs.webkit.org/show_bug.cgi?id=310814 · https://gitlab.gnome.org/GNOME/gtk/-/issues/6180 · https://github.com/diffplug/dormouse/pull/321 · https://mirror.umd.edu/gentoo-portage/net-libs/webkit-gtk/files/2.52.3-disable-nvidia-dmabuf.patch
- **Flatpak / platform:** https://docs.flathub.org/docs/for-app-authors/runtimes · https://docs.flathub.org/docs/for-app-authors/requirements · https://docs.flathub.org/docs/for-app-authors/maintenance · https://discourse.flathub.org/t/support-for-device-input/6645 · https://feaneron.com/2025/01/14/flatpak-1-16-is-out/ · https://www.phoronix.com/news/FreeDesktop-SDK-26.08 · https://discourse.flathub.org/t/gnome-runtime-version-51-will-drop-gcr-3-and-libhandy-modules/12240 · https://wiki.archlinux.org/title/Gamescope · https://steamcommunity.com/app/1675200/discussions/0/3818544339813264129

---

_Note on your question about the critique sub-agent:_ I could not check whether it is stuck. `ListAgents` only lists peer sessions, not the workflow script's sub-agents, so the orchestrating session ("Tauri to Iced/GPUI migration research") is the place to check it. This revision step did finish.
