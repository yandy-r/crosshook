"""G8 observation CSVs: the memory samples and the presented frames of the replay session.

`observations/G8-<scenario>-memory.csv` (`t_ns,phase,pss_mb,uss_mb,gpu_mb,gpu_source,failed_pids`) and
`observations/G8-<scenario>-frames.csv` (`presented_ns,frame_ms,in_window`) are written beside the shim
logs and sealed by `finalize_raw`. `frame_ms` is the interval to the previous frame of the same launch
(blank for a launch's first frame); `in_window` marks the frames inside `[start, done + 300 ms]`, and
the statistics use the intervals between consecutive in-window frames only. Times are CLOCK_MONOTONIC ns,
so rows of several launches stay in order. A missing reading is a blank, never 0.
"""
from __future__ import annotations

import csv
import io
import os
from dataclasses import dataclass
from pathlib import Path

from bench_g5io import OBSERVATIONS_DIR
from bench_memory import MemoryRow
from bench_provenance import write_private

MEMORY_COLUMNS = ("t_ns", "phase", "pss_mb", "uss_mb", "gpu_mb", "gpu_source", "failed_pids")
FRAME_COLUMNS = ("presented_ns", "frame_ms", "in_window")


@dataclass(frozen=True)
class FrameRecord:
    presented_ns: int
    frame_ms: float | None
    in_window: bool


def frame_records(presented_ns: list[int], start_ns: int, end_ns: int) -> list[FrameRecord]:
    """One record per frame, in time order, flagged against the window `[start_ns, end_ns]`."""
    ordered = sorted(presented_ns)
    return [FrameRecord(now, None if index == 0 else (now - ordered[index - 1]) / 1e6,
                        start_ns <= now <= end_ns)
            for index, now in enumerate(ordered)]


def _blank(value: float | None) -> str:
    return "" if value is None else f"{value:.3f}"


def memory_text(rows: list[MemoryRow]) -> str:
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(MEMORY_COLUMNS)
    for row in rows:
        writer.writerow([row.t_ns, row.phase, _blank(row.pss_mb), _blank(row.uss_mb), _blank(row.gpu_mb),
                         row.gpu_source, row.failed_pids])
    return out.getvalue()


def frames_text(frames: list[FrameRecord]) -> str:
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(FRAME_COLUMNS)
    for frame in frames:
        writer.writerow([frame.presented_ns, _blank(frame.frame_ms), int(frame.in_window)])
    return out.getvalue()


def write_observations(env: dict[str, str], scenario: str, rows: list[MemoryRow],
                       frames: list[FrameRecord]) -> tuple[Path, Path]:
    """Write both CSVs, replacing the previous (shorter) copy so a later failure keeps the evidence."""
    directory = Path(env["HOME"]).parent / OBSERVATIONS_DIR
    directory.mkdir(mode=0o700, exist_ok=True)
    paths = []
    for suffix, text in (("memory", memory_text(rows)), ("frames", frames_text(frames))):
        path = directory / f"G8-{scenario}-{suffix}.csv"
        if os.path.lexists(path):
            path.unlink()
        write_private(path, text)
        paths.append(path)
    return paths[0], paths[1]
