"""G8 on the CLI: presentation acceptance, schema, scenario, driven env, provenance and sealing."""
import contextlib
import csv
import io
import json
import signal
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_cli
import bench_session
import bench_shim
from bench_guard import Originals
from bench_provenance import finalize_raw, verify_capture

BASE = ["--variant", "A", "--env", "E1"]
PRESENT = [*BASE, "--collect", "presentation"]
G8_COLUMNS = ("scenario,p50_ms,p95_ms,p99_ms,stddev_ms,over_interval_percent,frames,memory_growth_mb,"
              "peak_pss_mb,gpu_growth_mb,gpu_source,follow,status")


def unexpected_signal(signum, _frame):
    raise AssertionError(f"{signal.Signals(signum).name} reached no handler the runner installed")


def parse(argv):
    err = io.StringIO()
    with contextlib.redirect_stderr(err):
        try:
            return bench.arguments(argv), None
        except SystemExit as caught:
            assert caught.code == 2, caught.code
            return None, err.getvalue()


class ArgumentTests(unittest.TestCase):
    def test_g8_is_a_presentation_metric_with_the_replay_scroll_scenario(self):
        args = bench.arguments([*PRESENT, "--metric", "G8"])
        self.assertEqual((args.collect, args.scenarios), ("presentation", {"G8": ["replay_scroll"]}))
        self.assertIn("G8", bench_cli.PRESENTATION_METRICS)
        self.assertEqual(bench_cli.SCENARIOS["G8"], ("replay_scroll",))
        self.assertIn("G8", bench.INPUT_DRIVEN)

    def test_the_phase_3_refusal_is_gone(self):
        self.assertFalse(hasattr(bench_cli, "PHASE_3_REFUSAL"))
        args, error = parse([*PRESENT, "--metrics", "G5,G8"])
        self.assertEqual((error, args.scenarios.keys()), (None, {"G5", "G8"}))

    def test_the_schema_is_the_single_session_columns(self):
        self.assertEqual(bench.SCHEMAS["G8"], G8_COLUMNS)

    def test_scenario_flag_accepts_replay_scroll_and_refuses_other_names(self):
        args = bench.arguments([*PRESENT, "--metric", "G8", "--scenario", "replay_scroll"])
        self.assertEqual(args.scenarios, {"G8": ["replay_scroll"]})
        args = bench.arguments([*PRESENT, "--metric", "G4", "--metric", "G8", "--scenario", "console,replay_scroll"])
        self.assertEqual(args.scenarios, {"G4": ["console"], "G8": ["replay_scroll"]})
        for value in ("resize", "console"):
            with self.subTest(value=value):
                args, error = parse([*PRESENT, "--metric", "G8", "--scenario", value])
                self.assertIsNone(args)
                self.assertIn(repr(value), error)

    def test_capture_csv_is_still_incompatible_with_presentation(self):
        args, error = parse([*PRESENT, "--metric", "G8", "--capture-csv", "x.csv"])
        self.assertIsNone(args)
        self.assertIn("cannot be combined with --capture-csv", error)

    def test_csv_mode_g8_is_unchanged(self):
        args = bench.arguments([*BASE, "--metric", "G8", "--capture-csv", "x.csv"])
        self.assertEqual((args.collect, args.selected), ("csv", ["G8"]))

    def test_help_lists_g8_under_presentation(self):
        out = subprocess.run([sys.executable, str(Path(bench.__file__)), "--help"], capture_output=True,
                             text=True, check=True).stdout
        self.assertIn("G8 replay_scroll", " ".join(out.split()))


