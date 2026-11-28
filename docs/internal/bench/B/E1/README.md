# B on E1 (native)

M0-06 spike-matrix capture ([YAN-846](https://linear.app/itcn/issue/YAN-846)), 2026-11-26/27, fixtures seed 42 (500 profiles), 180.06 Hz. Native release binary of `main` app source `ff035d9d`, built at `68617cad`. G1 is 20 single-iteration cold runs (page cache dropped before each) merged into one CSV. G4 is split per scenario (`library`, `settings`, opt-in `health`); `console` was not captured (finding 1).

Results, method and caveats: [`spike-matrix.md`](../../spike-matrix.md). Only runs with `meta.json` status `captured` are kept; `provenance/` holds each run's `provenance.json` with home paths replaced by `~` (`meta.json` and `SHA256SUMS` are not committed because they hash local paths).
