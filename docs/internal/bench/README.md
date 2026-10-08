# CrossHook benchmark contract

Canonical environment and metric IDs for baseline A (current Tauri), tuned Tauri B, and native C comparisons. Values below define measurements; they do not claim any target has passed.

## Environments

| ID    | Hardware / session                                                                                    | Builds                                  |
| ----- | ----------------------------------------------------------------------------------------------------- | --------------------------------------- |
| E1    | Developer laptop: RTX 5070 Laptop GPU, NVIDIA 615.71.09, Wayland, CachyOS kernel 7.x, 120 Hz panel    | Native release binary and local Flatpak |
| E2-D  | Steam Deck (LCD; OLED when available), SteamOS stable, Desktop Mode (KWin Wayland)                    | Flatpak only                            |
| E2-G  | Same Deck, Game Mode (gamescope, XWayland), launched as non-Steam shortcut                            | Flatpak only                            |
| E3    | Mesa AMD or Intel Wayland desktop (RADV/ANV), isolating NVIDIA DMA-BUF issue                          | Native and Flatpak                      |
| E3-sw | E3 with Vulkan unavailable (`VK_ICD_FILENAMES=/dev/null`, `LIBGL_ALWAYS_SOFTWARE=1`, llvmpipe); M0-06 | Native and Flatpak                      |

Record exact host, driver, kernel, compositor, refresh rate, display scale and CPU governor for each capture in [`environments.md`](environments.md). Unavailable inventory stays marked unavailable; do not infer machine details from target definitions.

## Metrics

| ID  | Metric                                                                                                                                                                       | Unit / reported statistic                                                      |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| G1  | Cold start to `READY` and first frame painted                                                                                                                                | ms, p50/p95                                                                    |
| G2  | Warm start to `READY`                                                                                                                                                        | ms, p50/p95                                                                    |
| G3  | Idle PSS + USS for whole process tree (including WebKitWebProcess/NetworkProcess), empty and 500-profile states after visiting all routes; growth over 10 min and GPU memory | MB                                                                             |
| G4  | Scroll frame time/pacing: library grid, 50k-line console, longest settings page                                                                                              | p50/p95/p99 ms, share over one refresh interval, frame-time standard deviation |
| G5  | Keystroke-to-photon latency: library search, launch-optimization toggle, favorite toggle, D-pad step, route switch                                                           | ms, p95                                                                        |
| G6  | UI-thread stalls over 16 ms and over 50 ms during scripted 5-minute session                                                                                                  | count                                                                          |
| G7  | Idle CPU, wakeups/s, Deck battery discharge power measured by `upower` for 10 min, Game Mode off and on                                                                      | %, /s, W                                                                       |
| G8  | Replay 50k log lines at 5,000 lines/s while scrolling; frame p95 and memory growth                                                                                           | ms, MB                                                                         |
| G9  | Resize frame time p95 over 5 s drag                                                                                                                                          | ms                                                                             |
| G10 | `.flatpak` bundle, `flatpak info` installed size, stripped binary, runtime download delta                                                                                    | MB                                                                             |

## Fixture and app contract

Fixture generation command:

```sh
cargo run --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --example bench_fixtures -- --seed 42 --out DIR [--empty]
```

Materialize canonical fixtures into an isolated launcher root with:

```sh
-- materialize --from canonical --root isolated
```

Canonical tree includes `DIR/config/crosshook/profiles` and settings, `DIR/data/crosshook/metadata.db`, cache/images portraits, `DIR/logs/launch-50k.log`, and a manifest. Portable paths use `@CROSSHOOK_BENCH_ROOT@`. Same-seed canonical contents must compare equal; materialized paths intentionally depend on root. Seeded timestamps/UUIDs are normalized through public SQL API in the example; production schema and user data remain unchanged. Cover fixture is one generated 600×900 JPEG copied 500 times (identical-pixel limitation).

