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
# X records after which the shim's log can no longer vouch for the run: a capped log stopped,
# a failed bind or wrap never observed presentation or focus, no global means nothing was logged.
INCOMPLETE_CONDITIONS = ("log_cap", "bind_failed", "wrap_failed", "no_presentation_global")


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
    # Per device, the latest focus event (log order breaks ties) whose surface no later S record
    # destroyed: what `FocusTail` ends with. `focus` keeps every event as it was at event time.
    live_focus: dict[str, Focus] = field(default_factory=dict)


def _read_from(path: Path, offset: int = 0) -> tuple[bytes, int]:
    """The bytes of `path` from `offset` to its end, and its size.

    The one place a log is opened and read: O_NOFOLLOW, the size cap, and every OS error mapped
    to the log codes. Callers that tail a growing file pass the offset they stopped at.
    """
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_CLOEXEC", 0))
    except OSError as error:
        raise CaptureError(f"PRESENT_LOG_UNREADABLE: {path.name}: {error.strerror}") from error
    try:
        size = os.fstat(fd).st_size
        if size > MAX_LOG_BYTES:
            raise CaptureError(f"PRESENT_LOG_TOO_LARGE: {path.name}")
        data = os.pread(fd, size - offset, offset) if size > offset else b""
    except OSError as error:
        raise CaptureError(f"PRESENT_LOG_UNREADABLE: {path.name}: {error.strerror}") from error
    finally:
        os.close(fd)
    return data, size


def _read_text(path: Path) -> str:
    return _read_from(path)[0].decode("utf-8", errors="replace")


def _decode_role(fields: list[str]) -> tuple[int, str]:
    """An `S` record: (surface id, role)."""
    return int(fields[2]), fields[3]


def _decode_focus(fields: list[str], roles: dict[int, str]) -> Focus:
    """A `K` or `M` record, with the surface's role as of this point of the log."""
    surface_id = int(fields[3])
    return Focus(int(fields[1]), FOCUS_DEVICES[fields[0]], FOCUS_EVENTS[fields[2]], surface_id,
                 roles.get(surface_id) == "toplevel")


def _track_focus(live: dict[str, Focus], event: Focus) -> None:
    """Make `event` the device's live focus unless an event stamped later is already live."""
    previous = live.get(event.device)
    if previous is None or event.ns >= previous.ns:
        live[event.device] = event


def _end_focus(live: dict[str, Focus], surface_id: int) -> None:
    """A surface was destroyed: a focus on it, entered or not, is over for both devices."""
    for device in [device for device, event in live.items() if event.surface_id == surface_id]:
        del live[device]


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
    live_focus: dict[str, Focus] = {}
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
                surface_id, role = _decode_role(fields)
                roles[surface_id] = role
                if role == "destroyed":
                    _end_focus(live_focus, surface_id)
            elif kind == "C":
                surface_id = int(fields[2])
                commit = Commit(int(fields[1]), surface_id, int(fields[3]), int(fields[4]) == 1,
                                roles.get(surface_id) == "toplevel")
                commits[commit.seq] = commit
            elif kind == "P":
                record = Presented(*(int(value) for value in fields[1:]))
                presented[record.seq] = record
            elif kind in FOCUS_DEVICES:
                focus.append(_decode_focus(fields, roles))
                _track_focus(live_focus, focus[-1])
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
    return PresentLog(header, roles, commits, presented, discarded, conditions, focus, geometry, live_focus)


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
        # A capped log stops mid-run, a failed bind never observed presentation and a failed
        # wrap leaves a proxy's focus/geometry events unrecorded: none may stand in for a
        # complete capture.
        for condition in ("log_cap", "bind_failed", "wrap_failed"):
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


def toplevel_pids(logs: list[PresentLog]) -> set[int]:
    """The header `pid=` of every log that presented a toplevel frame (the shim writes it at load time).

    PRESENT_LOG_MALFORMED when such a log has no integer `pid=`.
    """
    pids = set()
    for log in logs:
        if not any(_is_toplevel_frame(log, seq) for seq in log.presented):
            continue
        try:
            pids.add(int(log.header["pid"]))
        except (KeyError, ValueError) as error:
            raise CaptureError("PRESENT_LOG_MALFORMED: header without an integer pid") from error
    return pids


