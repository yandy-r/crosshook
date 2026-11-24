"""Linux benchmark captures. No synthetic measurement values."""
from __future__ import annotations

import contextlib
import csv
import os
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

from bench_flatpak import FlatpakSandboxScope, extract_app_id
from bench_session import PRESENTATION_HINT, locked_error, session_locked


class CaptureError(RuntimeError):
    """Missing evidence, invalid capture, or unsuccessful application run."""


def stats(values: list[float]) -> dict[str, float]:
    if not values:
        raise CaptureError("EMPTY_CAPTURE")
    ordered = sorted(values)
    def percentile(p: float) -> float:
        index = (len(ordered) - 1) * p
        lo = int(index)
        hi = min(lo + 1, len(ordered) - 1)
        return ordered[lo] + (ordered[hi] - ordered[lo]) * (index - lo)
    mean = sum(values) / len(values)
    return {"p50_ms": percentile(.5), "p95_ms": percentile(.95),
            "p99_ms": percentile(.99), "stddev_ms": (sum((v - mean)**2 for v in values) / len(values))**.5}


def over_interval_percent(values: list[float], refresh_hz: float) -> float:
    """Share of frame intervals longer than 1.5 refresh periods (a missed vblank).

    Presentation intervals land on whole refresh periods, so a missed vblank shows
    up as roughly two periods, while a normal frame is one period plus jitter. The
    1.5-period threshold sits between them. A bare 1000/refresh_hz threshold would
    misfire: at a 119.88 Hz mode the normal 8.342 ms interval already exceeds
    1000/120 (8.333 ms), flagging nearly every frame.
    """
    limit = 1.5 * 1000 / refresh_hz
    return 100 * sum(v > limit for v in values) / len(values)


_UNWIND_SIGNALS = (signal.SIGTERM, signal.SIGHUP, signal.SIGINT)


@contextlib.contextmanager
def _signals_deferred():
    """Hold SIGTERM, SIGHUP and SIGINT while an app is torn down, then deliver the first one.

    A signal that arrives while a first one is already unwinding the run would otherwise raise
    out of `stop()` and leave the app's process group alive. Handlers can only be installed from
    the main thread; elsewhere there is nothing to defer.
    """
    if threading.current_thread() is not threading.main_thread():
        yield
        return
    caught: list[int] = []
    previous = {number: signal.signal(number, lambda signum, _frame: caught.append(signum))
                for number in _UNWIND_SIGNALS}
    try:
        yield
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler if handler is not None else signal.SIG_DFL)
        if caught:
            signal.raise_signal(caught[0])  # the restored handler decides: abort, ignore or default


def _isolated_root_from_env(env: dict[str, str]) -> Path | None:
    raw_home = env.get("HOME")
    if not raw_home:
        return None
    home = Path(raw_home)
    if home.name == "home" and home.parent.name.startswith("crosshook-bench-"):
        return home.parent
    return None


def stop(proc: subprocess.Popen) -> None:
    """Terminate the app's whole process group and reap it; a signal received meanwhile waits."""
    with _signals_deferred():
        scope = getattr(proc, "_flatpak_scope", None)
        tree_pids: set[int] = set()
        if scope is not None:
            try:
                if proc.poll() is None:
                    tree_pids = process_tree(proc.pid)
            except Exception as error:  # noqa: BLE001 - teardown cleanup must never mask termination
                print(f"FLATPAK_TREE_PROBE_FAILED: {error}", file=sys.stderr)
            proc._flatpak_scope = None  # type: ignore[attr-defined]
        try:
            _terminate(proc)
        finally:
            if scope is not None:
                try:
                    scope.cleanup(tree_pids=tree_pids)
                except Exception as error:  # noqa: BLE001 - teardown cleanup must never block pipe close/unwind
                    print(f"FLATPAK_CLEANUP_FAILED: {error}", file=sys.stderr)


def _terminate(proc: subprocess.Popen) -> None:
    # Child owns a new process group; never match or kill by application name.
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(proc.pid, sig)
        except ProcessLookupError:
            break
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            continue
        # Leader exit does not imply all WebKit descendants exited.
        if sig == signal.SIGTERM:
            continue
    proc.wait()


