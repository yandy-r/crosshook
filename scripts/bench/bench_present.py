"""Parser for the wlpresent v1 presentation log written by the Wayland shim.

Each line is one tab-separated record: H header, S surface role (`toplevel` or
`destroyed`), C commit, P presented, D discarded, X condition. Timestamps are CLOCK_MONOTONIC
nanoseconds. The parser is strict: a malformed line fails the capture instead
of silently skewing frame statistics. Files are read with O_NOFOLLOW and a
size cap, and an unterminated final line (a writer killed mid-record) is ignored.
"""
from __future__ import annotations

import os
import statistics
from dataclasses import dataclass
from pathlib import Path

from bench_capture import CaptureError

MAX_LOG_BYTES = 64 * 1024 * 1024
LOG_GLOB = "wlpresent-*.log"
CLOCK_MONOTONIC_ID = "1"
FIELD_COUNTS = {"S": 4, "C": 5, "P": 7, "D": 3, "X": 3}


@dataclass(frozen=True)
class Commit:
    commit_ns: int
    surface_id: int
    seq: int
    has_buffer: bool
    toplevel: bool  # the latest S record for the surface, before this commit, said toplevel


@dataclass(frozen=True)
class Presented:
    recv_ns: int
    seq: int
    presented_ns: int
    refresh_ns: int
    msc: int
    flags: int


@dataclass(frozen=True)
class PresentLog:
    header: dict
    roles: dict[int, str]  # final role per surface id; a reused id keeps only its latest role
    commits: dict[int, Commit]
    presented: dict[int, Presented]
    discarded: set[int]
    conditions: list[str]


def _read_text(path: Path) -> str:
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0))
    except OSError as error:
        raise CaptureError(f"PRESENT_LOG_UNREADABLE: {path.name}: {error.strerror}") from error
    with os.fdopen(fd, "rb") as handle:
        if os.fstat(handle.fileno()).st_size > MAX_LOG_BYTES:
            raise CaptureError(f"PRESENT_LOG_TOO_LARGE: {path.name}")
        data = handle.read(MAX_LOG_BYTES + 1)
    if len(data) > MAX_LOG_BYTES:
        raise CaptureError(f"PRESENT_LOG_TOO_LARGE: {path.name}")
    return data.decode("utf-8", errors="replace")


def parse_log(path: Path) -> PresentLog:
    path = Path(path)
    text = _read_text(path)
    lines = text.split("\n")
    lines.pop()  # text after the last newline is a partial record (or empty)
    header: dict = {}
    roles: dict[int, str] = {}
    commits: dict[int, Commit] = {}
    presented: dict[int, Presented] = {}
    discarded: set[int] = set()
    conditions: list[str] = []
    for lineno, line in enumerate(lines, 1):
        fields = line.split("\t")
        kind = fields[0]
        try:
            if kind == "H":
                for pair in fields[1:]:
                    key, separator, value = pair.partition("=")
                    if not separator or not key:
                        raise ValueError(pair)
                    header[key] = value
            elif FIELD_COUNTS.get(kind) != len(fields):
                raise ValueError(kind)
            elif kind == "S":
                roles[int(fields[2])] = fields[3]
            elif kind == "C":
                surface_id = int(fields[2])
                commit = Commit(int(fields[1]), surface_id, int(fields[3]), int(fields[4]) == 1,
                                roles.get(surface_id) == "toplevel")
                commits[commit.seq] = commit
            elif kind == "P":
                record = Presented(*(int(value) for value in fields[1:]))
                presented[record.seq] = record
            elif kind == "D":
                int(fields[1])
                discarded.add(int(fields[2]))
            else:
                int(fields[1])
                conditions.append(fields[2])
        except ValueError as error:
            raise CaptureError(f"PRESENT_LOG_MALFORMED: {path.name}:{lineno}") from error
    return PresentLog(header, roles, commits, presented, discarded, conditions)


def parse_dir(directory: Path) -> list[PresentLog]:
    paths = sorted(Path(directory).glob(LOG_GLOB), key=lambda p: p.name)
    if not paths:
        raise CaptureError(f"SHIM_NOT_LOADED: no wlpresent log in {directory}")
    return [parse_log(path) for path in paths]


def require_clock(logs: list[PresentLog]) -> None:
    # The shim always writes clock_id=-1 when there is no wp_presentation, so that case is
    # decided before the clock is looked at.
    without_global = [log for log in logs
                      if "no_presentation_global" in log.conditions or log.header.get("clock_id") == "-1"]
    if len(without_global) == len(logs):
        raise CaptureError("NO_PRESENTATION_GLOBAL: compositor does not expose wp_presentation")
    for log in logs:
        if log in without_global:
            continue
        clock = log.header.get("clock_id")
        if clock is None:
            raise CaptureError("PRESENT_LOG_MALFORMED: header without clock_id")
        if clock != CLOCK_MONOTONIC_ID:
            raise CaptureError(f"PRESENTATION_CLOCK: clock_id={clock}, expected 1 (CLOCK_MONOTONIC)")
        # A capped log stops mid-run and a failed bind never observed presentation:
        # neither may stand in for a complete capture.
        for condition in ("log_cap", "bind_failed"):
            if condition in log.conditions:
                raise CaptureError(f"PRESENT_LOG_INCOMPLETE: {condition}")


def _is_toplevel_frame(log: PresentLog, seq: int) -> bool:
    commit = log.commits.get(seq)
    return commit is not None and commit.has_buffer and commit.toplevel


def toplevel_frames(logs: list[PresentLog]) -> list[Presented]:
    frames = [record for log in logs for seq, record in log.presented.items()
              if _is_toplevel_frame(log, seq)]
    if not frames:
        raise CaptureError("NO_TOPLEVEL_COMMITS: no presented toplevel frame in the wlpresent log")
    return sorted(frames, key=lambda frame: frame.presented_ns)


def first_present_ns(logs: list[PresentLog]) -> int:
    return toplevel_frames(logs)[0].presented_ns


def frame_intervals_ms(frames: list[Presented], start_ns: int | None = None,
                       end_ns: int | None = None) -> list[float]:
    times = [frame.presented_ns for frame in frames
             if (start_ns is None or frame.presented_ns >= start_ns)
             and (end_ns is None or frame.presented_ns <= end_ns)]
    return [(later - earlier) / 1e6 for earlier, later in zip(times, times[1:])]


def check_refresh(frames: list[Presented], refresh_hz: float, tolerance: float = 0.005) -> float | None:
    periods = [frame.refresh_ns for frame in frames if frame.refresh_ns > 0]
    if not periods:
        return None
    median = statistics.median(periods)
    expected = 1e9 / refresh_hz
    observed_hz = 1e9 / median
    if abs(median - expected) / expected > tolerance:
        raise CaptureError(f"REFRESH_MISMATCH: compositor refresh {observed_hz:.2f} Hz, "
                           f"expected {refresh_hz:.2f} Hz; switch the output mode or pass --refresh-hz")
    return observed_hz


def discard_rate(logs: list[PresentLog]) -> float:
    shown = sum(1 for log in logs for seq in log.presented if _is_toplevel_frame(log, seq))
    dropped = sum(1 for log in logs for seq in log.discarded if _is_toplevel_frame(log, seq))
    return dropped / (shown + dropped) if shown + dropped else 0.0
