#!/usr/bin/env python3
"""Isolated CrossHook benchmark runner. Dry-run schemas are never measurements."""
from __future__ import annotations

import argparse
import csv
import json
import os
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

import bench_guard
from bench_capture import CaptureError, capture
from bench_guard import (
    FLATPAK_APP_ID,
    GuardError,
    Originals,
    build_env,
    make_dirs,
    preflight,
)
from bench_instruments import InstrumentError
from bench_provenance import boot_id, create_raw_dir, finalize_raw, git_state, new_run_id, verify_capture

REPO = Path(__file__).resolve().parents[2]
SCHEMAS = {
    "G1": "iteration,ready_ms,first_present_ms,p50_ms,p95_ms,first_present_p50_ms,first_present_p95_ms,status",
    "G2": "iteration,ready_ms,first_present_ms,p50_ms,p95_ms,first_present_p50_ms,first_present_p95_ms,status",
    "G3": "elapsed_s,pss_mb,uss_mb,growth_mb,gpu_mb,status",
    "G4": "p50_ms,p95_ms,p99_ms,stddev_ms,over_interval_percent,status",
    "G5": "scenario,latency_ms,p95_ms,status",
    "G6": "stalls_over_16ms,stalls_over_50ms,duration_s,status",
    "G7": "elapsed_s,cpu_percent,wakeups_s,power_w,status",
    "G8": "p50_ms,p95_ms,p99_ms,stddev_ms,over_interval_percent,memory_growth_mb,status",
    "G9": "p50_ms,p95_ms,p99_ms,stddev_ms,over_interval_percent,status",
    "G10": "binary_mb,bundle_mb,installed_mb,runtime_delta_mb,status",
}
TOOLS = ("hyperfine", "ydotool", "gpu-screen-recorder", "perf", "pidstat", "upower", "flatpak", "ffmpeg", "cc")


def positive(raw: str) -> float:
    import math
    value = float(raw)
    if not math.isfinite(value) or value <= 0:
        raise argparse.ArgumentTypeError("must be finite and positive")
    return value


def arguments(argv: list[str] | None = None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--variant", choices=("A", "B", "C", "C-iced", "C-gpui"), required=True)
    parser.add_argument("--env", choices=("E1", "E2-D", "E2-G", "E3", "E3-sw"), required=True)
    parser.add_argument("--metric", choices=tuple(SCHEMAS), action="append")
    parser.add_argument("--metrics", default="all")
    parser.add_argument("--iterations", type=int, default=10)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--binary")
    parser.add_argument("--bundle")
    parser.add_argument("--runtime-delta-mb", type=float)
    parser.add_argument("--flatpak", action="store_true")
    parser.add_argument("--branch", default="master")
    parser.add_argument("--fixtures")
    parser.add_argument("--output", default="results")
    parser.add_argument("--tmp-root", default=tempfile.gettempdir())
    parser.add_argument("--timeout", type=positive, default=60)
    parser.add_argument("--duration", type=positive, default=None)
    parser.add_argument("--refresh-hz", type=positive, default=120)
    parser.add_argument("--capture-csv")
    parser.add_argument("--collect", choices=("csv", "presentation"), default="csv")
    parser.add_argument("--cold-cache-ack", action="store_true")
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args(argv)
    args.selected = args.metric or (list(SCHEMAS) if args.metrics == "all" else args.metrics.split(","))
    if args.duration is None:
        args.duration = 600
        if not args.dry_run and "G6" in args.selected and len(args.selected) > 1:
            parser.error("G6 uses a 300 s session; run it separately or pass --duration")
        if args.selected == ["G6"]:
            args.duration = 300
    if not args.selected or any(m not in SCHEMAS for m in args.selected):
        parser.error("--metrics requires all or comma-separated G1..G10")
    if not 1 <= args.iterations <= 1000:
        parser.error("--iterations requires 1..1000")
    if args.collect == "presentation":
        if any(m not in ("G1", "G2") for m in args.selected):
            parser.error("--collect presentation supports G1/G2 in this release; "
                         "G4/G5/G8/G9 producers are not implemented yet")
        if args.capture_csv:
            parser.error("--collect presentation cannot be combined with --capture-csv")
    if not args.branch or args.branch.startswith("-"):
        parser.error("invalid Flatpak branch")
    return args


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
    root: Path | None = None
    meta_path: Path | None = None
    meta: dict = {}
    raw: Path | None = None
    output: Path | None = None
    provenance: dict = {}
    try:
        original = Originals.capture()
        paths = preflight(original, output=args.output, fixtures=args.fixtures,
                          tmp_root=args.tmp_root, capture_csv=args.capture_csv)
        if args.binary and not Path(args.binary).is_file():
            raise CaptureError("BINARY_REQUIRED: executable file not found")
        if args.bundle:
            preflight(original, bundle=args.bundle)
        if not args.dry_run:
            required = {"G7": ("perf", "upower"),
                        "G10": ("strip", "flatpak")}
            missing = {tool for metric in args.selected for tool in required.get(metric, ())
                       if shutil.which(tool) is None}
            if missing:
                raise CaptureError("MISSING_TOOL: " + ", ".join(sorted(missing)))
            if "G1" in args.selected and not args.cold_cache_ack:
                raise CaptureError("NEEDS_COLD_CACHE_PREP: G1 requires --cold-cache-ack")
            for metric in args.selected:
                if metric in ("G4", "G5", "G6", "G8", "G9") and not args.capture_csv:
                    if metric == "G6" and shutil.which("perf") is None:
                        raise CaptureError("MISSING_TOOL: perf")
                    raise CaptureError(f"CAPTURE_REQUIRED: --capture-csv for {metric}")
            if "G8" in args.selected and not args.fixtures:
                raise CaptureError("FIXTURES_REQUIRED: G8 needs the 50k-line replay fixture")
        meta = metadata(args)
        # No filesystem mutation before all path/schema preflights succeed.
        output = paths["output"] / args.variant / args.env
        output.mkdir(parents=True, exist_ok=True)
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
        external_only = all(m in ("G4", "G5", "G6", "G9", "G10") for m in args.selected)
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
        written: list[Path] = []
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
            rows = [{"status": "dry-run"}] if args.dry_run else capture(metric, args, command, env,
                                                                 command_for=lambda e: launch_command(args, e))
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
        return 10
    except (CaptureError, InstrumentError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        meta["status"] = "failed"
        meta["error"] = str(error)
        if str(error).startswith("NEEDS_COLD_CACHE_PREP"):
            return 20
        if str(error).startswith(("NO_PRESENTATION_GLOBAL", "PRESENTATION_CLOCK", "REFRESH_MISMATCH")):
            return 40
        return 30 if str(error).startswith("MISSING_TOOL") else 1
    finally:
        if meta.get("status") == "pending":
            meta["status"] = "failed"
        if meta_path:
            meta_path.write_text(json.dumps(meta, indent=2)+"\n")
        try:
            if raw is not None:
                provenance["status"] = meta.get("status", "failed")
                finalize_raw(raw, root, output, written, provenance,
                             Path(args.binary) if args.binary and not args.flatpak else None)
        except (OSError, CaptureError, ValueError) as error:
            print(f"RAW_FINALIZE_FAILED: {error}", file=sys.stderr)
        finally:
            if root:
                if args.keep:
                    print(f"Retained isolated root: {root}")
                else:
                    shutil.rmtree(root)


if __name__ == "__main__":
    sys.exit(main())
