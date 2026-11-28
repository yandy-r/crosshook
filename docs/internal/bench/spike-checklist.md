# Spike hardware checklist (M0-06)

Per-device manual and semi-scripted checks for the M0-06 spike matrix ([`spike-matrix.md`](spike-matrix.md)). Scripted G1–G10 runs use [`scripts/bench/run.sh`](../../../scripts/bench/run.sh); the recipes are in [`README.md`](README.md#recipes). This list covers what the runner does not.

Run every item **twice per device**, on two different launches. Record each result in `spike-matrix.md` as **pass**, **fail** (with a reproduction recipe and a linked upstream issue) or **not run** (with the reason). Results that conflict with E1 are re-run on a second day.

## Variants and builds

| Variant  | Native binary                                                          | Flatpak branch (`dev.crosshook.CrossHook`) |
| -------- | ---------------------------------------------------------------------- | ------------------------------------------ |
| `B`      | `./scripts/build-release-binary.sh` on `main`                          | local branch (never `master`)              |
| `C-iced` | `cargo build --release -p crosshook-ui-spike-iced` on `spike/iced`     | `spike-iced`                               |
| `C-gpui` | `cargo build --release -p crosshook-ui-spike-gpui` on `spike/gpui`     | `spike-gpui`                               |

Every run uses an isolated XDG tree materialized from the seed-42 fixtures (`bench_fixtures materialize --from <fixtures> --root <root>`), never the real user stores.

## Renderer selection

| Variant  | Renderer       | How                                                                                                                                                |
| -------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `C-iced` | wgpu-Vulkan    | `--renderer wgpu-vulkan`                                                                                                                           |
| `C-iced` | wgpu-GL        | `--renderer wgpu-gl`                                                                                                                               |
| `C-iced` | software       | `--renderer tiny-skia`                                                                                                                             |
| `C-gpui` | Vulkan         | `--renderer vulkan`                                                                                                                                |
| `C-gpui` | GL             | `--renderer gl`                                                                                                                                    |
| `C-gpui` | software       | `--renderer lavapipe` (CPU Vulkan) or `--renderer llvmpipe-gl` (CPU GL)                                                                            |
| `B`      | WebKitGTK      | Shipping default; DMA-BUF policy logged at startup (`WebKit DMA-BUF renderer policy: …`)                                                          |
| all      | E3-sw          | `VK_ICD_FILENAMES=/dev/null LIBGL_ALWAYS_SOFTWARE=1` (the runner's `--env E3-sw` sets both)                                                        |
| Flatpak  | E1-mismatch    | `flatpak run --env=FLATPAK_GL_DRIVERS=<name>`: `nvidia-<older>` for a mismatched extension, `default` for a missing one (Mesa userspace only)      |

The active adapter is logged at startup as `SPIKE_RENDERER backend=… adapter=…`. Record that line for every run; a run without it is **not run**, not a pass.

## Checks

### 1. Launch and renderer

- [ ] The window reaches `READY` within 30 s.
- [ ] The `SPIKE_RENDERER` line names the expected backend and adapter (or, for a fallback, the adapter actually used).
- [ ] No `panicked`, `SIGSEGV`, `DeviceLost` or core dump in stderr.

### 2. Soak (E1: one per renderer)

- [ ] 30 minutes with the 50k-line log replayed into a visible console (1600×1000 window, `CROSSHOOK_BENCH_SCENARIO=replay_scroll`, `CROSSHOOK_BENCH_LOG_RATE=28`, `GO` on stdin), ended by `timeout` (exit 124).
- [ ] `BENCH_REPLAY … start` and `done` markers present; no crash markers.

This is the explicit `iced#2572` check on NVIDIA Vulkan: zero crashes over the soak.

### 3. Colours (`iced#3401`)

- [ ] A screenshot of the library at a fixed position matches the theme's background and accent tokens within ±3 per channel (sRGB). A washed-out or gamma-shifted render fails.

### 4. Redraw (`iced#3496`)

- [ ] After minimize/restore and after moving the window to another output, the console still updates during a replay (the line count keeps changing).

### 5. Input and focus

- [ ] Keyboard: Tab and the arrow keys reach every interactive widget of the slice, with a visible focus ring.
- [ ] Modal: focus is trapped while open and restored to the opener on close.

### 6. Flatpak

- [ ] The Flatpak branch launches with the default GL extension and logs the same adapter as the native build.
- [ ] E1-mismatch: with a mismatched and with a missing NVIDIA GL extension, record what each toolkit falls back to (adapter line, or the exact startup error).

### 7. Deck (E2-D, E2-G)

Moved to [YAN-2492](https://linear.app/itcn/issue/YAN-2492) until a Steam Deck is available: Deck idle power, D-pad zone navigation with Steam Input (virtual pad vs `--device=input`, duplicate input), OSK entry, and the Game Mode window path (X11 or Wayland).

## Status after the M0-06 run (2026-11-27)

| Check               | E1                                                                                          | E3, E3-sw                                         |
| ------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| 1. Launch, renderer | done: [`probes/`](probes/)                                                                  | done: [`probes/`](probes/)                        |
| 2. Soak             | done: four 30-min soaks, all pass                                                           | not run (E1-only check)                           |
| 3. Colours          | not run: no reference-token comparison was made; the screenshot probe checks only "not black" | not run                                           |
| 4. Redraw           | not run                                                                                     | not run                                           |
| 5. Input and focus  | not run in M0-06; see the spike write-ups and M0-09                                         | not run                                           |
| 6. Flatpak          | done: [`E1-mismatch/`](E1-mismatch/)                                                        | —                                                 |
| 7. Deck             | [YAN-2492](https://linear.app/itcn/issue/YAN-2492)                                          | —                                                 |

Each check ran once per environment, not twice: the second pass was lost to the `/tmp` wipe and the AMD link failure ([`spike-matrix.md`](spike-matrix.md)).
