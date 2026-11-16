#!/usr/bin/env python3
"""Isolated CrossHook benchmark runner. Dry-run schemas are never measurements."""
from __future__ import annotations

import csv
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

import bench_guard
from bench_capture import CaptureError, capture
from bench_cli import SCHEMAS, arguments
from bench_guard import (
    FLATPAK_APP_ID,
    GuardError,
    Originals,
    build_env,
    make_dirs,
    preflight,
    require_no_instance,
)
from bench_instruments import InstrumentError
from bench_provenance import (
    boot_id,
    create_raw_dir,
    finalize_raw,
    git_state,
    lock_output,
    new_run_id,
    sha256_file,
    verify_capture,
)

REPO = Path(__file__).resolve().parents[2]
TOOLS = ("hyperfine", "ydotool", "gpu-screen-recorder", "perf", "pidstat", "upower", "flatpak", "ffmpeg", "cc",
         "gdbus")
INPUT_DRIVEN = ("G4", "G9")  # presentation runs of these launch the app and inject input
EXIT_PRESENTATION = ("NO_PRESENTATION_GLOBAL", "PRESENTATION_CLOCK", "REFRESH_MISMATCH",
                     "UNSUPPORTED_COMPOSITOR", "INPUT_DEVICE_DENIED")


class BenchAborted(BaseException):
    """SIGTERM/SIGHUP while a run is in flight; BaseException so no `except Exception` swallows it."""

    def __init__(self, signal_name: str):
        super().__init__(signal_name)
        self.signal_name = signal_name


def _abort(signum, _frame):
    raise BenchAborted(signal.Signals(signum).name)


def launch_command(args, env: dict[str, str]) -> list[str]:
    if not args.flatpak:
        if not args.binary:
            raise CaptureError("BINARY_REQUIRED: --binary release executable")
        return [str(Path(args.binary).resolve())]
    home = Path(env["HOME"])
    if not home.name == "home" or not home.parent.name.startswith("crosshook-bench-"):
        raise GuardError("Flatpak requires fresh owned launcher HOME")
    command = ["flatpak", "run", f"--branch={args.branch}", "--nofilesystem=host:reset",
               f"--filesystem={home.parent}", "--env=CROSSHOOK_FLATPAK_HOST_XDG=0", "--env=CROSSHOOK_BENCH=1"]
    for key in ("CROSSHOOK_BENCH_ROOT", "CROSSHOOK_BENCH_LOG", "CROSSHOOK_BENCH_LOG_RATE",
                "LD_PRELOAD", "CROSSHOOK_BENCH_PRESENT_DIR"):
        if key in env:
            command.append(f"--env={key}={env[key]}")
    command.append(FLATPAK_APP_ID)
    return command


def materialize(source: Path, root: Path) -> None:
    subprocess.run(["cargo", "run", "--quiet", "--manifest-path", str(REPO / "src/crosshook-native/Cargo.toml"),
                    "-p", "crosshook-core", "--example", "bench_fixtures", "--", "materialize",
                    "--from", str(source), "--root", str(root)], check=True, cwd=REPO)


