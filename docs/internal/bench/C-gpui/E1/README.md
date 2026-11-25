# C-gpui on E1

Runner output from the gpui decision spike (M0-05, [YAN-836](https://linear.app/itcn/issue/YAN-836)), copied from `<output>/C-gpui/E1/<Gx>.csv`. The code that produced them is on the `spike/gpui` branch only. The method, the comparison with iced C and Tauri B, and the open items are in [`../../../spikes/gpui.md`](../../../spikes/gpui.md).

| File      | Metric                                                                                                             | Run started (`meta.json` `captured_at`)          | Spike commit |
| --------- | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------ | ------------ |
| `G1.csv`  | Cold start: 5 runs of `--iterations 1 --cold-cache-ack`, page cache dropped before each, merged (see below)       | 2026-11-25 22:14–22:15 UTC                       | `f4fec19c`   |
| `G2.csv`  | Warm start, 10 iterations, `--collect presentation`                                                                | 2026-11-25 21:50 UTC                             | `75aa6fc8`   |
| `G3.csv`  | Idle PSS/USS/GPU, 600 s                                                                                            | 2026-11-25 21:59 UTC                             | `75aa6fc8`   |
| `G4.csv`  | Scroll pacing: `library`, `console`, `settings`, 4 iterations, after a pointer-quiet idle control (see below)       | 2026-11-25 22:58 UTC                             | `caf4d25b`   |
| `G5.csv`  | 5 scenarios × 20 trials × 2 iterations, `CROSSHOOK_BENCH_CLOCK_FLOOR_NS=1000`                                      | 2026-11-25 21:55 UTC                             | `75aa6fc8`   |
| `G8.csv`  | `replay_scroll`, 2 iterations                                                                                      | 2026-11-25 21:56 UTC                             | `75aa6fc8`   |
| `G9.csv`  | Resize, 2 iterations                                                                                               | 2026-11-25 21:50 UTC                             | `75aa6fc8`   |
| `G10.csv` | `--branch spike-gpui`, SDK-built `--binary`, `--bundle` of the local Flatpak, `--runtime-delta-mb 0` (same method as the pending iced G10, Linear YAN-2444); total excludes the GNOME 50 runtime | 2026-11-25 22:22 UTC                             | `c4ca90e1`   |

All runs: E1, 180.06 Hz, fixtures seed 42 (500 profiles), clean tree (`dirty: false`), status `captured`. G2, G3, G5, G8 and G9 launched the native release binary built from `75aa6fc8`, and G1 the same app code at `f4fec19c`. G4 ran at `caf4d25b` (run `6de9a586`; docs only on top of `f6c12fe7`). The runner records the repo commit and the binary's sha256 (`289bcf1b…`), not the commit the binary was built from: that it was built from `f6c12fe7`, which adds the palette fix (see below), is the operator's statement. G10 used the SDK build of `c4ca90e1`, before the palette fix. The G4 CSV passes the runner's `verify_capture`. The raw run directories (shim logs, `provenance.json`, `SHA256SUMS`) are not committed. They stay in the local output directory, like every other capture's raw directory.

**G1 merge.** The runner never drops caches, so each cold launch is its own run: `sync && echo 3 | sudo tee /proc/sys/vm/drop_caches`, then one `--iterations 1` run. Every drop exited 0 (operator: this spike, host E1, 17:14:44–17:15:08 EST). `G1.csv` lists the five iteration rows and a summary row recomputed with the runner's own `bench_capture.stats` (the same percentile function the runner uses for multi-iteration G1/G2). The bench README has no other merge convention, and no earlier G1 capture exists.

**G10 runner fixes.** Two runner bugs blocked G10 on this host; both are fixed on `spike/gpui` only (commits `fix(bench): …`):

- `strip --output` is ambiguous in binutils 2.47 (`INSTRUMENT_FAILED: strip`). The runner now uses `-o`.
- `flatpak info --show-size --branch=…` does not exist (`INSTRUMENT_FAILED: flatpak`). The runner now passes the ref as `dev.crosshook.CrossHook//<branch>`.

`python3 -m unittest test_bench` passes (41 tests). These need a `main` PR before anyone else can produce G10.

**G4.** Earlier G4 attempts stopped with `FOCUS_LOST`: a physical pointer device on E1 moved the cursor off the window during the timed scroll (traced in the spike write-up). The capture ran right after an idle control (spike placed like G4, nothing injected, 12 s) saw 0 `wl_pointer.motion` events.

The palette bug: before `f6c12fe7`, Enter in the command palette did nothing, so the `settings` scenario scrolled the library page with the palette still open. A G4 run at `c4ca90e1` reported `settings` p50/p95 500.0/505.6 ms (44 frames), mostly the 500 ms cursor blink of the palette input. That run is superseded and not committed. An intermediate G4 run at `f6c12fe7` had the right numbers but a dirty tree (docs being edited), so it is not evidence and was replaced by the clean run. `settings` in `G4.csv` is on the settings page: 722 frames pooled over the 4 iterations' timed windows (about 180 each), with the same p50/p95 as `library`.

Not captured:

- **G6, G7** cannot be produced on E1 (no producer, `perf` missing), as for every variant.
