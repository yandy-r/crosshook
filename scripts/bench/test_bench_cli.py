"""Runner gates, --scenario, schemas, exit codes, abort signals and instance refusal; stdlib only."""
import contextlib
import csv
import io
import json
import os
import signal
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_cli
import bench_guard
import bench_session
import bench_shim
from bench_capture import CaptureError
from bench_guard import GuardError, Originals
from bench_provenance import verify_capture

BASE = ["--variant", "A", "--env", "E1"]
PRESENT = [*BASE, "--collect", "presentation"]
COLUMNS = "scenario,p50_ms,p95_ms,p99_ms,stddev_ms,over_interval_percent,frames,status"


def unexpected_signal(signum, _frame):
    raise AssertionError(f"{signal.Signals(signum).name} reached no handler the runner installed")


def parse(argv):
    """(args, None) on success, (None, stderr) when argparse exits 2."""
    err = io.StringIO()
    with contextlib.redirect_stderr(err):
        try:
            return bench.arguments(argv), None
        except SystemExit as caught:
            assert caught.code == 2, caught.code
            return None, err.getvalue()


class ArgumentTests(unittest.TestCase):
    def test_collect_defaults_to_csv(self):
        self.assertEqual(bench.arguments([*BASE, "--metric", "G2"]).collect, "csv")

    def test_presentation_gate_matrix(self):
        allowed = {"G1", "G2", "G4", "G5", "G9"}
        for metric in bench.SCHEMAS:
            with self.subTest(metric=metric):
                args, error = parse([*PRESENT, "--metric", metric])
                if metric in allowed:
                    self.assertEqual(args.collect, "presentation")
                    continue
                self.assertIsNone(args)
                if metric == "G8":
                    self.assertIn("G8 producer arrives with YAN-2056", error)
                else:
                    self.assertIn("supports G1/G2/G4/G5/G9 only", error)

    def test_presentation_rejects_a_mixed_selection_with_a_refused_metric(self):
        for refused in ("G8", "G3"):
            with self.subTest(refused=refused):
                self.assertIsNone(parse([*PRESENT, "--metrics", f"G4,{refused}"])[0])

    def test_capture_csv_is_incompatible_with_presentation(self):
        for metric in ("G2", "G4", "G5", "G9"):
            with self.subTest(metric=metric):
                args, error = parse([*PRESENT, "--metric", metric, "--capture-csv", "x.csv"])
                self.assertIsNone(args)
                self.assertIn("cannot be combined with --capture-csv", error)

    def test_csv_collect_still_accepts_every_metric(self):
        for metric in bench.SCHEMAS:
            with self.subTest(metric=metric):
                self.assertEqual(bench.arguments([*BASE, "--metric", metric]).selected, [metric])

    def test_scenario_defaults_cover_only_selected_metrics(self):
        self.assertEqual(bench.arguments([*PRESENT, "--metric", "G4"]).scenarios,
                         {"G4": ["library", "console", "settings"]})
        self.assertEqual(bench.arguments([*PRESENT, "--metric", "G9"]).scenarios, {"G9": ["resize"]})
        self.assertEqual(bench.arguments([*PRESENT, "--metrics", "G1,G4,G9"]).scenarios,
                         {"G4": ["library", "console", "settings"], "G9": ["resize"]})
        self.assertEqual(bench.arguments([*PRESENT, "--metric", "G2"]).scenarios, {})

    def test_scenario_subsets_are_parsed(self):
        cases = [("G4", "settings,library", {"G4": ["library", "settings"]}),
                 ("G4", "console", {"G4": ["console"]}),
                 ("G4", "library,console,settings", {"G4": ["library", "console", "settings"]}),
                 ("G9", "resize", {"G9": ["resize"]})]
        for metric, value, expected in cases:
            with self.subTest(metric=metric, value=value):
                self.assertEqual(bench.arguments([*PRESENT, "--metric", metric, "--scenario", value]).scenarios,
                                 expected)

    def test_scenario_routes_values_to_the_metric_that_owns_them(self):
        args = bench.arguments([*PRESENT, "--metric", "G4", "--metric", "G9", "--scenario", "resize,console"])
        self.assertEqual(args.scenarios, {"G4": ["console"], "G9": ["resize"]})
        args = bench.arguments([*PRESENT, "--metric", "G4", "--metric", "G9", "--scenario", "settings"])
        self.assertEqual(args.scenarios, {"G4": ["settings"], "G9": ["resize"]})

    def test_invalid_scenarios_are_refused(self):
        cases = [(["--metric", "G4", "--scenario", "resize"], "resize"),
                 (["--metric", "G9", "--scenario", "library"], "library"),
                 (["--metric", "G4", "--scenario", "library,nope"], "nope"),
                 (["--metric", "G4", "--scenario", "library,"], "''"),
                 (["--metric", "G4", "--scenario", ""], "''"),
                 (["--metric", "G4", "--scenario", "LIBRARY"], "LIBRARY"),
                 (["--metric", "G2", "--scenario", "library"], "requires --collect presentation with G4, G5 or G9")]
        for extra, text in cases:
            with self.subTest(extra=extra):
                args, error = parse([*PRESENT, *extra])
                self.assertIsNone(args)
                self.assertIn(text, error)

    def test_scenario_requires_presentation_collect(self):
        args, error = parse([*BASE, "--metric", "G4", "--scenario", "library"])
        self.assertIsNone(args)
        self.assertIn("requires --collect presentation", error)

    def test_input_driven_schemas_carry_scenario_and_frames(self):
        self.assertEqual(bench.SCHEMAS["G4"], COLUMNS)
        self.assertEqual(bench.SCHEMAS["G9"], COLUMNS)
        self.assertEqual(bench.SCHEMAS["G5"],
                         "scenario,p50_ms,p95_ms,p99_ms,measured,ambiguous,timeout,early,status")

    def test_g5_scenarios_default_to_all_five_and_subsets_parse(self):
        names = ["search", "toggle_favorite", "toggle_optimization", "dpad", "route_switch"]
        self.assertEqual(bench.arguments([*PRESENT, "--metric", "G5"]).scenarios, {"G5": names})
        args = bench.arguments([*PRESENT, "--metric", "G5", "--scenario", "route_switch,search"])
        self.assertEqual(args.scenarios, {"G5": ["search", "route_switch"]})
        args = bench.arguments([*PRESENT, "--metric", "G4", "--metric", "G5", "--scenario", "dpad,console"])
        self.assertEqual(args.scenarios, {"G4": ["console"], "G5": ["dpad"]})
        args, error = parse([*PRESENT, "--metric", "G5", "--scenario", "library"])
        self.assertIsNone(args)
        self.assertIn("'library'", error)

    def test_trials_default_and_bounds(self):
        self.assertEqual(bench.arguments([*PRESENT, "--metric", "G5"]).trials, 20)
        for value in ("2", "40", "200"):
            with self.subTest(value=value):
                self.assertEqual(bench.arguments([*PRESENT, "--metric", "G5", "--trials", value]).trials,
                                 int(value))
        for value in ("1", "3", "21", "0", "-2", "202", "400"):
            with self.subTest(value=value):
                args, error = parse([*PRESENT, "--metric", "G5", "--trials", value])
                self.assertIsNone(args)
                self.assertIn("--trials requires an even number 2..200", error)

    def test_g8_refusal_names_the_followup_issue(self):
        args, error = parse([*PRESENT, "--metrics", "G5,G8"])
        self.assertIsNone(args)
        self.assertIn("G8 producer arrives with YAN-2056", error)
        self.assertEqual(bench_cli.PHASE_3_REFUSAL, "G8 producer arrives with YAN-2056")


