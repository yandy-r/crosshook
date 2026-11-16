"""The opt-in health scenarios on the CLI: selection, unchanged defaults, fixtures gate, dry run."""
import contextlib
import csv
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_cli
from bench_capture import CaptureError
from bench_guard import Originals

BASE = ["--variant", "B", "--env", "E1"]
PRESENT = [*BASE, "--collect", "presentation"]
G4_DEFAULT = ["library", "console", "settings"]
G5_DEFAULT = ["search", "toggle_favorite", "toggle_optimization", "dpad", "route_switch"]


def parse(argv):
    err = io.StringIO()
    with contextlib.redirect_stderr(err):
        try:
            return bench.arguments(argv), None
        except SystemExit as caught:
            assert caught.code == 2, caught.code
            return None, err.getvalue()


class SelectionTests(unittest.TestCase):
    def test_the_supplementals_are_declared_per_metric(self):
        self.assertEqual(bench_cli.SUPPLEMENTAL_SCENARIOS, {"G4": ("health",), "G5": ("health_table",)})

    def test_the_default_lists_do_not_change_and_exclude_the_supplementals(self):
        self.assertEqual(bench_cli.SCENARIOS["G4"], tuple(G4_DEFAULT))
        self.assertEqual(bench_cli.SCENARIOS["G5"], tuple(G5_DEFAULT))
        args = bench.arguments([*PRESENT, "--metrics", "G4,G5"])
        self.assertEqual(args.scenarios, {"G4": G4_DEFAULT, "G5": G5_DEFAULT})

    def test_health_alone_is_selectable_for_g4(self):
        args = bench.arguments([*PRESENT, "--metric", "G4", "--scenario", "health"])
        self.assertEqual(args.scenarios, {"G4": ["health"]})

    def test_health_table_alone_is_selectable_for_g5(self):
        args = bench.arguments([*PRESENT, "--metric", "G5", "--scenario", "health_table"])
        self.assertEqual(args.scenarios, {"G5": ["health_table"]})

    def test_a_supplemental_can_be_combined_with_defaults_in_canonical_order(self):
        args = bench.arguments([*PRESENT, "--metric", "G4", "--scenario", "health,library"])
        self.assertEqual(args.scenarios, {"G4": ["library", "health"]})
        args = bench.arguments([*PRESENT, "--metric", "G5", "--scenario", "health_table,search"])
        self.assertEqual(args.scenarios, {"G5": ["search", "health_table"]})

    def test_a_metric_no_value_addresses_keeps_its_defaults(self):
        args = bench.arguments([*PRESENT, "--metric", "G4", "--metric", "G5", "--scenario", "health_table"])
        self.assertEqual(args.scenarios, {"G4": G4_DEFAULT, "G5": ["health_table"]})
        args = bench.arguments([*PRESENT, "--metric", "G4", "--metric", "G5", "--scenario", "health"])
        self.assertEqual(args.scenarios, {"G4": ["health"], "G5": G5_DEFAULT})

    def test_each_supplemental_belongs_to_its_own_metric_only(self):
        for metric, name in (("G4", "health_table"), ("G5", "health"), ("G8", "health"), ("G9", "health_table")):
            with self.subTest(metric=metric, name=name):
                args, error = parse([*PRESENT, "--metric", metric, "--scenario", name])
                self.assertIsNone(args)
                self.assertIn(f"--scenario value {name!r} is not valid", error)

    def test_the_error_lists_the_opt_in_scenarios(self):
        _args, error = parse([*PRESENT, "--metric", "G5", "--scenario", "nope"])
        self.assertIn("G5: search,toggle_favorite,toggle_optimization,dpad,route_switch,health_table", error)

    def test_the_help_says_the_health_scenarios_are_opt_in(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out), self.assertRaises(SystemExit):
            bench.arguments(["--help"])
        text = " ".join(out.getvalue().split())
        for expected in ("health", "health_table", "opt-in", "never run by default"):
            self.assertIn(expected, text)

    def test_scenario_still_needs_presentation_collection(self):
        _args, error = parse([*BASE, "--metric", "G5", "--scenario", "health_table"])
        self.assertIn("--scenario requires --collect presentation", error)


class FixturesGateTests(unittest.TestCase):
    def gate(self, metric, scenario, *, fixtures):
        args = bench.arguments([*PRESENT, "--metric", metric, "--scenario", scenario,
                                *(["--fixtures", "/fixtures"] if fixtures else [])])
        with patch.object(bench.shutil, "which", return_value="/usr/bin/gdbus"), \
                patch.object(bench, "require_no_instance"):
            bench._require_gates(args, [metric])

    def test_the_health_scenarios_need_the_500_profile_fixtures(self):
        for metric, scenario in (("G4", "health"), ("G5", "health_table")):
            with self.subTest(scenario=scenario):
                with self.assertRaisesRegex(CaptureError, r"^FIXTURES_REQUIRED: the health scenarios need "
                                                          r"the 500-profile fixtures \(≥ 300 health rows\)$"):
                    self.gate(metric, scenario, fixtures=False)
                self.gate(metric, scenario, fixtures=True)

    def test_the_default_g5_scenarios_still_need_no_fixtures(self):
        self.gate("G5", "search", fixtures=False)
        self.gate("G4", "library", fixtures=False)


class DryRunTests(unittest.TestCase):
    def dry_run(self, metric, scenario):
        with tempfile.TemporaryDirectory() as tmp:
            original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
            with patch.object(bench.Originals, "capture", return_value=original), \
                    contextlib.redirect_stdout(io.StringIO()):
                code = bench.main([*PRESENT, "--metric", metric, "--scenario", scenario, "--dry-run",
                                   "--output", tmp])
            out = Path(tmp) / "B/E1"
            with (out / f"{metric}.csv").open(newline="") as stream:
                rows = list(csv.DictReader(stream))
            return code, rows, json.loads((out / "meta.json").read_text())

    def test_dry_runs_write_a_dry_run_row_without_fixtures(self):
        for metric, scenario in (("G4", "health"), ("G5", "health_table")):
            with self.subTest(scenario=scenario):
                code, rows, meta = self.dry_run(metric, scenario)
                self.assertEqual(code, 0)
                self.assertEqual([row["status"] for row in rows], ["dry-run"])
                self.assertEqual(meta["status"], "dry-run")


if __name__ == "__main__":
    unittest.main()
