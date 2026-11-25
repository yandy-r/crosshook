# C-iced on E1

Runner output from the iced decision spike (M0-04, [YAN-773](https://linear.app/itcn/issue/YAN-773)), copied from `<output>/C-iced/E1/<Gx>.csv`. The code that produced them is on the `spike/iced` branch only. The method, the comparison with Tauri B and the harness changes these runs needed are in [`../../../spikes/iced.md`](../../../spikes/iced.md).

| File | Metric | Run | Spike commit |
| --- | --- | --- | --- |
| `G2.csv` | Warm start, 10 iterations, `--collect presentation` | 2026-11-25 | `f8df9ccb` |
| `G3.csv` | Idle PSS/USS/GPU, 600 s | 2026-11-25 01:18 UTC | `ddc84705` |
| `G4.csv` | `library`, `console`, `settings`, 2 iterations | 2026-11-25 01:09 UTC | `689e4935` |
| `G5.csv` | 5 scenarios × 20 trials × 2 iterations, `CROSSHOOK_BENCH_CLOCK_FLOOR_NS=1000` | 2026-11-25 01:16 UTC | `ddc84705` |
| `G8.csv` | `replay_scroll`, 2 iterations | 2026-11-25 01:17 UTC | `ddc84705` |
| `G9.csv` | Resize, 2 iterations | 2026-11-25 01:04 UTC | `4997480a` |

All runs: E1, 180.06 Hz, native release binary, fixtures seed 42, clean tree, status `captured`. The raw run directories (shim logs, `provenance.json`, `SHA256SUMS`) are not committed. They stay in the local output directory, like every other capture's raw directory.

G2 ran on `f8df9ccb`, before the shim and geometry fixes. Those changes touch only input drivers and the console height, not startup.
