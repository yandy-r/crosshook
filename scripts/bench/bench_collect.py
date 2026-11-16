"""Presentation-time collection for G1/G2: first presented frame via the wlpresent shim."""
from __future__ import annotations

import time
from collections.abc import Callable
from pathlib import Path

from bench_capture import CaptureError, ready, stats, stop
from bench_present import check_refresh, first_present_ns, parse_dir, require_clock, toplevel_frames

PRESENT_POLL_S = 2.0
PRESENT_POLL_STEP_S = 0.05
# Flatpak receives these only through --env; the host `flatpak`/bwrap must never preload the shim.
FLATPAK_ONLY_ENV = ("LD_PRELOAD", "CROSSHOOK_BENCH_PRESENT_DIR")


def _wait_for_frame(present_dir: Path) -> None:
    deadline = time.monotonic() + PRESENT_POLL_S
    while time.monotonic() < deadline:
        try:
            toplevel_frames(parse_dir(present_dir))
            return
        except CaptureError:
            time.sleep(PRESENT_POLL_STEP_S)


def _first_present_ms(present_dir: Path, launch_ns: int, refresh_hz: float) -> float:
    logs = parse_dir(present_dir)
    require_clock(logs)
    frames = toplevel_frames(logs)
    check_refresh(frames, refresh_hz)
    value = (first_present_ns(logs) - launch_ns) / 1e6
    if value < 0:
        raise CaptureError("INVALID_PRESENT_CLOCK: first present precedes launch")
    return value


def _present_dir(env: dict[str, str], metric: str, number: int) -> Path:
    base = Path(env["HOME"]).parent / "present"
    for directory in (base, base / metric):
        directory.mkdir(mode=0o700, exist_ok=True)
    present_dir = base / metric / f"iter-{number}"
    try:
        present_dir.mkdir(mode=0o700)
    except FileExistsError:
        raise CaptureError(f"PRESENT_DIR_EXISTS: {present_dir} already exists") from None
    return present_dir


def _iteration(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str],
               metric: str, number: int) -> tuple[float, float]:
    present_dir = _present_dir(env, metric, number)
    env_i = {**env, "CROSSHOOK_BENCH_PRESENT_DIR": str(present_dir)}
    command_i = command_for(env_i)
    if command_i[0] == "flatpak":
        for key in FLATPAK_ONLY_ENV:
            env_i.pop(key, None)
    proc, ready_ms = ready(command_i, env_i, args.timeout, exit_after=False)
    try:
        _wait_for_frame(present_dir)
    finally:
        stop(proc)
        if proc.stderr:
            proc.stderr.close()
    return ready_ms, _first_present_ms(present_dir, proc.launch_ns, args.refresh_hz)


def presentation_rows(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str],
                      metric: str) -> list[dict]:
    rows = []
    for number in range(1, args.iterations + 1):
        ready_ms, first_ms = _iteration(args, command_for, env, metric, number)
        rows.append({"iteration": number, "ready_ms": ready_ms,
                     "first_present_ms": first_ms, "status": "measured"})
    ready_stats = stats([row["ready_ms"] for row in rows])
    present_stats = stats([row["first_present_ms"] for row in rows])
    rows.append({"iteration": "summary", "p50_ms": ready_stats["p50_ms"], "p95_ms": ready_stats["p95_ms"],
                 "first_present_p50_ms": present_stats["p50_ms"],
                 "first_present_p95_ms": present_stats["p95_ms"], "status": "measured"})
    return rows
