# C-iced on E3 (RX 9070 XT)

M0-06 spike-matrix capture ([YAN-846](https://linear.app/itcn/issue/YAN-846)), 2026-11-26/27, fixtures seed 42 (500 profiles), 180.06 Hz. `spike/iced` `3cd03955`, RADV only (`VK_ICD_FILENAMES=radeon_icd`). G4, G8 and G9 failed with `DISCARD_RATE` during PCIe AER bursts on the AMD link, and the re-run hung when the link failed (finding 3).

Results, method and caveats: [`spike-matrix.md`](../../spike-matrix.md). Only runs with `meta.json` status `captured` are kept; `provenance/` holds each run's `provenance.json` with home paths replaced by `~` (`meta.json` and `SHA256SUMS` are not committed because they hash local paths).