def drain_stderr(proc: subprocess.Popen) -> None:
    """Drain long sessions after READY so a full pipe cannot stall the app."""
    import threading
    assert proc.stderr is not None
    os.set_blocking(proc.stderr.fileno(), True)
    def drain():
        try:
            while proc.stderr.read(65536):
                pass
        except (OSError, ValueError):
            return
    threading.Thread(target=drain, daemon=True).start()


def ready(command: list[str], env: dict[str, str], timeout: float,
          exit_after: bool, events: bool = False, stdin: bool = False) -> tuple[subprocess.Popen, float]:
    """Launch and wait for READY. With `events`, `proc.events` reads the BENCH_* markers after READY.

    With `stdin`, the app's stdin is a pipe the caller keeps as `proc.stdin` (the G8 go signal) and
    closes; otherwise the app inherits the runner's stdin as before.
    """
    import selectors
    child_env = dict(env)
    if exit_after:
        child_env["CROSSHOOK_BENCH_EXIT_AFTER_READY"] = "1"
    else:
        child_env.pop("CROSSHOOK_BENCH_EXIT_AFTER_READY", None)
    # Flatpak's explicit --env overrides launcher inheritance.
    command = list(command)
    is_flatpak = bool(command) and Path(command[0]).name == "flatpak"
    if is_flatpak:
        command.insert(-1, f"--env=CROSSHOOK_BENCH_EXIT_AFTER_READY={int(exit_after)}")
    if session_locked():
        raise CaptureError(locked_error("before launch"))
    # Snapshot Flatpak instances before recording `start` so `flatpak ps` does not inflate ready_ms.
    flatpak_scope = (
        FlatpakSandboxScope.capture_before_launch(
            extract_app_id(command),
            child_env,
            isolated_root=_isolated_root_from_env(child_env),
        )
        if is_flatpak
        else None
    )
    start = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
    proc = subprocess.Popen(command, env=child_env, start_new_session=True,
                            stdin=subprocess.PIPE if stdin else None,
                            stderr=subprocess.PIPE, stdout=subprocess.DEVNULL)
    try:
        if flatpak_scope is not None:
            proc._flatpak_scope = flatpak_scope  # type: ignore[attr-defined]
            flatpak_scope.bind_launch(proc.pid)
        assert proc.stderr is not None
        os.set_blocking(proc.stderr.fileno(), False)
        pending = b""
        recent: list[str] = []
        with selectors.DefaultSelector() as selector:
            selector.register(proc.stderr, selectors.EVENT_READ)
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                for key, _ in selector.select(min(.1, max(0, deadline-time.monotonic()))):
                    chunk = os.read(key.fd, 65536)
                    if not chunk:
                        raise CaptureError("NO_READY: " + " | ".join(recent[-5:]))
                    pending += chunk
                    while b"\n" in pending:
                        line, pending = pending.split(b"\n", 1)
                        text = line.decode("utf-8", errors="replace")
                        recent.append(text)
                        recent = recent[-5:]
                        if text.startswith("BENCH_ERROR:"):
                            raise CaptureError(text)
                        if text.startswith("READY "):
                            try:
                                ns = int(text[6:])
                            except ValueError as error:
                                raise CaptureError("MALFORMED_READY") from error
                            now = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
                            if not start <= ns <= now:
                                raise CaptureError("INVALID_READY_CLOCK")
                            proc.launch_ns = start
                            proc.ready_ns = ns
                            if events:
                                from bench_events import EventReader
                                os.set_blocking(proc.stderr.fileno(), True)
                                proc.events = EventReader(proc.stderr, start, initial=pending)
                            else:
                                drain_stderr(proc)
                            if exit_after:
                                proc.wait(timeout=max(.1, deadline-time.monotonic()))
                                if proc.returncode != 0:
                                    raise CaptureError(f"APP_EXIT: {proc.returncode}")
                            return proc, (ns-start)/1_000_000
                    if len(pending) > 65536:
                        raise CaptureError("UNBOUNDED_STDERR_LINE")
        if session_locked():
            raise CaptureError(locked_error("after waiting for READY"))
        raise CaptureError(f"READY_TIMEOUT: no READY within {timeout:g} s; {PRESENTATION_HINT}. "
                           "Last stderr: " + " | ".join(recent[-5:]))
    except BaseException:
        stop(proc)
        if proc.stderr is not None:
            proc.stderr.close()
        if proc.stdin is not None:
            proc.stdin.close()
        raise


