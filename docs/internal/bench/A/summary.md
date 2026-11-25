# Classic variant A on E1

[YAN-795](https://linear.app/itcn/issue/YAN-795) records the E1 baseline needed by M0-03 and M0-08. Both uninstrumented builds came from `v0.6.1`, commit `948a415dbf8aa4fc00c21383977c42ad6277a332`. The Flatpak build used GNOME SDK 50 and ran against GNOME Platform 50 on the local `yan795-variant-a` branch. The [sync-command ranking](sync-command-ranking.md) used a separate native timing build.

## DMA-BUF A/B

The four passes ran on 2026-11-25 on G7 (E1), with Fedora 45, kernel 7.3.4-300.fc45, KWin Wayland, and NVIDIA RTX 5070 Laptop GPU driver 620.73.04. The host WebKitGTK was 2.54.2. Each pass started with a fresh isolated profile tree from seed 42 and was checked at the desktop once the Library loaded.

| Build   | Setting                                 | Outcome                                                                      |
| ------- | --------------------------------------- | ---------------------------------------------------------------------------- |
| Native  | Shipping default (application sets `1`) | Reached `READY`. Library worked.                                             |
| Native  | `0`                                     | Reached `READY`. Library worked and felt faster to the operator.             |
| Flatpak | Shipping default (manifest sets `1`)    | Reached `READY`. Library took a while to load and worked normally afterward. |
| Flatpak | `0`                                     | Reached `READY`. Library took a while to load and worked normally afterward. |

Both Flatpak logs report the same Mesa EGL warning, `egl: failed to create dri2 screen`. No blank window or crash was observed in any pass. The launcher captured absolute `READY` timestamps without a launch timestamp, so this check provides no elapsed startup comparison. The effective buffer transport was not measured.

**M0-08 verdict: safe to enable for the short E1 check on this software stack.** Both packages stayed usable with the override. This result is limited to E1 and does not establish stability on other driver versions or systems.

G6 and G7 are waived on E1. This capture covers the two Track 1 prerequisites. The optional G1–G10 matrix was not run. Steam Deck is out of scope, and E3 is tracked in [YAN-2054](https://linear.app/itcn/issue/YAN-2054).