class RunTestCase(unittest.TestCase):
    def setUp(self):
        before = {n: signal.signal(n, unexpected_signal) for n in (signal.SIGTERM, signal.SIGHUP, signal.SIGINT)}
        self.addCleanup(lambda: [signal.signal(n, handler) for n, handler in before.items()])
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.output = self.root / "out"
        self.fake_build = bench_shim.ShimBuild(self.root / "libwlpresent.so", "0" * 64, "0" * 64, "host")
        self.fixtures = self.root / "fixtures"
        self.fixtures.mkdir()
        (self.fixtures / "manifest.json").write_text('{"seed": 42}')

    def run_bench(self, *, capture, argv=PRESENT, fixtures=True, extra=()):
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        err = io.StringIO()
        with patch.object(bench.Originals, "capture", return_value=original), \
                patch.object(bench_shim, "build", return_value=self.fake_build), \
                patch.object(bench, "capture", side_effect=capture), \
                patch.object(bench, "make_dirs"), patch.object(bench, "materialize"), \
                patch.object(bench_session, "_session_bus_available", return_value=True), \
                patch.object(bench_session, "_probe", return_value="(false,)"), \
                patch.object(bench.shutil, "which", side_effect=lambda tool: f"/usr/bin/{tool}"), \
                contextlib.redirect_stderr(err):
            code = bench.main([*argv, "--metric", "G8", "--binary", sys.executable,
                               "--output", str(self.output), "--tmp-root", str(self.root / "runs"),
                               *(["--fixtures", str(self.fixtures)] if fixtures else []), *extra])
        return code, err.getvalue()


class G8RunTests(RunTestCase):
    def driven_rows(self, _metric, args, _command, env, **kwargs):
        self.seen_run_id = args.run_id
        self.seen_env = dict(env)
        observations = Path(env["CROSSHOOK_BENCH_ROOT"]) / "observations"
        observations.mkdir()
        for suffix in ("memory", "frames"):
            (observations / f"G8-replay_scroll-{suffix}.csv").write_text("x\n1\n")
        kwargs["evidence"]["G8"] = {"replay_scroll": [{"iteration": 1, "gpu_source": "fdinfo",
                                                       "nvidia_smi_path": None, "sampled_pids": []}]}
        return [{"scenario": "replay_scroll", "p50_ms": 8.3, "frames": 100, "memory_growth_mb": 1.5,
                 "peak_pss_mb": 300.0, "gpu_source": "unavailable:no_tool", "follow": "on", "status": "captured"}]

    def test_a_driven_g8_run_writes_the_schema_seals_observations_and_records_drive_stats(self):
        code, err = self.run_bench(capture=self.driven_rows)
        self.assertEqual((code, err), (0, ""))
        with (self.output / "A/E1/G8.csv").open() as stream:
            reader = csv.DictReader(stream)
            (row,) = list(reader)
        self.assertEqual(",".join(reader.fieldnames), G8_COLUMNS)
        self.assertEqual((row["gpu_growth_mb"], row["gpu_source"]), ("", "unavailable:no_tool"))
        (raw,) = (self.output / "raw").iterdir()
        manifest = json.loads((raw / "provenance.json").read_text())
        self.assertEqual((manifest["status"], manifest["scenarios"]["G8"]), ("captured", ["replay_scroll"]))
        self.assertEqual(manifest["drive_stats"]["G8"]["replay_scroll"][0]["gpu_source"], "fdinfo")
        self.assertEqual(self.seen_run_id, manifest["run_id"])
        sums = (raw / "SHA256SUMS").read_text()
        for name in ("G8-replay_scroll-memory.csv", "G8-replay_scroll-frames.csv"):
            self.assertTrue((raw / "observations" / name).is_file())
            self.assertIn(f"observations/{name}", sums)
        self.assertIn("G8.csv", sums)

    def test_the_driven_env_carries_no_replay_variables_the_collector_sets_them_per_launch(self):
        self.run_bench(capture=self.driven_rows)
        self.assertNotIn("CROSSHOOK_BENCH_LOG", self.seen_env)
        self.assertNotIn("CROSSHOOK_BENCH_LOG_RATE", self.seen_env)

    def test_g8_still_requires_the_fixtures(self):
        code, err = self.run_bench(capture=AssertionError("launched"), fixtures=False)
        self.assertEqual(code, 1)
        self.assertIn("FIXTURES_REQUIRED: G8 needs the 50k-line replay fixture", err)

    def test_a_failure_code_is_reported_and_the_run_is_marked_failed(self):
        def refuse(*_a, **_k):
            raise bench.CaptureError("CONSOLE_NOT_VISIBLE: the console log is not visible")
        code, err = self.run_bench(capture=refuse)
        self.assertEqual(code, 1)
        self.assertIn("CONSOLE_NOT_VISIBLE", err)
        self.assertEqual(json.loads((self.output / "A/E1/meta.json").read_text())["status"], "failed")

    def test_the_g8_dry_run_writes_the_new_schema(self):
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        with patch.object(bench.Originals, "capture", return_value=original), \
                patch.object(bench_session, "_probe", side_effect=AssertionError("probed during dry run")):
            code = bench.main([*PRESENT, "--metric", "G8", "--dry-run", "--output", str(self.output)])
        self.assertEqual(code, 0)
        with (self.output / "A/E1/G8.csv").open() as stream:
            reader = csv.DictReader(stream)
            (row,) = list(reader)
        self.assertEqual((",".join(reader.fieldnames), row["status"]), (G8_COLUMNS, "dry-run"))