class PresentationRunTests(unittest.TestCase):
    def setUp(self):
        # A signal these tests send must only ever reach a handler the runner installed; if one
        # escapes, fail the test instead of killing the test process.
        before = {n: signal.signal(n, unexpected_signal) for n in (signal.SIGTERM, signal.SIGHUP, signal.SIGINT)}
        self.addCleanup(lambda: [signal.signal(n, handler) for n, handler in before.items()])
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.output = self.root / "out"
        self.fake_build = bench_shim.ShimBuild(self.root / "libwlpresent.so", "0" * 64, "0" * 64, "host")
        self.probes = []
        self.fixtures = self.root / "fixtures"
        self.fixtures.mkdir()
        (self.fixtures / "manifest.json").write_text('{"seed": 42}')

    def run_bench(self, metric, *, capture, owner="(false,)", has_gdbus=True, extra=(), fixtures=True):
        def probe(command, *_a, **_k):
            self.probes.append(command)
            return owner

        def which(tool):
            return None if tool == "gdbus" and not has_gdbus else f"/usr/bin/{tool}"
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        err = io.StringIO()
        with patch.object(bench.Originals, "capture", return_value=original), \
                patch.object(bench_shim, "build", return_value=self.fake_build), \
                patch.object(bench, "capture", side_effect=capture), \
                patch.object(bench, "make_dirs"), patch.object(bench, "materialize"), \
                patch.object(bench_session, "_session_bus_available", return_value=True), \
                patch.object(bench_session, "_probe", side_effect=probe), \
                patch.object(bench.shutil, "which", side_effect=which), \
                contextlib.redirect_stderr(err):
            code = bench.main([*PRESENT, "--metric", metric, "--binary", sys.executable,
                               "--output", str(self.output), "--tmp-root", str(self.root / "runs"),
                               *(["--fixtures", str(self.fixtures)] if fixtures else []), *extra])
        return code, err.getvalue()

    def ok_rows(self, _metric, args, _command, env, **_kwargs):
        self.assertEqual(args.scenarios["G4"], ["library", "console", "settings"])
        self.assertIn("LD_PRELOAD", env)
        self.assertIn("CROSSHOOK_BENCH_ROOT", env)
        return [{"scenario": name, "p50_ms": 8.3, "frames": 10, "status": "ok"} for name in args.scenarios["G4"]]

    def test_g4_launches_without_capture_csv_and_writes_scenario_rows(self):
        code, _ = self.run_bench("G4", capture=self.ok_rows)
        self.assertEqual(code, 0)
        with (self.output / "A/E1/G4.csv").open() as stream:
            reader = csv.DictReader(stream)
            rows = list(reader)
        self.assertEqual(",".join(reader.fieldnames), COLUMNS)
        self.assertEqual([row["scenario"] for row in rows], ["library", "console", "settings"])
        self.assertEqual(rows[0]["p95_ms"], "")
        self.assertEqual(json.loads((self.output / "A/E1/meta.json").read_text())["status"], "captured")

    def test_rows_without_scenario_or_frames_still_write(self):
        code, _ = self.run_bench("G9", capture=lambda *a, **k: [{"p50_ms": 8.3, "status": "ok"}])
        self.assertEqual(code, 0)
        with (self.output / "A/E1/G9.csv").open() as stream:
            (row,) = csv.DictReader(stream)
        self.assertEqual((row["scenario"], row["frames"], row["status"]), ("", "", "ok"))

    def g5_rows(self, _metric, args, _command, env, **kwargs):
        self.assertEqual(args.scenarios["G5"], ["search", "toggle_favorite", "toggle_optimization",
                                                "dpad", "route_switch"])
        self.assertEqual(args.trials, 20)
        self.assertIn("LD_PRELOAD", env)  # a driven run launches the app, it is not external-only
        observations = Path(env["CROSSHOOK_BENCH_ROOT"]) / "observations"
        observations.mkdir()
        (observations / "G5-search.csv").write_text("seq,inject_ns\n0,1\n")
        kwargs["evidence"]["G5"] = {"search": [{"iteration": 1, "injected": 20}]}
        return [{"scenario": name, "p50_ms": 40.0, "p95_ms": 50.0, "p99_ms": 55.0, "measured": 20,
                 "ambiguous": 0, "timeout": 0, "early": 0, "status": "captured"} for name in args.scenarios["G5"]]

    def test_g5_launches_without_capture_csv_and_seals_provenance_and_observations(self):
        code, _ = self.run_bench("G5", capture=self.g5_rows)
        self.assertEqual(code, 0)
        with (self.output / "A/E1/G5.csv").open() as stream:
            reader = csv.DictReader(stream)
            rows = list(reader)
        self.assertEqual(",".join(reader.fieldnames), bench.SCHEMAS["G5"])
        self.assertEqual([row["scenario"] for row in rows],
                         ["search", "toggle_favorite", "toggle_optimization", "dpad", "route_switch"])
        self.assertEqual(rows[0]["measured"], "20")
        (raw,) = (self.output / "raw").iterdir()
        manifest = json.loads((raw / "provenance.json").read_text())
        self.assertEqual((manifest["status"], manifest["drivers"], manifest["trials"]), ("captured", "uinput+kwin/1", 20))
        self.assertEqual(manifest["scenarios"]["G5"][0], "search")
        self.assertEqual(manifest["drive_stats"]["G5"]["search"][0]["injected"], 20)
        self.assertTrue((raw / "observations/G5-search.csv").is_file())
        sums = (raw / "SHA256SUMS").read_text()
        self.assertIn("observations/G5-search.csv", sums)
        self.assertIn("G5.csv", sums)

    def test_a_partial_g5_run_exits_zero_is_marked_partial_and_is_refused_by_capture_csv(self):
        def partial_rows(_metric, args, _command, _env, **_kwargs):
            return [{"scenario": name, "p50_ms": 40.0, "p95_ms": 50.0, "p99_ms": 55.0, "measured": 17,
                     "ambiguous": 1, "timeout": 2, "early": 0, "status": "partial"}
                    for name in args.scenarios["G5"]]
        code, _ = self.run_bench("G5", capture=partial_rows)
        self.assertEqual(code, 0)
        self.assertEqual(json.loads((self.output / "A/E1/meta.json").read_text())["status"], "partial")
        (raw,) = (self.output / "raw").iterdir()
        self.assertEqual(json.loads((raw / "provenance.json").read_text())["status"], "partial")
        with (self.output / "A/E1/G5.csv").open() as stream:
            self.assertEqual({row["status"] for row in csv.DictReader(stream)}, {"partial"})
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: run status is not captured"):
            verify_capture(raw / "G5.csv", {})  # the sealed copy a later run would ingest

    def test_g5_scenario_and_trials_flags_reach_the_collector(self):
        seen = {}

        def capture(_metric, args, _command, _env, **_kwargs):
            seen.update(scenarios=args.scenarios, trials=args.trials, iterations=args.iterations)
            return [{"scenario": "dpad", "status": "captured"}]
        code, _ = self.run_bench("G5", capture=capture, extra=("--scenario", "dpad", "--trials", "40",
                                                             "--iterations", "2"))
        self.assertEqual(code, 0)
        self.assertEqual(seen, {"scenarios": {"G5": ["dpad"]}, "trials": 40, "iterations": 2})

    def test_g5_marker_failures_exit_one(self):
        for code in ("MARK_TIMEOUT", "CLOCK_SYNC_FAILED", "BENCH_EVENT_MALFORMED", "NOT_QUIET", "STATE_UNSAFE"):
            with self.subTest(code=code):
                result, err = self.run_bench("G5", capture=CaptureError(f"{code}: x"))
                self.assertEqual(result, 1)
                self.assertIn(f"{code}: x", err)

    def test_g5_dry_run_writes_the_schema_row(self):
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        with patch.object(bench.Originals, "capture", return_value=original):
            code = bench.main([*PRESENT, "--metric", "G5", "--dry-run", "--output", str(self.output),
                               "--tmp-root", str(self.root / "runs")])
        self.assertEqual(code, 0)
        with (self.output / "A/E1/G5.csv").open() as stream:
            reader = csv.DictReader(stream)
            (row,) = reader
        self.assertEqual(",".join(reader.fieldnames), bench.SCHEMAS["G5"])
        self.assertEqual((row["scenario"], row["status"]), ("", "dry-run"))

    def test_g5_without_gdbus_is_refused_before_any_write(self):
        code, err = self.run_bench("G5", capture=AssertionError("launched"), has_gdbus=False)
        self.assertEqual(code, 30)
        self.assertIn("MISSING_TOOL: gdbus", err)
        self.assertFalse(self.output.exists())

    def test_g8_stays_refused_with_the_followup_issue(self):
        err = io.StringIO()
        with contextlib.redirect_stderr(err), self.assertRaises(SystemExit):
            bench.arguments([*PRESENT, "--metric", "G8"])
        self.assertIn("G8 producer arrives with YAN-2056", err.getvalue())

    def test_dry_run_rows_leave_scenario_blank(self):
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        with patch.object(bench.Originals, "capture", return_value=original), \
                patch.object(bench_session, "_probe", side_effect=AssertionError("probed during dry run")):
            code = bench.main([*PRESENT, "--metric", "G4", "--dry-run", "--output", str(self.output),
                               "--tmp-root", str(self.root / "runs")])
        self.assertEqual(code, 0)
        with (self.output / "A/E1/G4.csv").open() as stream:
            (row,) = csv.DictReader(stream)
        self.assertEqual((row["scenario"], row["frames"], row["status"]), ("", "", "dry-run"))

    def test_exit_forty_prefixes(self):
        for prefix in ("NO_PRESENTATION_GLOBAL", "PRESENTATION_CLOCK", "REFRESH_MISMATCH",
                       "UNSUPPORTED_COMPOSITOR", "INPUT_DEVICE_DENIED"):
            with self.subTest(prefix=prefix):
                code, err = self.run_bench("G4", capture=CaptureError(f"{prefix}: x"))
                self.assertEqual(code, 40)
                self.assertIn(f"{prefix}: x", err)

    def test_driver_failures_exit_one(self):
        for code in ("FOCUS_LOST", "POINTER_NOT_OVER_WINDOW", "RESIZE_NOT_OBSERVED", "INPUT_NOT_DELIVERED",
                     "REPLAY_NOT_SETTLED", "APP_EXIT"):
            with self.subTest(code=code):
                result, err = self.run_bench("G4", capture=CaptureError(f"{code}: x"))
                self.assertEqual(result, 1)
                self.assertIn(f"{code}: x", err)

    def test_other_failures_keep_their_codes(self):
        for message, expected in (("NEEDS_COLD_CACHE_PREP: x", 20), ("MISSING_TOOL: ydotool", 30), ("NO_READY: x", 1)):
            with self.subTest(message=message):
                self.assertEqual(self.run_bench("G4", capture=CaptureError(message))[0], expected)

    def test_missing_gdbus_is_refused_before_any_write(self):
        code, err = self.run_bench("G4", capture=AssertionError("launched"), has_gdbus=False)
        self.assertEqual(code, 30)
        self.assertIn("MISSING_TOOL: gdbus", err)
        self.assertFalse(self.output.exists())
        self.assertEqual(self.probes, [])

    def test_gdbus_is_not_required_for_g2(self):
        code, _ = self.run_bench("G2", capture=lambda *a, **k: [{"iteration": "1", "status": "ok"}],
                                 has_gdbus=False, extra=("--cold-cache-ack",))
        self.assertEqual(code, 0)

    def test_running_instance_is_refused_before_launch(self):
        for metric in ("G4", "G9"):
            with self.subTest(metric=metric):
                code, err = self.run_bench(metric, capture=AssertionError("launched"), owner="(true,)")
                self.assertEqual(code, 10)
                self.assertIn("BENCH_REFUSE: INSTANCE_RUNNING: quit CrossHook before driving input", err)
                self.assertEqual(self.probes[-1], [*bench_session.NAME_HAS_OWNER, bench_guard.INSTANCE_BUS_NAME])
                self.assertEqual(self.probes[-1][-1], "dev.crosshook.CrossHook")
                self.assertFalse(self.output.exists())

    def test_free_instance_name_does_not_refuse(self):
        self.assertEqual(self.run_bench("G9", capture=lambda *a, **k: [{"status": "ok"}], owner="(false,)")[0], 0)

    def test_unknown_instance_state_is_refused_for_driven_runs(self):
        for metric in ("G4", "G9"):
            with self.subTest(metric=metric):
                code, err = self.run_bench(metric, capture=AssertionError("launched"), owner=None)
                self.assertEqual(code, 10)
                self.assertIn("BENCH_REFUSE: INSTANCE_UNKNOWN: could not ask the session bus whether "
                              "CrossHook is running; ", err)
                self.assertFalse(self.output.exists())

    def test_no_session_bus_is_unknown_and_does_not_probe(self):
        with patch.object(bench_session, "_session_bus_available", return_value=False), \
                patch.object(bench_session, "_probe", side_effect=AssertionError("probed without a bus")), \
                self.assertRaisesRegex(GuardError, "^INSTANCE_UNKNOWN: "):
            bench_guard.require_no_instance()

    def test_instance_that_appears_during_the_capture_exits_ten_and_seals(self):
        def capture(*_a, **_k):
            raise GuardError("INSTANCE_RUNNING: quit CrossHook before driving input; ...")
        for metric in ("G4", "G9"):
            with self.subTest(metric=metric):
                self.output = self.root / f"out-{metric}"
                code, err = self.run_bench(metric, capture=capture)
                self.assertEqual(code, 10)
                self.assertIn("BENCH_REFUSE: INSTANCE_RUNNING", err)
                (raw,) = (self.output / "raw").iterdir()
                self.assertTrue((raw / "SHA256SUMS").is_file())
                self.assertEqual(json.loads((raw / "provenance.json").read_text())["status"], "failed")

    def test_a_mid_run_guard_refusal_is_recorded_in_meta_and_provenance(self):
        message = "INSTANCE_RUNNING: quit CrossHook before driving input; ..."

        def capture(*_a, **_k):
            raise GuardError(message)
        code, _ = self.run_bench("G4", capture=capture)
        self.assertEqual(code, 10)
        meta = json.loads((self.output / "A/E1/meta.json").read_text())
        self.assertEqual((meta["status"], meta["error"]), ("failed", message))
        (raw,) = (self.output / "raw").iterdir()
        manifest = json.loads((raw / "provenance.json").read_text())
        self.assertEqual((manifest["status"], manifest["error"]), ("failed", message))

    def test_g4_console_needs_fixtures_before_any_launch(self):
        for scenarios in (None, "console", "library,console"):
            with self.subTest(scenarios=scenarios):
                code, err = self.run_bench("G4", capture=AssertionError("launched"), fixtures=False,
                                           extra=("--scenario", scenarios) if scenarios else ())
                self.assertEqual(code, 1)
                self.assertIn("FIXTURES_REQUIRED: ", err)
                self.assertFalse(self.output.exists())

    def test_g4_without_console_and_g9_do_not_need_fixtures(self):
        for metric, extra in (("G4", ("--scenario", "library,settings")), ("G9", ())):
            with self.subTest(metric=metric):
                self.output = self.root / f"out-{metric}"
                code, _ = self.run_bench(metric, capture=lambda *a, **k: [{"status": "ok"}], fixtures=False,
                                         extra=extra)
                self.assertEqual(code, 0)

    def test_driven_capture_receives_the_provenance_evidence_dict(self):
        def capture(metric, _args, _command, _env, *, command_for, evidence):
            evidence.setdefault(metric, {})["resize"] = [{"iteration": 1, "injected": 150, "driven_s": 5.0}]
            return [{"status": "ok"}]
        code, _ = self.run_bench("G9", capture=capture)
        self.assertEqual(code, 0)
        (raw,) = (self.output / "raw").iterdir()
        manifest = json.loads((raw / "provenance.json").read_text())
        self.assertEqual(manifest["drive_stats"],
                         {"G9": {"resize": [{"iteration": 1, "injected": 150, "driven_s": 5.0}]}})

    def test_instance_probe_is_skipped_for_other_presentation_metrics(self):
        self.run_bench("G2", capture=lambda *a, **k: [{"iteration": "1", "status": "ok"}],
                       owner="(true,)", extra=("--cold-cache-ack",))
        self.assertEqual(self.probes, [])

    def test_a_failing_seal_still_releases_the_lock_the_root_and_the_signal_handlers(self):
        numbers = (signal.SIGTERM, signal.SIGHUP, signal.SIGINT)
        before = {n: signal.getsignal(n) for n in numbers}
        real_write_text = Path.write_text
        meta_writes = []

        def write_text(path, *args, **kwargs):
            if path.name == "meta.json":
                meta_writes.append(path)
                if len(meta_writes) == 2:  # the seal in `finally`, not the initial write
                    raise OSError("disk full")
            return real_write_text(path, *args, **kwargs)
        with patch.object(Path, "write_text", write_text), self.assertRaisesRegex(OSError, "disk full"):
            self.run_bench("G4", capture=self.ok_rows)
        self.assertEqual(len(meta_writes), 2)
        self.assertEqual({n: signal.getsignal(n) for n in numbers}, before)
        os.close(bench.lock_output(self.output / "A/E1"))  # raises OUTPUT_BUSY while the lock is still held
        self.assertEqual(list((self.root / "runs").iterdir()), [])  # the isolated root was removed

    def test_termination_signals_abort_with_sealed_raw_dir(self):
        for number in (signal.SIGTERM, signal.SIGHUP):
            with self.subTest(signal=number.name):
                self.output = self.root / f"out-{number.name}"
                before = {n: signal.getsignal(n) for n in (signal.SIGTERM, signal.SIGHUP)}
                code, err = self.run_bench("G4", capture=lambda *a, **k: os.kill(os.getpid(), number))
                self.assertEqual(code, 130)
                self.assertIn(f"ABORTED: {number.name}", err)
                self.assertEqual({n: signal.getsignal(n) for n in before}, before)
                meta = json.loads((self.output / "A/E1/meta.json").read_text())
                self.assertEqual((meta["status"], meta["error"]), ("aborted", f"ABORTED: {number.name}"))
                (raw,) = (self.output / "raw").iterdir()
                manifest = json.loads((raw / "provenance.json").read_text())
                self.assertEqual((manifest["status"], manifest["error"]), ("aborted", f"ABORTED: {number.name}"))
                self.assertTrue((raw / "SHA256SUMS").is_file())
                self.assertFalse(list((self.root / "runs").glob("crosshook-bench-*")))

    def test_second_signal_during_the_finally_does_not_break_sealing(self):
        real_finalize = bench.finalize_raw

        def finalize_then_signal_again(*args, **kwargs):
            os.kill(os.getpid(), signal.SIGTERM)  # a second signal while the run is being sealed
            os.kill(os.getpid(), signal.SIGINT)
            return real_finalize(*args, **kwargs)
        numbers = (signal.SIGTERM, signal.SIGHUP, signal.SIGINT)
        before = {n: signal.getsignal(n) for n in numbers}
        with patch.object(bench, "finalize_raw", finalize_then_signal_again):
            code, err = self.run_bench("G4", capture=lambda *a, **k: os.kill(os.getpid(), signal.SIGHUP))
        self.assertEqual(code, 130)
        self.assertIn("ABORTED: SIGHUP", err)
        self.assertNotIn("RAW_FINALIZE_FAILED", err)
        self.assertEqual({n: signal.getsignal(n) for n in numbers}, before)
        (raw,) = (self.output / "raw").iterdir()
        manifest = json.loads((raw / "provenance.json").read_text())
        self.assertEqual((manifest["status"], manifest["error"]), ("aborted", "ABORTED: SIGHUP"))
        self.assertTrue((raw / "SHA256SUMS").is_file())
        self.assertFalse(list((self.root / "runs").glob("crosshook-bench-*")))

    def test_keyboard_interrupt_aborts_with_sigint_name(self):
        code, err = self.run_bench("G4", capture=KeyboardInterrupt())
        self.assertEqual(code, 130)
        self.assertIn("ABORTED: SIGINT", err)
        self.assertEqual(json.loads((self.output / "A/E1/meta.json").read_text())["status"], "aborted")
        (raw,) = (self.output / "raw").iterdir()
        self.assertTrue((raw / "SHA256SUMS").is_file())


if __name__ == "__main__":
    unittest.main()
