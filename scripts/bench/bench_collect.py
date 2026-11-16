"""Presentation-time collection via the wlpresent shim: G1/G2 first frame, G4/G9 driven frame
intervals, G5 input-to-photon trials, G8 single-session replay frames and memory."""
from __future__ import annotations

import contextlib
import secrets
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from bench_capture import CaptureError, _signals_deferred, over_interval_percent, ready, stats, stop
from bench_drivers import (
    LOG_ENV,
    DriveResult,
    drive_g4,
    drive_g9,
    prepare_scenario,
    remove_scenario_settings,
    settle_device,
)
from bench_drivers_g5 import G5Result, drive_g5, mark_timeout_s
from bench_drivers_g8 import SCENARIO as G8_SCENARIO
from bench_drivers_g8 import G8Result, drive_g8, prepare_g8_scenario
from bench_g5io import TOGGLE_SCENARIOS, ToggleState, write_observations
from bench_g5select import summarize
from bench_g8io import FrameRecord, frame_records
from bench_g8io import write_observations as write_g8_observations
from bench_g8session import (
    G8Iteration,
    G8Session,
    GpuProbe,
    PidRecorder,
    SessionFrames,
    SessionMemory,
    check_session,
    frames_pid,
    frames_session,
    iteration_gpu_source,
)
from bench_g8session import summarize as summarize_g8
from bench_guard import ENV_SCENARIO, require_no_instance
from bench_input import InputDevice
from bench_kwin import KWinScripts, require_kwin
from bench_memory import MemoryBenchError, MemoryRow, MemorySampler, growth, peak_pss
from bench_present import (
    check_refresh,
    discard_rate,
    first_present_ns,
    frame_intervals_ms,
    parse_dir,
    require_clock,
    toplevel_frames,
    toplevel_pids,
)

PRESENT_POLL_S = 2.0
PRESENT_POLL_STEP_S = 0.05
# Flatpak receives these only through --env; the host `flatpak`/bwrap must never preload the shim.
FLATPAK_ONLY_ENV = ("LD_PRELOAD", "CROSSHOOK_BENCH_PRESENT_DIR", ENV_SCENARIO,
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


def _stop_app(proc) -> None:
    stop(proc)
    events = getattr(proc, "events", None)
    if events is not None:
        events.close()
    if proc.stderr:
        proc.stderr.close()
    stdin = getattr(proc, "stdin", None)
    if stdin is not None:
        stdin.close()


def _g5_iteration(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str],
                  scenario: str, number: int, dev: InputDevice, state: ToggleState | None) -> G5Result:
    present_dir = _present_dir(env, "G5", number, scenario)
    env_s = {key: value for key, value in env.items() if key not in LOG_ENV}
    env_s[ENV_SCENARIO] = scenario
    command_i, env_i = _launch_spec(command_for, env_s, present_dir)
    try:
        require_no_instance()  # a CrossHook started since the last check would absorb this launch
        proc, _ = ready(command_i, env_i, args.timeout, exit_after=False, events=True)
        try:
            kwin = KWinScripts(Path(env["HOME"]).parent / "bench-tools", secrets.token_hex(4))
            result = drive_g5(proc, scenario, present_dir, kwin, dev, proc.events, args.trials,
                              mark_timeout_s(args.timeout, args.flatpak))
        finally:
            _stop_app(proc)
    finally:
        if state is not None:
            with _signals_deferred():  # a signal must not leave the profile half restored
                state.restore()  # the app is stopped (or never started): back to the fixture state
    logs = parse_dir(present_dir)
    require_clock(logs)
    check_refresh(toplevel_frames(logs), args.refresh_hz)
    return result


def _g5_row(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str], scenario: str,
            dev: InputDevice, evidence: dict | None) -> dict:
    """One G5 row: the trials of every iteration pooled; the observation CSV is rewritten per iteration."""
    if args.trials % 2:
        raise CaptureError(f"INVALID_TRIALS: {args.trials} is odd; toggles must return to their start state")
    state = ToggleState(env, args.flatpak) if scenario in TOGGLE_SCENARIOS else None
    records = []
    try:
        if state is not None:
            state.capture()
        for number in range(1, args.iterations + 1):
            result = _g5_iteration(args, command_for, env, scenario, number, dev, state)
            records += result.records
            write_observations(env, scenario, records)
            if evidence is not None:
                evidence.setdefault("G5", {}).setdefault(scenario, []).append(
                    {"iteration": number, "injected": len(result.records),
                     "driven_s": round(result.driven_ns / 1e9, 3),
                     "uncertainty_ms": round(result.clock.uncertainty_ns / 1e6, 3)})
    finally:
        if state is not None:
            state.discard()
    return summarize(scenario, [record.trial for record in records])


@dataclass(frozen=True)
class G8Capture:
    """One launch's contribution: the checked iteration, its raw rows and frames, and its provenance."""

    iteration: G8Iteration
    rows: list[MemoryRow]
    frames: list[FrameRecord]
    evidence: dict


def _g8_launch(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str],
               dev: InputDevice, present_dir: Path):
    """Launch once with events and the go pipe, drive the session, stop the app: (proc, result, recorder, probe)."""
    command_i, env_i = _launch_spec(command_for, prepare_g8_scenario(env, args.flatpak), present_dir)
    require_no_instance()  # a CrossHook started since the last check would absorb this launch
    proc, _ = ready(command_i, env_i, args.timeout, exit_after=False, events=True, stdin=True)
    try:
        recorder, probe = PidRecorder(proc.pid, args.flatpak), GpuProbe()
        kwin = KWinScripts(Path(env["HOME"]).parent / "bench-tools", secrets.token_hex(4))
        result = drive_g8(proc, present_dir, kwin, dev, proc.events, MemorySampler(recorder, probe),
                          mark_timeout_s(args.timeout, args.flatpak), args.flatpak)
    finally:
        _stop_app(proc)
    return proc, result, recorder, probe


