"""G5 (input-to-photon) driver: clock sync from the app's samples, then one stamped key per trial.

The app arms a trial (`armed`), waits for one key and marks the DOM change (`render`, a
`performance.now()` stamp). The runner injects the key with `InputDevice.key_stamped`, maps the
render stamp onto CLOCK_MONOTONIC with the estimated offset and lets `select_trial` choose the
presented frame. Marks and the offset only select the frame: `latency_ms` is always
`presented_ns - inject_ns`.

Keys go out only while the shim shows keyboard focus on the bench toplevel (the same witness
G4 uses), after the toplevel has been quiet for `max(250 ms, 2 x p50 frame)` (p50 over frame-period gaps of
at most 100 ms, so idle time cannot inflate it) so a stale in-flight
frame cannot be mistaken for the change. A `focus_lost` mark, a locked session or an app exit stop
the run before the next key. The caller owns the InputDevice, the KWinScripts and the EventReader.
"""
from __future__ import annotations

import itertools
import math
import os
import statistics
import sys
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from bench_capture import CaptureError
from bench_drivers import (
    MARK_SLICE_S,
    _key_check,
    _live_logs,
    _LockGate,
    _lost,
    _notice,
    _require_running,
    _require_unlocked,
    _wait_focus,
    _window,
)
from bench_events import HEALTH_WAIT_S, ClockSample, require_probe_ok
from bench_g5select import EARLY, TIMEOUT, Trial, select_trial
from bench_input import (
    ALLOWED_SEARCH,
    KEY_BACKSPACE,
    KEY_ENTER,
    KEY_LEFT,
    KEY_RIGHT,
    LETTER_KEYS,
    InputDevice,
)
from bench_kwin import KWinScripts
from bench_present import FocusTail, toplevel_frames

G5_KEYS = {"search": (LETTER_KEYS[ALLOWED_SEARCH], KEY_BACKSPACE),
           "toggle_favorite": (KEY_ENTER,),
           "toggle_optimization": (KEY_ENTER,),
           "dpad": (KEY_RIGHT, KEY_LEFT),
           "route_switch": (KEY_ENTER,),
           "health_table": (KEY_ENTER,)}
CURSOR_DX, CURSOR_DY = 640, 450
CLOCK_MAX_HALF_RTT_NS = 2_000_000    # a best round trip above 4 ms cannot support a frame-level mapping
# performance.now() is floored to 1 ms, added to the uncertainty. SPIKE BRANCH (YAN-773): a native
# app whose t_perf_us is exact CLOCK_MONOTONIC microseconds sets CROSSHOOK_BENCH_CLOCK_FLOOR_NS=1000.
CLOCK_FLOOR_NS = int(os.environ.get("CROSSHOOK_BENCH_CLOCK_FLOOR_NS", "1000000"))
MIN_CLOCK_PAIRS = 10                 # adjacent pairs the app's samples must form (it sends at least 20)
MIN_FAST_CLOCK_PAIRS = 5             # pairs within CLOCK_FLOOR_NS of the best round trip the offset uses
CLOCK_SYNC_BUDGET_S = 35             # the app samples for up to 30 s while it is busy after READY
QUIET_MIN_NS = 250_000_000
QUIET_FRAME_FACTOR = 2
QUIET_FRAMES = 30                    # recent presented frames the p50 frame interval is taken over
QUIET_MAX_FRAME_GAP_NS = 200_000_000  # longer gaps are idle time or the runner's own waits (>= 250 ms), not a frame period
QUIET_TIMEOUT_S = 10.0              # native floor; scaled to 2 x the mark timeout (Flatpak: 40 s)
POLL_S = 0.02
QUIET_POLL_S = 0.05
MARK_TIMEOUT_S = 5.0
FIRST_ARMED_EXTRA_S = 40.0           # BENCH_BEGIN precedes the probe's prepare(): up to 4 waits of 10 s
# Per scenario: health_table navigates to the health page, waits up to HEALTH_WAIT_S for its rows, then returns.
FIRST_ARMED_EXTRA_BY_SCENARIO = {"health_table": HEALTH_WAIT_S + 20.0}
FLATPAK_FACTOR = 4                   # the sandbox is slower to arm and present than the native app


def first_armed_extra_s(scenario: str) -> float:
    """Seconds beyond the mark timeout the first `armed` mark may take after BENCH_BEGIN."""
    return FIRST_ARMED_EXTRA_BY_SCENARIO.get(scenario, FIRST_ARMED_EXTRA_S)


def mark_timeout_s(timeout_s: float, flatpak: bool) -> float:
    """Seconds to wait for one marker: scaled for Flatpak, and raised by a larger `--timeout`."""
    return max(MARK_TIMEOUT_S * (FLATPAK_FACTOR if flatpak else 1), timeout_s / 6)


@dataclass(frozen=True)
class ClockOffset:
    offset_ns: int       # CLOCK_MONOTONIC ns = performance.now() ns + offset_ns
    uncertainty_ns: int  # best round trip / 2 plus the 1 ms timer floor


