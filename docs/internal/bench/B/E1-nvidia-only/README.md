# B on E1, NVIDIA only

M0-06 spot check ([YAN-846](https://linear.app/itcn/issue/YAN-846)), 2026-11-28, fresh boot, AMD output disabled so NVIDIA renders and scans out. Same native binary as [`../E1/`](../E1/) (`main` app source `ff035d9d`, built at `68617cad`), fixtures seed 42, 180.06 Hz. G4 is one CSV per scenario: `console` (1 iteration, the finding-1 check), `library` and `settings` (20 each); G8 has 20 iterations.

Results and comparison with the cross-GPU E1 runs: [`spike-matrix.md`](../../spike-matrix.md#pure-nvidia-spot-check-e1). `provenance/` holds each run's `provenance.json` with home paths replaced by `~`.