Fixtures and test state are runtime-only artifacts in isolated roots. No TOML user settings or SQLite user metadata changes are permitted. No real game/trainer launch. Production behavior is untouched unless `CROSSHOOK_BENCH=1`; when unset, benchmark instrumentation does one no-op IPC call.

For readiness, app prints `READY <CLOCK_MONOTONIC ns>` after summary fetch and double `requestAnimationFrame`, including empty state. This does not promise cover decode/scan completion. `CROSSHOOK_BENCH_EXIT_AFTER_READY=1` exits through `AppHandle.exit`. Log replay uses `CROSSHOOK_BENCH_ROOT` and `CROSSHOOK_BENCH_LOG=ROOT/logs/...`, at no more than 5,000 lines/s; it never launches a real workload. Console drawer must be open to measure log rendering.

### Isolation requirements

All four XDG roots (`HOME` plus `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME`, and `XDG_RUNTIME_DIR`) must resolve inside the isolated fixture root; host-XDG overrides (`HOST_XDG_*`) stay disabled. Refuse real CrossHook data paths, symlink escapes, and custom `profiles_directory` roots (read from the real `settings.toml`). Preserve existing runtime/Wayland/DBus sockets. Never use `sudo`, drop host caches automatically, or start a real game/trainer. Guards run before any filesystem mutation; refusal prints `BENCH_REFUSE: …` and exits 10.

For Flatpak, per-app data derives from the **launcher** `HOME` (`flatpak-context.c` / `flatpak_get_data_dir`); sandbox `--env=HOME=...` alone does not isolate the per-app data path. The runner launches with launcher `HOME=<root>/home`, `--nofilesystem=host:reset`, `--filesystem=<root>`, `--env=CROSSHOOK_FLATPAK_HOST_XDG=0`, and materializes fixture data under `<root>/home/.var/app/dev.crosshook.CrossHook`. It requires a fresh owned `crosshook-bench-*` launcher root. Isolation protects against host per-app data access; still inspect resolved paths before execution.

## Runner CLI

Entry point `scripts/bench/run.sh` (thin wrapper; captures pre-isolation environment for the guard, then execs the stdlib Python helper). Implemented flags:

| Flag                                             | Meaning                                                                                                                        |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| `--variant A\|B\|C\|C-iced\|C-gpui`              | Required. Build under test.                                                                                                    |
| `--env E1\|E2-D\|E2-G\|E3\|E3-sw`                | Required. Environment ID from this document.                                                                                   |
| `--metric G1 … --metric G10`                     | Repeatable metric selection.                                                                                                   |
| `--metrics all\|G1,G2,…`                         | `all` (default) or comma-separated list.                                                                                       |
| `--iterations N`                                 | 1–1000, default 10 (G1/G2 iterations).                                                                                         |
| `--dry-run`                                      | Synthetic status rows only; never a measurement.                                                                               |
| `--binary PATH`                                  | Native release executable (required for launched metrics without `--flatpak`).                                                 |
| `--bundle PATH`                                  | `.flatpak` bundle file for G10.                                                                                                |
| `--flatpak`                                      | Launch through `flatpak run` with isolation flags (see above); `--branch` selects branch (default `master`).                   |
| `--fixtures DIR`                                 | Canonical fixture tree; materialized into the isolated root before launch.                                                     |
| `--output DIR`                                   | Default `results`. Writes `<output>/<variant>/<env>/<Gx>.csv` plus `meta.json`.                                                |
| `--tmp-root DIR`                                 | Isolated root parent, default `/tmp/opencode`.                                                                                 |
| `--timeout S`, `--duration S`, `--refresh-hz HZ` | READY wait (default 60), G3/G6/G7 window (default 600), interval threshold (default 120; set to the panel mode actually used). |
| `--capture-csv PATH`                             | External raw-observation CSV; required for real G4/G5/G6/G8/G9 (see schemas below).                                            |
| `--cold-cache-ack`                               | Required for real G1; without it the runner exits 20 with `NEEDS_COLD_CACHE_PREP`.                                             |
| `--keep`                                         | Retain the isolated root for inspection.                                                                                       |