def process_tree(pid: int) -> set[int]:
    found: set[int] = set()
    queue = [pid]
    while queue:
        current = queue.pop()
        if current in found:
            continue
        found.add(current)
        for file in Path(f"/proc/{current}/task").glob("*/children"):
            try:
                queue.extend(int(p) for p in file.read_text().split())
            except (OSError, ValueError):
                continue
    return found


def memory(pid: int) -> tuple[float, float]:
    pss = uss = 0
    for child in process_tree(pid):
        try:
            fields = {line.split(":", 1)[0]: int(line.split()[1])
                      for line in Path(f"/proc/{child}/smaps_rollup").read_text().splitlines()[1:]}
        except (OSError, ValueError):
            continue
        pss += fields.get("Pss", 0)
        uss += fields.get("Private_Clean", 0) + fields.get("Private_Dirty", 0)
    if pss == 0:
        raise CaptureError("PROCESS_MEMORY_UNAVAILABLE")
    return pss/1024, uss/1024


def cpu_ticks(pid: int) -> int:
    total = 0
    for child in process_tree(pid):
        try:
            # comm can contain spaces/parentheses; fields after final ')' begin at #3.
            fields = Path(f"/proc/{child}/stat").read_text().rsplit(")", 1)[1].split()
            total += int(fields[11]) + int(fields[12])
        except (OSError, ValueError, IndexError):
            continue
    return total


def artifact_rows(path: Path | None, field: str) -> list[dict[str, str]]:
    if path is None:
        raise CaptureError(f"CAPTURE_REQUIRED: CSV with {field}; see docs/internal/bench/README.md")
    with path.open(newline="") as stream:
        reader = csv.DictReader(stream)
        if field not in (reader.fieldnames or []):
            raise CaptureError(f"CAPTURE_SCHEMA: missing {field}")
        rows = list(reader)
    if not rows:
        raise CaptureError("EMPTY_CAPTURE")
    return rows


def numeric(rows: list[dict[str, str]], field: str) -> list[float]:
    import math
    values = [float(row[field]) for row in rows]
    if any(not math.isfinite(v) or v < 0 for v in values):
        raise CaptureError(f"CAPTURE_SCHEMA: invalid {field}")
    return values


