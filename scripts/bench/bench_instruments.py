"""Read-only measurement helpers for G7 and G10."""
from __future__ import annotations

import math
import re
import subprocess
import tempfile
from pathlib import Path


class InstrumentError(RuntimeError):
    """Required instrumentation was unavailable or returned invalid evidence."""


def output(command: list[str], timeout: float = 10) -> str:
    try:
        return subprocess.run(command, check=True, capture_output=True, text=True, timeout=timeout).stdout
    except FileNotFoundError as error:
        raise InstrumentError(f"MISSING_TOOL: {command[0]}") from error
    except (subprocess.SubprocessError, OSError) as error:
        raise InstrumentError(f"INSTRUMENT_FAILED: {command[0]}") from error


def wakeup_events(pids: set[int], duration: float) -> float:
    """Count sched_wakeup events targeting the app tree; requires perf permission."""
    if not pids:
        raise InstrumentError("WAKEUP_TARGETS_EMPTY")
    filter_expr = " || ".join(f"pid == {pid}" for pid in sorted(pids))
    try:
        result = subprocess.run(["perf", "stat", "-x,", "-a", "-e", "sched:sched_wakeup",
                                 "--filter", filter_expr, "--", "sleep", str(duration)],
                                capture_output=True, text=True, timeout=duration + 30,
                                check=False)  # parse perf's explicit unsupported/denied rows
    except FileNotFoundError as error:
        raise InstrumentError("MISSING_TOOL: perf") from error
    except subprocess.SubprocessError as error:
        raise InstrumentError("INSTRUMENT_FAILED: perf") from error
    for line in result.stderr.splitlines():
        fields = line.split(",")
        if len(fields) > 2 and fields[2] == "sched:sched_wakeup":
            if fields[0] in ("<not supported>", "<not counted>"):
                break
            count = float(fields[0])
            if result.returncode or not math.isfinite(count) or count < 0:
                break
            return count / duration
    raise InstrumentError("INSTRUMENT_FAILED: perf sched_wakeup unavailable or permission denied")


def gpu_memory_mb(pids: set[int]) -> float:
    """NVIDIA per-process GPU memory; other GPUs need a vendor-specific collector."""
    rows = output(["nvidia-smi", "--query-compute-apps=pid,used_memory",
                   "--format=csv,noheader,nounits"])
    values = []
    for row in rows.splitlines():
        fields = [field.strip() for field in row.split(",")]
        if len(fields) == 2 and fields[0].isdigit() and int(fields[0]) in pids:
            value = float(fields[1])
            if not math.isfinite(value) or value < 0:
                raise InstrumentError("INSTRUMENT_FAILED: invalid NVIDIA memory")
            values.append(value)
    if not values:
        raise InstrumentError("GPU_MEMORY_UNREPORTED: NVIDIA reported no app-tree compute process")
    return sum(values)


def battery_power_w() -> float:
    paths = [line.strip() for line in output(["upower", "--enumerate"]).splitlines() if "battery" in line]
    if not paths:
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
        output(["strip", "--strip-unneeded", "--output", str(stripped), str(binary)], 60)
        stripped_mb = stripped.stat().st_size / 1024**2
    installed = output(["flatpak", "info", "--show-size", f"--branch={branch}",
                        "dev.crosshook.CrossHook"])
    return {"binary_mb": stripped_mb, "bundle_mb": bundle.stat().st_size / 1024**2,
            "installed_mb": size_mb(installed), "runtime_delta_mb": runtime_delta_mb,
            "status": "measured"}
