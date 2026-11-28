# C-gpui 30-minute soaks on E1

M0-06 soak results ([YAN-846](https://linear.app/itcn/issue/YAN-846)), the `#2572` check: 1,800 s with the 50,000-line seed-42 log replayed into a visible console (window 1600×1000) at 28 lines/s. Pass = `timeout` exit 124, `READY`, replay started, no crash markers (`panicked`, `SIGSEGV`, `DeviceLost`, …). Each `.txt` is the soak script's `result.txt`. The iced and gpui soaks for the same renderer class ran in parallel on the same host. Summary: [`spike-matrix.md`](../../spike-matrix.md).
