# iced decision spike (M0-04)

Spike for [YAN-773](https://linear.app/itcn/issue/YAN-773) / GitHub #642. It is evidence for the ADR-0005 native UI framework decision (M0-07, YAN-860). The code is throwaway and lives only on the [`spike/iced`](https://github.com/yandy-r/crosshook/tree/spike/iced) branch (head `3cd03955`), as crate `src/crosshook-native/crates/crosshook-ui-spike-iced`. It is never merged to `main`. This file and the E1 results under [`../bench/C-iced/E1/`](../bench/C-iced/E1/) are the only parts on `main`.

This is evidence, not a decision. ADR-0005 still needs the gpui control spike (M0-05), the AccessKit/IME probe (M0-09) and the hardware matrix (M0-06).

## Summary

- All items in the slice were built. No iced widget was forked.
- Focus works through wrapper widgets, as the brief asked:
  - Keyboard and gamepad reach every interactive widget in the slice.
  - Focus zones cycle and remember their last target.
  - The modal and the command palette trap focus.
- E1 frame times at 180.06 Hz (one refresh interval is 5.554 ms):
  - Library and settings scroll: p50 and p95 at one interval (p95 5.558 ms, within timestamp quantisation).
  - Console replay at 5,000 lines/s: p95 16.7 ms, about 3 intervals.
  - Resize: p50 one interval, p95 33.3 ms, about 6 intervals.
  - Keystroke-to-photon (input to presented change, not frame time): p95 at most 18.3 ms.
- Memory stays bounded:
  - The 50k-line replay grows memory by 2.3 MB, against 994 MB for Tauri B.
  - Idle PSS stays flat over 10 minutes.
- Renderer override works for wgpu-Vulkan and tiny-skia. wgpu-GL works only with the Mesa EGL vendor; on NVIDIA's EGL it finds no adapter (see below).
- `cargo tree -i gtk` and `-i glib-sys` are empty. The file dialog goes through `rfd` with only `xdg-portal`, via `ashpd`.
- Main costs:
  - About 600 lines of focus layer, because upstream only makes text inputs focusable.
  - A hand-written culled log widget.
  - At this revision a widget does not convert into `Element` implicitly, so every layout child needs `.boxed()`.
  - The bench presentation shim had to learn how wayland-rs registers its listeners.

## Pinned revision and build

Build figures below are local observations on E1 (no build logs are committed).

| Item                                                                    | Value                                                                                                                          |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| iced                                                                    | git `a8231fd9ddf3570f95affba2d45a9be092007f71` (master, 2026-10-07, `0.15.0-dev`)                                              |
| iced features                                                           | `tokio`, `image`, `svg`, `wgpu`, `tiny-skia`, `wayland`, `x11`, `crisp`, `thread-pool`, `sysinfo`, `advanced`                  |
| iced MSRV at this rev                                                   | 1.93 (repo toolchain 1.95.0)                                                                                                   |
| Other deps                                                              | `gilrs 0.11`, `rfd 0.16` (`xdg-portal`, `tokio`, `wayland`; no `gtk3`), `lru 0.16`, `image 0.25` (jpeg, png), `crosshook-core` |
| Lockfile churn                                                          | `web-sys`, `js-sys`, `wasm-bindgen`, `wasm-bindgen-futures` had to move up for iced to resolve next to `reqwest`               |
| Unique crates (`cargo tree -e normal,build`, host target, name+version) | 436                                                                                                                            |
| Clean release build                                                     | 63 s on E1 (i9-14900HX, 32 threads; dependency sources already fetched)                                                        |
| Incremental release build (one file touched)                            | 4.9 s                                                                                                                          |
| Release binary                                                          | 32.2 MB; 25.0 MB stripped                                                                                                      |

A host build on E1 links glibc 2.44 symbols (`sinh`, `cosh`), which the GNOME 50 runtime (glibc 2.42) does not have. Building inside `org.gnome.Sdk//50` brings the requirement down to `GLIBC_2.39`. The M1 build scripts must build the native UI inside the SDK (or vendor one), as M7-05 plans for the final runtime.

## Effort

About 3 hours of wall time with an AI pair, from an empty crate to every E1 capture. Writing the code took about 1 hour; most of the rest went into bench integration (the shim, a clock floor and two driver-geometry fixes, all below) and Flatpak packaging.

## Code size

`wc -l` on the spike branch at `3cd03955` (3,807 lines of Rust in total, including 151 lines of simulator tests):

| Area                                                                      | Lines | Files                                                       |
| ------------------------------------------------------------------------- | ----- | ----------------------------------------------------------- |
| Focus layer (targets, pure engine, tree collector, wrapper widget, tests) | 607   | `src/focus/*.rs`                                            |
| Library grid culling and card view                                        | 226   | `src/library/grid.rs`, `src/library/view.rs`                |
| Log console (ring, culled widget, paced replay)                           | 414   | `src/console/*.rs`                                          |
| Modal and scrim                                                           | 43    | `src/app/view.rs` (`modal`, `scrim`)                        |
| Gamepad (gilrs thread, mapping)                                           | 135   | `src/gamepad.rs`                                            |
| Bench protocol and probe                                                  | 408   | `src/bench.rs`, `src/app/bench_flow.rs`, `src/app/probe.rs` |

The focus layer is the custom-widget cost the ADR should weigh. It works as follows:

- A `Focus` wrapper forwards every `Widget` call to the wrapped widget unchanged. It adds an `operation.focusable(..)` report with its own `Id` and draws the focus ring.
- A tree `Operation` collects every target and its bounds in tree order.
- A pure engine handles Tab order, D-pad direction, zone cycling with memory and the overlay trap. The engine has no iced types except `Rectangle`.
- A (A / Enter) is routed by the app from the focused target. The wrapped button, checkbox, toggler or pick_list therefore keeps its own mouse behaviour, and nothing is forked.

## Acceptance criteria

| Criterion                                                                                                                   | Result                         | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------------------- | ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D-pad and keyboard reach every interactive widget, including modal trap and zone cycling, without forking iced core widgets | **Pass, with one caveat**      | 31 tests pass, 6 of them through the headless `iced_test` simulator: Tab visits every collected target in tree order and wraps; F6 cycles sidebar, content, inspector with memory; the modal traps Tab and Escape returns focus to the opener; D-pad reaches cards that culling removed from the widget tree; the wheel scrolls the console. No widget is forked. Caveat: the `pick_list` is reachable, but A cycles its value instead of opening its menu (pain point 4). |
| E1 library scroll p95 ≤ 1 refresh interval (G4)                                                                             | **Pass (within quantisation)** | `library` p95 5.558 ms against a 5.554 ms interval: 4 µs over, within the presentation timestamp jitter (p50 5.556 ms)                                                                                                                                                                                                                                                                                                                                                     |
| Console replay keeps memory bounded (G8 ≤ 15 MB growth)                                                                     | **Pass**                       | G8 `memory_growth_mb` 2.31, peak PSS 270 MB                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Zero UI-thread stalls over 16 ms from core work (G6)                                                                        | **Not measured**               | G6 has no producer on E1 (`perf` missing, kernel tracing root-only; see the bench README). By design every `crosshook-core` call runs through `Task::perform` + `spawn_blocking`. Frame data alone cannot prove the absence of core stalls: G4 `console` (p99 44.4 ms) and G9 (p95 33.3 ms) do contain long frames, whose cause was not traced.                                                                                                                            |
| Renderer override works for wgpu-Vulkan, wgpu-GL and tiny-skia; adapter logged                                              | **Partial**                    | See [Renderer override](#renderer-override): Vulkan and tiny-skia work natively and in the Flatpak; wgpu-GL finds no adapter through NVIDIA's EGL.                                                                                                                                                                                                                                                                                                                         |
| `cargo tree -i gtk` / `-i glib-sys` empty; portal file dialog inside the Flatpak                                            | **Partial**                    | Both trees are empty (the same check on `crosshook-native` finds `gtk v0.18.2`). The Flatpak smoke test of the dialog was blocked by a systemd user-manager fault on E1 (see [Open items](#open-items)).                                                                                                                                                                                                                                                                   |
| G1–G10 on E1 committed to `docs/internal/bench/C-iced/E1/`                                                                  | **Partial**                    | G2, G3, G4, G5, G8, G9 captured. G1 needs a manual cache drop (sudo), G6/G7 cannot be produced on E1, and G10 needs the Flatpak bundle.                                                                                                                                                                                                                                                                                                                                    |
| This report                                                                                                                 | **Done**                       | —                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

## E1 results

E1, 2026-11-24/25, 3840×2160 @ 180.06 Hz (KWin 6.8, NVIDIA RTX 5070 Laptop, driver 620.70.06, kernel 7.3.4), native release binary, `--fixtures` seed 42 (500 profiles). Every run exited 0 and ended `captured` with a clean tree. CSVs are in [`../bench/C-iced/E1/`](../bench/C-iced/E1/).

The Tauri B column is the E1 baseline B evidence already in [`../bench/README.md`](../bench/README.md) (native, same host, 2026-11-16). The G2 rows come from its First-content supplemental table (native, 10 iterations). That baseline was captured as evidence, not as a gate, and it was not re-run for this comparison.

| Metric                                               | iced C (this spike)                            | Tauri B (native, README)               |
| ---------------------------------------------------- | ---------------------------------------------- | -------------------------------------- |
| G2 warm start to READY p50 / p95                     | 281 / 333 ms                                   | 1038.1 / 1100.5 ms                     |
| G2 first presented frame p50 / p95                   | 220 / 293 ms                                   | 245.8 / 316.3 ms                       |
| G3 idle PSS (empty start → after 10 min)             | 242 → 254 MB, flat after 10 s                  | —                                      |
| G4 `library` frame p50 / p95                         | 5.56 / 5.56 ms                                 | 211 / 418 ms                           |
| G4 `settings` frame p50 / p95                        | 5.56 / 5.56 ms                                 | 117 / 344 ms                           |
| G4 `console` frame p50 / p95 (scroll during replay)  | 33.3 / 44.4 ms                                 | 61.2 / 128.4 ms                        |
| G5 `search` p50 / p95                                | 14.7 / 17.1 ms                                 | 184.3 / 244.0 ms                       |
| G5 `toggle_favorite` p50 / p95                       | 15.2 / 17.4 ms                                 | 34.9 / 48.9 ms                         |
| G5 `toggle_optimization` p50 / p95                   | 15.8 / 17.3 ms                                 | 130.8 / 196.8 ms                       |
| G5 `dpad` p50 / p95                                  | 14.0 / 18.1 ms                                 | 57.3 / 80.1 ms                         |
| G5 `route_switch` p50 / p95                          | 16.4 / 18.3 ms                                 | 155.0 / 187.6 ms                       |
| G8 replay frame p50 / p95 / p99                      | 11.1 / 16.7 / 16.7 ms (1,822 frames)           | 347.1 / 1009.4 / 1269.0 ms (46 frames) |
| G8 memory growth / peak PSS                          | 2.3 / 270 MB                                   | 993.8 / 3119.1 MB                      |
| G9 resize frame p50 / p95                            | 5.56 / 33.3 ms                                 | 33 / 39 ms                             |
| Idle CPU (20 s sample, window open, nothing moving)¹ | 1 tick (0.05 %)                                | —                                      |
| 30-min idle soak, NVIDIA, default renderer¹          | no crash (`timeout` exit 124, no panic output) | —                                      |

¹ Local observation on E1, not a runner capture; nothing committed. `/proc/<pid>/stat` utime+stime over 20 s, and `timeout 1800` on the release binary with stderr captured.

Notes:

- READY is not defined the same way on both sides. Classic prints READY after the profile summary fetch and a double `requestAnimationFrame`; the spike prints it after the summaries are loaded and two `RedrawRequested` events have been delivered (iced's redraw requests, not presentation confirmations). Neither READY gate observes compositor presentation, and they wait for different milestones, so the G2 READY gap is not a like-for-like gate figure. The only like-for-like start-up figure is `first_present_ms` (launch to the first presented toplevel frame, from the shim for both apps). `first_content_ms` is not: the runner picks the first frame presented at or after each app's own READY, so it inherits the READY difference. A shared content-ready milestone would be needed before it counts as framework-decision evidence.
- The console holds 10,000 lines (a ring) and lays out only the visible ones. Tauri B keeps all 50,000 as DOM rows. This explains most of the G8 gap, so a Classic ring buffer (M0-08) narrows it. The G8 `over_interval_percent` of 65.6 means two-thirds of frames took over 1.5 refresh periods (8.3 ms); p95 stays at three intervals.
- G5 used `CROSSHOOK_BENCH_CLOCK_FLOOR_NS=1000` (see below): the spike's `t_perf_us` is exact CLOCK_MONOTONIC microseconds, not a 1 ms-floored `performance.now()`.
- G3's `gpu_mb` of 50 is `nvidia-smi pmon` `fb`, which stayed flat.

## Renderer override

`--renderer wgpu-vulkan|wgpu-gl|tiny-skia` sets `ICED_BACKEND` and `WGPU_BACKEND`. An explicit environment value wins over the flag. The active backend and adapter are logged at startup as `SPIKE_RENDERER backend=… adapter=…`.

| Mode          | Native E1                                                                                                                                                   | Flatpak (GNOME 50)                         |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| default       | Vulkan, NVIDIA GeForce RTX 5070 Laptop GPU                                                                                                                  | Vulkan, NVIDIA GeForce RTX 5070 Laptop GPU |
| `wgpu-vulkan` | Vulkan, NVIDIA                                                                                                                                              | —                                          |
| `wgpu-gl`     | **fails**: `GraphicsAdapterNotFound` (wgpu found no GL adapter through NVIDIA's EGL); works with `__EGL_VENDOR_LIBRARY_FILENAMES=…/50_mesa.json` (llvmpipe) | —                                          |
| `tiny-skia`   | tiny-skia, CPU                                                                                                                                              | tiny-skia, CPU; READY then exit 0          |

The wgpu-GL failure is the NVIDIA GL path that M5-03 (broken-GPU fallback) must handle. iced's fallback renderer chooses tiny-skia when wgpu fails, but only if `ICED_BACKEND` does not pin `wgpu`. Vulkan probing also loads the Mesa `radv` ICD for the AMD iGPU and prints its "not a conformant Vulkan implementation" warning before choosing NVIDIA.

## Evidence items

- **File dialog.** `rfd::AsyncFileDialog` with only `xdg-portal` goes through `ashpd`. `cargo tree -i gtk` and `-i glib-sys` return "did not match any packages". A Flatpak build is installed as a local branch next to the real app: `flatpak run --branch=spike-iced dev.crosshook.CrossHook`, manifest in `/tmp`, not committed. `SPIKE_SMOKE_PICK=1` opens the chooser at startup, but this run could not confirm it (see [Open items](#open-items)).
- **Icons.** A Lucide subset of 8 SVGs (3.3 KB) is embedded with `include_str!` and rendered by iced's `svg` widget (resvg). `currentColor` is replaced at load, because resvg has no ambient text colour. The licence is ISC, kept as `assets/icons/LICENSE`. Lucide fits the native UI (M1-47): it is small, permissively licensed, and needs no icon font.
- **Back/forward.** `History<T>` is a `Vec` plus a cursor, 40 lines plus tests. Navigating truncates the forward branch. In the Elm model it is a plain value in app state, so in-app history (M1-49) costs about half a day including the toolbar and Alt+←/→.

## API pain points at `a8231fd`

1. **No implicit `Element` conversion.** `row![]`, `column![]` and `Column::push` are generic over a single child type `W`. Mixed children need `.boxed()` everywhere, and a `Column::new()` is typed by its first push (`expected Text, found Focus`). See `widget/src/column.rs`, `widget/src/helpers.rs:63`.
2. **Focus only for text.** Only `text_input` and `text_editor` implement `Focusable` (`widget/src/text_input.rs:475`, `core/src/text/editor.rs:765`), and `operation::focus_next` walks text inputs only. Buttons, checkboxes, togglers and pick_lists need the wrapper above.
3. **`Id` hides its string.** `widget::Id` has no public accessor, so the target collector reads its `Debug` form (`core/src/widget/id.rs`). A stable accessor or a typed `Id` payload would remove that hack.
4. **`pick_list` has no keyboard API.** It opens on click only (`widget/src/pick_list.rs`). The spike makes A cycle the value; an accessible dropdown needs its own overlay.
5. **No virtualized list.** The grid uses manual windowing with spacer rows. The console needed a custom `Widget` calling `fill_text` per visible line (`console/log_view.rs`). Both work, but every list-heavy page will need this.
6. **`window::frames()` is the only frame clock.** It is fine for READY ("two redraw requests") and the probe marks. It is a redraw-request signal, not a presentation timestamp.
7. **`Simulator::operate` takes `&mut dyn Operation<()>`.** Collecting data in a test needs a unit-typed adapter (`CollectUnit`).
8. **The upstream modal example has no focus trap.** The trap lives in the app's focus engine, which is where the native UI's focus layer (M1-15) would put it.

## Bench harness findings

These changes were needed so the existing harness could measure a wayland-rs app. They live on `spike/iced` only. Before the native UI's perf gate (M1-28) they need a proper `main` PR with tests (follow-up filed).

- **Focus witness blind to wayland-rs.** The shim wrapped `wl_proxy_add_listener`, but wayland-rs registers every proxy with `wl_proxy_add_dispatcher`, so no pointer or keyboard records appeared (`POINTER_NOT_OVER_WINDOW`). Also, wgpu-hal enables wayland-sys `dlopen`, which resolves symbols with `dlsym(handle, …)` and so bypasses `LD_PRELOAD`. The spike shim wraps `wl_proxy_add_dispatcher` (keeping the `dispatcher_data` ownership tag unchanged) and interposes `dlsym` for that one symbol name.
- **G5 clock floor.** `CLOCK_FLOOR_NS` is fixed at 1 ms (the browser `performance.now()` floor). A native app commits its change frame within ~1 ms of the key, so every trial fell inside ±1 ms of `render_ns` and was `ambiguous` (`CAPTURE_TOO_SHORT`). The spike reads `CROSSHOOK_BENCH_CLOCK_FLOOR_NS` (default unchanged).
- **Gateway scan.** `scripts/check-host-gateway.sh` gains the spike crate's `src/` in `SCAN_DIRS`, so the ADR-0001 check covers it.
- **Console geometry.** The G4 `console` and G8 cursor sit at y = 860 of a 1600×1000 window, which assumes Classic's console height. The spike console had to grow from 300 to 380 px to sit under it.

## Open items

- **Flatpak portal dialog and Flatpak bench runs.** The spike Flatpak built and installed. Default Vulkan and `--renderer tiny-skia` (READY, exit 0) both ran. Then the E1 systemd user manager stopped starting `app-flatpak-*.scope` units: jobs stayed `start waiting`, `systemd --user` used ~95 % of a core, and every later `flatpak run`, including `org.gnome.Sdk//50 true`, hung. The portal file-chooser smoke test and Flatpak G-metrics are therefore not recorded. Re-run after the session recovers: `SPIKE_SMOKE_PICK=1 flatpak run --branch=spike-iced dev.crosshook.CrossHook`.
- **G1** needs a cold page cache before each launch (`sync && echo 3 | sudo tee /proc/sys/vm/drop_caches`). The runner never uses sudo; owner step.
- **G10** needs a bundle; the spike Flatpak is a local branch only.
- **G6, G7** cannot be produced on E1 (no producer, `perf` missing).
- **E2/E3 and llvmpipe runs** belong to M0-06.

## Persistence

Runtime-only. The spike reads fixture copies in an isolated XDG root. The form saves through `ProfileStore::save` to the fixture copy only. It adds no TOML settings keys and no SQLite schema.
