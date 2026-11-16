# Benchmark environments inventory

E1 is inventoried on hardware (values below observed directly). E2 and E3 are **not inventoried** here; their hardware rows stay unavailable until first capture. The maintainer has no Steam Deck, so E2 captures are out of scope until hardware is available. Do not use target definitions to infer machine details.

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

The earlier 2026-10-08 inventory (CachyOS, driver 615.71.09, KWin 6.7.5) is superseded.

Frame captures use whatever mode is active: pass the matching `--refresh-hz` (180.06 for the current E1 mode; 119.88 after switching the panel to the 120 Hz-class mode). The presentation collector checks the compositor's reported refresh against `--refresh-hz` and refuses a mismatch (`REFRESH_MISMATCH`). Record the mode actually used per capture.

E1 target metrics: G1–G10 (see [`README.md`](README.md)). Commands for each are in its [Recipes](README.md#recipes). Two cannot be captured on E1: G6 has no producer, and G7 needs `perf`, which is missing (see below).

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
| perf                                     | missing                                        |
| bpftrace                                 | missing                                        |

Kernel tracing is root-only on E1 (`/sys/kernel/tracing` is 0700, `perf_event_paranoid=2`); the runner never uses `sudo`. NVIDIA exposes no `drm-*` keys in `fdinfo`, so per-process GPU memory comes from `nvidia-smi pmon`. Missing tools block the affected captures (on E1 `perf` is missing, so G7 exits 30 with `MISSING_TOOL: perf`); [README.md](README.md) requires a real capture to fail `MISSING_TOOL` rather than approximate.

### G8 on E1

G8 (`--collect presentation`, see [README.md](README.md#g8-single-session-driver)) reads GPU memory from `nvidia-smi pmon` on E1, because NVIDIA exposes no `drm-*` keys in `fdinfo`. On E1 `pmon` listed the CrossHook pids with a numeric `fb` of 2 MiB on every sample, in the native and the Flatpak run, so a GPU growth of 0.0 is a measured value (a 2 MiB app tree that did not grow). When `pmon` lists the pids but every `fb` is `-`, the sample is `unavailable` with the reason `no_numeric_fb`, never 0, the same as G3's `gpu_mb`. The sampled pids are the app's process tree: the WebKit processes (WebKitWebProcess, WebKitNetworkProcess) are descendants of the app, so they are included. The console renders only in the wide layout, so G8 and the G4 `console` scenario place the window at 1600×1000 (at 1280 px the shell shows a status bar and no console).

### Supplemental results on E1

The opt-in health-table scenarios (G4 `health`, G5 `health_table`) and the G1/G2 `first_content_ms` column have their E1 result tables in [README.md](README.md#health-table-supplemental) and [README.md](README.md#first-content-supplemental).

## E2-D — Steam Deck Desktop Mode

Not inventoried. No hardware values recorded here until first capture.

## E2-G — Steam Deck Game Mode

Not inventoried. No hardware values recorded here until first capture.

## E3 — Mesa AMD/Intel desktop

Not inventoried. No hardware values recorded here until first capture.

## E3-sw — software rendering

Not inventoried. Software-rendering modifiers (`VK_ICD_FILENAMES=/dev/null`, `LIBGL_ALWAYS_SOFTWARE=1`, llvmpipe) are defined in [README.md](README.md) but no E3 host has been captured.
