"""Read-only measurement helpers for G7 and G10."""
from __future__ import annotations

import math
import re
import shutil
import subprocess
import tempfile
import threading
from pathlib import Path

import bench_gpu
from bench_proc import CANCELLED, CommandFailed, run_cancellable


class InstrumentError(RuntimeError):
    """Required instrumentation was unavailable or returned invalid evidence."""


def output(command: list[str], timeout: float = 10) -> str:
    try:
        return subprocess.run(command, check=True, capture_output=True, text=True, timeout=timeout).stdout
    except FileNotFoundError as error:
        raise InstrumentError(f"MISSING_TOOL: {command[0]}") from error
    except (subprocess.SubprocessError, OSError) as error:
        raise InstrumentError(f"INSTRUMENT_FAILED: {command[0]}") from error


def _run_cancellable(command: list[str], timeout: float, cancel: threading.Event | None) -> tuple[int, str]:
    """Run `command`, killing its process group on timeout or when `cancel` is set; returns its stderr."""
    try:
        return run_cancellable(command, timeout, cancel)
    except CommandFailed as error:
        raise InstrumentError(f"INSTRUMENT_{'CANCELLED' if error.kind == CANCELLED else 'FAILED'}: "
                              f"{command[0]}") from None


def wakeup_events(pids: set[int], duration: float, cancel: threading.Event | None = None) -> float:
    """Count sched_wakeup events targeting the app tree; requires perf permission.
    Setting `cancel` stops the perf run early (the capture failed elsewhere)."""
    if not pids:
        raise InstrumentError("WAKEUP_TARGETS_EMPTY")
    filter_expr = " || ".join(f"pid == {pid}" for pid in sorted(pids))
    try:
        # check=False semantics: parse perf's explicit unsupported/denied rows
        returncode, stderr = _run_cancellable(["perf", "stat", "-x,", "-a", "-e", "sched:sched_wakeup",
                                               "--filter", filter_expr, "--", "sleep", str(duration)],
                                              duration + 30, cancel)
    except FileNotFoundError as error:
        raise InstrumentError("MISSING_TOOL: perf") from error
    except (subprocess.SubprocessError, OSError) as error:
        raise InstrumentError("INSTRUMENT_FAILED: perf") from error
    for line in stderr.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2] == "sched:sched_wakeup":
            if fields[0] in ("<not supported>", "<not counted>"):
                break
            count = float(fields[0])
            if returncode or not math.isfinite(count) or count < 0:
                break
            return count / duration
    raise InstrumentError("INSTRUMENT_FAILED: perf sched_wakeup unavailable or permission denied")


def gpu_memory_mb(pids: set[int]) -> float | str:
    """NVIDIA per-process GPU memory; other GPUs report `unavailable` (YAN-1007).

    The pmon row parsing lives in `bench_gpu`; `bench_gpu.sample` is the
    vendor-neutral adapter (fdinfo first) used by the G8 sampler."""
    if shutil.which("nvidia-smi") is None:
        return "unavailable"
    # pmon covers graphics and compute contexts; fb is per (GPU, PID) MB, "-" when unavailable.
    rows = output(["nvidia-smi", "pmon", "-s", "m", "-c", "1"])
    try:
        totals = bench_gpu.parse_pmon(rows, pids)
    except ValueError as error:
        raise InstrumentError("INSTRUMENT_FAILED: invalid NVIDIA memory") from error
    return totals.mb if totals.numeric else "unavailable"


def battery_power_w(required: bool = True) -> float | str:
    """Battery discharge rate in W.

    A host without a battery fails with `BATTERY_REQUIRED` when power is a gate
    (the Deck, E2); elsewhere it reports `unavailable`, like `gpu_mb`.
    """
    paths = [line.strip() for line in output(["upower", "--enumerate"]).splitlines() if "battery" in line]
    if not paths:
        if not required:
            return "unavailable"
        raise InstrumentError("BATTERY_REQUIRED: no upower battery")
    for path in paths:
        match = re.search(r"energy-rate:\s+([0-9.]+)\s*W", output(["upower", "--show-info", path]))
        if match:
            value = float(match.group(1))
            if math.isfinite(value) and value >= 0:
                return value
    raise InstrumentError("INSTRUMENT_FAILED: battery energy-rate unavailable")


def size_mb(raw: str) -> float:
    match = re.fullmatch(r"\s*([0-9.]+)\s*([kKMGT]?)B?\s*", raw)
    if not match:
        raise InstrumentError("INSTRUMENT_FAILED: unreadable Flatpak size")
    scale = {"": 1, "k": 1e3, "K": 1e3, "M": 1e6, "G": 1e9, "T": 1e12}[match.group(2)]
    return float(match.group(1)) * scale / 1024**2


def sizes(binary: Path, bundle: Path, branch: str, runtime_delta_mb: float) -> dict:
    if not math.isfinite(runtime_delta_mb) or runtime_delta_mb < 0:
        raise InstrumentError("RUNTIME_DELTA_INVALID")
    with tempfile.TemporaryDirectory(prefix="crosshook-strip-") as directory:
        stripped = Path(directory) / "binary"
        # `-o`, not `--output`: newer binutils reject `--output` as ambiguous.
        output(["strip", "--strip-unneeded", "-o", str(stripped), str(binary)], 60)
        stripped_mb = stripped.stat().st_size / 1024**2
    # `flatpak info` takes the branch in the ref; it has no --branch option.
    installed = output(["flatpak", "info", "--show-size", f"dev.crosshook.CrossHook//{branch}"])
    return {"binary_mb": stripped_mb, "bundle_mb": bundle.stat().st_size / 1024**2,
            "installed_mb": size_mb(installed), "runtime_delta_mb": runtime_delta_mb,
            "status": "measured"}
