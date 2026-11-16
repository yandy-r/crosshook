"""Vendor-neutral GPU memory adapter for the bench harness.

Order: DRM fdinfo (`/proc/<pid>/fdinfo/*`, amdgpu/i915/xe/...) first, then
`nvidia-smi pmon` (NVIDIA prints no `drm-*` keys). When neither answers the
sample is `unavailable` with a typed reason; a missing source is never 0.
"""
from __future__ import annotations

import math
import os
import re
import shutil
import subprocess
import threading
from collections.abc import Callable, Iterable
from dataclasses import dataclass

from bench_proc import CommandFailed, run_cancellable

SOURCE_FDINFO = "fdinfo"
SOURCE_NVIDIA_SMI = "nvidia-smi"
UNAVAILABLE = "unavailable"  # the source of a reading that has no value; also the prefix of `unavailable:<reason>`

REASON_NONE = ""
REASON_NO_DRM_KEYS = "no_drm_keys"
REASON_NO_TOOL = "no_tool"
REASON_NO_MATCHING_PID = "no_matching_pid"
REASON_TOOL_FAILED = "tool_failed"
REASON_NO_NUMERIC_FB = "no_numeric_fb"

NVIDIA_SMI = "nvidia-smi"
PMON_ARGS = ("pmon", "-s", "m", "-c", "1")
TOOL_TIMEOUT_S = 3.0
MAX_FDS_PER_PID = 4096
MAX_FDINFO_BYTES = 8 * 1024
MAX_TOOL_OUTPUT_BYTES = 1024 * 1024
MAX_PMON_ROWS = 4096
_MIB = {"": 1 / (1024 * 1024), "KiB": 1 / 1024, "MiB": 1.0, "GiB": 1024.0}
_SIZE = re.compile(r"(\d+)(?:[ \t]*(KiB|MiB|GiB))?")


class GpuToolError(RuntimeError):
    """The external GPU tool could not be run to completion."""


@dataclass(frozen=True)
class GpuSample:
    """One GPU memory reading. `mb` is None exactly when `source` is `unavailable`.

    `denied` counts fdinfo reads skipped for permission (EACCES/EPERM or a
    foreign UID); `tool_path` is the resolved `nvidia-smi` path, set whenever
    that stage was attempted, for provenance.
    """

    mb: float | None
    source: str
    reason: str = REASON_NONE
    denied: int = 0
    tool_path: str | None = None


@dataclass(frozen=True)
class FdinfoScan:
    """The fdinfo texts read for one pid plus the count of permission skips."""

    texts: tuple[str, ...]
    denied: int = 0


@dataclass(frozen=True)
class PmonTotals:
    """Result of filtering `pmon` rows to a pid set.

    `listed` counts matching rows, `numeric` those with a numeric `fb` (`-` is
    listed but not numeric), and `mb` sums the numeric values.
    """

    listed: int
    numeric: int
    mb: float


FdinfoReader = Callable[[int], FdinfoScan]
CommandRunner = Callable[[list[str], float, threading.Event | None], tuple[int, str]]


