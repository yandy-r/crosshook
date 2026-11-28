# Spike hardware matrix (M0-06)

Results of [YAN-846](https://linear.app/itcn/issue/YAN-846) (M0-06): tuned Tauri **B**, the iced spike **C-iced** (M0-04) and the gpui spike **C-gpui** (M0-05) across renderers and environments, and each ADR-0002 gate criterion (M0-07) evaluated as met, not met or unknown. Steam Deck environments (E2-D, E2-G) are tracked in [YAN-2492](https://linear.app/itcn/issue/YAN-2492) until a Deck is available.

Captured 2026-11-27 on one host, after a system update (environment below). Raw runner output is in [`B/`](B/), [`C-iced/`](C-iced/) and [`C-gpui/`](C-gpui/) per environment; the method and manual checks are in [`spike-checklist.md`](spike-checklist.md). Medians, IQRs and p95s are computed by the capture-only summary in [`spike-matrix-summary.json`](spike-matrix-summary.json).

## Environment and builds

| Item        | Value                                                                                                                                                                                     |
| ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Host        | E1 laptop (see [`environments.md`](environments.md)): Fedora 45, kernel 7.3.6-300.fc45, KWin 6.8.3 (Wayland), host WebKitGTK 2.54.3, Mesa 26.2.7                                          |
| NVIDIA (E1) | RTX 5070 Laptop GPU, driver 620.73.04, output HDMI-A-2                                                                                                                                    |
| AMD (E3)    | Radeon RX 9070 XT (RADV GFX1201 / radeonsi) on an external PCIe link, output DP-1, **KWin's primary output**                                                                              |
| Display     | Both outputs 3840×2160 at 180.06 Hz, scale 1.05 (one refresh interval = 5.55 ms)                                                                                                          |
| B           | `main` app source at `ff035d9d` (M0-03 and M0-08 merged), built from `68617cad` (bench-only changes on top); native release binary and local Flatpak branch `yan846-b` (GNOME 50 runtime) |
| C-iced      | `spike/iced` at `3cd03955` (native release binary; Flatpak branch `spike-iced` for E1-mismatch)                                                                                           |
| C-gpui      | `spike/gpui` at `6c822ca7` (native release binary; Flatpak branch `spike-gpui` for E1-mismatch)                                                                                           |
| Fixtures    | `bench_fixtures --seed 42`: 500 profiles, 50,000-line log; same-seed `diff -r` clean                                                                                                      |
| Runner      | B with `scripts/bench/run.sh` on this branch; C-iced and C-gpui with each spike branch's own runner (the one that produced their M0-04/M0-05 E1 data)                                     |

**Display path caveat.** Every E1 window was rendered on NVIDIA but shown on DP-1, which the AMD card scans out (KWin places new windows on its primary output). The earlier A, C-iced and C-gpui E1 captures had the same host setup. A pure-NVIDIA spot check (AMD output disabled, so NVIDIA renders and scans out) ran on 2026-11-28 after a reboot; see [Pure-NVIDIA spot check](#pure-nvidia-spot-check-e1). Output names change between boots (DP-1/DP-3 for the AMD card, HDMI-A-1/HDMI-A-2 for NVIDIA).

**Host stability caveat.** The AMD card's PCIe link logged correctable AER errors all day and failed twice with an uncorrectable error (2026-11-26 21:19 and 2026-11-27 20:42), both times while C-iced was driving RADV in G4. The second failure oopsed the kernel (`amdgpu_device_halt_activities`); the iced process stayed in uninterruptible sleep in `amdgpu_info_ioctl`. A fatal PCIe bus error is a link or hardware fault, so this is recorded as a host fault, not a toolkit crash.

### Environment mapping

| ID          | How it was produced on this host                                                                                                                                                |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E1          | Default renderer: NVIDIA Vulkan (spikes), WebKitGTK with DMA-BUF disabled for NVIDIA (B, shipping policy)                                                                       |
| E1-mismatch | Flatpak with `FLATPAK_GL_DRIVERS=nvidia-620-70-06` (userspace one release behind the 620.73.04 kernel module) and `FLATPAK_GL_DRIVERS=default` (no NVIDIA extension, Mesa only) |
| E3          | Spikes: `VK_ICD_FILENAMES=radeon_icd` (RADV only). B: `DRI_PRIME=1`, Mesa EGL vendor, `WEBKIT_DISABLE_DMABUF_RENDERER=1` (see [B on E3](#b-on-e3-dma-buf))                      |
| E3-sw       | `VK_ICD_FILENAMES=/dev/null LIBGL_ALWAYS_SOFTWARE=1`, Mesa EGL vendor (as on a Mesa host): llvmpipe                                                                             |
| E2-D, E2-G  | Not run: no Steam Deck ([YAN-2492](https://linear.app/itcn/issue/YAN-2492))                                                                                                     |

E3 here is a dual-GPU host with KWin's primary GPU being NVIDIA, not a single-GPU Mesa desktop. Each environment block started with a render check (screenshot of the window, mean grey with alpha off; 0 means a black window) in [`probes/`](probes/).

## Pass/fail matrix

**pass** = launched, rendered (render check) and every scripted metric captured; **fail** = crash, no window or a metric that could not be captured; **pending** = not run on this host. Soaks: 30 minutes with the 50k-line log replayed into a visible console at 28 lines/s; pass = `timeout` exit 124, full replay, no crash markers.

| Variant | Renderer              | E1                                                                                                             | E1-mismatch: mismatched / missing                                                        | E2-D, E2-G | E3                                                                                                      | E3-sw                                                           |
| ------- | --------------------- | -------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| B       | WebKitGTK (GL)        | pass; G4 `console` fails on the AMD output, passes NVIDIA-only ([finding 1](#1-b-g4-console-stops-presenting)) | pass / pass                                                                              | pending    | pass, except G4 ([finding 1](#1-b-g4-console-stops-presenting)); DMA-BUF must be off                    | pass, except G4 ([finding 1](#1-b-g4-console-stops-presenting)) |
| C-iced  | wgpu-Vulkan           | **pass**: 30-min soak, 0 crashes (`#2572` check)                                                               | falls back to **tiny-skia** / RADV                                                       | pending    | G1, G2, G3, G5 pass; G4, G8, G9 **fail** ([finding 3](#3-c-iced-on-e3-discards-frames-then-host-fault)) | —                                                               |
| C-iced  | wgpu-GL               | **fail**: `GraphicsAdapterNotFound` on NVIDIA EGL ([finding 4](#4-gl-on-nvidia-egl))                           | —                                                                                        | pending    | renders on radeonsi (probe only)                                                                        | **pass**: auto-selected GL on llvmpipe                          |
| C-iced  | tiny-skia (CPU)       | **pass**: 30-min soak                                                                                          | (the mismatched fallback above)                                                          | pending    | —                                                                                                       | not selected automatically (GL won)                             |
| C-gpui  | Vulkan                | **pass**: 30-min soak                                                                                          | **fail**: no window ([finding 2](#2-c-gpui-opens-no-window-without-a-usable-gpu)) / RADV | pending    | **pass**                                                                                                | —                                                               |
| C-gpui  | GL                    | **fail**: no compatible surface format on NVIDIA EGL ([finding 4](#4-gl-on-nvidia-egl))                        | —                                                                                        | pending    | —                                                                                                       | **pass**: llvmpipe, no `zed#50996` crash                        |
| C-gpui  | lavapipe (CPU Vulkan) | **pass**: 30-min soak                                                                                          | —                                                                                        | pending    | —                                                                                                       | —                                                               |

Soak evidence: [`C-iced/E1-soak/`](C-iced/E1-soak/), [`C-gpui/E1-soak/`](C-gpui/E1-soak/). E1-mismatch logs: [`E1-mismatch/`](E1-mismatch/).

## G1–G10 summaries

Statistics are median / IQR / p95 over iterations (G1, G2) or trials (G5, 400 per scenario). G4, G8 and G9 frame intervals are the runner's pooled p50 / p95 / p99 (G8 also median / IQR / p95 from the frame observations). Times in ms; a refresh interval is 5.55 ms. Ratios are C ÷ B on the same environment (below 1 is better). **Ratios to A are unavailable**: the only A captures are the uninstrumented `v0.6.1` DMA-BUF check and IPC ranking ([`A/summary.md`](A/summary.md)); no A G1–G10 run exists to divide by.

### E3 (RX 9070 XT, all three variants)

| Metric                          | B                        | C-iced                   | C-gpui                         | iced ÷ B | gpui ÷ B    |
| ------------------------------- | ------------------------ | ------------------------ | ------------------------------ | -------- | ----------- |
| G1 cold READY                   | 1132 / 31 / 1166         | 452 / 24 / 514           | 483 / 45 / 523                 | 0.40     | 0.43        |
| G1 cold first present           | 354 / 22 / 372           | 411 / 25 / 459           | 462 / 26 / 492                 | 1.16     | 1.31        |
| G2 warm READY                   | 909 / 13 / 933           | 172 / 20 / 212           | 169 / 15 / 189                 | 0.19     | 0.19        |
| G2 warm first present           | 181 / 9 / 192            | 126 / 19 / 155           | 152 / 21 / 175                 | 0.70     | 0.84        |
| G3 idle PSS / USS (600 s, MB)   | 354 / 261                | 95 / 86                  | 109 / 100                      | 0.27     | 0.31        |
| G3 growth first → last sample   | −10.5 %                  | +15.2 %                  | 0.0 %                          | —        | —           |
| G4 scroll p50/p95/p99           | not captured (finding 1) | not captured (finding 3) | 33.3 / 44.4 / 50.0 (all three) | —        | —           |
| G5 `search`                     | 205 / 57 / 223           | 16.5 / 3.1 / 21.0        | 15.4 / 4.4 / 26.8              | 0.08     | 0.08        |
| G5 `toggle_favorite`            | 34.1 / 6.3 / 56.1        | 13.3 / 4.4 / 23.5        | 15.9 / 5.0 / 26.6              | 0.39     | 0.47        |
| G5 `toggle_optimization`        | 125 / 8 / 138            | 13.6 / 4.3 / 23.8        | 16.9 / 3.5 / 24.0              | 0.11     | 0.14        |
| G5 `dpad`                       | 54.7 / 4.1 / 71.1        | 13.2 / 4.8 / 23.4        | 16.2 / 4.5 / 26.0              | 0.24     | 0.30        |
| G5 `route_switch`               | 150 / 42 / 201           | 16.4 / 3.4 / 23.4        | 17.5 / 5.7 / 28.3              | 0.11     | 0.12        |
| G7 idle CPU (600 s) / wakeups/s | 0.90 % / 1.32            | not run (spike runner)   | not run (spike runner)         | —        | —           |
| G8 frame p50/p95/p99            | 194 / 211 / 221          | not captured (finding 3) | 11.1 / 16.7 / 22.2             | —        | 0.06        |
| G8 growth / peak PSS (MB)       | 42.8 / 774               | —                        | 76.0 / 212                     | —        | 1.78 / 0.27 |
| G9 resize p50/p95/p99           | 27.8 / 38.9 / 44.4       | not captured (finding 3) | 33.3 / 38.9 / 44.4             | —        | 1.20        |

### E3-sw (llvmpipe)

| Metric                    | B                         | C-iced              | C-gpui              | iced ÷ B    | gpui ÷ B    |
| ------------------------- | ------------------------- | ------------------- | ------------------- | ----------- | ----------- |
| Renderer (probe)          | WebKitGTK, no render node | wgpu GL on llvmpipe | gpui GL on llvmpipe | —           | —           |
| G2 warm READY             | 918 / 14 / 963            | 115 / 11 / 136      | 121 / 11 / 141      | 0.12        | 0.13        |
| G2 warm first present     | 186 / 5 / 208             | 95 / 5 / 108        | 108 / 8 / 137       | 0.51        | 0.58        |
| G4 `library` p50/p95/p99  | not captured (finding 1)  | 5.6 / 5.6 / 11.1    | 33.3 / 44.4 / 50.0  | —           | —           |
| G4 `console` p50/p95/p99  | not captured              | 5.6 / 16.7 / 16.7   | 33.3 / 44.4 / 50.0  | —           | —           |
| G4 `settings` p50/p95/p99 | not captured              | 5.6 / 5.6 / 5.6     | 33.3 / 44.4 / 50.0  | —           | —           |
| G8 frame p50/p95/p99      | 194 / 205 / 214           | 5.6 / 16.7 / 16.7   | 11.1 / 22.2 / 22.2  | 0.03        | 0.06        |
| G8 growth / peak PSS (MB) | 82.6 / 834                | 26.3 / 279          | 44.8 / 336          | 0.32 / 0.33 | 0.54 / 0.40 |

### E1 (NVIDIA): B native and Flatpak

The spikes' E1 G-metrics were not re-captured in M0-06; their M0-04/M0-05 captures ([`C-iced/E1/`](C-iced/E1/), [`C-gpui/E1/`](C-gpui/E1/)) ran on the same host, the previous kernel (7.3.4) and the same display path, and are compared here as reported. Spike E1 G5 is p50 / p95 from those CSVs.

| Metric                                        | B native                 | B Flatpak                | C-iced (M0-04)        | C-gpui (M0-05)        | iced ÷ B native | gpui ÷ B native |
| --------------------------------------------- | ------------------------ | ------------------------ | --------------------- | --------------------- | --------------- | --------------- |
| G1 cold READY                                 | 1196 / 23 / 1240         | 1370 / 16 / 1409         | —                     | 939 / 12 / 963 (n=5)  | —               | 0.79            |
| G1 cold first present                         | 421 / 17 / 460           | 655 / 16 / 686           | —                     | —                     | —               | —               |
| G2 warm READY                                 | 970 / 13 / 996           | 1029 / 19 / 1066         | 282 / 20 / 333 (n=10) | 261 / 47 / 410 (n=10) | 0.29            | 0.27            |
| G2 warm first present                         | 235 / 14 / 245           | 357 / 16 / 389           | 220 / 7 / 293         | 258 / 32 / 394        | 0.94            | 1.10            |
| G3 idle PSS / USS (MB)                        | 390 / 296                | 482 / 368                | see M0-04             | see M0-05             | —               | —               |
| G4 `library` p50/p95/p99                      | 200 / 217 / 222          | not captured (finding 1) | 5.6 / 5.6 / 11.1      | 33.3 / 44.4 / 44.5    | 0.03            | 0.17            |
| G4 `settings` p50/p95/p99                     | 111 / 244 / 302          | not captured             | 5.6 / 5.6 / 8.7       | 33.3 / 44.4 / 44.4    | 0.05            | 0.30            |
| G4 `console`                                  | not captured (finding 1) | not captured             | 33.3 / 44.4 / 44.5    | 33.3 / 44.4 / 44.5    | —               | —               |
| G4 `health` (opt-in)                          | 72.2 / 88.9 / 100        | 83.3 / 239 / 250         | —                     | —                     | —               | —               |
| G5 `search`                                   | 204 / 57 / 224           | 215 / 98 / 245           | 14.7 / 17.1           | 11.7 / 14.1           | 0.07            | 0.06            |
| G5 `toggle_favorite`                          | 33.5 / 6.3 / 55.6        | 35.5 / 6.3 / 53.3        | 15.2 / 17.4           | 13.5 / 17.4           | 0.45            | 0.40            |
| G5 `toggle_optimization`                      | 124 / 8 / 138            | 131 / 6 / 144            | 15.8 / 17.3           | 13.6 / 18.0           | 0.13            | 0.11            |
| G5 `dpad`                                     | 55.0 / 4.8 / 73.9        | 62.1 / 5.3 / 79.9        | 14.0 / 18.1           | 16.2 / 18.6           | 0.25            | 0.29            |
| G5 `route_switch`                             | 159 / 41 / 187           | 164 / 45 / 246           | 16.4 / 18.3           | 13.7 / 17.9           | 0.10            | 0.09            |
| G7 idle CPU (600 s) / wakeups/s               | 0.90 % / 1.40            | 2.74 % / 3.09            | —                     | —                     | —               | —               |
| G8 frame p50/p95/p99                          | 194 / 205 / 222          | 233 / 250 / 261          | 11.1 / 16.7 / —       | 16.7 / 17.6 / —       | 0.06            | 0.09            |
| G8 growth / peak PSS (MB)                     | 55.2 / 824               | 119.5 / 923              | 2.3 / 270             | 2.5 / 229             | 0.04 / 0.33     | 0.04 / 0.28     |
| G9 resize p50/p95/p99                         | 27.8 / 38.9 / 44.4       | 33.3 / 38.9 / 38.9       | 5.6 / 33.3 / 33.3     | 33.3 / 38.9 / 38.9    | 0.20            | 1.20            |
| G10 stripped binary / bundle / installed (MB) | 36.8 / 9.1 / 37.0        | (same bundle)            | 23.9 / — / —          | 42.1 / 10.3 / 42.2    | 0.65            | 1.14            |

G1 is 20 single-iteration runs with the page cache dropped before each (`sysctl vm.drop_caches=3`). G7 CPU is the 600 s average including start-up; wakeups/s are `perf sched:sched_wakeup` for the app tree (`perf` installed and `perf_event_paranoid` lowered for this boot only). G10 runtime delta is 0 because GNOME 50 was already installed: not a first-install figure.

### Pure-NVIDIA spot check (E1)

2026-11-28, fresh boot, AMD output (DP-3 this boot) disabled with `kscreen-doctor`, so only NVIDIA's HDMI-A-1 was active. Data: [`B/E1-nvidia-only/`](B/E1-nvidia-only/), [`C-iced/E1-soak/iced-vulkan-E1-nvidia-only.txt`](C-iced/E1-soak/iced-vulkan-E1-nvidia-only.txt).

| Run                               | NVIDIA-only                                       | Cross-GPU E1 (above)     |
| --------------------------------- | ------------------------------------------------- | ------------------------ |
| C-iced wgpu-Vulkan 30-min soak    | **pass** (exit 124, full replay, 0 crash markers) | pass                     |
| B G4 `library` p50/p95/p99 (×20)  | 199.9 / 210.9 / 216.7                             | 199.9 / 216.6 / 222.1    |
| B G4 `settings` p50/p95/p99 (×20) | 116.7 / 255.4 / 299.9                             | 111.1 / 244.4 / 302.1    |
| B G4 `console` p50/p95/p99 (×1)   | **captured**: 77.6 / 133.3 / 140.1                | not captured (finding 1) |
| B G8 p50/p95/p99 (×20)            | 166.5 / 261.0 / 266.6                             | 194.4 / 205.5 / 222.1    |
| B G8 growth / peak PSS (MB)       | 74.8 / 847                                        | 55.2 / 824               |

The display path does not change B's `library`/`settings` pacing or the gate verdicts. G8 shifts in both directions (lower p50, higher p95), so treat B's G8 spread as run-to-run variation of about ±25 %.

## Findings

Each failure has a reproduction recipe. A fatal PCIe error is a host fault and has no toolkit issue.

### 1. B G4 `console` stops presenting

- **What:** in every environment (E1 native, E1 Flatpak, E3, E3-sw) the G4 `console` scenario fails with `FOCUS_LOST: presentation log stopped growing`, at the first iteration, about 3.8 s into the scroll. `library`, `settings` and `health` pass. The same binary passed `console` on 2026-11-26, before the update.
- **Evidence:** the window was on top (screenshots every ~1 s); its pixels stayed identical for 7 s while input was still injected; commits ran at 60 Hz, then stopped. The console header froze at 47,595 received lines, below the 50,000-line replay. The view draws at most 2,000 lines (`CONSOLE_MAX_LINES`, M0-08).
- **Reproduce:** `scripts/bench/run.sh --variant B --env E1 --refresh-hz 180.06 --binary <B> --fixtures <seed-42> --metric G4 --collect presentation --scenario console --iterations 1`.
- **NVIDIA-only:** with the AMD output disabled (2026-11-28, after a reboot), the same binary captured `console` (1 iteration: p50 77.6, p95 133.3, p99 140.1 ms, 78 frames). Every failing run had its window on the AMD-driven output (E1 and E3-sw cross-GPU, E3 rendered on AMD with KWin compositing on NVIDIA); the 2026-11-26 pass was on the same output before the update.
- **Status:** points at the cross-GPU display path after the 2026-11-27 update (KWin 6.8.3, WebKitGTK 2.54.3, NVIDIA 620.73.04), not at the console code; not proven, because the NVIDIA-only run was also a fresh boot and a single iteration. Follow-up issue to file. No toolkit issue applies (B is Classic).

### 2. C-gpui opens no window without a usable GPU

- **What:** with a mismatched NVIDIA GL extension (`FLATPAK_GL_DRIVERS=nvidia-620-70-06`), gpui logs `could not open window: Failed to create surface for any enabled backend: {}` and keeps running headless. The same happens on E1 with `--renderer gl` (NVIDIA EGL offers no compatible surface format).
- **Reproduce:** `FLATPAK_GL_DRIVERS=nvidia-620-70-06 flatpak run --branch=spike-gpui dev.crosshook.CrossHook`.
- **Upstream:** [zed-industries/zed#65960](https://github.com/zed-industries/zed/issues/65960) (open, regression: same error, no window, process keeps running).
- **Contrast:** iced falls back to tiny-skia in the same setup; B reaches READY.

### 3. C-iced on E3 discards frames, then host fault

- **What:** C-iced G4 (`console`), G8 and G9 on RADV failed with `DISCARD_RATE: more than 5% of frames were discarded` (each retried once), during bursts of PCIe AER errors on the AMD link. C-gpui passed every E3 metric on the same card and link, during AER bursts too. A later G9 re-run (2 iterations, default present mode `AutoVsync`, and `ICED_PRESENT_MODE=fifo`) captured cleanly. The full G4 re-run then hung when the link failed fatally (20:42:29, host stability caveat above).
- **Reproduce:** `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json DRI_PRIME=1 scripts/bench/run.sh --variant C-iced --env E3 --refresh-hz 180.06 --binary <iced> --fixtures <seed-42> --metric G4 --collect presentation --iterations 20` (spike branch runner).
- **Status:** intermittent; not attributable to iced while the host link is unstable. Re-run on a stable Mesa desktop before any conclusion. No upstream issue.

### 4. GL on NVIDIA EGL

- **What:** neither spike renders through wgpu-GL on NVIDIA's EGL: iced exits with `GraphicsAdapterNotFound` (no adapter for the surface), gpui finds an adapter but no compatible surface format (finding 2). Both work on GL through Mesa (radeonsi on E3, llvmpipe on E3-sw). This matches M0-04/M0-05.
- **Upstream:** no matching NVIDIA report found. Closest: [zed-industries/zed#66030](https://github.com/zed-industries/zed/issues/66030) (GL/EGL offers no adapter on Wayland) and [alplabai/iced#47](https://github.com/alplabai/iced/issues/47) (software adapter fallback). Candidate to file upstream.
- **Impact:** a broken-GPU fallback (M5-03) cannot rely on wgpu-GL on NVIDIA; the CPU paths (tiny-skia, lavapipe, llvmpipe) are the fallback.

### B on E3: DMA-BUF

With `DRI_PRIME=1` and the Mesa EGL vendor, B's window is fully black when WebKitGTK's DMA-BUF renderer is on (render check mean grey 0). With DMA-BUF off it renders on the AMD render node. This is a dual-GPU host artifact (render on AMD, KWin compositing on NVIDIA); a single-GPU Mesa desktop keeps DMA-BUF on by default. The first E3 B pass with DMA-BUF on was discarded and is not in these results.

### Earlier, not reproduced

On 2026-11-26 B G5 hit `MARK_TIMEOUT` twice in about 45 launches (`toggle_favorite` on E1, `toggle_optimization` on E3, render not seen within 10 s). It did not recur in the 2026-11-27 runs (2,000 G5 trials per environment).

## Game Mode window system

Pending a Steam Deck: [YAN-2492](https://linear.app/itcn/issue/YAN-2492).

## ADR-0002 gate criteria

Thresholds from [`docs/research/native-ui-migration/README.md`](../../research/native-ui-migration/README.md) §6. The thresholds were written for a 120 Hz panel (8.3 ms interval); this host ran at 180.06 Hz (5.55 ms), and both are given where it matters.

### Hard thresholds

| #   | Criterion     | C-iced                                                                                                                                                                                                                                                                        | C-gpui                                                                                                                                                                                                                  |
| --- | ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H1  | Rendering     | **unknown**. E1 30-min soak with no crash: met (Vulkan and tiny-skia). Correct colours (`iced#3401`): not measured objectively. E2: unknown (YAN-2492). E3: not clean, confounded by the host link (finding 3). No-Vulkan: renders; library scroll p50 5.6 ms (180 fps): met. | **unknown**. E1 soak: met (Vulkan and lavapipe). Colours: not measured. E2: unknown. E3: clean, met. No-Vulkan: renders (no `zed#50996`); library scroll p50 33.3 ms (30 fps), p95 44.4 ms (22.5 fps): met at p50 only. |
| H2  | Focus         | **unknown** in M0-06; evaluated by the spike write-ups ([`spikes/iced.md`](../spikes/iced.md)).                                                                                                                                                                               | **unknown** in M0-06; see [`spikes/gpui.md`](../spikes/gpui.md).                                                                                                                                                        |
| H3  | Scroll        | E1 library p95 5.6 ms ≤ 1 interval (M0-04 data): **met**. Dropped frames < 1 %: unknown (not reported by the runner). E2: unknown.                                                                                                                                            | E1 library p95 44.4 ms (8 intervals): **not met**.                                                                                                                                                                      |
| H4  | Console flood | G8 E1 p95 16.7 ms (3 intervals at 180 Hz; 2 at 120 Hz): **not met**. Ring ≤ 10k lines: met (spike design). RSS growth 2.3 MB ≤ 15 MB: met. E3: not captured.                                                                                                                  | G8 p95 17.6 ms (E1) / 16.7 ms (E3): **not met**. Ring ≤ 10k: met. RSS growth 2.5 MB (E1, met) but 76 MB on E3: **not met**.                                                                                             |
| H5  | Idle          | **unknown**: G7 was not run for the spikes (their runners predate the batteryless-host fix). M0-04 recorded 0.05 % CPU over 20 s as a local observation, not a capture.                                                                                                       | **unknown** (as iced).                                                                                                                                                                                                  |

### Soft thresholds

| #   | Criterion                                          | C-iced                                                                                              | C-gpui                                                       |
| --- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| S1  | Cold start to first frame ≤ 300 ms (E1)            | **not met**: E3 G1 first present 411 ms; no E1 cold capture.                                        | **not met**: E3 G1 first present 462 ms; E1 G1 READY 939 ms. |
| S2  | Keystroke-to-photon ≤ 33 ms (E1); D-pad ≤ 2 frames | **met**: every E1 G5 scenario p95 ≤ 18.3 ms; D-pad p95 18.1 ms (≈ 3 frames at 180 Hz, 2 at 120 Hz). | **met**: p95 ≤ 18.6 ms; D-pad p95 18.6 ms.                   |
| S3  | Accessibility                                      | See M0-09 ([`spikes/a11y-ime-probe.md`](../spikes/a11y-ime-probe.md)); not evaluated here.          | Same.                                                        |
| S4  | IME                                                | E1: see M0-09. Steam OSK in Game Mode: unknown (YAN-2492).                                          | Same.                                                        |
| S5  | Effort                                             | See the spike write-ups; not evaluated here.                                                        | Same.                                                        |
| S6  | Stripped binary ≤ 30 MB                            | **met**: 23.9 MB.                                                                                   | **not met**: 42.1 MB.                                        |

### Desktop G-gates (§9.2), E1/E3 thresholds

| Gate | Threshold                               | B (E1 native)                                    | C-iced                                            | C-gpui                                                                   |
| ---- | --------------------------------------- | ------------------------------------------------ | ------------------------------------------------- | ------------------------------------------------------------------------ |
| G1   | cold READY p95 ≤ 500 ms                 | not met (1240)                                   | not met (E3 514)                                  | not met (E3 523)                                                         |
| G2   | warm READY p95 ≤ 250 ms                 | not met (996)                                    | met (E3 212); not met on E1 (333)                 | met (E3 189); not met on E1 (410)                                        |
| G3   | idle PSS ≤ 120 MB (500 profiles)        | not met (390)                                    | met (E3 95)                                       | met (E3 109)                                                             |
| G4   | scroll p95 ≤ 8.3 ms                     | not met                                          | met on E1 `library`/`settings`; `console` not met | not met (44.4)                                                           |
| G5   | keystroke ≤ 25 ms; route switch ≤ 50 ms | not met                                          | met (p95 ≤ 23.8 on E3)                            | not met on E3 (route switch p95 28.3 ≤ 50 met; keystroke p95 up to 26.8) |
| G6   | 0 stalls > 16 ms                        | unknown (no producer)                            | unknown                                           | unknown                                                                  |
| G7   | idle CPU ≤ 0.2 %, wakeups ≤ 2/s         | not met (0.90 %, start-up included) / met (1.40) | unknown                                           | unknown                                                                  |
| G8   | p95 ≤ 1 interval, ≤ 15 MB               | not met                                          | not met (p95 16.7)                                | not met                                                                  |
| G9   | (no desktop threshold)                  | —                                                | —                                                 | —                                                                        |
| G10  | binary ≤ 25 MB                          | not met (36.8)                                   | met (23.9)                                        | not met (42.1)                                                           |

## Open items

- **Re-run C-iced E3 G4/G8/G9** on a stable Mesa host (finding 3).
- **File a follow-up** for B G4 `console` (finding 1), and decide on filing the NVIDIA EGL report upstream (finding 4).
- **Spike G7** (H5): run the spike runners with the batteryless-host fix, or port it to the spike branches.
- **Deck** (E2-D, E2-G, Game Mode window system, S4 OSK): [YAN-2492](https://linear.app/itcn/issue/YAN-2492).
