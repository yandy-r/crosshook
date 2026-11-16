"""Phase-tagged PSS/USS/GPU memory sampler for the bench G8 session; stdlib only.

The sampler never touches the scroll pacer. PSS is read on its own cadence and the GPU
on a separate, slower thread (a GPU probe may block for seconds), so a slow GPU read can
not stretch the PSS cadence. Both cadences are exposed as `tick_pss`/`tick_gpu` so tests
drive them with a fake clock and no threads.
"""
from __future__ import annotations

import os
import statistics
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

from bench_capture import process_tree
from bench_gpu import UNAVAILABLE

MEMORY_UNAVAILABLE = "MEMORY_UNAVAILABLE"
MEMORY_SAMPLER_FAILED = "MEMORY_SAMPLER_FAILED"
MEMORY_SAMPLES_EXCEEDED = "MEMORY_SAMPLES_EXCEEDED"
MEMORY_SAMPLES_INSUFFICIENT = "MEMORY_SAMPLES_INSUFFICIENT"
MIN_GROWTH_SAMPLES = 3
GPU_PROBE_ERROR = f"{UNAVAILABLE}:probe_error"  # the GPU probe raised: recorded, never read as 0


class MemoryBenchError(RuntimeError):
    """Raised with a stable code as the message prefix."""


class GpuReading(Protocol):
    """Anything shaped like `bench_gpu.GpuSample`."""

    @property
    def mb(self) -> float | None: ...
    @property
    def source(self) -> str: ...


@dataclass(frozen=True)
class ProcMemory:
    """One process's memory read, in kB, with the start time that identifies the process."""

    start_time: int
    uid: int
    pss_kb: int
    uss_kb: int


@dataclass(frozen=True)
class MemoryRow:
    t_ns: int
    phase: str
    pss_mb: float | None
    uss_mb: float | None
    gpu_mb: float | None
    gpu_source: str
    gpu_seq: int = 0  # identifies the 1 s GPU sample this row carries (0 = none yet)
    failed_pids: int = 0  # pids whose read failed, were foreign, or were reused
    gpu_t_ns: int = 0  # when the carried GPU sample was taken (its read began); 0 = none yet

    @property
    def flagged(self) -> bool:
        return self.failed_pids > 0


@dataclass(frozen=True)
class Growth:
    """median(tail) - median(baseline); `gpu_mb` is None when either side has no usable GPU sample."""

    pss_mb: float
    gpu_mb: float | None
    base_pss_mb: float
    tail_pss_mb: float
    base_gpu_mb: float | None
    tail_gpu_mb: float | None


def read_proc_memory(pid: int) -> ProcMemory:
    """Read start time, UID and smaps_rollup for `pid`; raise OSError/ValueError if it is unreadable
    or the pid changed identity mid-read."""
    start = _start_time(pid)
    uid = os.stat(f"/proc/{pid}").st_uid
    fields = {line.split(":", 1)[0]: int(line.split()[1])
              for line in Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines()[1:]}
    if _start_time(pid) != start:
        raise OSError(f"pid {pid} changed during read")
    return ProcMemory(start, uid, fields.get("Pss", 0),
                      fields.get("Private_Clean", 0) + fields.get("Private_Dirty", 0))


def stat_start_time(stat: str) -> int:
    """The start time (field 22) of a `/proc/<pid>/stat` text; ValueError/IndexError if it is malformed."""
    # comm can contain spaces/parentheses; fields after the final ')' begin at #3, so #22 is index 19.
    return int(stat.rsplit(")", 1)[1].split()[19])


def _start_time(pid: int) -> int:
    return stat_start_time(Path(f"/proc/{pid}/stat").read_text())


def tree_pids_fn(root_pid: int) -> Callable[[], set[int]]:
    """A `pids_fn` that walks the process tree below `root_pid` on every call."""
    return lambda: process_tree(root_pid)


