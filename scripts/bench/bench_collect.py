"""Presentation-time collection via the wlpresent shim: G1/G2 first frame, G4/G9 driven frame intervals."""
from __future__ import annotations

import contextlib
import secrets
import time
from collections.abc import Callable
from pathlib import Path

from bench_capture import CaptureError, over_interval_percent, ready, stats, stop
from bench_drivers import (
    DriveResult,
    drive_g4,
    drive_g9,
    prepare_scenario,
    remove_scenario_settings,
    settle_device,
)
from bench_guard import require_no_instance
from bench_input import InputDevice
from bench_kwin import KWinScripts, require_kwin
from bench_present import (
    check_refresh,
    discard_rate,
    first_present_ns,
    frame_intervals_ms,
    parse_dir,
    require_clock,
    toplevel_frames,
)

PRESENT_POLL_S = 2.0
PRESENT_POLL_STEP_S = 0.05
# Flatpak receives these only through --env; the host `flatpak`/bwrap must never preload the shim.
FLATPAK_ONLY_ENV = ("LD_PRELOAD", "CROSSHOOK_BENCH_PRESENT_DIR",
                    "CROSSHOOK_BENCH_LOG", "CROSSHOOK_BENCH_LOG_RATE")
MIN_INTERVALS = 30  # per scenario, pooled; the library grid presents ~9 fps on E1, so 6 s of scroll yields ~50
MIN_ITERATION_INTERVALS = 5  # per iteration: a pooled count must not hide an iteration that barely presented
MAX_DISCARD_RATE = 0.05


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


def _present_dir(env: dict[str, str], metric: str, number: int, scenario: str | None = None) -> Path:
    base = Path(env["HOME"]).parent / "present"
    parent = base / metric if scenario is None else base / metric / scenario
    for directory in (base, base / metric, parent):
        directory.mkdir(mode=0o700, exist_ok=True)
    present_dir = parent / f"iter-{number}"
    try:
        present_dir.mkdir(mode=0o700)
    except FileExistsError:
        raise CaptureError(f"PRESENT_DIR_EXISTS: {present_dir} already exists") from None
    return present_dir


def _launch_spec(command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str],
                 present_dir: Path) -> tuple[list[str], dict[str, str]]:
    """Command and host env for one launch; Flatpak gets the shim variables only through --env."""
    env_i = {**env, "CROSSHOOK_BENCH_PRESENT_DIR": str(present_dir)}
    command_i = command_for(env_i)
    if command_i[0] == "flatpak":
        for key in FLATPAK_ONLY_ENV:
            env_i.pop(key, None)
    return command_i, env_i


def _iteration(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str],
               metric: str, number: int) -> tuple[float, float]:
    present_dir = _present_dir(env, metric, number)
    command_i, env_i = _launch_spec(command_for, env, present_dir)
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


def _drive(metric: str, scenario: str, proc, present_dir: Path, tools: Path, dev: InputDevice | None):
    kwin = KWinScripts(tools, secrets.token_hex(4))
    if metric == "G4":
        return drive_g4(proc, scenario, present_dir, kwin, dev)
    return drive_g9(proc, present_dir, kwin)


def _driven_iteration(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str],
                      metric: str, scenario: str, number: int,
                      dev: InputDevice | None) -> tuple[list[float], DriveResult]:
    present_dir = _present_dir(env, metric, number, scenario)
    # G9's one scenario ("resize") needs no per-scenario setup; prepare_scenario only knows the G4 ones.
    env_s = prepare_scenario(scenario, env, args.flatpak) if metric == "G4" else env
    command_i, env_i = _launch_spec(command_for, env_s, present_dir)
    require_no_instance()  # a CrossHook started since the last check would absorb this launch
    proc, _ = ready(command_i, env_i, args.timeout, exit_after=False)
    try:
        result = _drive(metric, scenario, proc, present_dir, Path(env["HOME"]).parent / "bench-tools", dev)
    finally:
        stop(proc)
        if proc.stderr:
            proc.stderr.close()
    logs = parse_dir(present_dir)
    require_clock(logs)
    frames = toplevel_frames(logs)
    check_refresh(frames, args.refresh_hz)
    if metric == "G4" and not any(frame.presented_ns > result.start_ns for frame in frames):
        raise CaptureError(f"INPUT_NOT_DELIVERED: {scenario}: no frame presented after the first scroll notch")
    if discard_rate(logs, result.start_ns, result.end_ns) > MAX_DISCARD_RATE:
        raise CaptureError(f"DISCARD_RATE: {scenario}: more than {MAX_DISCARD_RATE:.0%} of frames were discarded")
    return frame_intervals_ms(frames, result.start_ns, result.end_ns), result


def driven_rows(args, metric: str, command_for: Callable[[dict[str, str]], list[str]],
                env: dict[str, str], evidence: dict | None = None) -> list[dict]:
    """One row per scenario: frame intervals inside the driven window, pooled over iterations.

    `evidence`, when given, gains `{metric: {scenario: [{iteration, injected, driven_s}, ...]}}`:
    what each iteration achieved, for provenance (the CSV schema has no column for it).
    """
    require_kwin()
    try:
        remove_scenario_settings(env, args.flatpak)  # an earlier metric's console file must not reach this one
        return _scenario_rows(args, metric, command_for, env, evidence)
    finally:
        remove_scenario_settings(env, args.flatpak)  # the console scenario's file never outlives its metric


def _scenario_rows(args, metric: str, command_for: Callable[[dict[str, str]], list[str]],
                   env: dict[str, str], evidence: dict | None) -> list[dict]:
    rows = []
    # Only G4 injects uinput input; G9 resizes through KWin and never opens a device.
    with (InputDevice() if metric == "G4" else contextlib.nullcontext()) as dev:  # closed on every exit path
        if dev is not None:
            settle_device(dev)
        for scenario in args.scenarios[metric]:
            values: list[float] = []
            for number in range(1, args.iterations + 1):
                intervals, result = _driven_iteration(args, command_for, env, metric, scenario, number, dev)
                if evidence is not None:
                    evidence.setdefault(metric, {}).setdefault(scenario, []).append(
                        {"iteration": number, "injected": result.injected,
                         "driven_s": round(result.driven_ns / 1e9, 3)})
                if len(intervals) < MIN_ITERATION_INTERVALS:
                    raise CaptureError(f"CAPTURE_TOO_SHORT: {scenario} iteration {number}: {len(intervals)} "
                                       f"frame intervals, need {MIN_ITERATION_INTERVALS}")
                values += intervals
            if len(values) < MIN_INTERVALS:
                raise CaptureError(f"CAPTURE_TOO_SHORT: {scenario}: {len(values)} frame intervals, "
                                   f"need {MIN_INTERVALS}")
            rows.append({"scenario": scenario, **stats(values),
                         "over_interval_percent": over_interval_percent(values, args.refresh_hz),
                         "frames": len(values), "status": "captured"})
    return rows
