"""G8 (console replay) single-session driver: one launch measures the scroll and the memory.

The app parks a 50k-line replay at READY. Its probe checks the console is on screen and reports
`follow_on`/`follow_off` (or `console_hidden`, which ends the run); the runner then holds the
replay back until everything it needs is in place and releases it with a `GO\\n` line on the app's
stdin. That pipe is the runner's alone, so the webview cannot forge the go signal.

Sequence: place the window at 1600x1000 (the wide layout draws the drawer) with the cursor over
the console, wait for `bench_begin`, require a visible console, wait for the presentation to go
idle (1 s without a frame), sample memory for the `baseline` phase, prime the pointer, send `GO`, scroll with the G4
pattern while the replay runs (`replay` phase) until `BENCH_REPLAY done`, wait for the window to
settle (`settle`: 3 s without a frame, as a replay frame can take over 2 s), then sample the `tail`. Witnesses (app alive, session unlocked, pointer on the
window) run before every notch; `proc.stdin` is closed on every exit path. The caller owns the
InputDevice, KWinScripts, EventReader and the sampler's readers.
"""
from __future__ import annotations

import contextlib
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

from bench_capture import CaptureError
from bench_drivers import (
    CONSOLE_CURSOR,
    CONSOLE_WINDOW_H,
    CONSOLE_WINDOW_W,
    IDLE_S,
    IDLE_TIMEOUT_S,
    NOTCH_S,
    TAIL_NS,
    _GrowthWatch,
    _LockGate,
    _lost,
    _notice,
    _pace,
    _require_running,
    _require_unlocked,
    _scroll_pattern,
    _wait_focus,
    _wait_idle,
    _window,
    prepare_scenario,
    wait_sliced,
)
from bench_drivers_g5 import FLATPAK_FACTOR
from bench_events import Mark
from bench_guard import ENV_SCENARIO
from bench_input import InputDevice
from bench_kwin import KWinScripts
from bench_memory import MemoryBenchError, MemoryRow
from bench_present import FocusTail

SCENARIO = "replay_scroll"
WINDOW_W, WINDOW_H = CONSOLE_WINDOW_W, CONSOLE_WINDOW_H  # the narrow layout has no console drawer
CURSOR_DX, CURSOR_DY = CONSOLE_CURSOR
BEGIN_EXTRA_S = 35.0              # BENCH_BEGIN follows the probe's console wait (up to 30 s)
BASELINE_S = 3.0
SETTLE_IDLE_S = 3.0               # a replay frame can take over 2 s, so 1 s of quiet does not mean the replay is over
TAIL_S = 3.0
HOLD_SLICE_S = 0.5                # baseline/tail waits are sliced so a dead app or lock is noticed early
REPLAY_LINES_PER_S = 5000
REPLAY_SLACK_FACTOR = 4           # the replay may take up to 4x its nominal time
REPLAY_SLACK_S = 30.0
REPLAY_STALL_S = 10.0             # a replay frame can exceed 2 s: the shim log may stay quiet this long
GO_LINE = b"GO\n"
_NOTCHES = _scroll_pattern()


@dataclass(frozen=True)
class G8Result:
    start_ns: int            # BENCH_REPLAY start
    done_ns: int             # BENCH_REPLAY done
    settled_ns: int          # presentation time of the last frame once the window went idle (>= done_ns)
    follow: str              # "on" or "off": the console's follow state at READY
    rows: list[MemoryRow]
    notches: int             # wheel notches sent while the replay ran
    driven_ns: int           # first to last notch
    window_start_ns: int     # frame window [start, done + 300 ms]
    window_end_ns: int


class _Sampler(Protocol):
    def set_phase(self, name: str) -> None: ...
    def start(self) -> None: ...
    def stop(self) -> list[MemoryRow]: ...


def prepare_g8_scenario(env: dict[str, str], flatpak: bool) -> dict[str, str]:
    """`env` for a G8 launch: the G4 console prep (drawer expanded, 50k-line replay log and rate) plus the
    `replay_scroll` scenario that parks the replay until `GO`. Flatpak forwards both only through `--env`."""
    updated = prepare_scenario("console", env, flatpak)
    updated[ENV_SCENARIO] = SCENARIO
    return updated


def replay_bound_s(lines: int) -> float:
    """Wall-time bound on the replay: 4x its nominal duration at the 5k lines/s cap, plus 30 s."""
    return lines / REPLAY_LINES_PER_S * REPLAY_SLACK_FACTOR + REPLAY_SLACK_S


def _require_console(events) -> None:
    mark = events.status_mark(SCENARIO)
    if mark is not None and mark.phase == "console_hidden":
        raise CaptureError("CONSOLE_NOT_VISIBLE: the console log is not visible "
                           "(drawer collapsed or status-bar layout)")


def _follow_state(events) -> str:
    mark: Mark | None = events.status_mark(SCENARIO)
    if mark is None:
        raise CaptureError("REPLAY_MARK_MISSING: the app began without reporting the console follow state")
    return "on" if mark.phase == "follow_on" else "off"


def _hold(proc, gate: _LockGate, seconds: float, sleep: Callable[[float], None], context: str) -> None:
    """Sleep `seconds` in slices, requiring the app alive and the session unlocked between them."""
    steps = max(1, round(seconds / HOLD_SLICE_S))
    for _ in range(steps):
        _require_running(proc, context)
        gate.check(context)
        sleep(seconds / steps)


def _send_go(proc) -> None:
    """Write `GO\\n` to the app's stdin; a closed pipe means the app is gone."""
    stream = getattr(proc, "stdin", None)
    if stream is None:
        raise CaptureError("GO_STDIN_MISSING: the app was launched without a stdin pipe")
    try:
        stream.write(GO_LINE)
        stream.flush()
    except (BrokenPipeError, ValueError, OSError) as error:
        raise CaptureError("APP_EXIT: the app closed its stdin before the GO signal") from error