Exit codes: `0` captured/dry-run; `10` guard refusal (`BENCH_REFUSE`); `20` G1 without cold-cache acknowledgment (`NEEDS_COLD_CACHE_PREP`); `1` other capture failure (`NO_READY`, `READY_TIMEOUT`, `BENCH_ERROR`, missing CSV, malformed schema, …). `meta.json` records commit SHA, variant, environment, metrics, iterations, `synthetic`, flatpak mode, fixture seed, kernel, governor, per-tool availability, capture path, refresh Hz, duration, cold-cache acknowledgment, and final status. Baselines are identified by commit SHA, never a tag.

G1/G2/G3/G7 launch the app inside the isolated root; G4/G5/G6/G8/G9/G10 are ingest-only (`--capture-csv`) and launch nothing. Cold G1 requires explicit page-cache preparation for every iteration (manual step; the flag only acknowledges it — the runner does not drop caches). A warm-cache run must be recorded as G2, never as G1. Dry-run is available for all ten metrics and writes `status=dry-run` rows; dry-run CSVs are schema fixtures, not data.

## External capture CSV schemas

Real G4/G5/G6/G8/G9 ingest raw observations from `--capture-csv` (one CSV per run; select only metrics whose required columns that CSV carries). Header must match; values must be finite and non-negative.

| Metric | Required columns                  | Notes                                                                         |
| ------ | --------------------------------- | ----------------------------------------------------------------------------- |
| G4     | `frame_ms`                        | Adds `over_interval_percent` (share > `1000/--refresh-hz`).                   |
| G5     | `latency_ms`, optional `scenario` | Per-row scenario (`unspecified` when absent); appends `summary` row with p95. |
| G6     | `stall_ms`                        | Counts `> 16 ms` and `> 50 ms`; `duration_s` from `--duration`.               |
| G8     | `frame_ms`, `memory_growth_mb`    | Reports frame stats plus max `memory_growth_mb`.                              |
| G9     | `frame_ms`                        | Same statistics as G4.                                                        |

Missing file/column → `CAPTURE_REQUIRED`/`CAPTURE_SCHEMA` failure; empty data → `EMPTY_CAPTURE`. The runner never invents samples or derives frame times from compositor rAF proxies.

## Raw provenance recipe

For every real run, preserve raw observation CSV, raw recorder/perf artifacts, stdout/stderr, exact command line, tool versions, display mode/refresh, and SHA-256 checksums beside runner output. Example artifact setup:

```sh
OUT=results/raw/$(git rev-parse --short HEAD)-$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$OUT"
{ date -u --iso-8601=seconds; uname -a; printf 'refresh_hz=%s\n' "$REFRESH_HZ"; \
  gpu-screen-recorder --version; ffmpeg -version | head -1; ydotool --version; perf --version; } \
  >"$OUT/environment.txt" 2>&1
```

Record screen evidence with `gpu-screen-recorder` where supported, or capture presentation/frame timestamps with a compositor-aware tool. If transcoding or inspection is needed, use `ffmpeg` on the preserved original and keep both files; encoded output is not the timing source. Save the exact command and stderr in the raw-artifact directory. These external tools are prerequisites, not runner dependencies: availability varies by host and must be recorded, not assumed.

Record input events and presentation timestamps from a monotonic clock in the same clock domain. `ydotool` can inject keystrokes/pointer events, but an injection timestamp alone is not a photon timestamp. Store each injected scenario and event time in the raw CSV; correlate it to the measured first presentation change before emitting G5 observations.

For G6, collect scheduler evidence and retain raw data with the UI-thread identity mapping:

```sh
perf sched record -o "$OUT/perf.data" -- <isolated-app-command>
perf sched timehist -i "$OUT/perf.data" > "$OUT/stalls.timehist"
```