def metadata(args) -> dict:
    commit, dirty = git_state(REPO)
    governor = Path("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
    seed = None
    if args.fixtures:
        manifest = json.loads((Path(args.fixtures)/"manifest.json").read_text())
        seed = manifest.get("seed")
    return {"commit": commit, "dirty": dirty, "variant": args.variant, "environment": args.env,
            "metrics": args.selected, "iterations": args.iterations,
            "synthetic": args.dry_run, "flatpak": args.flatpak, "seed": seed,
            "kernel": os.uname().release, "governor": governor.read_text().strip() if governor.exists() else "unavailable",
            "tools": {tool: shutil.which(tool) is not None for tool in TOOLS},
            "captured_at": datetime.now(timezone.utc).isoformat(),
            "external_capture": str(Path(args.capture_csv).resolve()) if args.capture_csv else None,
            "refresh_hz": args.refresh_hz, "duration_s": args.duration,
            "cold_cache_ack": args.cold_cache_ack, "status": "dry-run" if args.dry_run else "pending"}


def main(argv: list[str] | None = None) -> int:
    args = arguments(argv)
    outcome = {"binary_changed": False}
    code = _run(args, outcome)
    return 1 if code == 0 and outcome["binary_changed"] else code


def _binary_changed(binary: Path, expected: str) -> bool:
    try:
        return sha256_file(binary) != expected
    except OSError:
        return True


def _require_gates(args, driven: list[str]) -> None:
    """Refuse a real run whose tools, session or inputs are missing, before anything is written."""
    required = {"G7": ("perf", "upower"),
                "G10": ("strip", "flatpak")}
    if driven:
        required.update({metric: ("gdbus",) for metric in driven})
    missing = {tool for metric in args.selected for tool in required.get(metric, ())
               if shutil.which(tool) is None}
    if missing:
        raise CaptureError("MISSING_TOOL: " + ", ".join(sorted(missing)))
    if driven:
        require_no_instance()
    if "G1" in args.selected and not args.cold_cache_ack:
        raise CaptureError("NEEDS_COLD_CACHE_PREP: G1 requires --cold-cache-ack")
    for metric in args.selected:
        if metric in ("G4", "G5", "G6", "G8", "G9") and not args.capture_csv and metric not in driven:
            if metric == "G6" and shutil.which("perf") is None:
                raise CaptureError("MISSING_TOOL: perf")
            raise CaptureError(f"CAPTURE_REQUIRED: --capture-csv for {metric}")
    if "G8" in args.selected and not args.fixtures:
        raise CaptureError("FIXTURES_REQUIRED: G8 needs the 50k-line replay fixture")
    if "G4" in driven and "console" in args.scenarios["G4"] and not args.fixtures:
        raise CaptureError("FIXTURES_REQUIRED: the G4 console scenario needs the 50k-line replay fixture")


def _run(args, outcome: dict[str, bool]) -> int:
    root: Path | None = None
    meta_path: Path | None = None
    meta: dict = {}
    raw: Path | None = None
    output: Path | None = None
    written: list[Path] = []  # set before try: finally seals the raw dir even if setup fails
    provenance: dict = {}
    lock_fd: int | None = None
    binary: Path | None = None
    binary_sha256: str | None = None  # hashed before launch: what ran must be what is recorded
    driven = [m for m in args.selected if args.collect == "presentation" and m in INPUT_DRIVEN]
    previous = {number: signal.signal(number, _abort) for number in (signal.SIGTERM, signal.SIGHUP)}
    sealing_signals = (signal.SIGTERM, signal.SIGHUP, signal.SIGINT)
    try:
        original = Originals.capture()
        paths = preflight(original, output=args.output, fixtures=args.fixtures,
                          tmp_root=args.tmp_root, capture_csv=args.capture_csv)
        if args.binary and not Path(args.binary).is_file():
            raise CaptureError("BINARY_REQUIRED: executable file not found")
        if args.binary and not args.flatpak and not args.dry_run:
            binary = Path(args.binary).resolve()
            args.binary = str(binary)  # every launch uses this path; retargeting a symlink cannot swap it
            binary_sha256 = sha256_file(binary)
        if args.bundle:
            preflight(original, bundle=args.bundle)
        if not args.dry_run:
            _require_gates(args, driven)
        meta = metadata(args)
        # No filesystem mutation before all path/schema preflights succeed.
        output = paths["output"] / args.variant / args.env
        output.mkdir(parents=True, exist_ok=True)
        lock_fd = lock_output(output)  # held until the raw dir is sealed; before any CSV or meta write
        meta_path = output / "meta.json"
        meta_path.write_text(json.dumps(meta, indent=2)+"\n")
        if not args.dry_run:
            for metric in args.selected:  # never let an earlier run's CSV be sealed under this run
                stale = output / f"{metric}.csv"
                if os.path.lexists(stale):
                    stale.unlink()
        if args.capture_csv and not args.dry_run:
            verify_capture(Path(args.capture_csv), {"commit": meta["commit"], "variant": args.variant,
                                                    "environment": args.env, "boot_id": boot_id()})
        presenting = args.collect == "presentation" and not args.dry_run
        if presenting:
            run_id = new_run_id()
            raw = create_raw_dir(paths["output"], run_id)
            meta.update(run_id=run_id, collector="wlpresent/1", raw_dir=str(raw))
            provenance = {"run_id": run_id, "collector": "wlpresent/1", "commit": meta["commit"],
                          "dirty": meta["dirty"],
                          "variant": args.variant, "environment": args.env, "boot_id": boot_id(),
                          "flatpak": args.flatpak, "fixtures_seed": meta["seed"], "shim": None}
            if driven:
                provenance["drivers"] = "uinput+kwin/1"
                provenance["drive_stats"] = {}  # filled by the collector: injected events and driven span
                provenance["scenarios"] = {metric: args.scenarios[metric] for metric in driven}
            if binary_sha256 is not None:
                provenance["binary_sha256"] = binary_sha256
        external_only = all(m in ("G5", "G6", "G10") or (m in INPUT_DRIVEN and m not in driven)
                            for m in args.selected)
        command: list[str] = []
        env: dict[str, str] = {}
        if not args.dry_run and not external_only:
            paths["tmp_root"].mkdir(parents=True, exist_ok=True)
            root = Path(tempfile.mkdtemp(prefix="crosshook-bench-", dir=paths["tmp_root"]))
            preflight(original, isolated=root)
            env = build_env(root, flatpak=args.flatpak)
            overrides_of = getattr(bench_guard, "flatpak_overrides", None)
            if args.flatpak and overrides_of is not None:
                meta["flatpak_overrides"] = provenance["flatpak_overrides"] = overrides_of(env.get("FLATPAK_USER_DIR"))
            if args.flatpak:
                target = root / "home/.var/app" / FLATPAK_APP_ID
            else:
                target = root
            if args.fixtures:
                materialize(paths["fixtures"], target)
            make_dirs(root, env)
            if presenting:
                import bench_shim
                (root / "bench-tools").mkdir(mode=0o700)
                build = bench_shim.build(root / "bench-tools", "flatpak" if args.flatpak else "native")
                env["LD_PRELOAD"] = str(build.path)
                provenance["shim"] = {"so_sha256": build.so_sha256, "source_sha256": build.source_sha256,
                                      "built_in": build.built_in}
            env["CROSSHOOK_BENCH_ROOT"] = str(target)
            if args.env == "E3-sw":
                env.update(VK_ICD_FILENAMES="/dev/null", LIBGL_ALWAYS_SOFTWARE="1")
            command = launch_command(args, env)
        partial = False
        for metric in args.selected:
            if metric == "G8" and not args.dry_run:
                env["CROSSHOOK_BENCH_LOG"] = str(target / "logs/launch-50k.log")
                env["CROSSHOOK_BENCH_LOG_RATE"] = "5000"
                command = launch_command(args, env)
            else:
                env.pop("CROSSHOOK_BENCH_LOG", None)
                env.pop("CROSSHOOK_BENCH_LOG_RATE", None)
                if command:
                    command = launch_command(args, env)
            if args.dry_run:
                rows = [{"status": "dry-run"}]
            else:
                extra = {"evidence": provenance["drive_stats"]} if metric in driven else {}
                rows = capture(metric, args, command, env, command_for=lambda e: launch_command(args, e), **extra)
            partial |= any(row.get("status") == "partial" for row in rows)
            path = output / f"{metric}.csv"
            with path.open("w", newline="") as stream:
                writer = csv.DictWriter(stream, fieldnames=SCHEMAS[metric].split(","))
                writer.writeheader()
                writer.writerows(rows)
            written.append(path)
            print(path)
        meta["status"] = "dry-run" if args.dry_run else "partial" if partial else "captured"
        return 0
    except GuardError as error:
        print(f"BENCH_REFUSE: {error}", file=sys.stderr)
        meta["status"] = "failed"
        meta["error"] = provenance["error"] = str(error)
        return 10
    except (BenchAborted, KeyboardInterrupt) as interrupt:
        name = interrupt.signal_name if isinstance(interrupt, BenchAborted) else "SIGINT"
        meta["status"] = "aborted"
        meta["error"] = provenance["error"] = f"ABORTED: {name}"
        print(meta["error"], file=sys.stderr)
        return 130
    except (CaptureError, InstrumentError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        meta["status"] = "failed"
        meta["error"] = str(error)
        if str(error).startswith("NEEDS_COLD_CACHE_PREP"):
            return 20
        if str(error).startswith(EXIT_PRESENTATION):
            return 40
        return 30 if str(error).startswith("MISSING_TOOL") else 1
    finally:
        # A second signal while sealing would abort the seal and leak the isolated root.
        handlers = {number: signal.signal(number, signal.SIG_IGN) for number in sealing_signals}
        if binary is not None and binary_sha256 is not None and _binary_changed(binary, binary_sha256):
            outcome["binary_changed"] = True
            meta["status"] = "failed"
            meta["error"] = f"BINARY_CHANGED: {binary} changed while the capture ran"
            provenance["error"] = meta["error"]
            print(meta["error"], file=sys.stderr)
        if meta.get("status") == "pending":
            meta["status"] = "failed"
        if meta_path:
            meta_path.write_text(json.dumps(meta, indent=2)+"\n")
        try:
            if raw is not None:
                provenance["status"] = meta.get("status", "failed")
                finalize_raw(raw, root, output, written, provenance, binary_sha256)
        except (OSError, CaptureError, ValueError) as error:
            print(f"RAW_FINALIZE_FAILED: {error}", file=sys.stderr)
        finally:
            try:
                if lock_fd is not None:
                    os.close(lock_fd)
                if root:
                    if args.keep:
                        print(f"Retained isolated root: {root}")
                    else:
                        shutil.rmtree(root)
            finally:
                handlers.update(previous)  # SIGTERM/SIGHUP go back to what they were before the run
                for number, handler in handlers.items():
                    signal.signal(number, handler or signal.SIG_DFL)


if __name__ == "__main__":
    sys.exit(main())