class CsvModeTests(RunTestCase):
    def test_csv_mode_g8_still_launches_with_the_replay_env_and_ingests(self):
        seen = {}

        def ingest(_metric, _args, _command, env, **_kwargs):
            seen.update(env)
            return [{"scenario": "external", "p50_ms": 8.3, "frames": 4, "memory_growth_mb": 2.0,
                     "status": "measured"}]
        external = self.root / "frames.csv"
        external.write_text("frame_ms\n8\n")
        with patch.object(bench, "verify_capture"):
            code, err = self.run_bench(capture=ingest, argv=BASE, extra=["--capture-csv", str(external)])
        self.assertEqual((code, err), (0, ""))
        self.assertEqual(seen["CROSSHOOK_BENCH_LOG_RATE"], "5000")
        self.assertTrue(seen["CROSSHOOK_BENCH_LOG"].endswith("logs/launch-50k.log"))
        with (self.output / "A/E1/G8.csv").open() as stream:
            (row,) = list(csv.DictReader(stream))
        self.assertEqual((row["scenario"], row["peak_pss_mb"], row["follow"], row["status"]),
                         ("external", "", "", "measured"))


class SealingTests(unittest.TestCase):
    def test_finalize_raw_seals_g8_and_g5_observations_and_ignores_other_files(self):
        with tempfile.TemporaryDirectory() as temp:
            root, output, raw = (Path(temp) / name for name in ("root", "out", "raw"))
            (root / "observations").mkdir(parents=True)
            output.mkdir()
            raw.mkdir()
            for name in ("G8-replay_scroll-memory.csv", "G8-replay_scroll-frames.csv", "G5-search.csv",
                         "notes.csv"):
                (root / "observations" / name).write_text("a\n1\n")
            finalize_raw(raw, root, output, [], {"status": "captured"})
            sealed = sorted(path.name for path in (raw / "observations").iterdir())
            self.assertEqual(sealed, ["G5-search.csv", "G8-replay_scroll-frames.csv",
                                      "G8-replay_scroll-memory.csv"])
            sums = (raw / "SHA256SUMS").read_text()
            self.assertEqual(sums.count("observations/"), 3)
            self.assertNotIn("notes.csv", sums)
            for path in (raw / "observations").iterdir():
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)

    def test_a_tampered_g8_observation_breaks_the_seal(self):
        with tempfile.TemporaryDirectory() as temp:
            root, output, raw = (Path(temp) / name for name in ("root", "out", "raw"))
            (root / "observations").mkdir(parents=True)
            output.mkdir()
            raw.mkdir()
            (output / "G8.csv").write_text("scenario\nreplay_scroll\n")
            (root / "observations/G8-replay_scroll-memory.csv").write_text("a\n1\n")
            finalize_raw(raw, root, output, [output / "G8.csv"],
                         {"status": "captured", "dirty": False, "boot_id": "b"})
            (raw / "observations/G8-replay_scroll-memory.csv").write_text("a\n2\n")
            with self.assertRaisesRegex(Exception, "^STALE_CAPTURE: checksum mismatch"):
                verify_capture(raw / "G8.csv", {})


if __name__ == "__main__":
    unittest.main()
