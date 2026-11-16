"""Live reader for the app's `BENCH_*` stderr markers (bench mode only).

Wire contract, one integer-only line per marker:
  BENCH_BEGIN <ns> <scenario>
  BENCH_MARK <ns> <scenario> <seq> <phase> <t_perf_us>
  BENCH_CLOCK <ns> <seq> <t_perf_us>
  BENCH_REPLAY <ns> start|done <lines>
  BENCH_ERROR: <text>
Every other line is discarded. A malformed `BENCH_*` line is recorded, never guessed at.
"""
from __future__ import annotations

import re
import threading
import time
from collections.abc import Callable
from dataclasses import dataclass

from bench_capture import CaptureError

LINE_CAP = 65536
G5_SCENARIOS = frozenset({"search", "toggle_favorite", "toggle_optimization", "dpad", "route_switch"})
SCENARIOS = G5_SCENARIOS | {"replay_scroll"}
PHASES = frozenset({"armed", "render", "focus_lost", "console_hidden", "follow_on", "follow_off"})
REPLAY_PHASES = frozenset({"console_hidden", "follow_on", "follow_off"})
REPLAY_KINDS = ("start", "done")
REPLAY_MAX_LINES = 50000
_INT = re.compile(r"-?[0-9]+")


@dataclass(frozen=True)
class Begin:
    ns: int
    scenario: str


@dataclass(frozen=True)
class Mark:
    ns: int
    scenario: str
    seq: int
    phase: str
    t_perf_us: int


@dataclass(frozen=True)
class ClockSample:
    ns: int
    seq: int
    t_perf_us: int


@dataclass(frozen=True)
class ReplayEvent:
    """A `BENCH_REPLAY` marker: when the replay `start`ed or was `done`, and its line count."""
    ns: int
    lines: int


@dataclass(frozen=True)
class _Replay:
    kind: str
    event: ReplayEvent