def _close_stdin(proc) -> None:
    stream = getattr(proc, "stdin", None)
    if stream is not None:
        with contextlib.suppress(OSError, ValueError):
            stream.close()


def _stop_sampler(sampler: _Sampler) -> list[MemoryRow]:
    try:
        return sampler.stop()
    except MemoryBenchError as error:
        raise CaptureError(str(error)) from error


def _scroll_replay(proc, events, tail: FocusTail, dev: InputDevice, gate: _LockGate, since_ns: int,
                   start_lines: int, clock, sleep) -> tuple[int, int, int, int]:
    """Scroll until `BENCH_REPLAY done`; return (done_ns, notches, first notch ns, last notch ns)."""
    period_ns = int(NOTCH_S * 1e9)
    growth = _GrowthWatch(tail, clock, REPLAY_STALL_S)
    started = due_ns = clock()
    deadline_ns = started + int(replay_bound_s(start_lines) * 1e9)
    first_ns = last_ns = notches = 0
    while True:
        try:
            done = events.wait_replay("done", 0)
            break
        except CaptureError as error:
            text = str(error)
            if text.startswith("EVENT_STREAM_ENDED"):
                _require_running(proc, "during the replay")  # a stream that ended is the app's exit when it is
                raise
            if not text.startswith("REPLAY_NOT_DONE"):
                raise
        if clock() >= deadline_ns:
            raise CaptureError(f"REPLAY_NOT_DONE: no replay done within {replay_bound_s(start_lines):.0f} s")
        due_ns = _pace(clock, sleep, due_ns, period_ns)
        _require_running(proc, "during the replay")
        gate.check("during the replay")
        if events.focus_lost(SCENARIO, since_ns) is not None:
            raise CaptureError("FOCUS_LOST: the app reported a lost window focus during the replay")
        if not tail.focused("pointer"):
            raise _lost("pointer", "during the replay")
        growth.check()
        last_ns = dev.wheel(_NOTCHES[notches % len(_NOTCHES)])
        first_ns = first_ns or last_ns
        notches += 1
    if not notches:
        raise CaptureError("REPLAY_NOT_SCROLLED: the replay finished before the first wheel notch")
    return done.ns, notches, first_ns, last_ns


def _drive(proc, log_dir: Path, kwin: KWinScripts, dev: InputDevice, events, sampler: _Sampler,
           mark_timeout: float, flatpak: bool, clock, sleep) -> G8Result:
    _notice()
    win = _window(proc, kwin, CURSOR_DX, CURSOR_DY, WINDOW_W, WINDOW_H)
    tail, gate = FocusTail(log_dir), _LockGate(clock)
    since_ns = clock()
    wait_sliced(proc, gate, events.wait_begin, "BEGIN_TIMEOUT", mark_timeout + BEGIN_EXTRA_S,
                 "while waiting for the app to begin", lambda: _require_console(events))
    _require_console(events)
    follow = _follow_state(events)
    idle_timeout_s = IDLE_TIMEOUT_S * (FLATPAK_FACTOR if flatpak else 1)
    # The baseline gate keeps the G4 1 s of quiet: the replay is parked, so nothing paces frames at 2 s.
    _wait_idle(proc, log_dir, clock, sleep, idle_timeout_s, "BASELINE_NOT_IDLE", IDLE_S)
    sampler.set_phase("baseline")
    sampler.start()
    try:
        _hold(proc, gate, BASELINE_S, sleep, "during the memory baseline")
        # One untimed notch makes KWin deliver the pointer enter it withholds after the window moved.
        _require_running(proc, "before the priming notch")
        _require_unlocked("before the priming notch")
        kwin.arm_under_cursor(win)
        dev.wheel(1)
        _wait_focus(tail, "pointer", clock, sleep, lambda: CaptureError(
            "POINTER_NOT_OVER_WINDOW: no pointer enter on the app window after placing it"))
        _require_running(proc, "before the GO signal")
        _send_go(proc)
        start = wait_sliced(proc, gate, lambda slice_s: events.wait_replay("start", slice_s),
                             "REPLAY_NOT_STARTED", mark_timeout, "after the GO signal")
        sampler.set_phase("replay")
        done_ns, notches, first_ns, last_ns = _scroll_replay(
            proc, events, tail, dev, gate, since_ns, start.lines, clock, sleep)
        sampler.set_phase("settle")
        last_frame = _wait_idle(proc, log_dir, clock, sleep, idle_timeout_s, idle_s=SETTLE_IDLE_S)
        sampler.set_phase("tail")
        _hold(proc, gate, TAIL_S, sleep, "during the memory tail")
        rows = _stop_sampler(sampler)
    finally:
        with contextlib.suppress(Exception):
            sampler.stop()  # idempotent; the failure path must not hide the original error
    events.check()  # a malformed marker line or BENCH_ERROR the waits never reached
    return G8Result(start.ns, done_ns, max(last_frame, done_ns), follow, rows, notches, last_ns - first_ns,
                    start.ns, done_ns + TAIL_NS)


def drive_g8(proc, log_dir: Path, kwin: KWinScripts, dev: InputDevice, events, sampler: _Sampler,
             mark_timeout: float, flatpak: bool, clock: Callable[[], int] = time.monotonic_ns,
             sleep: Callable[[float], None] = time.sleep) -> G8Result:
    """Run one G8 session against an app launched with `stdin=True` and `CROSSHOOK_BENCH_SCENARIO=replay_scroll`."""
    try:
        return _drive(proc, log_dir, kwin, dev, events, sampler, mark_timeout, flatpak, clock, sleep)
    finally:
        _close_stdin(proc)
