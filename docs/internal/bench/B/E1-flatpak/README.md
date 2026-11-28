# B on E1 (Flatpak)

M0-06 spike-matrix capture ([YAN-846](https://linear.app/itcn/issue/YAN-846)), 2026-11-26/27, fixtures seed 42 (500 profiles), 180.06 Hz. Local Flatpak branch `yan846-b` of the same source on GNOME 50. G4 has only `health`: the default scenarios stop at `console` (finding 1).

Results, method and caveats: [`spike-matrix.md`](../../spike-matrix.md). Only runs with `meta.json` status `captured` are kept; `provenance/` holds each run's `provenance.json` with home paths replaced by `~` (`meta.json` and `SHA256SUMS` are not committed because they hash local paths).