Feed only validated observation CSV to the runner:

```sh
scripts/bench/run.sh --variant A --env E1 --metric G4 --capture-csv "$OUT/g4.csv" --output results
# G5/G6/G8/G9: same shape, one metric per run, matching that metric's CSV schema
find "$OUT" -type f -print0 | sort -z | xargs -0 sha256sum > "$OUT/SHA256SUMS"
```

Limitations (honest scope of these tools):

- An encoded fixed-rate stream (gpu-screen-recorder/ffmpeg at `-f N`) duplicates or drops frames on a fixed cadence. Frame duplication counts can indicate dropped frames, but per-frame `frame_ms` **cannot** be derived from such a stream. Feed G4/G8/G9 from a source with real per-frame timestamps: MangoHud CSV, Wayland presentation-time/frame-callback logs, or equivalent. The runner rejects CSVs without the required measured columns.
- Keystroke-to-photon (G5) requires correlating the input event timestamp with the first presentation change. ydotool gives injection time only; encoded video adds an unknown pipeline latency. Measure by pairing input-event monotonic timestamps with per-frame presentation timestamps; encoded recordings only bound the latency, they do not measure it.
- `perf sched timehist` attributes scheduling delays; map stalls to the UI thread explicitly and record the mapping with the artifact.

## Current coverage and remediation

Dry-run works for all ten metrics; real captures are partial:

| Item           | State                                                       | Needed                                                                                                                                                                                                                                                         |
| -------------- | ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| G1/G2          | Implemented (`READY` latency per iteration + p50/p95).      | First-frame-painted timing not yet captured (READY only). Cold-cache prep procedure manual and undocumented per-host.                                                                                                                                          |
| G3             | PSS/USS process-tree samples + growth.                      | `gpu_mb` recorded as `unavailable`.                                                                                                                                                                                                                            |
| G7             | CPU% only, `status=partial`.                                | `wakeups_s` and `power_w` unavailable (needs perf wakeup counters / `upower` or `/sys/class/power_supply` on Deck hardware).                                                                                                                                   |
| G10            | Binary and `.flatpak` bundle sizes only, `status=partial`.  | Installed size and runtime download delta.                                                                                                                                                                                                                     |
| G4/G5/G6/G8/G9 | Ingest path implemented; no producer wired into the runner. | Provide measured per-frame/per-event CSVs (see limitations); ydotool, gpu-screen-recorder and perf are absent on E1. G8 replay env (`CROSSHOOK_BENCH_LOG`, `CROSSHOOK_BENCH_LOG_RATE`) is forwarded for Flatpak but not set by the runner for native sessions. |
| Fixtures       | Files and contract declared.                                | `bench_fixtures` example must complete and prove same-seed determinism (`diff -r`) before any real capture.                                                                                                                                                    |
| E2-D/E2-G/E3   | Not inventoried.                                            | Deck and Mesa hardware access.                                                                                                                                                                                                                                 |

No hardware gate is established until real captures and review exist. Dry-run output is synthetic status and must never be reported as a measurement.

## Tests

Runner self-tests (stdlib only): `python3 scripts/bench/test_bench.py` — guard refusals (protected paths, ancestors, inherited XDG) and capture helpers. Runner-independent fixture checks, once the example completes: generate with seed 42 twice, `diff -r` the trees — no differences; a different seed must differ. `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`; `npm test`; `./scripts/lint.sh` cover the core and frontend lanes.

## Persistence

Runtime-only test data in temp/isolated directories. No change to TOML settings keys, no SQLite schema change, nothing written to user stores (`~/.config/crosshook`, `~/.local/share/crosshook`, `~/.cache/crosshook`, `~/.var/app/dev.crosshook.CrossHook`). Fixture trees, `results/` CSVs and `meta.json` are disposable artifacts; nothing here is user-editable state.
