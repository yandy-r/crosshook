"""G4 `health` readiness: wait until the app has rendered its health page, before the priming notch.

The app opens the health route itself (`CROSSHOOK_BENCH_SCENARIO=health_scroll`), waits for the live
table to show enough rows and reports `health_ready` or `health_rows_too_few` before `bench_begin`.
The page re-renders after mount (the per-mount offline-readiness refetch), so the runner also waits
for the presentation to go idle; only then may the scroll start. `await_health_page` is the
`before_prime` hook of `drive_g4`. The caller owns the EventReader.
"""
from __future__ import annotations

import time
from collections.abc import Callable
from pathlib import Path

from bench_capture import CaptureError
from bench_drivers import (
    HEALTH_SCENARIO,
    IDLE_S,
    IDLE_TIMEOUT_S,
    _LockGate,
    _wait_idle,
    wait_sliced,
)
from bench_drivers_g5 import FLATPAK_FACTOR
from bench_events import HEALTH_WAIT_S, EventReader, require_probe_ok

BEGIN_EXTRA_S = 5.0   # slack on top of the app's row wait for the first presentation and the begin call


def await_health_page(proc, events: EventReader, log_dir: Path, mark_timeout: float, flatpak: bool,
                      clock: Callable[[], int] = time.monotonic_ns,
                      sleep: Callable[[float], None] = time.sleep) -> None:
    """Block until the health page is rendered and idle, or raise a `CaptureError`.

    `HEALTH_ROWS_TOO_FEW` (reported by the app, noticed between wait slices), `BEGIN_TIMEOUT`,
    `HEALTH_MARK_MISSING` (the app began without reporting `health_ready`) or `HEALTH_NOT_SETTLED`.
    """
    gate = _LockGate(clock)
    wait_sliced(proc, gate, events.wait_begin, "BEGIN_TIMEOUT", mark_timeout + HEALTH_WAIT_S + BEGIN_EXTRA_S,
                "while waiting for the health page", lambda: require_probe_ok(events, HEALTH_SCENARIO))
    require_probe_ok(events, HEALTH_SCENARIO)
    mark = events.status_mark(HEALTH_SCENARIO)
    if mark is None or mark.phase != "health_ready":
        raise CaptureError("HEALTH_MARK_MISSING: the app began without reporting that the health page is ready")
    idle_timeout_s = IDLE_TIMEOUT_S * (FLATPAK_FACTOR if flatpak else 1)
    _wait_idle(proc, log_dir, clock, sleep, idle_timeout_s, "HEALTH_NOT_SETTLED", IDLE_S, "health page")