class MemorySampler:
    """Samples PSS/USS every `pss_period` and the GPU every `gpu_period`, tagging rows with a phase."""

    def __init__(
        self,
        pids_fn: Callable[[], set[int]],
        gpu_fn: Callable[[set[int]], GpuReading],
        clock: Callable[[], int] = time.monotonic_ns,
        sleep: Callable[[float], None] = time.sleep,
        pss_period: float = 0.1,
        gpu_period: float = 1.0,
        max_samples: int = 20000,
        *,
        read_fn: Callable[[int], ProcMemory] = read_proc_memory,
        uid: int | None = None,
        phase: str = "init",
    ) -> None:
        self._pids_fn, self._gpu_fn = pids_fn, gpu_fn
        self._clock, self._sleep = clock, sleep
        self._pss_period, self._gpu_period = pss_period, gpu_period
        self._max_samples = max_samples
        self._read_fn = read_fn
        self._uid = os.getuid() if uid is None else uid
        self._phase = phase
        self._lock = threading.Lock()
        self._rows: list[MemoryRow] = []
        self._start_times: dict[int, int] = {}  # the first start time seen per pid: it is never replaced
        self._gpu: tuple[int, float | None, str, int] = (0, None, UNAVAILABLE, 0)
        self._error: MemoryBenchError | None = None
        self._stop = threading.Event()
        self._threads: list[threading.Thread] = []

    def set_phase(self, name: str) -> None:
        with self._lock:
            self._phase = name

    def start(self) -> None:
        if self._threads:
            raise MemoryBenchError("MEMORY_SAMPLER_STARTED")
        self.tick_gpu()  # the first PSS row already carries a GPU sample
        self._threads = [threading.Thread(target=self._loop, args=(self.tick_pss, self._pss_period),
                                          name="bench-memory-pss", daemon=True),
                         threading.Thread(target=self._loop, args=(self.tick_gpu, self._gpu_period),
                                          name="bench-memory-gpu", daemon=True)]
        for thread in self._threads:
            thread.start()

    def stop(self) -> list[MemoryRow]:
        """Stop the threads and return the rows; raise the sampler's deferred error, if any."""
        self._stop.set()
        for thread in self._threads:
            thread.join(timeout=5)
        return self.rows()

    def rows(self) -> list[MemoryRow]:
        with self._lock:
            if self._error is not None:
                raise self._error
            return list(self._rows)

    def __enter__(self) -> MemorySampler:
        self.start()
        return self

    def __exit__(self, *exc: object) -> None:
        self._stop.set()
        for thread in self._threads:
            thread.join(timeout=5)

    def _loop(self, tick: Callable[[], None], period: float) -> None:
        while not self._stop.is_set():
            try:
                tick()
            except Exception as error:  # noqa: BLE001 - a dying thread must fail the run, not go quiet
                with self._lock:
                    if self._error is None:
                        self._error = MemoryBenchError(f"{MEMORY_SAMPLER_FAILED}: {type(error).__name__}: {error}")
                self._stop.set()
                return
            if self._error is not None:
                return
            self._sleep(period)

    def tick_gpu(self) -> None:
        """Take one GPU sample; a failing probe is recorded as `unavailable:probe_error`, never as 0."""
        taken_ns = self._clock()
        try:
            reading = self._gpu_fn(set(self._pids_fn()))
            mb, source = reading.mb, reading.source
        except Exception:  # noqa: BLE001 - a probe failure must not kill the sampler
            mb, source = None, GPU_PROBE_ERROR
        with self._lock:
            self._gpu = (self._gpu[0] + 1, mb, source, taken_ns)

    def tick_pss(self) -> None:
        """Take one PSS/USS row. Past `max_samples` the sampler records MEMORY_SAMPLES_EXCEEDED and stops."""
        with self._lock:  # the phase and the stamp are read together: a row is tagged with the phase it began in
            t_ns, phase = self._clock(), self._phase
        pss_kb = uss_kb = readable = failed = 0
        for pid in sorted(self._pids_fn()):
            try:
                reading = self._read_fn(pid)
            except (OSError, ValueError):
                failed += 1
                continue
            known = self._start_times.setdefault(pid, reading.start_time)
            if reading.uid != self._uid or known != reading.start_time:
                failed += 1  # foreign pid, or a reused one: its first start time stays, the newcomer is never adopted
                continue
            readable += 1
            pss_kb += reading.pss_kb
            uss_kb += reading.uss_kb
        with self._lock:
            if len(self._rows) >= self._max_samples:
                self._error = MemoryBenchError(f"{MEMORY_SAMPLES_EXCEEDED}: more than {self._max_samples} samples")
                self._stop.set()
                return
            seq, gpu_mb, gpu_source, gpu_t_ns = self._gpu
            self._rows.append(MemoryRow(
                t_ns, phase,
                pss_kb / 1024 if readable else None, uss_kb / 1024 if readable else None,
                gpu_mb, gpu_source, seq, failed, gpu_t_ns))


def _phase_rows(rows: list[MemoryRow], phase: str) -> list[MemoryRow]:
    return [row for row in rows if row.phase == phase and row.pss_mb is not None]


def _gpu_median(rows: list[MemoryRow]) -> float | None:
    # One value per distinct GPU sample: rows repeat the latest 1 s sample, which would skew the median.
    # A sample taken before the phase's first row belongs to the previous phase and is left out.
    start_ns = min(row.t_ns for row in rows)
    values = {row.gpu_seq: row.gpu_mb for row in rows if row.gpu_mb is not None and row.gpu_t_ns >= start_ns}
    return statistics.median(values.values()) if values else None


def growth(rows: list[MemoryRow], base_phase: str, tail_phase: str) -> Growth:
    """median(tail) - median(baseline) for PSS and GPU.

    Raises MEMORY_UNAVAILABLE when a phase has no readable sample and MEMORY_SAMPLES_INSUFFICIENT
    when either phase has fewer than 3 PSS samples.
    """
    base, tail = _phase_rows(rows, base_phase), _phase_rows(rows, tail_phase)
    for name, phase_rows in ((base_phase, base), (tail_phase, tail)):
        if not phase_rows:
            raise MemoryBenchError(f"{MEMORY_UNAVAILABLE}: no readable samples in phase {name!r}")
        if len(phase_rows) < MIN_GROWTH_SAMPLES:
            raise MemoryBenchError(f"{MEMORY_SAMPLES_INSUFFICIENT}: phase {name!r} has {len(phase_rows)} "
                                   f"PSS samples, need {MIN_GROWTH_SAMPLES}")
    base_pss = statistics.median(row.pss_mb for row in base if row.pss_mb is not None)
    tail_pss = statistics.median(row.pss_mb for row in tail if row.pss_mb is not None)
    base_gpu, tail_gpu = _gpu_median(base), _gpu_median(tail)
    return Growth(tail_pss - base_pss,
                  None if base_gpu is None or tail_gpu is None else tail_gpu - base_gpu,
                  base_pss, tail_pss, base_gpu, tail_gpu)


def peak_pss(rows: list[MemoryRow], phase: str | None = None) -> float:
    """Highest PSS in MB, over every row or just `phase`; MEMORY_UNAVAILABLE if none is readable."""
    values = [row.pss_mb for row in rows if row.pss_mb is not None and (phase is None or row.phase == phase)]
    if not values:
        raise MemoryBenchError(f"{MEMORY_UNAVAILABLE}: no readable PSS samples")
    return max(values)