@dataclass(frozen=True)
class TrialRecord:
    seq: int
    inject_ns: int
    render_ns: int       # the render mark on CLOCK_MONOTONIC; selection only, never a duration
    uncertainty_ns: int
    trial: Trial


@dataclass(frozen=True)
class G5Result:
    records: list[TrialRecord]
    clock: ClockOffset

    @property
    def driven_ns(self) -> int:
        stamps = [record.inject_ns for record in self.records]
        return stamps[-1] - stamps[0] if stamps else 0


def estimate_offset(samples: list[ClockSample]) -> ClockOffset:
    """NTP-style offset from the app's sequential `bench_clock` samples.

    Sample N+1's JS stamp is taken after call N returned, so for each adjacent pair the round trip
    is at most `t[N+1] - t[N]` and the Rust stamp lies between them: the offset is
    `rust_ns - midpoint`. Only the fast pairs count (round trip within `CLOCK_FLOOR_NS` of the
    best), because a pair taken while the app was busy has a wide midpoint; the result is their
    median offset and the uncertainty is half the slowest fast round trip plus the floor.
    `CLOCK_SYNC_FAILED` when there are too few pairs or fast pairs, the stamps go backwards or even
    the best round trip is too slow.
    """
    ordered = sorted(samples, key=lambda sample: sample.seq)
    pairs: list[tuple[int, int]] = []  # (round trip ns, offset ns)
    for current, following in itertools.pairwise(ordered):
        if following.seq != current.seq + 1:
            continue
        rtt_us = following.t_perf_us - current.t_perf_us
        if rtt_us < 0:
            raise CaptureError("CLOCK_SYNC_FAILED: performance.now() went backwards between samples")
        pairs.append((rtt_us * 1000, current.ns - (current.t_perf_us + following.t_perf_us) * 500))
    if len(pairs) < MIN_CLOCK_PAIRS:
        raise CaptureError(f"CLOCK_SYNC_FAILED: {len(pairs)} usable clock sample pairs, need {MIN_CLOCK_PAIRS}")
    best = min(rtt for rtt, _ in pairs)
    if best / 2 > CLOCK_MAX_HALF_RTT_NS:
        raise CaptureError(f"CLOCK_SYNC_FAILED: best round trip {best / 1e6:.2f} ms, "
                           f"half of it is above {CLOCK_MAX_HALF_RTT_NS / 1e6:.0f} ms")
    fast = [(rtt, offset) for rtt, offset in pairs if rtt <= best + CLOCK_FLOOR_NS]
    if len(fast) < MIN_FAST_CLOCK_PAIRS:
        raise CaptureError(f"CLOCK_SYNC_FAILED: {len(fast)} clock sample pairs within "
                           f"{CLOCK_FLOOR_NS / 1e6:.0f} ms of the best round trip, need {MIN_FAST_CLOCK_PAIRS}")
    offset = round(statistics.median(offset for _, offset in fast))
    return ClockOffset(offset, max(rtt for rtt, _ in fast) // 2 + CLOCK_FLOOR_NS)


def _quiet_ns(logs) -> tuple[int, int]:
    """(newest toplevel commit ns or 0, quiet time required) from the live logs."""
    last = max((commit.commit_ns for log in logs for commit in log.commits.values()
                if commit.toplevel and commit.has_buffer), default=0)
    try:
        times = [frame.presented_ns for frame in toplevel_frames(logs)][-QUIET_FRAMES:]
    except CaptureError as error:
        if not str(error).startswith("NO_TOPLEVEL_COMMITS"):
            raise
        times = []
    # Only gaps at frame-period scale: idle gaps and the runner's own quiet waits would otherwise
    # inflate the threshold trial after trial until the run hits NOT_QUIET.
    gaps = [gap for gap in (later - earlier for earlier, later in itertools.pairwise(times))
            if gap <= QUIET_MAX_FRAME_GAP_NS]
    frame_ns = statistics.median(gaps) if gaps else 0
    return last, max(QUIET_MIN_NS, math.ceil(QUIET_FRAME_FACTOR * frame_ns))


def _wait_quiet(proc, log_dir: Path, gate: _LockGate, seq: int, clock, sleep, timeout_s: float = QUIET_TIMEOUT_S) -> None:
    """Wait until no toplevel commit has been made for `max(250 ms, 2 x p50 frame)`."""
    deadline = clock() + int(timeout_s * 1e9)
    while True:
        _require_running(proc, f"before G5 trial {seq}")
        gate.check(f"before G5 trial {seq}")
        now = clock()
        last, needed = _quiet_ns(_live_logs(log_dir))
        if now - last >= needed:
            return
        if now >= deadline:
            raise CaptureError(f"NOT_QUIET: the window kept presenting for {timeout_s:.0f} s "
                               f"before G5 trial {seq}")
        sleep(QUIET_POLL_S)


class _Marks:
    """Mark waits that notice a dead app, a locked session or a lost focus between slices."""

    def __init__(self, proc, events, scenario: str, gate: _LockGate, since_ns: int, timeout_s: float):
        self._proc, self._events, self._scenario = proc, events, scenario
        self._gate, self._since_ns, self._timeout_s = gate, since_ns, timeout_s

    def check_focus(self, context: str) -> None:
        if self._events.focus_lost(self._scenario, self._since_ns) is not None:
            raise CaptureError(f"FOCUS_LOST: the app reported a lost window focus {context}")

    def wait(self, seq: int, phase: str, timeout_s: float | None = None):
        timeout_s = self._timeout_s if timeout_s is None else timeout_s
        context = f"while waiting for G5 {phase} {seq}"
        for _ in range(max(1, math.ceil(timeout_s / MARK_SLICE_S))):
            try:
                return self._events.wait_mark(self._scenario, seq, phase, MARK_SLICE_S)
            except CaptureError as error:
                if not str(error).startswith("MARK_TIMEOUT"):
                    raise
            _require_running(self._proc, context)
            self._gate.check(context)
            self.check_focus(context)
            require_probe_ok(self._events, self._scenario)
        raise CaptureError(f"MARK_TIMEOUT: {self._scenario} seq {seq} {phase} "
                           f"not seen within {timeout_s:g} s")


def _select(proc, log_dir: Path, inject_ns: int, render_ns: int, uncertainty_ns: int, deadline_ns: int,
            clock, sleep) -> Trial:
    """Select the frame once the logs show a decision; `timeout` only once the deadline has passed.

    The shim logs commits in time order, so a commit seen after the render window implies every
    commit inside it is already logged.
    """
    while True:
        _require_running(proc, "while selecting the G5 frame")
        trial = select_trial(_live_logs(log_dir), inject_ns, render_ns, uncertainty_ns, deadline_ns)
        if trial.status != TIMEOUT or clock() >= deadline_ns:
            return trial
        sleep(POLL_S)


def drive_g5(proc, scenario: str, log_dir: Path, kwin: KWinScripts, dev: InputDevice, events, trials: int,
             mark_timeout: float, clock: Callable[[], int] = time.monotonic_ns,
             sleep: Callable[[float], None] = time.sleep) -> G5Result:
    """Run `trials` stamped-key trials of `scenario` against an app that is armed by `events`."""
    if scenario not in G5_KEYS:
        raise CaptureError(f"UNKNOWN_SCENARIO: {scenario[:40]!r}")
    _notice()
    win = _window(proc, kwin, CURSOR_DX, CURSOR_DY)
    tail, gate = FocusTail(log_dir), _LockGate(clock)
    _wait_focus(tail, "keyboard", clock, sleep, lambda: _lost("keyboard", "after placing it"))
    _require_running(proc, "before arming the window")
    _require_unlocked("before arming the window")
    kwin.arm_under_cursor(win)  # active and on top, so the keys reach it
    marks = _Marks(proc, events, scenario, gate, clock(), mark_timeout)
    events.wait_begin(mark_timeout + CLOCK_SYNC_BUDGET_S)  # BEGIN follows the clock samples
    clock_offset = estimate_offset(events.clock_samples())
    check_key = _key_check(proc, tail, gate, "before a G5 key")
    keys = G5_KEYS[scenario]
    records: list[TrialRecord] = []
    noticed_early = False
    for seq in range(trials):
        # The first arm follows the probe's prepare() (setup steps after BENCH_BEGIN); later arms do not.
        marks.wait(seq, "armed", mark_timeout + first_armed_extra_s(scenario) if seq == 0 else None)
        _wait_quiet(proc, log_dir, gate, seq, clock, sleep, max(QUIET_TIMEOUT_S, 2 * mark_timeout))
        marks.check_focus(f"before G5 trial {seq}")
        check_key()  # app alive, session unlocked, keyboard enter on the toplevel
        inject_ns = dev.key_stamped(keys[seq % len(keys)])
        render = marks.wait(seq, "render")
        render_ns = render.t_perf_us * 1000 + clock_offset.offset_ns
        if render_ns < inject_ns - clock_offset.uncertainty_ns:
            # The change cannot precede the key: the clock mapping or the app's marks are off for this trial.
            if not noticed_early:
                noticed_early = True
                print(f"BENCH_NOTICE: G5 {scenario} seq {seq}: the render mark precedes the injected key by "
                      f"{(inject_ns - render_ns) / 1e6:.1f} ms (clock uncertainty "
                      f"{clock_offset.uncertainty_ns / 1e6:.1f} ms); such trials are `early` and not measured",
                      file=sys.stderr, flush=True)
            trial = Trial(EARLY)
        else:
            deadline_ns = inject_ns + int(mark_timeout * 1e9)
            trial = _select(proc, log_dir, inject_ns, render_ns, clock_offset.uncertainty_ns, deadline_ns,
                            clock, sleep)
        records.append(TrialRecord(seq, inject_ns, render_ns, clock_offset.uncertainty_ns, trial))
    events.check()  # a malformed marker line or BENCH_ERROR the waits never reached
    return G5Result(records, clock_offset)
