# Benchmark environments inventory

E1 is inventoried on hardware (values below observed directly). E3 and E3-sw were captured on the same host with a second, external AMD GPU (M0-06, below). E2 is **not inventoried**; its hardware rows stay unavailable until first capture. The maintainer has no Steam Deck, so E2 captures are out of scope until hardware is available. Do not use target definitions to infer machine details.

## E1 — developer laptop (re-inventoried 2026-11-16)

| Component    | Observed value                                                                                    |
| ------------ | ------------------------------------------------------------------------------------------------- |
| Host         | Minisforum G7 Pro (Intel Core i9-14900HX)                                                         |
| OS           | Fedora Linux 45                                                                                   |
| Kernel       | 7.3.3-300.fc45                                                                                    |
| GPU          | NVIDIA GeForce RTX 5070 Laptop GPU (GB206M)                                                       |
| Driver       | 620.70.06, open kernel module, `nvidia-drm.modeset=Y`, `nvidia-drm.vblank=N`                      |
| Compositor   | KWin 6.8.2 (KDE Plasma, Wayland session); `wp_presentation` v2, `clock_id` CLOCK_MONOTONIC        |
| Display      | HDMI-A-1, 3840×2160 @ 180.06 Hz active; the 120 Hz-class mode at that resolution is **119.88 Hz** |
| Scale        | 1.05 (`kscreen-doctor -o`)                                                                        |
| Web stack    | WebKitGTK 4.1 2.54.2, GTK 3.24.52, libwayland 1.26.0, glibc 2.44                                  |
| CPU governor | `performance`                                                                                     |
| Flatpak      | 1.18.4; app on `org.gnome.Platform//50` (glibc 2.42, libwayland 1.24)                             |

