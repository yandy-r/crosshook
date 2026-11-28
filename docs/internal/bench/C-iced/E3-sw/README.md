# C-iced on E3-sw (llvmpipe)

M0-06 spike-matrix capture ([YAN-846](https://linear.app/itcn/issue/YAN-846)), 2026-11-26/27, fixtures seed 42 (500 profiles), 180.06 Hz. `spike/iced` `3cd03955`; iced selected wgpu GL on llvmpipe (not tiny-skia).

Results, method and caveats: [`spike-matrix.md`](../../spike-matrix.md). Only runs with `meta.json` status `captured` are kept; `provenance/` holds each run's `provenance.json` with home paths replaced by `~` (`meta.json` and `SHA256SUMS` are not committed because they hash local paths).