def read_fdinfo(pid: int, *, proc_root: str = "/proc", uid: int | None = None) -> FdinfoScan:
    """Read bounded fdinfo texts for one own-UID pid without following symlinks."""
    owner = os.getuid() if uid is None else uid
    base = os.path.join(proc_root, str(pid))
    try:
        if os.lstat(base).st_uid != owner:
            return FdinfoScan((), 1)
        entries = os.scandir(os.path.join(base, "fdinfo"))
    except PermissionError:
        return FdinfoScan((), 1)
    except OSError:  # pid exited, or no fdinfo directory
        return FdinfoScan((), 0)
    texts: list[str] = []
    denied = 0
    with entries:
        for index, entry in enumerate(entries):
            if index >= MAX_FDS_PER_PID:
                break
            try:
                if not entry.is_file(follow_symlinks=False):
                    continue
                fd = os.open(entry.path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
                try:
                    raw = os.read(fd, MAX_FDINFO_BYTES)
                finally:
                    os.close(fd)
            except PermissionError:
                denied += 1
                continue
            except OSError:  # the fd closed while scanning
                continue
            texts.append(raw.decode("utf-8", errors="replace"))
    return FdinfoScan(tuple(texts), denied)


def _size_mb(value: str) -> float | None:
    match = _SIZE.fullmatch(value.strip())
    if match is None:
        return None
    return int(match.group(1)) * _MIB[match.group(2) or ""]


def parse_fdinfo(text: str) -> tuple[tuple[str, str], float] | None:
    """Return `((drm-pdev, drm-client-id), MiB)` for one fdinfo text.

    None when the file has no DRM client id or no parseable memory key.
    `drm-total-*` is summed; `drm-resident-*` is used only when no total parses.
    Malformed values are ignored, not read as 0.
    """
    keys: dict[str, str] = {}
    for line in text.splitlines():
        key, sep, value = line.partition(":")
        if sep and key.startswith("drm-"):
            keys.setdefault(key, value.strip())
    client = keys.get("drm-client-id")
    if not client:
        return None
    for prefix in ("drm-total-", "drm-resident-"):
        sizes = [size for key, raw in keys.items() if key.startswith(prefix)
                 if (size := _size_mb(raw)) is not None]
        if sizes:
            return (keys.get("drm-pdev", ""), client), sum(sizes)
    return None


def _fdinfo_stage(pids: Iterable[int], reader: FdinfoReader) -> tuple[float | None, int, int]:
    """Sum de-duplicated DRM client memory: `(mb or None, denied, files_read)`."""
    clients: dict[tuple[str, str], float] = {}
    denied = files = 0
    for pid in sorted(set(pids)):
        scan = reader(pid)
        denied += scan.denied
        files += len(scan.texts)
        for text in scan.texts:
            parsed = parse_fdinfo(text)
            if parsed is not None:
                clients.setdefault(parsed[0], parsed[1])
    return (sum(clients.values()) if clients else None), denied, files


def parse_pmon(text: str, pids: Iterable[int]) -> PmonTotals:
    """Filter `nvidia-smi pmon -s m` rows to `pids`.

    Rows that are not for a wanted pid are ignored whatever they contain. A
    wanted row with an unparseable, negative or non-finite `fb` raises
    ValueError. `-` is listed but not numeric.
    """
    wanted = set(pids)
    listed = numeric = 0
    total = 0
    for row in text.splitlines()[:MAX_PMON_ROWS]:
        fields = row.split()
        if len(fields) < 4 or row.lstrip().startswith("#"):
            continue
        if not (fields[1].isascii() and fields[1].isdigit()) or int(fields[1]) not in wanted:
            continue
        listed += 1
        if fields[3] == "-":
            continue
        try:
            value = float(fields[3])
        except ValueError as error:
            raise ValueError("invalid NVIDIA memory") from error
        if not math.isfinite(value) or value < 0:
            raise ValueError("invalid NVIDIA memory")
        numeric += 1
        total += value
    return PmonTotals(listed, numeric, float(total))


def run_capture(command: list[str], timeout: float, cancel: threading.Event | None) -> tuple[int, str]:
    """Run `command` without a shell and return `(returncode, stdout)`.

    Delegates to `bench_proc.run_cancellable` (own session, `killpg` on timeout, cancel or more than
    `MAX_TOOL_OUTPUT_BYTES` of stdout, stdin closed); the output is decoded with `errors="replace"`.
    """
    try:
        return run_cancellable(command, timeout, cancel, stdout_limit=MAX_TOOL_OUTPUT_BYTES, step=.1)
    except CommandFailed as error:
        raise GpuToolError(f"{error.kind.upper()}: {command[0]}") from None


def _unavailable(reason: str, denied: int, tool_path: str | None = None) -> GpuSample:
    return GpuSample(None, UNAVAILABLE, reason, denied, tool_path)


def sample(pids: Iterable[int], *, fdinfo_reader: FdinfoReader = read_fdinfo,
           runner: CommandRunner = run_capture, which: Callable[[str], str | None] = shutil.which,
           cancel: threading.Event | None = None, timeout: float = TOOL_TIMEOUT_S) -> GpuSample:
    """Sum GPU memory in MiB for `pids` from the first source that answers.

    Never returns 0 for a missing source. Reasons for `unavailable`:
    `no_drm_keys` (nvidia-smi missing, fdinfo readable but without memory
    keys), `no_tool` (nvidia-smi missing and no fdinfo was readable),
    `no_matching_pid` (pmon ran but lists none of the pids), `no_numeric_fb`
    (pmon lists the pids but every `fb` is `-`, so there is no figure; this
    matches G3's `gpu_mb`) and `tool_failed` (nvidia-smi failed, timed out,
    exited non-zero, printed more than `MAX_TOOL_OUTPUT_BYTES` or returned a
    bad row). A pid with a numeric `fb` of 0 is a real 0.
    """
    pid_set = set(pids)
    mb, denied, files = _fdinfo_stage(pid_set, fdinfo_reader)
    if mb is not None:
        return GpuSample(mb, SOURCE_FDINFO, REASON_NONE, denied)
    path = which(NVIDIA_SMI)
    if path is None:
        return _unavailable(REASON_NO_DRM_KEYS if files else REASON_NO_TOOL, denied)
    try:
        returncode, stdout = runner([path, *PMON_ARGS], timeout, cancel)
        if returncode != 0:
            return _unavailable(REASON_TOOL_FAILED, denied, path)
        totals = parse_pmon(stdout, pid_set)
    except (GpuToolError, subprocess.SubprocessError, OSError, ValueError):
        return _unavailable(REASON_TOOL_FAILED, denied, path)
    if totals.listed == 0:
        return _unavailable(REASON_NO_MATCHING_PID, denied, path)
    if totals.numeric == 0:
        # The pids are listed but every fb is `-`: the driver reported no figure, not a zero.
        return _unavailable(REASON_NO_NUMERIC_FB, denied, path)
    return GpuSample(totals.mb, SOURCE_NVIDIA_SMI, REASON_NONE, denied, path)