def _g8_iteration(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str], number: int,
                  dev: InputDevice) -> G8Capture:
    run_id = getattr(args, "run_id", None)
    if not run_id:
        raise CaptureError("RUN_ID_MISSING: G8 binds its frames and memory samples to the run id")
    present_dir = _present_dir(env, "G8", number, G8_SCENARIO)
    proc, result, recorder, probe = _g8_launch(args, command_for, env, dev, present_dir)
    logs = parse_dir(present_dir)
    require_clock(logs)
    frames = toplevel_frames(logs)
    check_refresh(frames, args.refresh_hz)
    window = (result.window_start_ns, result.window_end_ns)
    if discard_rate(logs, *window) > MAX_DISCARD_RATE:
        raise CaptureError(f"DISCARD_RATE: {G8_SCENARIO}: more than {MAX_DISCARD_RATE:.0%} of frames were discarded")
    session = G8Session(run_id, number, proc.pid, proc.launch_ns)
    times = [frame.presented_ns for frame in frames]
    # The frames' pid comes from the shim logs, not from the launch: only evidence can tie them to it.
    shim_pid = frames_pid(toplevel_pids(logs), proc.pid, recorder.sampled(), args.flatpak)
    check_session(session,
                  SessionFrames(frames_session(run_id, present_dir, shim_pid, proc.launch_ns), tuple(times)),
                  SessionMemory(G8Session(run_id, number, recorder.root_pid, proc.launch_ns), tuple(result.rows)),
                  *window)
    try:
        grown, peak = growth(result.rows, "baseline", "tail"), peak_pss(result.rows)
    except MemoryBenchError as error:
        raise CaptureError(str(error)) from error
    iteration = G8Iteration(session, frame_intervals_ms(frames, *window), grown, peak,
                            iteration_gpu_source(result.rows, grown), result.follow)
    return G8Capture(iteration, result.rows, frame_records(times, *window),
                     _g8_evidence(session, result, iteration, recorder, probe))


def _g8_evidence(session: G8Session, result: G8Result, iteration: G8Iteration, recorder: PidRecorder,
                 probe: GpuProbe) -> dict:
    return {"iteration": session.iteration,
            "session": {"run_id": session.run_id, "pid": session.pid, "launch_ns": session.launch_ns},
            "start_to_done_s": round((result.done_ns - result.start_ns) / 1e9, 3),
            "settle_s": round((result.settled_ns - result.done_ns) / 1e9, 3),
            "notches": result.notches, "driven_s": round(result.driven_ns / 1e9, 3),
            "follow": result.follow, "gpu_source": iteration.gpu_source, "nvidia_smi_path": probe.tool_path,
            "gpu_denied_max": probe.denied_max,
            "web_process_sampled": recorder.web_process_sampled(),
            "sampled_pids": [{"pid": entry.pid, "comm": entry.comm, **({"nspid": list(entry.nspid)}
                                                                         if entry.nspid else {})}
                             for entry in recorder.sampled()]}


def _g8_row(args, command_for: Callable[[dict[str, str]], list[str]], env: dict[str, str], dev: InputDevice,
            evidence: dict | None) -> dict:
    """The G8 row: one launch per iteration, each cut by the app's own replay markers and pooled."""
    iterations: list[G8Iteration] = []
    rows: list[MemoryRow] = []
    frames: list[FrameRecord] = []
    try:
        for number in range(1, args.iterations + 1):
            capture = _g8_iteration(args, command_for, env, number, dev)
            iterations.append(capture.iteration)
            rows, frames = rows + capture.rows, frames + capture.frames
            write_g8_observations(env, G8_SCENARIO, rows, frames)  # rewritten per iteration: a failure keeps it
            if evidence is not None:
                evidence.setdefault("G8", {}).setdefault(G8_SCENARIO, []).append(capture.evidence)
            if len(capture.iteration.intervals_ms) < MIN_ITERATION_INTERVALS:
                raise CaptureError(f"CAPTURE_TOO_SHORT: {G8_SCENARIO} iteration {number}: "
                                   f"{len(capture.iteration.intervals_ms)} frame intervals, "
                                   f"need {MIN_ITERATION_INTERVALS}")
    finally:
        remove_scenario_settings(env, args.flatpak)  # the drawer-expanded settings never outlive the capture
    total = sum(len(iteration.intervals_ms) for iteration in iterations)
    if total < MIN_INTERVALS:
        raise CaptureError(f"CAPTURE_TOO_SHORT: {G8_SCENARIO}: {total} frame intervals, need {MIN_INTERVALS}")
    return summarize_g8(G8_SCENARIO, iterations, args.refresh_hz)


def driven_rows(args, metric: str, command_for: Callable[[dict[str, str]], list[str]],
                env: dict[str, str], evidence: dict | None = None) -> list[dict]:
    """One row per scenario: frame intervals inside the driven window (G5: trial latencies; G8: replay
    frames plus memory), pooled over iterations.

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
    # Only G4, G5 and G8 inject uinput input; G9 resizes through KWin and never opens a device.
    with (InputDevice() if metric in ("G4", "G5", "G8") else contextlib.nullcontext()) as dev:  # closed on every exit path
        if dev is not None:
            settle_device(dev)
        for scenario in args.scenarios[metric]:
            if metric == "G5":
                rows.append(_g5_row(args, command_for, env, scenario, dev, evidence))
                continue
            if metric == "G8":
                rows.append(_g8_row(args, command_for, env, dev, evidence))
                continue
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
