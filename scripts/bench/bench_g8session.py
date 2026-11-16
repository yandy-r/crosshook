"""G8 session binding and summary: what one launch produced, and proof that it all came from that launch.

A G8 row is built from three things: presented frames (the shim's present directory), memory samples
(the sampler) and the replay markers. They are only comparable when they describe the same launch, so
each is tagged with a `G8Session` (run id, iteration, launch pid, launch time) and `check_session` (pure)
refuses the iteration with `SESSION_MISMATCH` unless the tags agree and the data fits the session's clock.

`GpuProbe` and `PidRecorder` wrap the sampler's two inputs to keep what provenance needs: the resolved
`nvidia-smi` path, the most fdinfo reads skipped for permission, every process that was sampled (a pid
and its start time; with its `NSpid` under Flatpak) and whether the tree included a `WebKitWebProcess`.

The frames' session pid is not taken from the launch: `frames_pid` reads it from the shim log headers
(`pid=` of every log that presented a toplevel frame) and accepts it only as evidence of the launch.
"""
from __future__ import annotations

import dataclasses
import os
import re
import statistics
import threading
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path

import bench_gpu
from bench_capture import CaptureError, over_interval_percent, stats
from bench_gpu import UNAVAILABLE
from bench_memory import Growth, MemoryRow, stat_start_time, tree_pids_fn

SESSION_MISMATCH = "SESSION_MISMATCH"
NO_SAMPLE = "no_sample"
WEB_PROCESS = "WebKitWebProcess"
MAX_PROC_READ = 4096  # bytes read from a /proc text file: comm, cmdline and stat start well inside it
_ITER_DIR = re.compile(r"iter-(\d+)")


@dataclass(frozen=True)
class G8Session:
    """Identity of one launch: every frame and memory sample of an iteration must carry the same one."""

    run_id: str
    iteration: int
    pid: int
    launch_ns: int


@dataclass(frozen=True)
class SessionFrames:
    session: G8Session
    presented_ns: tuple[int, ...]


@dataclass(frozen=True)
class SessionMemory:
    session: G8Session
    rows: tuple[MemoryRow, ...]


def frames_session(run_id: str, present_dir: Path, pid: int, launch_ns: int) -> G8Session:
    """The session a present directory belongs to: its `iter-N` name gives the iteration."""
    match = _ITER_DIR.fullmatch(Path(present_dir).name)
    if match is None:
        raise CaptureError(f"{SESSION_MISMATCH}: {Path(present_dir).name!r} is not an iteration directory")
    return G8Session(run_id, int(match.group(1)), pid, launch_ns)


def frames_pid(shim_pids: set[int], root_pid: int, sampled: Iterable["SampledPid"], flatpak: bool) -> int:
    """The pid the presented frames vouch for: `root_pid` when the shim log pids are evidenced to be the launch's.

    `shim_pids` are the header `pid=` values of the logs that presented a toplevel frame. Natively the shim
    runs in the launched process, so each must equal `root_pid`. Under Flatpak the shim logs its sandbox pid:
    it is accepted when it is the innermost `NSpid` of a sampled process or the host pid of a sampled one.
    Any other pid is returned as it is, so that `check_session` reports it as another session.
    """
    if not shim_pids:
        raise CaptureError(f"{SESSION_MISMATCH}: no shim log pid to tie the frames to this launch")
    allowed = {root_pid}
    if flatpak:
        for entry in sampled:
            allowed.add(entry.pid)
            allowed.update(entry.nspid[-1:])
    stray = sorted(shim_pids - allowed)
    return stray[0] if stray else root_pid


def check_session(expected: G8Session, frames: SessionFrames, memory: SessionMemory, window_start_ns: int,
                  window_end_ns: int) -> None:
    """Pure check that `frames` and `memory` both belong to `expected` and fit its clock.

    Besides the tags: nothing may be stamped before the launch, and the memory samples must span the
    whole frame window (a sampler of another launch cannot).
    """
    for label, held in (("frames", frames.session), ("memory", memory.session)):
        differing = [field.name for field in dataclasses.fields(G8Session)
                     if getattr(held, field.name) != getattr(expected, field.name)]
        if differing:
            raise CaptureError(f"{SESSION_MISMATCH}: the {label} come from another session "
                               f"({', '.join(differing)} differ)")
    if frames.presented_ns and min(frames.presented_ns) < expected.launch_ns:
        raise CaptureError(f"{SESSION_MISMATCH}: a presented frame precedes this launch")
    times = [row.t_ns for row in memory.rows]
    if not times:
        raise CaptureError(f"{SESSION_MISMATCH}: no memory samples for this launch")
    if min(times) < expected.launch_ns:
        raise CaptureError(f"{SESSION_MISMATCH}: a memory sample precedes this launch")
    if min(times) > window_start_ns or max(times) < window_end_ns:
        raise CaptureError(f"{SESSION_MISMATCH}: the memory samples do not span the frame window")


class GpuProbe:
    """`bench_gpu.sample` that labels an unavailable reading `unavailable:<reason>` and keeps the tool path
    and the highest `denied` count.

    The sampler records only a reading's `source`; folding the reason into it keeps every sample's
    source explicit, and an unavailable one never turns into 0.
    """

    def __init__(self, sample: Callable[[Iterable[int]], bench_gpu.GpuSample] = bench_gpu.sample):
        self._sample = sample
        self._lock = threading.Lock()
        self.tool_path: str | None = None
        self.denied_max = 0  # the most fdinfo reads skipped for permission in any one sample

    def __call__(self, pids: Iterable[int]) -> bench_gpu.GpuSample:
        reading = self._sample(pids)
        with self._lock:
            if reading.tool_path:
                self.tool_path = reading.tool_path
            self.denied_max = max(self.denied_max, reading.denied)
        if reading.mb is None:
            return dataclasses.replace(reading, source=f"{UNAVAILABLE}:{reading.reason or NO_SAMPLE}")
        return reading