def first_present_ns(logs: list[PresentLog]) -> int:
    return toplevel_frames(logs)[0].presented_ns


def first_present_at_or_after(logs: list[PresentLog], ns: int) -> int | None:
    """`presented_ns` of the first toplevel frame presented at or after `ns`, or None if there is none."""
    return next((frame.presented_ns for frame in toplevel_frames(logs) if frame.presented_ns >= ns), None)


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


def distinct_sizes(logs: list[PresentLog], start_ns: int | None = None, end_ns: int | None = None) -> int:
    return len({(item.w, item.h) for log in logs for item in log.geometry
                if item.toplevel
                and (start_ns is None or item.ns >= start_ns)
                and (end_ns is None or item.ns <= end_ns)})


@dataclass
class _Cursor:
    offset: int = 0  # bytes consumed: only complete records
    size: int = 0  # file size at the last read, partial record included
    lines: int = 0
    roles: dict[int, str] = field(default_factory=dict)
    live_focus: dict[str, Focus] = field(default_factory=dict)  # per device, this log's undestroyed focus


class FocusTail:
    """Keyboard and pointer focus from the shim logs, reading only what was appended since the last call.

    Decodes just the `S` role, `K`/`M` focus and `X` condition records, with the same strictness,
    the same reader and the same "toplevel at event time" rule as `parse_log`; the full parse
    happens once the app has stopped. An `X` record that ends the shim's evidence
    (INCOMPLETE_CONDITIONS) raises, because a log that stopped recording cannot witness focus.
    """

    def __init__(self, log_dir: Path):
        self._dir = Path(log_dir)
        self._cursors: dict[str, _Cursor] = {}

    @property
    def size(self) -> int:
        """Total bytes of every log as of the last read: it only grows while the shim keeps writing."""
        return sum(cursor.size for cursor in self._cursors.values())

    def focused(self, device: str) -> bool:
        """True when the latest `device` focus event, across every log, is an enter on a toplevel.

        A log that reports its focused surface destroyed drops that focus, even without a leave;
        surface ids are per process, so only that log's focus ends.
        """
        for path in sorted(self._dir.glob(LOG_GLOB)):
            self._read_new(path, self._cursors.setdefault(path.name, _Cursor()))
        latest: Focus | None = None
        for name in sorted(self._cursors):
            event = self._cursors[name].live_focus.get(device)
            if event is not None and (latest is None or event.ns >= latest.ns):
                latest = event
        return latest is not None and latest.entered and latest.toplevel

    def _read_new(self, path: Path, cursor: _Cursor) -> None:
        data, cursor.size = _read_from(path, cursor.offset)
        end = data.rfind(b"\n") + 1  # a trailing partial record waits for the next read
        cursor.offset += end
        for line in data[:end].decode("utf-8", errors="replace").split("\n")[:-1]:
            cursor.lines += 1
            self._apply(path.name, cursor, line)

    @staticmethod
    def _apply(name: str, cursor: _Cursor, line: str) -> None:
        fields = line.split("\t")
        kind = fields[0]
        if kind not in ("S", "X", *FOCUS_DEVICES):
            return
        try:
            if len(fields) != FIELD_COUNTS[kind]:
                raise ValueError(kind)
            if kind == "S":
                surface_id, role = _decode_role(fields)
                cursor.roles[surface_id] = role
                if role == "destroyed":
                    _end_focus(cursor.live_focus, surface_id)
                return
            if kind == "X":
                int(fields[1])
                if fields[2] in INCOMPLETE_CONDITIONS:
                    raise CaptureError(f"PRESENT_LOG_INCOMPLETE: {fields[2]}")
                return
            event = _decode_focus(fields, cursor.roles)
        except (ValueError, KeyError) as error:
            raise CaptureError(f"PRESENT_LOG_MALFORMED: {name}:{cursor.lines}") from error
        _track_focus(cursor.live_focus, event)
