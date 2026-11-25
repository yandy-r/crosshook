# gpui decision spike (M0-05)

Spike for [YAN-836](https://linear.app/itcn/issue/YAN-836) / GitHub #650. It is the control spike for the ADR-0005 native UI framework decision (M0-07, YAN-860), on the same slice as the [iced spike](iced.md) (M0-04). The code is throwaway and lives only on the [`spike/gpui`](https://github.com/yandy-r/crosshook/tree/spike/gpui) branch, as crate `src/crosshook-native/crates/crosshook-ui-spike-gpui`. It is never merged to `main`. This file and the E1 results under [`../bench/C-gpui/E1/`](../bench/C-gpui/E1/) are the only parts on `main`.

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
  - Console replay at 5,000 lines/s: p95 17.6 ms, but p50 16.7 ms: gpui presents at about 60 Hz during the replay.
  - Resize p50 33.3 ms, worse than iced (5.56 ms).
  - Warm start p50 261 ms to READY.
- Memory is bounded: the 50k-line replay grows PSS by 2.5 MB, peak 229 MB.
- G4 scroll pacing is gpui's weakest metric. All three scenarios (`library`, `settings`, `console`) present every 6th refresh interval (p50 33.3 ms, p95 44.4 ms) while scrolling. iced presents every interval (5.56 ms) on `library` and `settings`.
- The earlier G4 `FOCUS_LOST` failures were not gpui's fault: real pointer motion from a physical input device moved the cursor off the window (see [Bench harness findings](#bench-harness-findings)).
- `cargo tree -i gtk` and `-i glib-sys` are empty.
- `crosshook-core` is called only from `core_bridge`, on the tokio runtime. This is enforced by a test and a runtime assertion.

## Pinned revision and build

Build figures are local observations on E1 (i9-14900HX, 32 threads; dependency sources already fetched). No build logs are committed.

| Item                                                                    | Value                                                                                                                                                                                                                                                                                                                                        |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| gpui                                                                    | `gpui-pre =0.3.8` (crates.io snapshot of zed@279fe07, 2026-10-05) plus `gpui-pre-platform =0.3.8` (`wayland`, `x11`, `font-kit`), `gpui-pre-util =0.3.8`; gpui-pre pulls the other `gpui-pre-*` at `=0.3.8` itself (wgpu 29.0.4 renderer)                                                                                                  |
| GPUI Kit                                                                | `gpui-kit =0.7.1` (default features `component`, `assets`), plus `gpui-base`, `gpui-component`, `gpui-component-macros`, `gpui-kit-assets` all `=0.7.1` (see F2)                                                                                                                                                                             |
| gpui_tokio                                                              | Vendored from zed@279fe07 `crates/gpui_tokio` (104 lines, Apache-2.0, `vendor/gpui_tokio/{LICENSE-APACHE,NOTICE.md}`), compiled as a module; only `init_from_handle` is used                                                                                                                                                                  |
| MSRV                                                                    | None declared: no `rust-version` in gpui-pre, gpui-kit, gpui-component or gpui-pre-wgpu (all edition 2024). Builds on the repo toolchain 1.95.0                                                                                                                                                                                              |
| Other deps                                                              | `gilrs 0.11`, `lru 0.16`, `image 0.25` (jpeg, png), `tokio`, `futures`, `crosshook-core`                                                                                                                                                                                                                                                     |
| Lockfile churn                                                          | 474 new lockfile entries; 23 existing ones moved (`futures*` 0.3.32, `wasm-bindgen*` 0.2.117, `js-sys`/`web-sys` 0.3.94, `zvariant*`, `zbus_names`, `toml` 0.8.2, `bitflags`, `proc-macro-crate`)                                                                                                                                           |
| Unique crates (`cargo tree -e normal,build`, host target, name+version) | 618 (iced: 436)                                                                                                                                                                                                                                                                                                                              |
| Clean release build                                                     | 94.5 s (iced: 63 s)                                                                                                                                                                                                                                                                                                                          |
| Incremental release build (one file touched)                            | 4.6 s (iced: 4.9 s)                                                                                                                                                                                                                                                                                                                          |
| Clean / incremental dev build                                           | 50.8 s / 2.0 s                                                                                                                                                                                                                                                                                                                               |
| Release binary                                                          | 57.1 MB; 44.1 MB stripped (iced: 32.2 / 25.0 MB). The debug binary is 789 MB                                                                                                                                                                                                                                                                  |
| glibc                                                                   | The host build needs `GLIBC_2.44`. Built inside `org.gnome.Sdk//50` (repo toolchain 1.95.0 mounted into the SDK, separate target dir), it needs `GLIBC_2.39`, as for iced. The SDK release build took 118 s                                                                                                                                 |
| Flatpak (G10)                                                           | Bundle 10.3 MB, installed 42.2 MB, stripped binary 42.1 MB (SDK build). iced's Flatpak was a local branch only, with no G10                                                                                                                                                                                                                   |

## Effort

About 2 hours of wall time with an AI pair on 2026-11-25, out of a ~3-day (5-point) box:

- First pass, 15:57–17:15 EST: the code, the tests, the zbus panic (F1) and G2/G3/G5/G8/G9.
- Second pass, 17:14–17:58: G1, Flatpak staging, the portal smoke test, G10, the G4 pointer trace, the palette fix and the G4 re-capture.

The time box was never the limiting factor. Each item still open below says why it is open.

## Code size

`wc -l` on the spike branch (3,610 lines of Rust, including 189 lines of `#[gpui::test]` tests; plus 104 vendored lines of gpui_tokio):

| Area                                                                       | Lines | Files                                                 | iced spike |
| -------------------------------------------------------------------------- | ----- | ----------------------------------------------------- | ---------- |
| Focus layer (targets, pure engine, bounds registry + ring wrapper, routing) | 589   | `src/focus/*.rs`, `src/app/nav.rs`                    | 607        |
| Gamepad (gilrs thread, mapping) + action/keymap layer                      | 183   | `src/gamepad.rs`, `src/actions.rs`                    | 135        |
| Library grid and card view (on Kit `v_virtual_list`)                       | 221   | `src/library/grid.rs`, `src/app/library.rs`           | 226        |
| Cover decode + byte LRU (`RenderImage`, `drop_image` on evict)             | 167   | `src/library/covers.rs`                               | 149        |
| Log console (ring, paced replay, view on `uniform_list`)                   | 317   | `src/console/*.rs`                                    | 414        |
| Modal and palette (Kit `Dialog` bodies)                                    | ~55   | `src/app/view.rs` (`modal_body`, `palette_body`)      | 43         |
| Core boundary (`core_bridge`, incl. its tests)                             | 200   | `src/core_bridge.rs`                                  | —          |
| Renderer override + adapter log                                            | 172   | `src/renderer.rs`                                     | 83         |
| Bench protocol and probe                                                   | 405   | `src/bench.rs`, `src/app/bench_flow.rs`               | 408        |

The line counts are close to iced's, but they buy different things:

- **Focus.** About half of the 589 lines is the iced spike's pure engine, ported unchanged. The rest does not reimplement focus. GPUI already has real focus (`FocusHandle`, `focus_next`, Tab order) and Kit already traps Tab in dialogs. The rest covers:
  - a per-frame bounds registry (a `canvas` probe in each wrapper);
  - mapping gpui focus back to targets, so the engine follows Tab and clicks;
  - routing D-pad, zone and Confirm actions.

  The iced spike had to make buttons focusable through a wrapper widget; gpui needs none of that.
- **Log widget.** The iced spike's culled console needed a custom `Widget` (190 lines). Here it is a `uniform_list` closure (100 lines including follow/scroll handling).
- **Pickers.** The `Select` opens and is navigable from the keyboard natively. iced's `pick_list` had no keyboard API, so A cycled the value there.

## Acceptance criteria

| Criterion | Result | Evidence |
| --------- | ------ | -------- |
| Time box respected, actual hours recorded | **Pass** | About 2 h of a ~3-day box ([Effort](#effort)) |
| Same metrics as M0-04 on E1 under `docs/internal/bench/C-gpui/E1/` | **Pass (everything producible on E1)** | G1 (5 cold launches), G2, G3, G4 (`library`, `console`, `settings`, 4 iterations), G5, G8, G9 and G10 captured. G6/G7 cannot be produced on E1: there is no producer and `perf` is missing, as for iced and Tauri B |
| Focus coverage (share of slice widgets reachable by D-pad and keyboard) | **Pass: 100 %, in headless tests** | Targets cover every interactive widget of the slice: 3 nav, back/forward, search, every card and its favourite, 15 form fields, 2 inspector actions, 2 modal buttons, palette. The `#[gpui::test]` suite drives real keystrokes through the real keymap and Kit Root. F6/Shift-F6 cycle zones with memory; arrows walk the library grid (cards culled by the list stay reachable); Tab moves gpui focus and the engine follows; Enter activates; Alt-←/→ walk history. A D-pad sweep reaches all 15 settings fields. Gamepad buttons map to the same actions (unit-tested; no physical pad driven on E1) |
| G6 stalls from core work | **Not measured** | No G6 producer on E1. By construction no core call can run on the foreground executor (next row) |
| `cargo tree -p … -i gtk` / `-i glib-sys` return nothing | **Pass** | Both `error: package ID specification … did not match any packages` (exit 101) |
| No `crosshook-core` call on the gpui foreground executor | **Pass** | See [Core boundary](#core-boundary) |
| Renderer override + adapter log | **Partial** | See [Renderer override](#renderer-override) |
| Evidence: ashpd portal dialog, Lucide icons, back/forward estimate | **Pass** | In the `spike-gpui` Flatpak, `SPIKE_SMOKE_PICK=1` made one `FileChooser.OpenFile` call and the KDE portal "Open File" window appeared; closing it returned `Response` code 1 and `SPIKE_PICK_RESULT No file picked`. See [Evidence items](#evidence-items) |
| Flatpak staging (`--branch=spike-gpui`, `app_id`) | **Pass** | Built in `org.gnome.Sdk//50` (`GLIBC_2.39`), installed as user branch `spike-gpui` next to `master`, `spike-iced` and `yan795-variant-a`. Those three are untouched (same commits before and after); `flatpak install` made `spike-gpui` the current branch, so the previously current `yan795-variant-a` was made current again. The default renderer reaches READY in the sandbox (Vulkan, NVIDIA, 1.6 s wall to exit-after-READY). The window's Wayland app id is `dev.crosshook.CrossHook`: KWin reports `resourceClass` `dev.crosshook.CrossHook` |
| This report | **Done** | — |

## Core boundary

- `src/core_bridge.rs` is the only file that names `crosshook_core`. The unit test `core_calls_only_here` scans `src/` and fails on any other occurrence.
- Every core function (`load_summaries`, `load_gamescope`, `save_gamescope`, `paths`) starts with `assert_off_ui_thread()`. That function panics if it runs on the thread `main` marked before `Application::run`, the gpui foreground thread. The test `guard_panics_on_ui_thread` checks the guard.
- Calls go through `core_bridge::blocking(cx, f)`: `Tokio::spawn` (vendored gpui_tokio) → `tokio::task::spawn_blocking(f)` on the app-owned runtime (2 workers, `spike-tokio-*` threads). `init_from_handle` gives gpui_tokio that runtime. The result comes back to gpui as a `Task` that the shell awaits in `cx.spawn_in`; only the result's state update runs on the foreground.
- Cover decoding (pure `image` work, no core) runs on gpui's background executor (`cx.background_spawn`).

## E1 results

E1, 2026-11-25, 3840×2160 @ 180.06 Hz (KWin 6.8, NVIDIA RTX 5070 Laptop, driver 620.73.04), `--fixtures` seed 42 (500 profiles). Every committed run exited 0 and ended `captured`. All but G4 ran on a clean tree; G4's `meta.json` says `dirty: true` because only this write-up and the E1 README were being edited, and no source file was modified. CSVs are in [`../bench/C-gpui/E1/`](../bench/C-gpui/E1/), and each run's commit is listed in that directory's README.

- G1–G3, G5, G8 and G9 use the native release binary built from `75aa6fc8` (G1 ran at `f4fec19c`, same app code).
- G4 uses the native binary from `f6c12fe7`, which adds only the palette Enter fix. Nothing it changes runs during G2, G3, G5, G8 or G9, which never open the palette.
- G10 uses the SDK-built binary and its bundle.

iced C numbers are from [`iced.md`](iced.md); Tauri B numbers are the E1 baseline B evidence in [`../bench/README.md`](../bench/README.md) (native, 2026-11-16, captured as evidence, not as a gate).

| Metric                                              | gpui C (this spike)                     | iced C                        | Tauri B (native, README)               |
| --------------------------------------------------- | --------------------------------------- | ----------------------------- | -------------------------------------- |
| G1 cold start to READY p50 / p95 (5 launches)       | 939 / 963 ms                            | not captured                  | not captured                           |
| G1 first presented frame p50 / p95                  | 936 / 951 ms                            | —                             | —                                      |
| G2 warm start to READY p50 / p95                    | 261 / 410 ms                            | 281 / 333 ms                  | 1038.1 / 1100.5 ms                     |
| G2 first presented frame p50 / p95                  | 258 / 394 ms                            | 220 / 293 ms                  | 245.8 / 316.3 ms                       |
| G3 idle PSS (start → after 10 min)                  | 210 → 210 MB, flat over 600 s; GPU 86 MB | 242 → 254 MB                  | —                                      |
| G4 `library` frame p50 / p95                        | 33.3 / 44.4 ms (741 frames)             | 5.56 / 5.56 ms                | 211 / 418 ms                           |
| G4 `settings` frame p50 / p95                       | 33.3 / 44.4 ms (725 frames)             | 5.56 / 5.56 ms                | 117 / 344 ms                           |
| G4 `console` frame p50 / p95 (scroll during replay) | 33.3 / 44.4 ms (741 frames)             | 33.3 / 44.4 ms                | 61.2 / 128.4 ms                        |
| G5 `search` p50 / p95                               | 11.7 / 14.1 ms                          | 14.7 / 17.1 ms                | 184.3 / 244.0 ms                       |
| G5 `toggle_favorite` p50 / p95                      | 13.5 / 17.4 ms                          | 15.2 / 17.4 ms                | 34.9 / 48.9 ms                         |
| G5 `toggle_optimization` p50 / p95                  | 13.6 / 18.0 ms                          | 15.8 / 17.3 ms                | 130.8 / 196.8 ms                       |
| G5 `dpad` p50 / p95                                 | 16.2 / 18.6 ms                          | 14.0 / 18.1 ms                | 57.3 / 80.1 ms                         |
| G5 `route_switch` p50 / p95                         | 13.7 / 17.9 ms                          | 16.4 / 18.3 ms                | 155.0 / 187.6 ms                       |
| G8 replay frame p50 / p95 / p99                     | 16.7 / 17.6 / 22.3 ms (1,236 frames)    | 11.1 / 16.7 / 16.7 ms (1,822) | 347.1 / 1009.4 / 1269.0 ms (46 frames) |
| G8 memory growth / peak PSS                         | 2.5 / 229 MB                            | 2.3 / 270 MB                  | 993.8 / 3119.1 MB                      |
| G9 resize frame p50 / p95                           | 33.3 / 38.9 ms                          | 5.56 / 33.3 ms                | 33 / 39 ms                             |
| G10 bundle / installed / stripped binary            | 10.3 / 42.2 / 42.1 MB (runtime delta 0) | not captured                  | not captured                           |

Notes:

- READY means the same thing as in the iced spike: summaries loaded, then two frames gpui drew (`Window::on_next_frame` scheduled from render). It is not a presentation confirmation. The like-for-like start-up figure is `first_present_ms`. G2 iteration 1 is a cold-ish outlier (502 ms); the p50 is the better comparison.
- **G8 at ~60 Hz.** gpui presented 1,236 frames over the 10 s replay, against iced's 1,822 at 180 Hz. Frame p50 16.7 ms is three refresh intervals: gpui is v-synced but throttles to about 60 fps while the console appends and scrolls. iced kept a p50 of two intervals. Why gpui paces at 60 Hz here was not traced (frame scheduling, wgpu present mode or the batch cadence of 16 ms).
- **G9 resize p50 33.3 ms** (6 intervals) against iced 5.56 ms: during KWin resize steps gpui presents at about 30 Hz. Tauri B matches gpui here.
- **G1** is five separate runner invocations of `--metric G1 --collect presentation --cold-cache-ack --iterations 1`.
  - Before each, the operator (this spike, on E1) ran `sync && echo 3 | sudo tee /proc/sys/vm/drop_caches` (2026-11-25 17:14:44–17:15:08 EST, all exit 0).
  - `G1.csv` merges the five rows and recomputes the summary row with the runner's own `bench_capture.stats` percentiles. The bench README documents no other merge convention, and no earlier G1 capture exists.
  - Cold start costs about 680 ms more than warm (p50 939 against 261 ms). The first presented frame tracks READY within about 3 ms: gpui presents nothing until its first full frame.
- **G10** used `--runtime-delta-mb 0`, because the GNOME 50 runtime is already installed, as specified for iced in YAN-2444. It also needed two runner fixes on this branch (see [Bench harness findings](#bench-harness-findings)). The stripped binary is 42.1 MB, against 44.1 MB for the host build: the SDK's older toolchain libraries link less.
- **G4 scrolls at about 30 Hz.** On all three pages frame p50 is 33.3 ms (6 refresh intervals), p95 44.4 ms, and 97–99 % of intervals exceed 1.5 periods.
  - The wheel notches come every 33 ms, so gpui presents about one frame per notch instead of animating at the panel rate.
  - Same 30 Hz class as G9 resize, and below the ~60 Hz of the G8 replay. iced scrolled `library` and `settings` at one interval (5.56 ms).
  - This is the clearest frame-pacing gap between the two toolkits on E1. Not traced: it is a cadence question for gpui's frame scheduling, and no acceptance item asks for it (open item).
  - G4 ran on the binary with the palette fix (`f6c12fe7`). An earlier G4 run without it is superseded: its `settings` row (500 ms p50) measured the library page under a still-open palette ([Bench harness findings](#bench-harness-findings)).
- G5 used `CROSSHOOK_BENCH_CLOCK_FLOOR_NS=1000`, as for iced (exact CLOCK_MONOTONIC µs marks).
- `gpu_growth_mb` 0.0 is from DRM fdinfo (`gpu_source=fdinfo`), not `nvidia-smi` as for iced.

## Renderer override

gpui 0.3.8 on Linux always renders through wgpu (29.0.4). The relevant behaviour:

- `WgpuContext::instance` builds the instance with a fixed `Backends::VULKAN | Backends::GL`. It does **not** read `WGPU_BACKEND`, so the iced-style override has no equivalent.
- Adapters are sorted by `ZED_DEVICE_ID` match, then compositor GPU, then device type (discrete > integrated > … > CPU), then backend (Vulkan before GL).
- Each adapter is tried by configuring the real surface; the first that works wins.

The spike's `--renderer vulkan|gl|lavapipe|llvmpipe-gl` therefore steers the loaders: `VK_ICD_FILENAMES`, `__EGL_VENDOR_LIBRARY_FILENAMES`, `LIBGL_ALWAYS_SOFTWARE`. `--device 0xNNNN` sets `ZED_DEVICE_ID`, and an explicit env value wins. The adapter is logged at startup as `SPIKE_RENDERER backend=… adapter=… software=… driver=… driver_info=…`, from gpui's "Selected GPU adapter" log line plus `Window::gpu_specs()`.

Native E1, one READY run per mode (each `SPIKE_RENDERER` line is quoted from the app's stderr):

| Mode (env it sets)                                                                                                 | Result                                                                                                                                                                                                                                                                                     |
| ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| default / `--renderer vulkan`                                                                                      | `backend=Vulkan adapter="NVIDIA GeForce RTX 5070 Laptop GPU" driver_info="620.73.04"`, READY in 0.86 s. Mesa radv is probed first and warns "not a conformant Vulkan implementation"                                                                                                         |
| `--device 0x7550` (`ZED_DEVICE_ID`, AMD Navi 48 dGPU)                                                              | `backend=Vulkan adapter="AMD Radeon RX 9070 XT (RADV GFX1201)"`, READY. The device filter works                                                                                                                                                                                             |
| `--renderer gl` (`VK_ICD_FILENAMES=/dev/null`, NVIDIA EGL)                                                         | **Fails**: `Adapter NVIDIA GeForce RTX 5070 Laptop GPU/PCIe/SSE2 (Gl) failed: no compatible surface formats` → `No GPU adapter found that can configure the display surface`. No window opens; `open_window` returns `Err` and `cx.quit()` is called, yet the process stayed alive until the 30 s timeout killed it |
| `--renderer lavapipe` (`VK_ICD_FILENAMES=lvp_icd.x86_64.json`)                                                     | `backend=Vulkan adapter="llvmpipe (LLVM 23.1.5, 256 bits)" software=true`, READY in 0.44 s. NVIDIA's GL adapter is tried first and rejected with the same surface-format error                                                                                                              |
| `--renderer llvmpipe-gl` (Vulkan hidden, `__EGL_VENDOR_LIBRARY_FILENAMES=50_mesa.json`, `LIBGL_ALWAYS_SOFTWARE=1`) | `backend=Gl adapter="llvmpipe (LLVM 23.1.5, 256 bits)" software=true driver_info="4.6 (Core Profile) Mesa 26.2.7"`, READY in 0.39 s. gpui's GL path runs on llvmpipe's GL 4.6; the `zed#50996` storage-block limit is not hit there                                                         |

NVIDIA's EGL exposes a GL adapter but no surface format gpui accepts. That is the same failure class iced hit (`GraphicsAdapterNotFound` through NVIDIA's EGL), surfacing differently. With Vulkan hidden, only a CPU adapter works: lavapipe, or llvmpipe through the Mesa EGL vendor. In the `gl` row the process also kept running after `open_window` failed until the 30 s timeout. The quit had to come from the app, so the broken-GPU path needs explicit handling.

`zed#50996` ("Zed crashes on non-Vulkan GPU: Too many vertex shader storage blocks") is open upstream. gpui's GL path uses storage buffers (`shaders_storage.wgsl`), and drivers that cap vertex-stage storage blocks fail there. The broken-GPU fallback (M5-03) cannot rely on wgpu-GL as the safety net. gpui-pre 0.3.8 has no tiny-skia/CPU renderer; its only software path is a CPU Vulkan or GL adapter (lavapipe or llvmpipe).

## Evidence items

- **File dialog.** gpui's own `cx.prompt_for_paths` goes through `ashpd::desktop::file_chooser::OpenFileRequest` in gpui-pre-linux. No rfd and no GTK: `cargo tree -i gtk` and `-i glib-sys` are empty. `SPIKE_SMOKE_PICK=1` opens the chooser at startup and prints `SPIKE_PICK_RESULT …`.

  **Flatpak smoke test**, `flatpak run --branch=spike-gpui` with `SPIKE_SMOKE_PICK=1`, isolated launcher `HOME`, `--nofilesystem=host:reset`:
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

- `gpui-pre` is a single-maintainer, unofficial crates.io snapshot of Zed's in-tree `gpui`, republished weekly. It went from 0.3.8 (2026-10-05, zed@279fe07) to 0.3.14 (2026-11-23, zed@f126950): 7 releases in 7 weeks. Each release pins its own `gpui-pre-*` family exactly, so any bump moves the whole family.
- GPUI Kit (Longbridge) went 0.7.1 → 0.11.0 in the same period: 10 releases, 4 minor bumps.
- The spike stays on the issue's pins. A bump to 0.3.14 / 0.11.0 was not attempted: the brief made it optional, and the pinned stack had to be measured first. What the bump breaks is an open item.
- Upstream Zed does not publish `gpui` to crates.io. Depending on gpui means choosing between this snapshot channel, a git pin of the Zed monorepo, or a fork. Every one of these requires an owner on our side.

## Licences

- gpui and the `gpui-pre-*` crates: Apache-2.0.
- gpui_tokio (vendored): Apache-2.0; the Zed `LICENSE-APACHE` is copied and attribution kept.
- GPUI Kit (`gpui-kit`, `gpui-base`, `gpui-component`, `gpui-kit-assets`): Apache-2.0.
- Lucide icons: ISC (`LICENSE-LUCIDE` in gpui-kit-assets).
- No GPL in the spike's own additions. `cargo deny` was not run on the spike branch. It is not one of this spike's acceptance items, and the repo's `deny.toml` does not cover a throwaway crate (open item).

## Bench harness findings

- The bench shim's `wl_proxy_add_dispatcher` hook and `dlsym` redirect from the iced spike (cherry-picked) work unchanged for gpui's wayland-rs client: the K/M/G records appear.
- The runner needs `WAYLAND_DISPLAY` in its environment. When launched from a shell without it, gpui fell back to X11/headless and READY never came (`READY_TIMEOUT`). This was a local operator error, not a harness bug.
- **G4 `FOCUS_LOST` and the `wl_pointer.leave` (resolved).** Six G4 runs in the first pass and five in the second stopped with `FOCUS_LOST: pointer focus is not on the app window during the scroll`. Tracing with `WAYLAND_DEBUG=client` on the app, in a probe that replays the G4 `library` drive:
  - Real motion events: `wl_pointer.motion` walked the cursor from (640, 420) to the left edge (x≈0) and to the top edge, then `wl_pointer.leave`. The runner's uinput device only emits wheel notches and never moves the pointer.
  - No new surfaces: the client created exactly two `wl_surface`s (the main surface and the cursor surface) and no `xdg_popup` or subsurface, so neither Kit nor gpui opened a tooltip or hover layer.
  - Not app-driven: the app made no pointer-constraint or warp request.
  - Control run, same placement, nothing injected: 54, then 318 and 149 `wl_pointer.motion` events and 1–2 leaves in 12 s, so the cursor moved without any bench input.
  - The source is a physical pointer device on E1 (Logitech USB receiver or the Keychron K4 HE's mouse interface) sending motion while the operator was away. That matches the "hands off the keyboard and mouse" prerequisite.
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

- **G4/G8/G9 cadence**: find why gpui presents at about 30 Hz while scrolling and resizing, and about 60 Hz during console replay, on a 180 Hz panel (iced: one interval on library scroll).
- **Bench pointer-quiet check**: add a pre-run check for host pointer motion to the G4/G8 drivers, so a moving physical mouse reports `POINTER_BUSY` instead of `FOCUS_LOST`.
- **Runner G10 fixes to `main`**: upstream the `strip -o` and `flatpak info APP//BRANCH` fixes with tests.
- **Flatpak G1–G9**: the Flatpak was staged and smoke-tested, but the G-metrics were taken natively, as iced's were. Flatpak captures were not attempted: no acceptance item requires them, and M0-06 owns the cross-environment matrix.
- **zbus backend (F1)**: decide one zbus backend for the process before any gpui (or iced+ashpd) UI ships next to `crosshook-core`.
- **Pin bump**: measure what moving to gpui-pre 0.3.14 / gpui-kit 0.11.0 breaks, as a churn sample.
- **Physical gamepad**: drive the gilrs path with a real controller on E1/E2 (only unit-tested here).
- **`cargo deny`**: run `cargo deny check` on the spike branch to list licence and advisory findings for the gpui tree.
- **IME/AccessKit**: M0-09 owns the XIM-under-gamescope and AccessKit probes; not exercised here.

## Persistence

Runtime-only. The spike reads fixture copies in an isolated XDG root. The form saves through `ProfileStore::save` to the fixture copy only. It adds no TOML settings keys and no SQLite schema.
