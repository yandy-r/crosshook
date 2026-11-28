# B on E3 (RX 9070 XT)

M0-06 spike-matrix capture ([YAN-846](https://linear.app/itcn/issue/YAN-846)), 2026-11-26/27, fixtures seed 42 (500 profiles), 180.06 Hz. `DRI_PRIME=1`, Mesa EGL vendor and `WEBKIT_DISABLE_DMABUF_RENDERER=1` (with DMA-BUF on, the window is black on this dual-GPU host). G4 was not captured (finding 1).

Results, method and caveats: [`spike-matrix.md`](../../spike-matrix.md). Only runs with `meta.json` status `captured` are kept; `provenance/` holds each run's `provenance.json` with home paths replaced by `~` (`meta.json` and `SHA256SUMS` are not committed because they hash local paths).
