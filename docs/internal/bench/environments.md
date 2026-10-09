# Benchmark environments inventory

E1 is inventoried on hardware (values below observed directly). E2 and E3 are **not inventoried** here; their hardware rows stay unavailable until first capture. Do not use target definitions to infer machine details.

## E1 — developer laptop (inventoried 2026-10-08)

| Component    | Observed value                                                        |
| ------------ | --------------------------------------------------------------------- |
| OS           | CachyOS Linux (`/etc/os-release` ID=cachyos)                          |
| Kernel       | 7.2.9-1-cachyos                                                       |
| GPU          | NVIDIA GeForce RTX 5070 Laptop GPU                                    |
| Driver       | 615.71.09 (`nvidia-smi`)                                              |
| Compositor   | kwin 6.7.5 (KDE, Wayland session)                                     |
| Display mode | 3840×2160 @ 180.06 Hz (active), 120 Hz available                      |
| Scale        | 1.05 (`kscreen-doctor -o`)                                            |
| CPU governor | `powersave` (`/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor`) |
| Flatpak      | 1.18.4                                                                |

Inventory records the active mode (180.06 Hz). **Frame captures (G4/G5/G8/G9) require the 120 Hz mode:** switch the panel to 120 Hz before capturing and pass `--refresh-hz 120`, so frame-interval thresholds (8.3 ms) stay comparable. Record the mode actually used per capture.

E1 target metrics: G1–G10 (see [`README.md`](README.md)).

### E1 available tooling (2026-10-08)

| Tool                             | Status                        |
| -------------------------------- | ----------------------------- |
| hyperfine                        | available                     |
| pidstat                          | available                     |
| upower                           | available                     |
| flatpak                          | available (1.18.4)            |
| ffmpeg                           | available (9.0.2)             |
| MangoHud (`mangohud`)            | available                     |
| gamescope                        | available                     |
| python3                          | available (3.14.7)            |
| ImageMagick (`magick`/`convert`) | available (7.1.2-32 Q16-HDRI) |
| ydotool                          | missing                       |
| gpu-screen-recorder              | missing                       |
| perf                             | missing                       |
| bats                             | missing                       |

Missing tools block affected captures; no tools were installed during inventory, per safety contract. Re-check this table at capture time; [README.md](README.md) requires a real capture to fail `MISSING_TOOL` rather than approximate.

## E2-D — Steam Deck Desktop Mode

Not inventoried. No hardware values recorded here until first capture.

## E2-G — Steam Deck Game Mode

Not inventoried. No hardware values recorded here until first capture.

## E3 — Mesa AMD/Intel desktop

Not inventoried. No hardware values recorded here until first capture.

## E3-sw — software rendering

Not inventoried. Software-rendering modifiers (`VK_ICD_FILENAMES=/dev/null`, `LIBGL_ALWAYS_SOFTWARE=1`, llvmpipe) are defined in [README.md](README.md) but no E3 host has been captured.
