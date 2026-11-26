# gpui decision spike (M0-05)

Spike for [YAN-836](https://linear.app/itcn/issue/YAN-836) / GitHub #650. It is the control spike for the ADR-0005 native UI framework decision (M0-07, YAN-860), on the same slice as the [iced spike](iced.md) (M0-04). The code is throwaway and lives only on the [`spike/gpui`](https://github.com/yandy-r/crosshook/tree/spike/gpui) branch (head `6c822ca7`), as crate `src/crosshook-native/crates/crosshook-ui-spike-gpui`. It is never merged to `main`. This file and the E1 results under [`../bench/C-gpui/E1/`](../bench/C-gpui/E1/) are the only parts on `main`.

This is evidence, not a decision.

## Summary

- The exact pins build, but `gpui-kit =0.7.1` does not pin its own sub-crates (F2), and the issue's feature names do not exist (F3).
- **F1 (blocker found and worked around):** `crosshook-core` enables zbus's `tokio` feature. Cargo feature unification turns it on for every zbus user in the binary: gpui-linux's portal listener, ashpd and `accesskit_unix`. These run on gpui's own threads (smol, gpui workers), outside any tokio context, and the app panicked at startup. The spike switches `crosshook-core`/`crosshook-app` zbus to `async-io` on this branch only. A real migration has to choose one zbus backend for the whole process.
- All slice items were built.
- Flatpak staging: the binary was built inside `org.gnome.Sdk//50` and installed as local branch `spike-gpui` of `dev.crosshook.CrossHook`.
  - The default renderer reaches READY inside the sandbox.
  - `SPIKE_SMOKE_PICK=1` makes one `org.freedesktop.portal.FileChooser.OpenFile` call, and KDE's portal "Open File" window opens. Closing it returns `Response` code 1 (cancelled), and the app reports `No file picked`.
- Most of what the iced spike had to hand-write is stock GPUI / GPUI Kit here:
  - virtualised lists (`VirtualList`, `uniform_list`);
  - real focus (`FocusHandle`, Tab order);
  - modal focus trap and Escape (Kit `Dialog` + `focus_trap`);
  - the form widgets (`Form`, `NumberInput`, `Select`, `Switch`);
  - Lucide icons (`gpui-kit-assets`) and the portal file chooser (`prompt_for_paths`, via ashpd).
- The custom layer is only zones with memory, D-pad spatial moves and the gamepad. The engine is the iced spike's, with its rectangle type swapped.
- E1 at 180.06 Hz (one refresh interval is 5.554 ms):
  - Keystroke-to-photon p95 at most 18.6 ms, on par with iced.
  - Console replay at 5,000 lines/s (G8): p95 17.6 ms, p50 16.7 ms, about 62 presented frames/s (iced p50 11.1 ms). The spike's own replay flushes one batch per 16 ms (`console/replay.rs`), a candidate cause of the ~60 Hz.
  - Resize (G9) and wheel scroll (G4, all three pages) p50 33.3 ms, p95 38.9–44.4 ms. 33.3 ms is also the input cadence (G9 steps the window at 30 Hz, G4 sends a wheel notch every 33 ms), so the interval p50 shows about one frame per input event, not a measured throttle. iced presented every interval (5.56 ms) on G9 and on G4 `library`/`settings`, but 33.3/44.4 ms on G4 `console` too. Per-event latency and missed frames were not measured.
  - Warm start (G2) p50 261 ms to READY. The like-for-like figure is the first presented frame: p50/p95 258/394 ms, against iced 220/293 ms.
- Memory is bounded: the 50k-line replay grows PSS by 2.5 MB, peak 229 MB.
- The earlier G4 `FOCUS_LOST` failures were not gpui's fault: real pointer motion from a physical input device moved the cursor off the window (see [Bench harness findings](#bench-harness-findings)).
- `cargo tree -i gtk` and `-i glib-sys` are empty (the same command on `crosshook-native` finds `gtk v0.18.2`).
- `crosshook-core` is called only from `core_bridge`, on the tokio runtime. This is enforced by a test and a runtime assertion.

## Pinned revision and build

Build figures are local observations on E1 (i9-14900HX, 32 threads; dependency sources already fetched). No build logs are committed.

| Item                                                                    | Value                                                                                                                                                                                                                                                                                     |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| gpui                                                                    | `gpui-pre =0.3.8` (crates.io snapshot of zed@279fe07, 2026-10-05) plus `gpui-pre-platform =0.3.8` (`wayland`, `x11`, `font-kit`), `gpui-pre-util =0.3.8`; gpui-pre pulls the other `gpui-pre-*` at `=0.3.8` itself (wgpu 29.0.4 renderer)                                                 |
| GPUI Kit                                                                | `gpui-kit =0.7.1` (default features `component`, `assets`), plus `gpui-base`, `gpui-component`, `gpui-component-macros`, `gpui-kit-assets` all `=0.7.1` (see F2)                                                                                                                          |
| gpui_tokio                                                              | Vendored from zed@279fe07 `crates/gpui_tokio` (104 lines, Apache-2.0, `vendor/gpui_tokio/{LICENSE-APACHE,NOTICE.md}`), compiled as a module; only `init_from_handle` is used                                                                                                              |
| MSRV                                                                    | None declared: no `rust-version` in gpui-pre, gpui-kit, gpui-component or gpui-pre-wgpu (all edition 2024). Builds on the repo toolchain 1.95.0                                                                                                                                           |
| Other deps                                                              | `gilrs 0.11`, `lru 0.16`, `image 0.25` (jpeg, png), `tokio`, `futures`, `crosshook-core`                                                                                                                                                                                                  |
| Lockfile churn                                                          | 490 net new packages (642 → 1,132 `[[package]]` entries in `Cargo.lock`, against `main` at `aaeddf10`); 23 existing crates moved version (`futures*` 0.3.32, `wasm-bindgen*` 0.2.117, `js-sys`/`web-sys` 0.3.94, `zvariant*`, `zbus_names`, `toml` 0.8.2, `bitflags`, `proc-macro-crate`) |
| Unique crates (`cargo tree -e normal,build`, host target, name+version) | 618 (iced: 436)                                                                                                                                                                                                                                                                           |
| Clean release build                                                     | 94.5 s (iced: 63 s)                                                                                                                                                                                                                                                                       |
| Incremental release build (one file touched)                            | 4.6 s (iced: 4.9 s)                                                                                                                                                                                                                                                                       |
| Clean / incremental dev build                                           | 50.8 s / 2.0 s                                                                                                                                                                                                                                                                            |
| Release binary                                                          | 57.1 MB; 44.1 MB stripped (iced: 32.2 / 25.0 MB). The debug binary is 789 MB                                                                                                                                                                                                              |
| glibc                                                                   | The host build needs `GLIBC_2.44`. Built inside `org.gnome.Sdk//50` (repo toolchain 1.95.0 mounted into the SDK, separate target dir), it needs `GLIBC_2.39`, as for iced. The SDK release build took 118 s                                                                               |
| Flatpak (G10)                                                           | Bundle 10.3 MB, installed 42.2 MB (excluding the GNOME 50 runtime: not a first-install figure), stripped binary 42.1 MB (SDK build of spike commit `c4ca90e1`). iced's Flatpak was a local branch only, with no G10                                                                       |

## Effort

About 2 hours of wall time with an AI pair on 2026-11-25, out of a ~3-day (5-point) box:

- First pass, 15:57–17:15 EST: the code, the tests, the zbus panic (F1) and G2/G3/G5/G8/G9.
- Second pass, 17:14–17:58: G1, Flatpak staging, the portal smoke test, G10, the G4 pointer trace, the palette fix and the G4 re-capture.

The time box was never the limiting factor. Each item still open below says why it is open.

The hours are not comparable with the iced spike's: gpui reused iced's focus engine, the bench shim and the runner changes, all written during the iced spike.

## Code size

`wc -l` at spike head `6c822ca7` (3,729 lines of Rust under `src/`, including the 260-line `#[gpui::test]` file `src/app/tests.rs`; inline unit tests are counted with their modules; plus 104 vendored lines of gpui_tokio):

| Area                                                                        | Lines | Files                                            | iced spike |
| --------------------------------------------------------------------------- | ----- | ------------------------------------------------ | ---------- |
| Focus layer (targets, pure engine, bounds registry + ring wrapper, routing) | 611   | `src/focus/*.rs`, `src/app/nav.rs`               | 607        |
| Gamepad (gilrs thread, mapping) + action/keymap layer                       | 183   | `src/gamepad.rs`, `src/actions.rs`               | 135        |
| Library grid and card view (on Kit `v_virtual_list`)                        | 221   | `src/library/grid.rs`, `src/app/library.rs`      | 226        |
| Cover decode + byte LRU (`RenderImage`, `drop_image` on evict)              | 167   | `src/library/covers.rs`                          | 149        |
| Log console (ring, paced replay, view on `uniform_list`)                    | 317   | `src/console/*.rs`                               | 414        |
| Modal and palette (Kit `Dialog` bodies)                                     | ~55   | `src/app/view.rs` (`modal_body`, `palette_body`) | 43         |
| Core boundary (`core_bridge`, incl. its tests)                              | 200   | `src/core_bridge.rs`                             | —          |
| Renderer override + adapter log                                             | 172   | `src/renderer.rs`                                | 83         |
| Bench protocol and probe                                                    | 405   | `src/bench.rs`, `src/app/bench_flow.rs`          | 408        |

The line counts are close to iced's, but they buy different things:

- **Focus.** About half of the 611 lines is the iced spike's pure engine, ported unchanged. The rest does not reimplement focus. GPUI already has real focus (`FocusHandle`, `focus_next`, Tab order) and Kit already traps Tab in dialogs. The rest covers:
  - a per-frame bounds registry (a `canvas` probe in each wrapper);
  - mapping gpui focus back to targets, so the engine follows Tab and clicks;
  - routing D-pad, zone and Confirm actions.

  The iced spike had to make buttons focusable through a wrapper widget; gpui needs none of that.

- **Log widget.** The iced spike's culled console needed a custom `Widget` (190 lines). Here it is a `uniform_list` closure (100 lines including follow/scroll handling).
- **Pickers.** Kit's `Select` is meant to open and navigate from the keyboard, but this was not verified: no test or real-display check opens it, and `activate` does nothing for it (`Field(7)`). Because the spike's context-free `enter` binding outranks the Kit input context (see [Bench harness findings](#bench-harness-findings)), Enter on the focused `Select` may reach `activate` instead of opening it. iced's `pick_list` had no keyboard API, so A cycled the value there.

## Acceptance criteria

| Criterion                                                               | Result                                 | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ----------------------------------------------------------------------- | -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Time box respected, actual hours recorded                               | **Pass**                               | About 2 h of a ~3-day box ([Effort](#effort))                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Same metrics as M0-04 on E1 under `docs/internal/bench/C-gpui/E1/`      | **Pass (everything producible on E1)** | G1 (5 cold launches), G2, G3, G4 (`library`, `console`, `settings`, 4 iterations), G5, G8, G9 and G10 captured. G6/G7 cannot be produced on E1: there is no producer and `perf` is missing, as for iced and Tauri B                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Focus coverage (share of slice widgets reachable by D-pad and keyboard) | **Partial**                            | 100 % of slice widgets are focus targets by construction: 3 nav, back/forward, search, every card and its favourite, 15 form fields, 2 inspector actions, 2 modal buttons, palette. Asserted in headless `#[gpui::test]` (real keymap, Kit Root): `Nav(0)` and `Nav(1)` (F6 landing; Enter on `Nav(1)` navigates); `Card(0)`, `Card(1)`, `Card(1+cols)` and `Card(3)` (arrow walk, zone memory, modal opener); the `Field(0..=14)` D-pad sweep; `Overlay(0)`/`Overlay(1)` with the Tab trap and Escape; `Palette` (native Enter navigates); the Inspector zone (F6). Not asserted: `Nav(2)`; the `Back`/`Forward` targets (history only through Alt-←/→); `Search`; `Favorite(i)`; `InspectorAction(0)`/`(1)`. No sweep on a real display. `activate` (Enter/Space/A) is a no-op for `Field(0..=5)` (number inputs), `Field(7)` (`Select`) and `Search`. Gamepad buttons map to the same actions (unit-tested; no physical pad driven on E1) |
| G6 stalls from core work                                                | **Not measured**                       | No G6 producer on E1. By construction no core call can run on the foreground executor (next row)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `cargo tree -p … -i gtk` / `-i glib-sys` return nothing                 | **Pass**                               | Both `error: package ID specification … did not match any packages` (exit 101). Positive control: `cargo tree -p crosshook-native -i gtk` in the spike worktree finds `gtk v0.18.2`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| No `crosshook-core` call on the gpui foreground executor                | **Pass**                               | See [Core boundary](#core-boundary)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Renderer override + adapter log                                         | **Partial**                            | See [Renderer override](#renderer-override)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Evidence: ashpd portal dialog, Lucide icons, back/forward estimate      | **Pass**                               | In the `spike-gpui` Flatpak, `SPIKE_SMOKE_PICK=1` made one `FileChooser.OpenFile` call and the KDE portal "Open File" window appeared; closing it returned `Response` code 1 and `SPIKE_PICK_RESULT No file picked`. See [Evidence items](#evidence-items)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Flatpak staging (`--branch=spike-gpui`, `app_id`)                       | **Pass**                               | Built in `org.gnome.Sdk//50` (`GLIBC_2.39`), installed as user branch `spike-gpui` next to `master`, `spike-iced` and `yan795-variant-a`. Those three are untouched (same commits before and after); `flatpak install` made `spike-gpui` the current branch, so the previously current `yan795-variant-a` was made current again. The default renderer reaches READY in the sandbox (Vulkan, NVIDIA, 1.6 s wall to exit-after-READY). The window's Wayland app id is `dev.crosshook.CrossHook`: KWin reports `resourceClass` `dev.crosshook.CrossHook`                                                                                                                                                                                                                                                                                                                                                                                       |
| This report                                                             | **Done**                               | —                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

## Core boundary

- `src/core_bridge.rs` is the only file that names `crosshook_core`. The unit test `core_calls_only_here` scans `src/` and fails on any other occurrence.
- Every core function (`load_summaries`, `load_gamescope`, `save_gamescope`, `paths`) starts with `assert_off_ui_thread()`. That function panics if it runs on the thread `main` marked before `Application::run`, the gpui foreground thread. The test `guard_panics_on_ui_thread` checks the guard.
- Calls go through `core_bridge::blocking(cx, f)`: `Tokio::spawn` (vendored gpui_tokio) → `tokio::task::spawn_blocking(f)` on the app-owned runtime (2 workers, `spike-tokio-*` threads). `init_from_handle` gives gpui_tokio that runtime. The result comes back to gpui as a `Task` that the shell awaits in `cx.spawn_in`; only the result's state update runs on the foreground.
- Cover decoding (pure `image` work, no core) runs on gpui's background executor (`cx.background_spawn`).

## E1 results

E1, 2026-11-25, 3840×2160 @ 180.06 Hz (KWin 6.8, NVIDIA RTX 5070 Laptop, driver 620.73.04), `--fixtures` seed 42 (500 profiles). Every committed run exited 0 and ended `captured` with a clean tree (`dirty: false`). The iced runs used NVIDIA driver 620.70.06 (kernel 7.3.4), gpui's 620.73.04. gpui's G4 ran 4 iterations, iced's 2. CSVs are in [`../bench/C-gpui/E1/`](../bench/C-gpui/E1/), and each run's commit is listed in that directory's README.

The runner records the repo commit at run time and the binary's sha256, not the commit the binary was built from. Each "built from" below is the operator's statement.

- G1–G3, G5, G8 and G9 use the native release binary built from `75aa6fc8` (G1 ran at `f4fec19c`, same app code).
- G4 ran at `caf4d25b` (docs only on top of `f6c12fe7`) with the native binary built from `f6c12fe7`, which adds only the palette Enter fix. No behaviour exercised by G2, G3, G5, G8 or G9 changes: G5 `route_switch` and `dpad` go through the navigation actions and `activate`, whose `Nav` and `Card` arms are unchanged.
- G10 uses the SDK-built binary of spike commit `c4ca90e1` (before the palette fix) and its bundle.

iced C numbers are from [`iced.md`](iced.md); Tauri B numbers are the E1 baseline B evidence in [`../bench/README.md`](../bench/README.md) (native, 2026-11-16, captured as evidence, not as a gate). B was captured before #647 (blocking work off async workers) and #649 (blur, infinite repaint and scroll-hijack removal, console ring buffer), on driver 620.70.06, so the B column likely overstates the gap to current Tauri.

| Metric                                              | gpui C (this spike)                                                         | iced C                                                           | Tauri B (native, README)               |
| --------------------------------------------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------- | -------------------------------------- |
| G1 cold start to READY p50 / p95 (5 launches)       | 939 / 963 ms                                                                | not captured                                                     | not captured                           |
| G1 first presented frame p50 / p95                  | 936 / 951 ms                                                                | —                                                                | —                                      |
| G2 warm start to READY p50 / p95                    | 261 / 410 ms                                                                | 281 / 333 ms                                                     | 1038.1 / 1100.5 ms                     |
| G2 first presented frame p50 / p95                  | 258 / 394 ms                                                                | 220 / 293 ms                                                     | 245.8 / 316.3 ms                       |
| G3 idle PSS (start → after 10 min)                  | 210 → 210 MB, flat over 600 s (first sample 0.08 s); GPU 86 MB (DRM fdinfo) | 242 → 254 MB (first sample 0.015 s); GPU 50 MB (nvidia-smi `fb`) | —                                      |
| G4 `library` frame p50 / p95                        | 33.3 / 44.4 ms (739 frames)                                                 | 5.56 / 5.56 ms                                                   | 211 / 418 ms                           |
| G4 `settings` frame p50 / p95                       | 33.3 / 44.4 ms (722 frames)                                                 | 5.56 / 5.56 ms                                                   | 117 / 344 ms                           |
| G4 `console` frame p50 / p95 (scroll during replay) | 33.3 / 44.4 ms (739 frames)                                                 | 33.3 / 44.4 ms                                                   | 61.2 / 128.4 ms                        |
| G5 `search` p50 / p95                               | 11.7 / 14.1 ms                                                              | 14.7 / 17.1 ms                                                   | 184.3 / 244.0 ms                       |
| G5 `toggle_favorite` p50 / p95                      | 13.5 / 17.4 ms                                                              | 15.2 / 17.4 ms                                                   | 34.9 / 48.9 ms                         |
| G5 `toggle_optimization` p50 / p95                  | 13.6 / 18.0 ms                                                              | 15.8 / 17.3 ms                                                   | 130.8 / 196.8 ms                       |
| G5 `dpad` p50 / p95                                 | 16.2 / 18.6 ms                                                              | 14.0 / 18.1 ms                                                   | 57.3 / 80.1 ms                         |
| G5 `route_switch` p50 / p95                         | 13.7 / 17.9 ms                                                              | 16.4 / 18.3 ms                                                   | 155.0 / 187.6 ms                       |
| G8 replay frame p50 / p95 / p99                     | 16.7 / 17.6 / 22.3 ms (1,236 frames)                                        | 11.1 / 16.7 / 16.7 ms (1,822)                                    | 347.1 / 1009.4 / 1269.0 ms (46 frames) |
| G8 memory growth / peak PSS                         | 2.5 / 229 MB                                                                | 2.3 / 270 MB                                                     | 993.8 / 3119.1 MB                      |
| G9 resize frame p50 / p95                           | 33.3 / 38.9 ms                                                              | 5.56 / 33.3 ms                                                   | 33 / 39 ms                             |
| G10 bundle / installed / stripped binary            | 10.3 / 42.2 / 42.1 MB (runtime excluded)                                    | not captured                                                     | not captured                           |

Notes:

- READY means the same thing as in the iced spike: summaries loaded, then two frames gpui drew (`Window::on_next_frame` scheduled from render). It is not a presentation confirmation. The like-for-like start-up figure is `first_present_ms`. G2 iteration 1 is a cold-ish outlier (502 ms); the p50 is the better comparison.
- **G8 at ~60 Hz.** gpui presented 1,236 frames over two 10 s replays (pooled), ~62 fps; iced presented 1,822 over its two 10 s replays (pooled), ~91 fps. gpui's frame p50 16.7 ms is three refresh intervals, iced's two. The spike's replay coalesces appended lines into one batch per 16 ms (`console/replay.rs`, `BATCH_INTERVAL`), so the ~60 Hz may be the app's own batch rate rather than gpui throttling. Not traced (batch cadence, gpui frame scheduling, wgpu present mode).
- **G9 resize p50 33.3 ms** (6 intervals) against iced 5.56 ms. The driver resizes the window in 30 Hz steps (`RESIZE_HZ`), so 33.3 ms is about one frame per step; iced also presented between steps. Whether gpui lags or skips a step was not measured. Tauri B matches gpui here.
- **G1** is five separate runner invocations of `--metric G1 --collect presentation --cold-cache-ack --iterations 1`.
  - Before each, the operator (this spike, on E1) ran `sync && echo 3 | sudo tee /proc/sys/vm/drop_caches` (2026-11-25 17:14:44–17:15:08 EST, all exit 0; a local observation, the drop log is not committed).
  - `G1.csv` merges the five rows and recomputes the summary row with the runner's own `bench_capture.stats` percentiles. The bench README documents no other merge convention, and no earlier G1 capture exists.
  - Cold start costs about 680 ms more than warm (p50 939 against 261 ms). The first presented frame tracks READY within about 3 ms: gpui presents nothing until its first full frame.
- **G10** used `--runtime-delta-mb 0`, because the GNOME 50 runtime was already installed, so the installed total excludes the runtime and is not a first-install figure. This is the same method as the pending iced G10 (Linear YAN-2444). The binary is the SDK build of spike commit `c4ca90e1`, before the palette fix. G10 also needed two runner fixes on this branch (see [Bench harness findings](#bench-harness-findings)). The stripped binary is 42.1 MB, against 44.1 MB for the host build; a hypothesis, not checked, is that the SDK's older toolchain libraries link less.
- **G4 p50 33.3 ms on all three pages**, p95 44.4 ms, 97–99 % of intervals above 1.5 periods.
  - The driver sends a wheel notch every 33 ms, so the interval p50 reflects the input cadence: gpui presents about one frame per notch and nothing between notches.
  - iced presented every refresh interval (5.56 ms) on `library` and `settings`, but also 33.3/44.4 ms on `console`.
  - Per-notch latency and missed or late frames were not measured. The data shows gpui producing no frames between events, not gpui dropping input.
  - Not traced further (open item).
  - G4 ran on the binary with the palette fix (`f6c12fe7`), on a clean tree at `caf4d25b`. A run with the same binary but a dirty tree (docs being edited) was discarded as non-evidence and re-run. An earlier G4 run without the fix is superseded: its `settings` row (500 ms p50) measured the library page under a still-open palette ([Bench harness findings](#bench-harness-findings)).
- G5 used `CROSSHOOK_BENCH_CLOCK_FLOOR_NS=1000`, as for iced (exact CLOCK_MONOTONIC µs marks).
- GPU memory (G3 `gpu_mb`, G8 `gpu_growth_mb` 0.0) is from DRM fdinfo (`gpu_source=fdinfo`) for gpui and from `nvidia-smi pmon` `fb` for iced, so the two are not comparable.

## Renderer override

gpui 0.3.8 on Linux always renders through wgpu (29.0.4). The relevant behaviour:

- `WgpuContext::instance` builds the instance with a fixed `Backends::VULKAN | Backends::GL`. It does **not** read `WGPU_BACKEND`, so the iced-style override has no equivalent.
- Adapters are sorted by `ZED_DEVICE_ID` match, then compositor GPU, then device type (discrete > integrated > … > CPU), then backend (Vulkan before GL).
- Each adapter is tried by configuring the real surface; the first that works wins.

The spike's `--renderer vulkan|gl|lavapipe|llvmpipe-gl` therefore steers the loaders: `VK_ICD_FILENAMES`, `__EGL_VENDOR_LIBRARY_FILENAMES`, `LIBGL_ALWAYS_SOFTWARE`. `--device 0xNNNN` sets `ZED_DEVICE_ID`, and an explicit env value wins. The adapter is logged at startup as `SPIKE_RENDERER backend=… adapter=… software=… driver=… driver_info=…`, from gpui's "Selected GPU adapter" log line plus `Window::gpu_specs()`.

Native E1, one READY run per mode (each `SPIKE_RENDERER` line is quoted from the app's stderr; local observations, the logs are not committed):

| Mode (env it sets)                                                                                                 | Result                                                                                                                                                                                                                                                                                                              |
| ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| default / `--renderer vulkan`                                                                                      | `backend=Vulkan adapter="NVIDIA GeForce RTX 5070 Laptop GPU" driver_info="620.73.04"`, READY in 0.86 s. Mesa radv is probed first and warns "not a conformant Vulkan implementation"                                                                                                                                |
| `--device 0x7550` (`ZED_DEVICE_ID`, AMD Navi 48 dGPU)                                                              | `backend=Vulkan adapter="AMD Radeon RX 9070 XT (RADV GFX1201)"`, READY. The device filter works                                                                                                                                                                                                                     |
| `--renderer gl` (`VK_ICD_FILENAMES=/dev/null`, NVIDIA EGL)                                                         | **Fails**: `Adapter NVIDIA GeForce RTX 5070 Laptop GPU/PCIe/SSE2 (Gl) failed: no compatible surface formats` → `No GPU adapter found that can configure the display surface`. No window opens; `open_window` returns `Err` and `cx.quit()` is called, yet the process stayed alive until the 30 s timeout killed it |
| `--renderer lavapipe` (`VK_ICD_FILENAMES=lvp_icd.x86_64.json`)                                                     | `backend=Vulkan adapter="llvmpipe (LLVM 23.1.5, 256 bits)" software=true`, READY in 0.44 s. NVIDIA's GL adapter is tried first and rejected with the same surface-format error                                                                                                                                      |
| `--renderer llvmpipe-gl` (Vulkan hidden, `__EGL_VENDOR_LIBRARY_FILENAMES=50_mesa.json`, `LIBGL_ALWAYS_SOFTWARE=1`) | `backend=Gl adapter="llvmpipe (LLVM 23.1.5, 256 bits)" software=true driver_info="4.6 (Core Profile) Mesa 26.2.7"`, READY in 0.39 s. gpui's GL path runs on llvmpipe's GL 4.6; the `zed#50996` storage-block limit is not hit there                                                                                 |

NVIDIA's EGL exposes a GL adapter but no surface format gpui accepts. That is the same failure class iced hit (`GraphicsAdapterNotFound` through NVIDIA's EGL), surfacing differently. With Vulkan hidden, only a CPU adapter works: lavapipe, or llvmpipe through the Mesa EGL vendor. In the `gl` row the process also kept running after `open_window` failed until the 30 s timeout. The quit had to come from the app, so the broken-GPU path needs explicit handling.

`zed#50996` ("Zed crashes on non-Vulkan GPU: Too many vertex shader storage blocks") is open upstream. gpui's GL path uses storage buffers (`shaders_storage.wgsl`), and drivers that cap vertex-stage storage blocks fail there. The broken-GPU fallback (M5-03) cannot rely on wgpu-GL as the safety net. gpui-pre 0.3.8 has no tiny-skia/CPU renderer; its only software path is a CPU Vulkan or GL adapter (lavapipe or llvmpipe).

## Evidence items

- **File dialog.** gpui's own `cx.prompt_for_paths` goes through `ashpd::desktop::file_chooser::OpenFileRequest` in gpui-pre-linux. No rfd and no GTK: `cargo tree -i gtk` and `-i glib-sys` are empty. `SPIKE_SMOKE_PICK=1` opens the chooser at startup and prints `SPIKE_PICK_RESULT …`.

  **Flatpak smoke test** (local observation; the `dbus-monitor` and KWin output are not committed), `flatpak run --branch=spike-gpui` with `SPIKE_SMOKE_PICK=1`, isolated launcher `HOME`, `--nofilesystem=host:reset`:
  - `dbus-monitor` saw one `method call … destination=org.freedesktop.portal.Desktop … interface=org.freedesktop.portal.FileChooser; member=OpenFile` from the sandboxed app.
  - KWin's window list (`/WindowsRunner` Match + `getWindowInfo`, as the bench does) then showed a new window with caption `Open File` and `resourceClass` `org.freedesktop.impl.portal.desktop.kde`, next to the spike window (`resourceClass` `dev.crosshook.CrossHook`).
  - A one-shot KWin script closed the dialog. The portal answered with `org.freedesktop.portal.Request.Response` code 1 (user cancelled), and the app printed `SPIKE_PICK_RESULT No file picked`.
  - Nothing was picked, because the smoke test cannot click.
  - The whole path is the sandboxed app → xdg-desktop-portal → the KDE portal backend, with no GTK in the app.

- **Icons.** `gpui-kit-assets` embeds the Lucide catalog through `rust-embed` (`Assets` = the 106-icon default set, `AllAssets` = the full catalog; `LICENSE-LUCIDE`, ISC). It is registered with `Application::with_assets(Assets)`, and `IconName::*` is used directly as an element. No `currentColor` handling was needed: the svg takes the text colour. Lucide comes with the toolkit here; iced needed a hand-copied subset.
- **Back/forward.** `gpui_base::History<T>` is stock (back/forward entries, truncation on push, max entries), and Kit also ships `NavStack` with transitions. The spike's history is that type plus Alt+←/→ bindings (about 20 lines). Estimate for M1-49: under half a day, less than iced's because the type exists.

## API pain points at gpui-pre 0.3.8 / gpui-kit 0.7.1

1. **zbus backend unification (F1).** Any gpui app that also links a `zbus/tokio` user panics at startup. The fix had to go into `crosshook-core`'s features, not into the UI crate. Entering a tokio runtime on the main thread does not help, because gpui's smol and worker threads also open D-Bus connections.
2. **`gpui-kit =0.7.1` does not pin its family (F2).** Its sub-crates are `^0.7.1`. An exact pin needs five `=` lines, and a lockfile refresh silently mixes 0.7.1 with 0.7.2/0.7.3.
3. **Feature names (F3).** `gpui-component` and `gpui-kit-assets` are `dep:`-style optional deps; the features are `component` and `assets`.
4. **Actions do not reach Kit dialogs from the app root.** Kit renders dialogs in its Root layer, outside the app view's element tree, so actions dispatched from a focused dialog button never bubble through the shell root. The spike registers its navigation actions on each dialog body too (`nav_actions`).
5. **No focused element → actions go to the window root.** With nothing focused, gpui dispatches keystrokes at Kit's Root, above the app view. The shell root therefore tracks its own `FocusHandle` and takes focus when nothing has it, or F6 and the D-pad do nothing on a cold start.
6. **No `WGPU_BACKEND`.** Backend selection is internal (see above); overrides only work through loader environment variables.
7. **Kit widgets own their focus handles via keyed state** (`Button`, `Switch` use `window.use_keyed_state`). `Switch`/`Button` take no external handle at this version, so the spike wraps them in a tab-stop div and sets `tab_stop(false)` on the widget. That doubles the element count per control.
8. **Two `History` types.** Kit `History` is a different API from the iced spike's; the slice uses Kit's.
9. **Logging.** gpui logs adapter selection at `info` through `log`. Without a logger installed the adapter is invisible, so the spike installs a minimal `log` sink to recover it.

## API churn and governance

- `gpui-pre` is an unofficial crates.io snapshot of Zed's in-tree `gpui`, republished weekly. After 0.3.8 (2026-10-05, zed@279fe07) it had 6 releases in about 7 weeks, up to 0.3.14 (2026-11-23, zed@f126950). Each release pins its own `gpui-pre-*` family exactly, so any bump moves the whole family.
- GPUI Kit (Longbridge) had 8 releases after 0.7.1 in the same period (0.7.2 to 0.11.0), with 4 minor bumps.
- **One owner for both.** The crates.io API (`/api/v1/crates/<name>/owners`, checked 2026-11-25) lists a single owner for `gpui-pre` and for `gpui-kit`: the same individual account (`huacnlee`), with no team. The gpui snapshot channel and the component kit both depend on one person.
- The benchmarked spike stays on its original pins. The isolated upgrade below measures source and dependency churn; it does not update the benchmark results.
- Zed does own crates.io `gpui` (owners include the `zed-industries` team), but it is stale at 0.2.2 (2025-10-22); newer builds exist on crates.io only as `gpui-pre` snapshots. Depending on current gpui means choosing between this snapshot channel, a git pin of the Zed monorepo, or a fork. Every one of these requires an owner on our side.

## Licences

- gpui and the `gpui-pre-*` crates: Apache-2.0.
- gpui_tokio (vendored): Apache-2.0; the Zed `LICENSE-APACHE` is copied and attribution kept.
- GPUI Kit (`gpui-kit`, `gpui-base`, `gpui-component`, `gpui-kit-assets`): Apache-2.0.
- Lucide icons: ISC (`LICENSE-LUCIDE` in gpui-kit-assets).
- No GPL in the spike's own additions. This is not a dependency-policy pass; see the workspace audit below.

## Measured upgrade: gpui-pre 0.3.14 / GPUI Kit 0.11.0

[YAN-2477](https://linear.app/itcn/issue/YAN-2477) / [GitHub #652](https://github.com/yandy-r/crosshook/issues/652) tested these exact versions on 2026-11-26 UTC on x86_64 Linux with Rust 1.95.0. The upgraded worktree started at `6c822ca7`; the baseline audit used a separate pristine copy of that revision. The `spike/gpui` ref and the `main` lockfile were not changed.

- **Resolution:** all five Kit packages listed above moved to `0.11.0`. The gpui snapshot family moved to `0.3.14`, except its separately versioned `gpui-pre-reqwest`, which stayed at `0.12.15`. Cargo also moved `wgpu` from `29.0.4` to `30.0.1` and `taffy` from `0.13.0` to `0.15.1`.
- **Compiler output:** after editing the exact pins and refreshing the lockfile with `cargo update -p gpui-pre --precise 0.3.14`, Cargo resolved and the first check passed. There were **zero compile errors** and three deprecation warnings in `console/view.rs`. Replacing `ScrollWheelEvent` with `ScrollEvent` and `on_scroll_wheel` with `on_scroll` cleared the warnings.
- **Churn (`git diff --numstat`):** Rust +4/−4 in one file; spike manifest +10/−10; generated `Cargo.lock` +902/−671 for this targeted update (Cargo reported 43 additions, 45 removals and 53 updates).
- **Elapsed:** about **0.072 h (4.3 min)** of wall time from the approximate start through the compile checks, test link, dependency audit and graph queries. The final formatting and recheck were outside that window. This is one run, not a maintenance estimate.

From the native Cargo workspace, `cargo check --locked -j6 -p crosshook-ui-spike-gpui --all-targets` passed on both the baseline and final graph. `cargo test --locked -j6 -p crosshook-ui-spike-gpui --no-run` linked the test executable. This sample did not run the tests or recheck real-display behaviour, rendering performance, or Flatpak compatibility.

### Dependency-policy audit

`cargo deny --manifest-path src/crosshook-native/Cargo.toml --locked check` (cargo-deny 0.20.2, unchanged workspace policy; local advisory DB at `b9b8a3c`, dated 2026-11-25) reported **five licence errors and one advisory error** on each graph; bans and sources passed on both graphs. These are workspace graph findings, not a list of libraries linked into the Linux executable.

| Finding                                                                  | Baseline   | Upgraded   |
| ------------------------------------------------------------------------ | ---------- | ---------- |
| `cbindgen 0.28.0` and `dwrote 0.11.5`: MPL-2.0                           | Rejected   | Rejected   |
| `enum-iterator 2.3.0` and `enum-iterator-derive 1.5.0`: 0BSD             | Rejected   | Rejected   |
| `libbz2-rs-sys 0.2.7`: `bzip2-1.0.6`                                     | Rejected   | No finding |
| `libfuzzer-sys 0.4.13`: NCSA clause, through `image` → `ravif` → `rav1e` | No finding | Rejected   |
| `lru 0.16.4`: RUSTSEC-2026-0253, a direct spike dependency               | Reported   | Reported   |

The upgraded audit paths lead to the spike, not to the Classic shell. `libfuzzer-sys` was already in the baseline lockfile but not reported on that audited graph. The upgrade changed both the renderer version and the dependency audit results despite little source churn. If gpui is selected, resolve the advisory and licence findings before adopting this graph, then rerun the display and packaging checks; the compile sample is not a toolkit decision.

## Bench harness findings

- The bench shim's `wl_proxy_add_dispatcher` hook and `dlsym` redirect from the iced spike (cherry-picked) work unchanged for gpui's wayland-rs client: the K/M/G records appear.
- The runner needs `WAYLAND_DISPLAY` in its environment. When launched from a shell without it, gpui fell back to X11/headless and READY never came (`READY_TIMEOUT`). This was a local operator error, not a harness bug.
- **G4 `FOCUS_LOST` and the `wl_pointer.leave` (resolved).** Six G4 runs in the first pass and five in the second stopped with `FOCUS_LOST: pointer focus is not on the app window during the scroll`. Tracing with `WAYLAND_DEBUG=client` on the app, in a probe that replays the G4 `library` drive:
  - Real motion events: `wl_pointer.motion` walked the cursor from (640, 420) to the left edge (x≈0) and to the top edge, then `wl_pointer.leave`. The runner's uinput device only emits wheel notches and never moves the pointer.
  - No new surfaces: the client created exactly two `wl_surface`s (the main surface and the cursor surface) and no `xdg_popup` or subsurface, so neither Kit nor gpui opened a tooltip or hover layer.
  - Not app-driven: the app made no pointer-constraint or warp request.
  - Control run, same placement, nothing injected: 54, then 318 and 149 `wl_pointer.motion` events and 1–2 leaves in 12 s, so the cursor moved without any bench input.
  - The source was most likely a physical USB pointer device on E1 sending motion while the operator was away. That matches the "hands off the keyboard and mouse" prerequisite.
  - A G4 loop ran the idle control before every attempt and skipped attempts whose control saw motion. Two attempts after a quiet control still failed (the motion started mid-run); the captures came after quiet controls. G8 had captured earlier because it happened to run in a quiet period.
  - A runner-side fix (an idle-motion pre-check before the timed scroll) is an open item; the gpui app needs no change for G4.
- **Palette Enter did nothing (app bug, found through G4 `settings`).** The first G4 capture reported `settings` p50 500 ms. Its frame logs showed ~230 one-interval frames, then a frame every 500 ms (the Kit input's cursor blink), so the page was not scrolling. A screenshot probe of the bench's key sequence (Ctrl+K, type `settings`, Enter) showed the palette staying open on the library page.
  - Cause: the spike binds `enter` to its own `Confirm` with no key context. gpui ranks a context-free binding at the deepest context depth, so it outranks the Kit input's `Input`-context Enter. `Confirm` reached `Shell::activate`, which had no case for the palette.
  - A first fix (`bf258010`) submitted through the Kit dialog's `on_ok`. It passed a headless test that dispatched `Confirm` directly, but did nothing on real key input; a G4 re-run at that commit still measured the blink.
  - Fix (`f6c12fe7`): `activate` submits the palette, and `palette_submit` also closes Kit's dialog layer.
  - The test `palette_navigates_and_settings_page_scrolls` now sends a native Enter key-down/up (`simulate_keystrokes("enter")` enters as IME text) and checks `has_active_dialog`. It fails without each half of the fix. The probe screenshot confirms the release binary reaches and scrolls Settings.
  - Lesson for the comparison: gpui's binding precedence (no context wins) differs from iced's widget-first key handling, and Kit's headless helpers can hide the difference.
- `scripts/check-host-gateway.sh` gains the spike crate's `src/` in `SCAN_DIRS` (passes).

## Open items

- **G4/G8/G9 cadence**: measure per-event latency and missed frames for scroll (G4) and resize (G9), and test whether G8's ~60 Hz is the spike's 16 ms replay batch (`console/replay.rs`) or gpui's frame scheduling.
- **Space in text inputs**: the context-free `space` → `Confirm` binding has the same precedence as the Enter finding, so typing a space in the search or palette input may trigger `activate` instead of inserting text. Not verified.
- **Keyboard `Select` and real-display focus sweep**: check that Enter/A opens Kit's `Select`, and walk every focus target on a real display, including the targets the headless tests do not assert.
- **Bench pointer-quiet check**: add a pre-run check for host pointer motion to the G4/G8 drivers, so a moving physical mouse reports `POINTER_BUSY` instead of `FOCUS_LOST`.
- **Runner G10 fixes to `main`**: upstream the `strip -o` and `flatpak info APP//BRANCH` fixes with tests.
- **Flatpak G1–G9**: the Flatpak was staged and smoke-tested, but the G-metrics were taken natively, as iced's were. Flatpak captures were not attempted: no acceptance item requires them, and M0-06 owns the cross-environment matrix.
- **zbus backend (F1)**: decide one zbus backend for the process before any gpui (or iced+ashpd) UI ships next to `crosshook-core`.
- **Physical gamepad**: drive the gilrs path with a real controller on E1/E2 (only unit-tested here).
- **IME/AccessKit**: M0-09 owns the XIM-under-gamescope and AccessKit probes; not exercised here.

## Persistence

Runtime-only. The spike reads fixture copies in an isolated XDG root. The form saves through `ProfileStore::save` to the fixture copy only. It adds no TOML settings keys and no SQLite schema.
