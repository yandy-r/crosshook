"""Argument parsing for the isolated benchmark runner (`bench.py`)."""
from __future__ import annotations

import argparse
import math
import tempfile

_SCENARIO_COLUMNS = "scenario,p50_ms,p95_ms,p99_ms,stddev_ms,over_interval_percent,frames,status"
SCHEMAS = {
    "G1": "iteration,ready_ms,first_present_ms,p50_ms,p95_ms,first_present_p50_ms,first_present_p95_ms,status",
    "G2": "iteration,ready_ms,first_present_ms,p50_ms,p95_ms,first_present_p50_ms,first_present_p95_ms,status",
    "G3": "elapsed_s,pss_mb,uss_mb,growth_mb,gpu_mb,status",
    "G4": _SCENARIO_COLUMNS,
    "G5": "scenario,p50_ms,p95_ms,p99_ms,measured,ambiguous,timeout,early,status",
    "G6": "stalls_over_16ms,stalls_over_50ms,duration_s,status",
    "G7": "elapsed_s,cpu_percent,wakeups_s,power_w,status",
    "G8": "p50_ms,p95_ms,p99_ms,stddev_ms,over_interval_percent,memory_growth_mb,status",
    "G9": _SCENARIO_COLUMNS,
    "G10": "binary_mb,bundle_mb,installed_mb,runtime_delta_mb,status",
}
# Metrics a presentation run can produce, and the input-driven scenarios each one owns (default order).
PRESENTATION_METRICS = ("G1", "G2", "G4", "G5", "G9")
SCENARIOS = {"G4": ("library", "console", "settings"),
             "G5": ("search", "toggle_favorite", "toggle_optimization", "dpad", "route_switch"),
             "G9": ("resize",)}
PHASE_3_REFUSAL = "G8 producer arrives with YAN-2056"
DEFAULT_TRIALS, MIN_TRIALS, MAX_TRIALS = 20, 2, 200


def positive(raw: str) -> float:
    value = float(raw)
    if not math.isfinite(value) or value <= 0:
        raise argparse.ArgumentTypeError("must be finite and positive")
    return value


def _scenarios(parser: argparse.ArgumentParser, args) -> dict[str, list[str]]:
    """Resolve `--scenario` into {metric: [scenario, ...]} for the selected G4/G5/G9 metrics.

    Each listed value must belong to a selected metric's allowlist; a selected metric
    that no listed value addresses keeps its full default list.
    """
    owners = {m: SCENARIOS[m] for m in SCENARIOS if m in args.selected}
    if args.scenario is None:
        return {m: list(names) for m, names in owners.items()}
    if args.collect != "presentation" or not owners:
        parser.error("--scenario requires --collect presentation with G4, G5 or G9")
    requested = [item.strip() for item in args.scenario.split(",")]
    for item in requested:
        if not any(item in names for names in owners.values()):
            allowed = "; ".join(f"{metric}: {','.join(names)}" for metric, names in owners.items())
            parser.error(f"--scenario value {item!r} is not valid for the selected metrics ({allowed})")
    return {metric: [name for name in names if name in requested] or list(names)
            for metric, names in owners.items()}


def arguments(argv: list[str] | None = None):
    parser = argparse.ArgumentParser(description="Isolated CrossHook benchmark runner. "
                                     "Dry-run schemas are never measurements.")
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
    parser.add_argument("--scenario", help="comma list of input scenarios for --collect presentation: "
                        "G4 library,console,settings (default all); "
                        "G5 search,toggle_favorite,toggle_optimization,dpad,route_switch (default all); "
                        "G9 resize (default)")
    parser.add_argument("--trials", type=int, default=DEFAULT_TRIALS,
                        help=f"G5 trials per scenario and iteration (even, {MIN_TRIALS}..{MAX_TRIALS}, "
                        f"default {DEFAULT_TRIALS})")
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
    if not MIN_TRIALS <= args.trials <= MAX_TRIALS or args.trials % 2:
        parser.error(f"--trials requires an even number {MIN_TRIALS}..{MAX_TRIALS}, "
                     "so toggles return to their start state")
    if args.collect == "presentation":
        if "G8" in args.selected:
            parser.error(f"--collect presentation: {PHASE_3_REFUSAL}")
        if any(m not in PRESENTATION_METRICS for m in args.selected):
            parser.error("--collect presentation supports G1/G2/G4/G5/G9 only")
        if args.capture_csv:
            parser.error("--collect presentation cannot be combined with --capture-csv")
    if not args.branch or args.branch.startswith("-"):
        parser.error("invalid Flatpak branch")
    args.scenarios = _scenarios(parser, args)
    return args