class EventReader:
    """Reads `stream` on a daemon thread and exposes the parsed markers.

    `initial` carries bytes the caller already read past READY so none are lost.
    """

    def __init__(self, stream, launch_ns: int, clock: Callable[[], int] = time.monotonic_ns,
                 initial: bytes = b""):
        self._stream = stream
        self._launch_ns = launch_ns
        self._clock = clock
        self._cond = threading.Condition()
        self._begin: Begin | None = None
        self._marks: dict[tuple[str, int, str], Mark] = {}
        self._focus_lost: list[Mark] = []  # every blur is kept: a later one must not hide behind an earlier one
        self._replay_marks: list[Mark] = []
        self._replay: dict[str, ReplayEvent] = {}
        self._clocks: list[ClockSample] = []
        self._errors: list[str] = []
        self._eof = False
        self._closed = False
        self._thread = threading.Thread(target=self._run, args=(initial,), daemon=True)
        self._thread.start()

    # -- reader thread --------------------------------------------------------------------------
    def _run(self, pending: bytes) -> None:
        discarding = False  # inside an over-cap line: drop bytes up to its newline
        read = getattr(self._stream, "read1", self._stream.read)
        try:
            while not self._closed:
                while b"\n" in pending:
                    line, pending = pending.split(b"\n", 1)
                    if discarding:
                        discarding = False
                    elif len(line) > LINE_CAP:
                        self._record("UNBOUNDED_STDERR_LINE")
                    else:
                        self._handle(line.decode("utf-8", errors="replace").rstrip("\r"))
                if discarding:
                    pending = b""
                elif len(pending) > LINE_CAP:
                    self._record("UNBOUNDED_STDERR_LINE")
                    pending, discarding = b"", True
                chunk = read(LINE_CAP)
                if not chunk:
                    break
                pending += chunk
            if pending and not discarding and not self._closed:
                self._handle(pending.decode("utf-8", errors="replace").rstrip("\r"))
        except (OSError, ValueError):
            pass
        finally:
            with self._cond:
                self._eof = True
                self._cond.notify_all()

    def _record(self, error: str) -> None:
        with self._cond:
            self._errors.append(error)
            self._cond.notify_all()

    def _handle(self, text: str) -> None:
        if text.startswith("BENCH_ERROR:"):
            self._record(text)
            return
        fields = text.split()
        kind = fields[0] if fields else ""
        if kind not in ("BENCH_BEGIN", "BENCH_MARK", "BENCH_CLOCK", "BENCH_REPLAY"):
            return
        try:
            record = self._parse(kind, fields)
        except ValueError:
            self._record("BENCH_EVENT_MALFORMED: " + text[:80])
            return
        with self._cond:
            if isinstance(record, _Replay):
                problem = self._replay_problem(record)
                if problem:
                    self._errors.append(f"BENCH_EVENT_MALFORMED: {problem}: {text[:80]}")
                else:
                    self._replay[record.kind] = record.event
            elif isinstance(record, Begin):
                self._begin = self._begin or record
            elif isinstance(record, Mark) and record.phase in REPLAY_PHASES:
                self._replay_marks.append(record)
            elif isinstance(record, Mark) and record.phase == "focus_lost":
                self._focus_lost.append(record)
            elif isinstance(record, Mark):
                self._marks.setdefault((record.scenario, record.seq, record.phase), record)
            else:
                self._clocks.append(record)
            self._cond.notify_all()

    def _replay_problem(self, record: _Replay) -> str:
        """Why `record` breaks the once-each, start-before-done contract, or "" (caller holds the lock)."""
        if record.kind in self._replay:
            return f"duplicate replay {record.kind}"
        start = self._replay.get("start")
        if record.kind == "done" and (start is None or record.event.ns <= start.ns):
            return "replay done without an earlier start"
        return ""

    def _number(self, token: str, minimum: int = 0) -> int:
        if not _INT.fullmatch(token):
            raise ValueError(token)
        value = int(token)
        if value < minimum:
            raise ValueError(token)
        return value

    def _parse(self, kind: str, fields: list[str]) -> Begin | Mark | ClockSample | _Replay:
        count = {"BENCH_BEGIN": 3, "BENCH_MARK": 6, "BENCH_CLOCK": 4, "BENCH_REPLAY": 4}[kind]
        if len(fields) != count:
            raise ValueError("field count")
        ns = self._number(fields[1])
        if not self._launch_ns <= ns <= self._clock():
            raise ValueError("ns outside the launch window")
        if kind == "BENCH_REPLAY":
            if fields[2] not in REPLAY_KINDS:
                raise ValueError("replay kind")
            lines = self._number(fields[3])
            if lines > REPLAY_MAX_LINES:
                raise ValueError("replay lines")
            return _Replay(fields[2], ReplayEvent(ns, lines))
        if kind == "BENCH_CLOCK":
            return ClockSample(ns, self._number(fields[2]), self._number(fields[3]))
        if fields[2] not in SCENARIOS:
            raise ValueError("scenario")
        if kind == "BENCH_BEGIN":
            return Begin(ns, fields[2])
        if fields[4] not in PHASES:
            raise ValueError("phase")
        return Mark(ns, fields[2], self._number(fields[3]), fields[4], self._number(fields[5]))

    # -- runner API -----------------------------------------------------------------------------
    def _wait(self, found: Callable[[], object | None], timeout_s: float, code: str, detail: str = ""):
        suffix = f": {detail}" if detail else ""
        deadline = time.monotonic() + timeout_s
        with self._cond:
            while True:
                if self._errors:
                    raise CaptureError(self._errors[0])
                result = found()
                if result is not None:
                    return result
                if self._eof or self._closed:
                    raise CaptureError("EVENT_STREAM_ENDED: " + code + suffix)
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise CaptureError(code + suffix)
                self._cond.wait(remaining)

    def wait_begin(self, timeout_s: float) -> Begin:
        return self._wait(lambda: self._begin, timeout_s, "BEGIN_TIMEOUT")

    def wait_mark(self, scenario: str, seq: int, phase: str, timeout_s: float) -> Mark:
        """The first `armed`/`render` mark of the trial; `focus_lost` marks are read with `focus_lost`."""
        return self._wait(lambda: self._marks.get((scenario, seq, phase)), timeout_s,
                          "MARK_TIMEOUT", f"{scenario} seq {seq} {phase}")

    def wait_replay(self, kind: str, timeout_s: float) -> ReplayEvent:
        """The app's `BENCH_REPLAY` `start` or `done` marker (each is accepted once, `done` after `start`)."""
        if kind not in REPLAY_KINDS:
            raise ValueError(f"replay kind must be one of {REPLAY_KINDS}: {kind!r}")
        code = "REPLAY_NOT_STARTED" if kind == "start" else "REPLAY_NOT_DONE"
        return self._wait(lambda: self._replay.get(kind), timeout_s, code)

    def replay_mark(self, scenario: str) -> Mark | None:
        """The earliest `console_hidden`/`follow_on`/`follow_off` mark of `scenario`, if one arrived."""
        with self._cond:
            marks = [mark for mark in self._replay_marks if mark.scenario == scenario]
        return min(marks, key=lambda mark: mark.ns) if marks else None

    def focus_lost(self, scenario: str, since_ns: int) -> Mark | None:
        """The first `focus_lost` mark of `scenario` stamped at or after `since_ns`, if any arrived.

        A blur before the runner had placed the window (`since_ns`) says nothing about the trials.
        """
        with self._cond:
            lost = [mark for mark in self._focus_lost if mark.scenario == scenario and mark.ns >= since_ns]
        return min(lost, key=lambda mark: mark.ns) if lost else None

    def clock_samples(self) -> list[ClockSample]:
        with self._cond:
            return list(self._clocks)

    def check(self) -> None:
        with self._cond:
            if self._errors:
                raise CaptureError(self._errors[0])

    def close(self) -> None:
        """Stop reading and wake waiters. The stream stays the caller's to close."""
        with self._cond:
            self._closed = True
            self._cond.notify_all()
        self._thread.join(timeout=.5)