@dataclass(frozen=True)
class SampledPid:
    pid: int
    comm: str
    nspid: tuple[int, ...]  # host pid first, innermost namespace last; empty unless asked for
    web_process: bool
    start_time: int | None = None  # /proc stat field 22: with the pid it identifies the process


def read_proc_file(pid: int, name: str) -> str:
    """At most `MAX_PROC_READ` bytes of `/proc/<pid>/<name>`, so a hostile cmdline cannot grow the record."""
    fd = os.open(f"/proc/{pid}/{name}", os.O_RDONLY | os.O_CLOEXEC)
    try:
        return os.read(fd, MAX_PROC_READ).decode("utf-8", errors="replace")
    finally:
        os.close(fd)


def _start_time(pid: int, read: Callable[[int, str], str]) -> int | None:
    try:
        return stat_start_time(read(pid, "stat"))
    except (OSError, ValueError, IndexError):
        return None  # the process left, or the text is unusable: it is recorded without a start time


def _describe(pid: int, start_time: int | None, flatpak: bool, read: Callable[[int, str], str]) -> SampledPid:
    comm, nspid, cmdline = "", (), ""
    try:
        for line in read(pid, "status").splitlines():
            key, _, value = line.partition(":")
            if key == "Name":
                comm = value.strip()
            elif key == "NSpid" and flatpak:
                nspid = tuple(int(part) for part in value.split())
        cmdline = read(pid, "cmdline").replace("\0", " ")
    except (OSError, ValueError):
        pass  # the process left before it could be read: it is still recorded, with what is known
    return SampledPid(pid, comm, nspid, WEB_PROCESS in cmdline or "WebKitWebProc" in comm, start_time)


class PidRecorder:
    """A `pids_fn` for the sampler that also remembers every process it returned, described on first sight.

    A process is its pid and start time, so a pid that was reused by another process is described again
    instead of being taken for the first one.
    """

    def __init__(self, root_pid: int, flatpak: bool = False,
                 pids_fn: Callable[[], set[int]] | None = None,
                 read: Callable[[int, str], str] = read_proc_file):
        self.root_pid = root_pid
        self._flatpak, self._read = flatpak, read
        self._pids_fn = pids_fn or tree_pids_fn(root_pid)
        self._seen: dict[tuple[int, int | None], SampledPid] = {}
        self._lock = threading.Lock()

    def __call__(self) -> set[int]:
        pids = self._pids_fn()
        with self._lock:
            for pid in sorted(pids):
                start_time = _start_time(pid, self._read)
                if (pid, start_time) not in self._seen:
                    self._seen[pid, start_time] = _describe(pid, start_time, self._flatpak, self._read)
        return pids

    def sampled(self) -> list[SampledPid]:
        with self._lock:
            return [self._seen[key] for key in sorted(self._seen, key=lambda key: (key[0], key[1] or 0))]

    def web_process_sampled(self) -> bool:
        return any(entry.web_process for entry in self.sampled())


@dataclass(frozen=True)
class G8Iteration:
    """What one launch contributes to the row; built only after `check_session` passed."""

    session: G8Session
    intervals_ms: list[float]
    growth: Growth
    peak_pss_mb: float
    gpu_source: str
    follow: str


def iteration_gpu_source(rows: Iterable[MemoryRow], growth: Growth) -> str:
    """`fdinfo`/`nvidia-smi` when GPU growth exists, else an `unavailable:<reason>`.

    Without growth the reason is the first `unavailable:*` source among the baseline and tail rows (in row
    order), or `unavailable:no_sample` when none carries one: a side that has numeric sources but no GPU
    sample of its own phase. A numeric source is never given a reason.
    """
    kept = [row for row in rows if row.phase in ("baseline", "tail")]
    if growth.gpu_mb is not None:
        return "+".join(sorted({row.gpu_source for row in kept if row.gpu_mb is not None}))
    prefix = f"{UNAVAILABLE}:"
    return next((row.gpu_source for row in kept if row.gpu_source.startswith(prefix)), f"{prefix}{NO_SAMPLE}")


def _joined(values: Iterable[str], mixed: str) -> str:
    unique = sorted(set(values))
    return unique[0] if len(unique) == 1 else mixed


def summarize(scenario: str, iterations: list[G8Iteration], refresh_hz: float) -> dict:
    """The G8 row: intervals pooled over iterations; growth is the median of the per-iteration growths.

    GPU growth is blank unless every iteration has it, so a partial GPU view is never presented as whole.
    """
    values = [value for iteration in iterations for value in iteration.intervals_ms]
    gpu = [iteration.growth.gpu_mb for iteration in iterations]
    return {"scenario": scenario, **stats(values),
            "over_interval_percent": over_interval_percent(values, refresh_hz), "frames": len(values),
            "memory_growth_mb": statistics.median(iteration.growth.pss_mb for iteration in iterations),
            "peak_pss_mb": max(iteration.peak_pss_mb for iteration in iterations),
            "gpu_growth_mb": None if any(value is None for value in gpu) else statistics.median(gpu),
            "gpu_source": "+".join(sorted({iteration.gpu_source for iteration in iterations})),
            "follow": _joined((iteration.follow for iteration in iterations), "mixed"),
            "status": "captured"}
