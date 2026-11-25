# C-gpui on E1

Runner output from the gpui decision spike (M0-05, [YAN-836](https://linear.app/itcn/issue/YAN-836)), copied from `<output>/C-gpui/E1/<Gx>.csv`. The code that produced them is on the `spike/gpui` branch only. The method, the comparison with iced C and Tauri B, and the open items are in [`../../../spikes/gpui.md`](../../../spikes/gpui.md).

| File     | Metric                                                                        | Run finished (`meta.json` `captured_at`) | Spike commit |
| -------- | ----------------------------------------------------------------------------- | ---------------------------------------- | ------------ |
| `G2.csv` | Warm start, 10 iterations, `--collect presentation`                           | 2026-11-25 21:50 UTC                     | `75aa6fc8`   |
| `G3.csv` | Idle PSS/USS/GPU, 600 s                                                       | see `G3.csv` note in the spike write-up  | `75aa6fc8`   |
| `G5.csv` | 5 scenarios × 20 trials × 2 iterations, `CROSSHOOK_BENCH_CLOCK_FLOOR_NS=1000` | 2026-11-25 21:55 UTC                     | `75aa6fc8`   |
| `G8.csv` | `replay_scroll`, 2 iterations                                                 | 2026-11-25 21:56 UTC                     | `75aa6fc8`   |
| `G9.csv` | Resize, 2 iterations                                                          | 2026-11-25 21:50 UTC                     | `75aa6fc8`   |

All runs: E1, 180.06 Hz, native release binary, fixtures seed 42 (500 profiles), clean tree, status `captured`. The raw run directories (shim logs, `provenance.json`, `SHA256SUMS`) are not committed; they stay in the local output directory, like every other capture's raw directory.

Not captured:

- **G4** failed every attempt (six runs, 2 and 4 iterations, every scenario list) with `FOCUS_LOST: pointer focus is not on the app window during the scroll`. The shim records a `wl_pointer.leave` about 1.5 s after the priming notch, with no new configure. G8, which uses the same placement, priming notch and 33 ms wheel cadence, captured twice, so the harness and session work. The leave comes from gpui on the G4 placement (1280×900 under the cursor at 640,450). It is unexplained within the time box (see the spike write-up).
- **G1** was not run (cold page cache before each launch).
- **G6, G7** cannot be produced on E1 (no producer, `perf` missing), as for every variant.
- **G10** needs a Flatpak bundle; the spike was not staged as a Flatpak within the time box.
