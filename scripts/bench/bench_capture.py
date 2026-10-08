"""Linux benchmark captures. No synthetic measurement values."""
from __future__ import annotations

import csv
import os
import signal
import subprocess
import time
from pathlib import Path


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


def stop(proc: subprocess.Popen) -> None:
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
          exit_after: bool) -> tuple[subprocess.Popen, float]:
    import selectors
    child_env = dict(env)
    if exit_after:
        child_env["CROSSHOOK_BENCH_EXIT_AFTER_READY"] = "1"
    else:
        child_env.pop("CROSSHOOK_BENCH_EXIT_AFTER_READY", None)
    # Flatpak's explicit --env overrides launcher inheritance.
    command = list(command)
    if command[0] == "flatpak":
        command.insert(-1, f"--env=CROSSHOOK_BENCH_EXIT_AFTER_READY={int(exit_after)}")
    start = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
    proc = subprocess.Popen(command, env=child_env, start_new_session=True,
                            stderr=subprocess.PIPE, stdout=subprocess.DEVNULL)
    assert proc.stderr is not None
    os.set_blocking(proc.stderr.fileno(), False)
    pending = b""
    recent: list[str] = []
    try:
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
                            drain_stderr(proc)
                            if exit_after:
                                proc.wait(timeout=max(.1, deadline-time.monotonic()))
                                if proc.returncode != 0:
                                    raise CaptureError(f"APP_EXIT: {proc.returncode}")
                            return proc, (ns-start)/1_000_000
                    if len(pending) > 65536:
                        raise CaptureError("UNBOUNDED_STDERR_LINE")
        raise CaptureError("READY_TIMEOUT: " + " | ".join(recent[-5:]))
    except BaseException:
        stop(proc)
        proc.stderr.close()
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


def capture(metric: str, args, command: list[str], env: dict[str, str]) -> list[dict]:
    if metric in ("G1", "G2"):
        if metric == "G1" and not args.cold_cache_ack:
            raise CaptureError("NEEDS_COLD_CACHE_PREP: G1 requires --cold-cache-ack; never label warm runs cold")
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
    if metric == "G8":
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
            return [{**stats(values), "over_interval_percent": sum(v>1000/args.refresh_hz for v in values)/len(values)*100,
                     "memory_growth_mb": peak-baseline, "status": "measured"}]
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
        wakeups = worker.submit(wakeup_events, process_tree(proc.pid), args.duration) if metric == "G7" else None
        try:
            start = time.monotonic()
            ticks = cpu_ticks(proc.pid)
            pss0, _ = memory(proc.pid)
            while True:
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
            stop(proc)
            worker.shutdown(wait=True)
            if proc.stderr:
                proc.stderr.close()
    if metric == "G10":
        from bench_instruments import sizes
        if not args.binary or not args.bundle or args.runtime_delta_mb is None:
            raise CaptureError("SIZE_INPUT_REQUIRED: --binary, --bundle, --runtime-delta-mb (measured download delta)")
        return [sizes(Path(args.binary), Path(args.bundle), args.branch, args.runtime_delta_mb)]
    # External captures are raw observations, not invented samples or compositor rAF proxies.
    field = "stall_ms" if metric == "G6" else "latency_ms" if metric == "G5" else "frame_ms"
    rows = artifact_rows(Path(args.capture_csv) if args.capture_csv else None, field)
    values = numeric(rows, field)
    if metric == "G6":
        return [{"stalls_over_16ms": sum(v>16 for v in values), "stalls_over_50ms": sum(v>50 for v in values),
                 "duration_s": args.duration, "status": "measured"}]
    result = stats(values)
    if metric == "G5":
        return [{"scenario": row.get("scenario", "unspecified"), "latency_ms": value, "status": "measured"}
                for row, value in zip(rows, values)] + [{"scenario": "summary", "p95_ms": result["p95_ms"], "status": "measured"}]
    result["over_interval_percent"] = sum(v>1000/args.refresh_hz for v in values)/len(values)*100
    result["status"] = "measured"
    return [result]