**2026-11-27 update (M0-06).** A system update between the M0-04/05 captures and M0-06 changed: kernel 7.3.6-300.fc45, NVIDIA driver 620.73.04, KWin 6.8.3, host WebKitGTK 2.54.3, Mesa 26.2.7, GNOME 50 runtime build of 2026-11-25. An external Radeon RX 9070 XT is attached (see [E3](#e3--mesa-amdintel-desktop)); KWin's primary output is the AMD one (DP-1), and NVIDIA drives HDMI-A-2 (renamed from HDMI-A-1). Both run 3840×2160 @ 180.06 Hz, scale 1.05. New windows open on DP-1, so E1 captures render on NVIDIA and are scanned out by the AMD card; the M0-04/05 captures had the same setup.

The earlier 2026-10-08 inventory (CachyOS, driver 615.71.09, KWin 6.7.5) is superseded.

Frame captures use whatever mode is active: pass the matching `--refresh-hz` (180.06 for the current E1 mode; 119.88 after switching the panel to the 120 Hz-class mode). The presentation collector checks the compositor's reported refresh against `--refresh-hz` and refuses a mismatch (`REFRESH_MISMATCH`). Record the mode actually used per capture.

E1 target metrics: G1–G10 (see [`README.md`](README.md)). Commands for each are in its [Recipes](README.md#recipes). G6 cannot be captured (no producer). G7 needs `perf` (installed 2026-11-26) and wakeup-trace access (see below).

### E1 available tooling (2026-11-16)

| Tool                                     | Status                                         |
| ---------------------------------------- | ---------------------------------------------- |
| cc (GCC)                                 | available (16.2.1)                             |
| wayland-scanner / wayland-client         | available (1.26.0)                             |
| `org.gnome.Sdk//50` (Flatpak shim build) | available (GCC 15.2.0, wayland-scanner 1.24.0) |
| `/dev/uinput`                            | user-writable (ACL)                            |
| ydotool / ydotoold                       | available (1.0.4; daemon not running)          |
| gpu-screen-recorder                      | available (6.1.7; not evidence, optional)      |
| upower                                   | available                                      |
| flatpak                                  | available (1.18.4)                             |
| ffmpeg                                   | available                                      |
| MangoHud (`mangohud`)                    | available                                      |
| gamescope                                | available                                      |
| python3                                  | available (3.15.2)                             |
| ImageMagick (`magick`)                   | available                                      |
| bats                                     | available                                      |
| hyperfine                                | missing                                        |
| pidstat                                  | missing                                        |
| perf                                     | available (installed 2026-11-26)               |
| bpftrace                                 | missing                                        |

Kernel tracing is root-only on E1 by default (`/sys/kernel/tracing` is 0700, `perf_event_paranoid=2`); the runner never uses `sudo`. For the M0-06 G7 captures the operator lowered `perf_event_paranoid` to -1 and opened `/sys/kernel/tracing` (0755) for one boot only; a reboot restores the defaults. NVIDIA exposes no `drm-*` keys in `fdinfo`, so per-process GPU memory comes from `nvidia-smi pmon`. Missing tools block the affected captures (without `perf`, G7 exits 30 with `MISSING_TOOL: perf`); [README.md](README.md) requires a real capture to fail `MISSING_TOOL` rather than approximate.

### G8 on E1

G8 (`--collect presentation`, see [README.md](README.md#g8-single-session-driver)) reads GPU memory from `nvidia-smi pmon` on E1, because NVIDIA exposes no `drm-*` keys in `fdinfo`. On E1 `pmon` listed the CrossHook pids with a numeric `fb` of 2 MiB on every sample, in the native and the Flatpak run, so a GPU growth of 0.0 is a measured value (a 2 MiB app tree that did not grow). When `pmon` lists the pids but every `fb` is `-`, the sample is `unavailable` with the reason `no_numeric_fb`, never 0, the same as G3's `gpu_mb`. The sampled pids are the app's process tree: the WebKit processes (WebKitWebProcess, WebKitNetworkProcess) are descendants of the app, so they are included. The console renders only in the wide layout, so G8 and the G4 `console` scenario place the window at 1600×1000 (at 1280 px the shell shows a status bar and no console).

### Supplemental results on E1

The opt-in health-table scenarios (G4 `health`, G5 `health_table`) and the G1/G2 `first_content_ms` column have their E1 result tables in [README.md](README.md#health-table-supplemental) and [README.md](README.md#first-content-supplemental).

## E2-D — Steam Deck Desktop Mode

Not inventoried. No hardware values recorded here until first capture.

## E2-G — Steam Deck Game Mode

Not inventoried. No hardware values recorded here until first capture.

## E3 — Mesa AMD/Intel desktop

Captured 2026-11-27 (M0-06) on the E1 host with a second GPU, not on a dedicated Mesa desktop:

| Component | Observed value                                                                                                                                                                   |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GPU       | AMD Radeon RX 9070 XT (RDNA 4, GFX1201) on an external PCIe link, `renderD128`; NVIDIA stays the boot VGA device and KWin's main render device                                   |
| Driver    | amdgpu (kernel 7.3.6-300.fc45), Mesa 26.2.7 (RADV, radeonsi)                                                                                                                     |
| Display   | DP-1 on the AMD card, KWin's primary output, 3840×2160 @ 180.06 Hz, scale 1.05                                                                                                   |
| Selection | Spikes: `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json`. B: `DRI_PRIME=1`, `__EGL_VENDOR_LIBRARY_FILENAMES=…/50_mesa.json`, `WEBKIT_DISABLE_DMABUF_RENDERER=1` |

Caveats: compositing stays on NVIDIA, so this is a cross-GPU path, not a single-GPU Mesa desktop; B's WebKitGTK window is black with DMA-BUF on in this setup. The AMD link logged correctable PCIe AER errors throughout and failed fatally twice (2026-11-26 21:19 and 2026-11-27 20:42, both during C-iced G4 on RADV), the second time with a kernel oops; see [`spike-matrix.md`](spike-matrix.md#3-c-iced-on-e3-discards-frames-then-host-fault). Re-run E3 on a stable single-GPU Mesa host before treating its frame-pacing results as final.

## E3-sw — software rendering

Captured 2026-11-27 (M0-06) on the E1 host with `VK_ICD_FILENAMES=/dev/null LIBGL_ALWAYS_SOFTWARE=1` and the Mesa EGL vendor (llvmpipe, Mesa 26.2.7). B renders with no render node open; iced selects wgpu GL and gpui selects GL, both on llvmpipe. Render checks are in [`probes/`](probes/).
