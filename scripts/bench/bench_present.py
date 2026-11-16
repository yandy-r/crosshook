"""Parser for the wlpresent v1 presentation log written by the Wayland shim.

Each line is one tab-separated record: H header, S surface role (`toplevel` or
`destroyed`), C commit, P presented, D discarded, X condition, K keyboard focus,
M pointer focus, G toplevel configure geometry. Timestamps are CLOCK_MONOTONIC
nanoseconds. The parser is strict: a malformed line fails the capture instead
of silently skewing frame statistics. Files are read with O_NOFOLLOW and a
size cap, and an unterminated final line (a writer killed mid-record) is ignored.
"""
from __future__ import annotations

import os
import statistics
from dataclasses import dataclass, field
from pathlib import Path

from bench_capture import CaptureError

MAX_LOG_BYTES = 64 * 1024 * 1024
LOG_GLOB = "wlpresent-*.log"
CLOCK_MONOTONIC_ID = "1"
FIELD_COUNTS = {"S": 4, "C": 5, "P": 7, "D": 3, "X": 3, "K": 4, "M": 4, "G": 6}
FOCUS_DEVICES = {"K": "keyboard", "M": "pointer"}
FOCUS_EVENTS = {"enter": True, "leave": False}


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
class Focus:
    ns: int
    device: str  # "keyboard" or "pointer"
    entered: bool
    surface_id: int
    toplevel: bool = False  # the latest S record for the surface, before this event, said toplevel


@dataclass(frozen=True)
class Geometry:
    ns: int
    surface_id: int
    w: int
    h: int
    activated: bool
    toplevel: bool = False  # the latest S record for the surface, before this event, said toplevel


@dataclass(frozen=True)
class PresentLog:
    header: dict
    roles: dict[int, str]  # final role per surface id; a reused id keeps only its latest role
    commits: dict[int, Commit]
    presented: dict[int, Presented]
    discarded: set[int]
    conditions: list[str]
    focus: list[Focus] = field(default_factory=list)  # in log order
    geometry: list[Geometry] = field(default_factory=list)  # in log order


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
    """Parse a log, including one the shim is still writing: an incomplete last line is dropped."""
    path = Path(path)
    return _parse_text(_read_text(path), path.name)


def _parse_text(text: str, name: str) -> PresentLog:
    lines = text.split("\n")
    lines.pop()  # text after the last newline is a partial record (or empty)
    header: dict = {}
    roles: dict[int, str] = {}
    commits: dict[int, Commit] = {}
    presented: dict[int, Presented] = {}
    discarded: set[int] = set()
    conditions: list[str] = []
    focus: list[Focus] = []
    geometry: list[Geometry] = []
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
            elif kind in FOCUS_DEVICES:
                surface_id = int(fields[3])
                focus.append(Focus(int(fields[1]), FOCUS_DEVICES[kind], FOCUS_EVENTS[fields[2]],
                                   surface_id, roles.get(surface_id) == "toplevel"))
            elif kind == "G":
                surface_id = int(fields[2])
                activated = int(fields[5])
                if activated not in (0, 1):
                    raise ValueError(fields[5])
                geometry.append(Geometry(int(fields[1]), surface_id, int(fields[3]), int(fields[4]),
                                         activated == 1, roles.get(surface_id) == "toplevel"))
            elif kind == "D":
                int(fields[1])
                discarded.add(int(fields[2]))
            else:
                int(fields[1])
                conditions.append(fields[2])
        except (ValueError, KeyError) as error:
            raise CaptureError(f"PRESENT_LOG_MALFORMED: {name}:{lineno}") from error
    return PresentLog(header, roles, commits, presented, discarded, conditions, focus, geometry)


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


def discard_rate(logs: list[PresentLog], start_ns: int | None = None, end_ns: int | None = None) -> float:
    """Share of toplevel frames the compositor discarded; with a window, only frames committed inside it."""
    def counted(log: PresentLog, seq: int) -> bool:
        if not _is_toplevel_frame(log, seq):
            return False
        commit_ns = log.commits[seq].commit_ns
        return (start_ns is None or commit_ns >= start_ns) and (end_ns is None or commit_ns <= end_ns)

    shown = sum(1 for log in logs for seq in log.presented if counted(log, seq))
    dropped = sum(1 for log in logs for seq in log.discarded if counted(log, seq))
    return dropped / (shown + dropped) if shown + dropped else 0.0


def focused(logs: list[PresentLog], device: str, at_ns: int | None = None) -> bool:
    """True when the latest `device` focus event (at or before at_ns) is an enter on a toplevel."""
    latest: Focus | None = None
    for log in logs:
        for event in log.focus:
            if event.device != device or (at_ns is not None and event.ns > at_ns):
                continue
            if latest is None or event.ns >= latest.ns:
                latest = event
    return latest is not None and latest.entered and latest.toplevel


def distinct_sizes(logs: list[PresentLog], start_ns: int | None = None, end_ns: int | None = None) -> int:
    return len({(item.w, item.h) for log in logs for item in log.geometry
                if item.toplevel
                and (start_ns is None or item.ns >= start_ns)
                and (end_ns is None or item.ns <= end_ns)})