def capture(metric: str, args, command: list[str], env: dict[str, str], command_for=None,
            evidence: dict | None = None) -> list[dict]:
    if metric in ("G1", "G2"):
        if metric == "G1" and not args.cold_cache_ack:
            raise CaptureError("NEEDS_COLD_CACHE_PREP: G1 requires --cold-cache-ack; never label warm runs cold")
        if getattr(args, "collect", "csv") == "presentation":
            from bench_collect import presentation_rows
            return presentation_rows(args, command_for or (lambda _env: command), env, metric)
        rows = []
        for iteration in range(args.iterations):
            proc, ms = ready(command, env, args.timeout, True)
            stop(proc)
            if proc.stderr:
                proc.stderr.close()
            rows.append({"iteration": iteration+1, "ready_ms": ms, "status": "measured"})
        summary = stats([r["ready_ms"] for r in rows])
        rows.append({"iteration": "summary", **{k: summary[k] for k in ("p50_ms", "p95_ms")}, "status": "measured"})
        return rows
    if metric == "G8" and getattr(args, "collect", "csv") != "presentation":
        rows = artifact_rows(Path(args.capture_csv) if args.capture_csv else None, "frame_ms")
        values = numeric(rows, "frame_ms")
        proc, _ = ready(command, env, args.timeout, False)
        try:
            baseline, _ = memory(proc.pid)
            peak = baseline
            start = time.monotonic()
            while time.monotonic()-start < 11:
                if proc.poll() is not None:
                    raise CaptureError("APP_EXIT_DURING_REPLAY")
                pss, _ = memory(proc.pid)
                peak = max(peak, pss)
                time.sleep(.1)
            # External frames: the single-session columns (peak PSS, GPU, follow) stay blank.
            return [{"scenario": "external", **stats(values),
                     "over_interval_percent": over_interval_percent(values, args.refresh_hz),
                     "frames": len(values), "memory_growth_mb": peak-baseline, "status": "measured"}]
        finally:
            stop(proc)
            if proc.stderr:
                proc.stderr.close()
    if metric in ("G3", "G7"):
        from concurrent.futures import ThreadPoolExecutor

        from bench_instruments import battery_power_w, gpu_memory_mb, wakeup_events
        proc, _ = ready(command, env, args.timeout, False)
        rows = []
        worker = ThreadPoolExecutor(max_workers=1)
        cancel = threading.Event()
        wakeups = worker.submit(wakeup_events, process_tree(proc.pid), args.duration, cancel) if metric == "G7" else None
        try:
            start = time.monotonic()
            ticks = cpu_ticks(proc.pid)
            pss0, _ = memory(proc.pid)
            while True:
                if session_locked():  # before timing, so probe latency never skews a sample
                    raise CaptureError(locked_error(f"during {metric}"))
                elapsed = time.monotonic()-start
                if proc.poll() is not None:
                    raise CaptureError(f"APP_EXIT: {proc.returncode}")
                if metric == "G3":
                    pss, uss = memory(proc.pid)
                    rows.append({"elapsed_s": elapsed, "pss_mb": pss, "uss_mb": uss,
                                 "growth_mb": pss-pss0, "gpu_mb": gpu_memory_mb(process_tree(proc.pid)), "status": "measured"})
                elif elapsed > 0:
                    rows.append({"elapsed_s": elapsed,
                                 "cpu_percent": (cpu_ticks(proc.pid)-ticks)/os.sysconf("SC_CLK_TCK")/elapsed*100,
                                 "wakeups_s": "pending", "power_w": battery_power_w(), "status": "measured"})
                if elapsed >= args.duration:
                    break
                time.sleep(min(10, args.duration-elapsed))
            if wakeups:
                rate = wakeups.result()
                for row in rows:
                    row["wakeups_s"] = rate
            return rows
        finally:
            cancel.set()  # no-op once perf finished; otherwise a failed capture stops it now
            stop(proc)
            worker.shutdown(wait=True)
            if proc.stderr:
                proc.stderr.close()
    if metric == "G10":
        from bench_instruments import sizes
        if not args.binary or not args.bundle or args.runtime_delta_mb is None:
            raise CaptureError("SIZE_INPUT_REQUIRED: --binary, --bundle, --runtime-delta-mb (measured download delta)")
        return [sizes(Path(args.binary), Path(args.bundle), args.branch, args.runtime_delta_mb)]
    if metric in ("G4", "G5", "G8", "G9") and getattr(args, "collect", "csv") == "presentation":
        from bench_collect import driven_rows
        return driven_rows(args, metric, command_for or (lambda _env: command), env, evidence)
    # External captures are raw observations, not invented samples or compositor rAF proxies.
    field = "stall_ms" if metric == "G6" else "latency_ms" if metric == "G5" else "frame_ms"
    rows = artifact_rows(Path(args.capture_csv) if args.capture_csv else None, field)
    values = numeric(rows, field)
    if metric == "G6":
        return [{"stalls_over_16ms": sum(v>16 for v in values), "stalls_over_50ms": sum(v>50 for v in values),
                 "duration_s": args.duration, "status": "measured"}]
    result = stats(values)
    if metric == "G5":  # external observations carry no trial statuses: `ambiguous`, `timeout` and `early` stay empty
        return [{"scenario": "external", "p50_ms": result["p50_ms"], "p95_ms": result["p95_ms"],
                 "p99_ms": result["p99_ms"], "measured": len(values), "status": "measured"}]
    result["over_interval_percent"] = over_interval_percent(values, args.refresh_hz)
    result.update(scenario="external", frames=len(values), status="measured")
    return [result]
