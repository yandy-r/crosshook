"""G5 collection: launch env, per-iteration state restore, observations and rows; fake app and driver."""
import contextlib
import csv
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from typing import ClassVar
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_capture
import bench_cli
import bench_collect
import bench_drivers
import bench_drivers_g5
import bench_events
import bench_g5io
import bench_guard
from bench_capture import CaptureError
from bench_collect import driven_rows
from bench_drivers_g5 import ClockOffset, G5Result, TrialRecord
from bench_g5select import AMBIGUOUS, EARLY, MEASURED, TIMEOUT, Trial

PERIOD_NS = 8_333_333
BASE_NS = 1_000_000_000_000


class FakeDevice:
    instances: ClassVar[list["FakeDevice"]] = []

    def __init__(self):
        self.closed = False
        FakeDevice.instances.append(self)

    def __enter__(self):
        return self

    def __exit__(self, *_exc):
        self.closed = True


class FakeKWin:
    instances: ClassVar[list["FakeKWin"]] = []

    def __init__(self, tools_dir, run_tag):
        self.tools_dir, self.run_tag = tools_dir, run_tag
        FakeKWin.instances.append(self)


class FakeEvents:
    def __init__(self):
        self.closed = False

    def close(self):
        self.closed = True


class FakeReady:
    """Stands in for bench_capture.ready: writes a present log and hands out a proc with events."""

    def __init__(self):
        self.calls: list[tuple[list[str], dict[str, str], bool, bool]] = []
        self.procs: list[SimpleNamespace] = []
        self.refresh_ns = PERIOD_NS
        self.error: BaseException | None = None

    def __call__(self, command, env, timeout, exit_after, events=False):
        self.calls.append((list(command), dict(env), exit_after, events))
        if self.error is not None:
            raise self.error
        flag = "--env=CROSSHOOK_BENCH_PRESENT_DIR="
        present = Path(env.get("CROSSHOOK_BENCH_PRESENT_DIR") or
                       next(arg[len(flag):] for arg in command if arg.startswith(flag)))
        (present / "wlpresent-1.log").write_text(
            f"H\tclock_id=1\nS\t1\t7\ttoplevel\nC\t{BASE_NS}\t7\t1\t1\n"
            f"P\t{BASE_NS + 1000}\t1\t{BASE_NS + 1000}\t{self.refresh_ns}\t1\t0\n")
        proc = SimpleNamespace(launch_ns=BASE_NS, events=FakeEvents(), stderr=SimpleNamespace(
            close=lambda: setattr(proc, "stderr_closed", True)), stderr_closed=False)
        self.procs.append(proc)
        return proc, 10.0


def trial_records(statuses: list[str], base_ns: int = 10_000) -> list[TrialRecord]:
    records = []
    for index, status in enumerate(statuses):
        inject = base_ns + index * 1_000_000
        latency = 40.0 + index if status == MEASURED else None
        trial = Trial(status, inject + 20_000_000 if latency else None,
                      inject + int(latency * 1e6) if latency else None, latency)
        records.append(TrialRecord(index, inject, inject + 5_000_000, 1_500_000, trial))
    return records


class G5CollectTestCase(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve() / "crosshook-bench-x"
        for name in ("home", "bench-tools"):
            (self.root / name).mkdir(parents=True)
        self.profiles = self.root / "config/crosshook/profiles"
        self.profiles.mkdir(parents=True)
        (self.profiles / "game.toml").write_text("optimizations = []\nfavorite = false\n")
        self.data = self.root / "data/crosshook"
        self.data.mkdir(parents=True)
        (self.data / "metadata.db").write_bytes(b"fixture-db")
        self.env = {"HOME": str(self.root / "home"), "XDG_CONFIG_HOME": str(self.root / "config"),
                    "XDG_DATA_HOME": str(self.root / "data"), "CROSSHOOK_BENCH_ROOT": str(self.root),
                    "LD_PRELOAD": "/shim.so", "PATH": "/usr/bin", "CROSSHOOK_BENCH_LOG": "/x",
                    "CROSSHOOK_BENCH_LOG_RATE": "5"}
        self.args = SimpleNamespace(iterations=2, timeout=60, refresh_hz=120.0, flatpak=False, trials=10,
                                    branch="master", binary=None,
                                    scenarios={"G5": ["search", "toggle_favorite"]})
        self.fake = FakeReady()
        self.stopped: list[object] = []
        self.drives: list[dict] = []
        self.statuses = [MEASURED] * 10
        self.mutate_state = False
        self.drive_error: BaseException | None = None
        self.instance_checks: list[int] = []
        FakeDevice.instances, FakeKWin.instances = [], []
        self.settled: list[FakeDevice] = []
        self.profile_seen: list[str] = []  # the profile file as each iteration starts

        def drive(proc, scenario, present_dir, kwin, dev, events, trials, mark_timeout):
            self.drives.append({"scenario": scenario, "dir": present_dir, "kwin": kwin, "dev": dev,
                                "events": events, "trials": trials, "mark_timeout": mark_timeout})
            self.profile_seen.append((self.profiles / "game.toml").read_text())
            if self.mutate_state:
                (self.profiles / "game.toml").write_text("optimizations = ['x']\nfavorite = true\n")
                (self.data / "metadata.db").write_bytes(b"mutated")
            if self.drive_error is not None:
                raise self.drive_error
            return G5Result(trial_records(self.statuses), ClockOffset(0, 1_500_000))

        for target, replacement in (
                ("ready", self.fake), ("stop", self.stopped.append), ("InputDevice", FakeDevice),
                ("KWinScripts", FakeKWin), ("drive_g5", drive), ("settle_device", self.settled.append),
                ("require_kwin", lambda: None),
                ("require_no_instance", lambda: self.instance_checks.append(len(self.fake.calls)))):
            patcher = patch.object(bench_collect, target, replacement)
            patcher.start()
            self.addCleanup(patcher.stop)

    def rows(self, command_for=None, evidence=None):
        return driven_rows(self.args, "G5", command_for or (lambda env: ["app"]), self.env, evidence)

    def observations(self, scenario):
        path = self.root / f"observations/G5-{scenario}.csv"
        return list(csv.DictReader(path.read_text().splitlines()))


class G5RowTests(G5CollectTestCase):
    def test_one_row_per_scenario_with_the_pooled_trials_of_every_iteration(self):
        rows = self.rows()
        self.assertEqual([row["scenario"] for row in rows], ["search", "toggle_favorite"])
        for row in rows:
            self.assertEqual((row["measured"], row["ambiguous"], row["status"]), (20, 0, "captured"))
            self.assertEqual(sorted(row), sorted(bench.SCHEMAS["G5"].split(",")))
            self.assertLess(row["p50_ms"], row["p95_ms"])
        self.assertEqual(len(self.fake.calls), 4)

    def test_too_many_ambiguous_trials_make_the_row_partial(self):
        self.statuses = [MEASURED] * 8 + [AMBIGUOUS] * 2
        self.assertEqual({row["status"] for row in self.rows()}, {"partial"})

    def test_timeouts_and_early_trials_make_the_row_partial_and_are_counted(self):
        self.statuses = [MEASURED] * 7 + [AMBIGUOUS, TIMEOUT, EARLY]  # 3 of 10 are not measured
        for row in self.rows():
            self.assertEqual((row["measured"], row["ambiguous"], row["timeout"], row["early"], row["status"]),
                             (14, 2, 2, 2, "partial"))
            self.assertEqual(sorted(row), sorted(bench.SCHEMAS["G5"].split(",")))

    def test_too_few_measured_trials_fail_the_scenario(self):
        self.args.iterations, self.statuses = 1, [MEASURED] * 9 + [TIMEOUT]
        with self.assertRaisesRegex(CaptureError, "^CAPTURE_TOO_SHORT: search: 9 measured trials, need 10"):
            self.rows()

    def test_an_odd_trial_count_is_refused_before_any_launch(self):
        self.args.trials = 11
        with self.assertRaisesRegex(CaptureError, "^INVALID_TRIALS: 11 is odd"):
            self.rows()
        self.assertEqual(self.fake.calls, [])

    def test_evidence_records_each_iteration(self):
        evidence: dict = {}
        self.rows(evidence=evidence)
        entries = evidence["G5"]["search"]
        self.assertEqual([entry["iteration"] for entry in entries], [1, 2])
        self.assertEqual(entries[0]["injected"], 10)
        self.assertEqual(entries[0]["uncertainty_ms"], 1.5)
        self.assertEqual(entries[0]["driven_s"], 0.009)

    def test_refresh_is_checked(self):
        self.fake.refresh_ns = 16_666_666
        with self.assertRaisesRegex(CaptureError, "^REFRESH_MISMATCH: "):
            self.rows()

    def test_input_device_opened_once_settled_once_and_closed(self):
        self.rows()
        self.assertEqual(len(FakeDevice.instances), 1)
        self.assertEqual(self.settled, FakeDevice.instances)
        self.assertTrue(FakeDevice.instances[0].closed)
        self.assertTrue(all(call["dev"] is FakeDevice.instances[0] for call in self.drives))

    def test_trials_and_mark_timeout_reach_the_driver(self):
        self.rows()
        self.assertEqual({call["trials"] for call in self.drives}, {10})
        self.assertEqual({call["mark_timeout"] for call in self.drives}, {10.0})


class G5LaunchTests(G5CollectTestCase):
    def test_every_launch_sets_the_scenario_reads_events_and_drops_the_replay_env(self):
        self.rows()
        scenarios = [call[1]["CROSSHOOK_BENCH_SCENARIO"] for call in self.fake.calls]
        self.assertEqual(scenarios, ["search", "search", "toggle_favorite", "toggle_favorite"])
        for _command, env, exit_after, events in self.fake.calls:
            self.assertFalse(exit_after)
            self.assertTrue(events)
            self.assertNotIn("CROSSHOOK_BENCH_LOG", env)
            self.assertNotIn("CROSSHOOK_BENCH_LOG_RATE", env)
            self.assertEqual(env["LD_PRELOAD"], "/shim.so")
        self.assertNotIn("CROSSHOOK_BENCH_SCENARIO", self.env)  # the caller's env is not modified

    def test_fresh_dirs_and_kwin_scripts_per_iteration(self):
        self.rows()
        base = self.root / "present/G5"
        self.assertEqual([call["dir"] for call in self.drives],
                         [base / s / f"iter-{n}" for s in ("search", "toggle_favorite") for n in (1, 2)])
        self.assertEqual(len({kwin.run_tag for kwin in FakeKWin.instances}), 4)
        self.assertEqual({kwin.tools_dir for kwin in FakeKWin.instances}, {self.root / "bench-tools"})
        self.assertTrue(all(call["events"] is proc.events for call, proc in zip(self.drives, self.fake.procs)))

    def test_existing_iteration_dir_refused(self):
        (self.root / "present/G5/search/iter-1").mkdir(parents=True)
        with self.assertRaisesRegex(CaptureError, "^PRESENT_DIR_EXISTS: .*search/iter-1"):
            self.rows()
        self.assertEqual(self.fake.calls, [])

    def test_instance_is_rechecked_immediately_before_every_launch(self):
        self.rows()
        self.assertEqual(self.instance_checks, [0, 1, 2, 3])

    def test_the_app_is_stopped_and_its_reader_and_pipe_closed_every_iteration(self):
        self.rows()
        self.assertEqual(self.stopped, self.fake.procs)
        self.assertTrue(all(proc.events.closed and proc.stderr_closed for proc in self.fake.procs))

    def test_flatpak_forwards_the_scenario_only_through_env_flags(self):
        flatpak_root = self.root / "home/.var/app/dev.crosshook.CrossHook"
        (flatpak_root / "config/crosshook/profiles").mkdir(parents=True)
        (flatpak_root / "data/crosshook").mkdir(parents=True)
        self.env["CROSSHOOK_BENCH_ROOT"] = str(flatpak_root)
        self.args.flatpak, self.args.timeout, self.args.scenarios = True, 5, {"G5": ["search"]}
        launch = SimpleNamespace(flatpak=True, branch="master", binary=None)
        self.rows(command_for=lambda env: bench.launch_command(launch, env))
        command, env, _exit, _events = self.fake.calls[0]
        self.assertIn("--env=CROSSHOOK_BENCH_SCENARIO=search", command)
        self.assertIn("--env=LD_PRELOAD=/shim.so", command)
        self.assertNotIn("CROSSHOOK_BENCH_SCENARIO", env)
        self.assertNotIn("LD_PRELOAD", env)
        self.assertEqual({call["mark_timeout"] for call in self.drives}, {20.0})  # 4x the native base


class G5ObservationTests(G5CollectTestCase):
    def test_one_pooled_observation_csv_per_scenario(self):
        self.statuses = [MEASURED] * 9 + [TIMEOUT]
        self.rows()
        for scenario in ("search", "toggle_favorite"):
            rows = self.observations(scenario)
            self.assertEqual([row["seq"] for row in rows], [str(n) for n in range(20)])
            self.assertEqual(rows[9]["status"], "timeout")
            self.assertEqual(rows[9]["latency_ms"], "")
            self.assertEqual(rows[0]["latency_ms"], "40.000")
            self.assertEqual(rows[0]["uncertainty_ms"], "1.500")
        self.assertEqual(oct((self.root / "observations/G5-search.csv").stat().st_mode & 0o777), "0o600")

    def test_evidence_survives_a_later_iteration_failing(self):
        def failing_second(proc, scenario, present_dir, kwin, dev, events, trials, mark_timeout):
            if len(self.fake.calls) == 2:
                raise CaptureError("MARK_TIMEOUT: search seq 3 render not seen within 10 s")
            return G5Result(trial_records([MEASURED] * 10), ClockOffset(0, 1_500_000))
        with patch.object(bench_collect, "drive_g5", failing_second), \
                self.assertRaisesRegex(CaptureError, "^MARK_TIMEOUT: "):
            self.rows()
        self.assertEqual(len(self.observations("search")), 10)  # the first iteration's trials are kept


class G5StateTests(G5CollectTestCase):
    FIXTURE = "optimizations = []\nfavorite = false\n"

    def test_toggle_scenarios_restore_the_fixture_state_after_every_iteration(self):
        self.args.scenarios = {"G5": ["toggle_favorite", "toggle_optimization"]}
        self.mutate_state = True
        self.rows()
        self.assertEqual(self.profile_seen, [self.FIXTURE] * 4)  # every iteration of both scenarios
        self.assertEqual((self.profiles / "game.toml").read_text(), self.FIXTURE)
        self.assertEqual((self.data / "metadata.db").read_bytes(), b"fixture-db")
        self.assertFalse((self.root / "g5-state").exists())

    def test_non_toggle_scenarios_take_no_snapshot(self):
        self.args.scenarios = {"G5": ["search"]}
        self.mutate_state = True
        self.rows()
        self.assertNotEqual((self.profiles / "game.toml").read_text(), self.FIXTURE)

    def test_state_is_restored_when_the_driver_fails(self):
        self.args.scenarios = {"G5": ["toggle_optimization"]}
        self.mutate_state = True
        self.drive_error = CaptureError("MARK_TIMEOUT: toggle_optimization seq 0 armed not seen within 10 s")
        with self.assertRaisesRegex(CaptureError, "^MARK_TIMEOUT: "):
            self.rows()
        self.assertEqual((self.profiles / "game.toml").read_text(), self.FIXTURE)
        self.assertEqual((self.data / "metadata.db").read_bytes(), b"fixture-db")
        self.assertFalse((self.root / "g5-state").exists())
        self.assertEqual(len(self.stopped), 1)  # the app was stopped before the restore

    def test_state_is_restored_on_an_abort_signal(self):
        self.args.scenarios = {"G5": ["toggle_favorite"]}
        self.mutate_state = True
        self.drive_error = bench.BenchAborted("SIGTERM")
        with self.assertRaises(bench.BenchAborted):
            self.rows()
        self.assertEqual((self.profiles / "game.toml").read_text(), self.FIXTURE)
        self.assertEqual((self.data / "metadata.db").read_bytes(), b"fixture-db")
        self.assertTrue(FakeDevice.instances[0].closed)

    def test_state_is_restored_when_the_app_never_reaches_ready(self):
        self.args.scenarios = {"G5": ["toggle_favorite"]}
        (self.profiles / "game.toml").write_text("changed before ready\n")
        self.fake.error = CaptureError("READY_TIMEOUT")
        # The snapshot was taken after the change, so that is the state to return to.
        with self.assertRaisesRegex(CaptureError, "^READY_TIMEOUT"):
            self.rows()
        self.assertEqual((self.profiles / "game.toml").read_text(), "changed before ready\n")
        self.assertFalse((self.root / "g5-state").exists())

    def test_state_is_restored_when_a_stale_instance_is_found_before_launch(self):
        self.args.scenarios = {"G5": ["toggle_favorite"]}

        def refuse():
            raise CaptureError("INSTANCE_RUNNING: another CrossHook")
        with patch.object(bench_collect, "require_no_instance", refuse), \
                self.assertRaisesRegex(CaptureError, "^INSTANCE_RUNNING"):
            self.rows()
        self.assertEqual((self.profiles / "game.toml").read_text(), self.FIXTURE)
        self.assertFalse((self.root / "g5-state").exists())

    def test_a_snapshot_directory_that_already_exists_is_refused_and_left_untouched(self):
        self.args.scenarios = {"G5": ["toggle_favorite"]}
        store = self.root / "g5-state"
        (store / "0").mkdir(parents=True)
        (store / "0/keep.toml").write_text("someone else's snapshot\n")
        with self.assertRaisesRegex(CaptureError, "^STATE_UNSAFE: g5-state already exists"):
            self.rows()
        self.assertEqual((store / "0/keep.toml").read_text(), "someone else's snapshot\n")
        self.assertEqual(self.fake.calls, [])
        self.assertEqual((self.profiles / "game.toml").read_text(), self.FIXTURE)

    def test_a_snapshot_this_run_made_is_still_discarded_when_capture_fails_midway(self):
        self.args.scenarios = {"G5": ["toggle_favorite"]}
        (self.profiles / "bad.toml").symlink_to(self.profiles / "game.toml")  # refused by the copy
        with self.assertRaisesRegex(CaptureError, "^STATE_UNSAFE: bad.toml is not a regular file"):
            self.rows()
        self.assertFalse((self.root / "g5-state").exists())
        self.assertEqual(self.fake.calls, [])

    def test_the_restore_runs_with_abort_signals_deferred(self):
        self.args.scenarios = {"G5": ["toggle_favorite"]}
        self.args.iterations = 1
        self.mutate_state = True
        events: list[str] = []

        @contextlib.contextmanager
        def deferred():
            events.append("defer")
            try:
                yield
            finally:
                events.append("deliver")

        original = bench_collect.ToggleState.restore

        def restore(state):
            events.append("restore")
            original(state)
        with patch.object(bench_collect, "_signals_deferred", deferred), \
                patch.object(bench_collect.ToggleState, "restore", restore):
            self.rows()
        self.assertEqual(events, ["defer", "restore", "deliver"])
        self.assertEqual((self.profiles / "game.toml").read_text(), self.FIXTURE)

    def test_a_state_path_outside_the_bench_root_fails_before_any_launch(self):
        self.args.scenarios = {"G5": ["toggle_favorite"]}
        outside = Path(self.temp.name) / "elsewhere"
        outside.mkdir(parents=True)
        (self.data / "metadata.db").unlink()
        self.data.rmdir()
        self.data.symlink_to(outside)
        with self.assertRaisesRegex(CaptureError, "^STATE_UNSAFE: metadata.db resolves outside"):
            self.rows()
        self.assertEqual(self.fake.calls, [])


class G5ConsistencyTests(unittest.TestCase):
    def test_the_scenario_allowlists_cannot_drift_apart(self):
        names = set(bench_cli.SCENARIOS["G5"] + bench_cli.SUPPLEMENTAL_SCENARIOS["G5"])
        self.assertEqual(set(bench_events.G5_SCENARIOS), names)
        # The reader also accepts the one-shot probes' `replay_scroll` (G8) and `health_scroll` (G4).
        self.assertEqual(set(bench_events.SCENARIOS), names | {"replay_scroll", "health_scroll"})
        self.assertEqual(set(bench_drivers_g5.G5_KEYS), names)

    def test_the_scenario_env_var_has_one_definition(self):
        self.assertEqual(bench_guard.ENV_SCENARIO, "CROSSHOOK_BENCH_SCENARIO")
        self.assertIs(bench_collect.ENV_SCENARIO, bench_guard.ENV_SCENARIO)
        self.assertIs(bench.ENV_SCENARIO, bench_guard.ENV_SCENARIO)
        self.assertIn(bench_guard.ENV_SCENARIO, bench_collect.FLATPAK_ONLY_ENV)

    def test_state_paths_use_the_shared_app_dir_layout(self):
        env = {"HOME": "/h", "XDG_CONFIG_HOME": "/c", "XDG_DATA_HOME": "/d"}
        for flatpak in (False, True):
            with self.subTest(flatpak=flatpak):
                config = bench_drivers.app_dir(env, flatpak, "config")
                data = bench_drivers.app_dir(env, flatpak, "data")
                self.assertEqual(bench_g5io._state_paths(env, flatpak),
                                 [config / "profiles", *(data / name for name in bench_g5io.DB_FILES)])
                self.assertEqual(bench_drivers._settings_path(env, flatpak), config / "settings.toml")
        self.assertEqual(bench_drivers.app_dir(env, False, "config"), Path("/c/crosshook"))
        self.assertEqual(bench_drivers.app_dir(env, True, "data"),
                         Path("/h/.var/app/dev.crosshook.CrossHook/data/crosshook"))


class G5GateTests(G5CollectTestCase):
    def test_unavailable_kwin_opens_no_device_and_launches_nothing(self):
        def refuse():
            raise CaptureError("UNSUPPORTED_COMPOSITOR: KWin is required")
        with patch.object(bench_collect, "require_kwin", refuse), \
                self.assertRaisesRegex(CaptureError, "^UNSUPPORTED_COMPOSITOR"):
            self.rows()
        self.assertEqual((FakeDevice.instances, self.fake.calls), ([], []))

    def test_unregistered_device_fails_before_any_launch_and_closes_it(self):
        def unregistered(_dev):
            raise CaptureError("INPUT_NOT_DELIVERED: KWin did not register the bench input device")
        with patch.object(bench_collect, "settle_device", unregistered), \
                self.assertRaisesRegex(CaptureError, "^INPUT_NOT_DELIVERED: KWin did not register"):
            self.rows()
        self.assertEqual(self.fake.calls, [])
        self.assertTrue(FakeDevice.instances[0].closed)


class G5IngestTests(unittest.TestCase):
    """The external `latency_ms` CSV path maps onto the new G5 schema."""

    def ingest(self, text: str) -> list[dict]:
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "latency.csv"
            path.write_text(text)
            args = SimpleNamespace(collect="csv", capture_csv=str(path), refresh_hz=120.0)
            return bench_capture.capture("G5", args, ["app"], {})

    def test_external_latencies_become_one_external_row_in_the_g5_schema(self):
        rows = self.ingest("scenario,latency_ms\nsearch,10\nsearch,20\ndpad,30\ndpad,40\n")
        (row,) = rows
        self.assertEqual(sorted(row), sorted(["scenario", "p50_ms", "p95_ms", "p99_ms", "measured", "status"]))
        self.assertLessEqual(set(row), set(bench.SCHEMAS["G5"].split(",")))
        self.assertEqual((row["scenario"], row["measured"], row["status"]), ("external", 4, "measured"))
        self.assertEqual(row["p50_ms"], 25.0)
        for column in ("ambiguous", "timeout", "early"):
            self.assertNotIn(column, row)  # an external CSV has no trial statuses; the columns stay empty

    def test_an_external_row_writes_with_the_g5_schema_and_leaves_the_status_counts_empty(self):
        import io
        (row,) = self.ingest("latency_ms\n10\n20\n")
        out = io.StringIO()
        writer = csv.DictWriter(out, fieldnames=bench.SCHEMAS["G5"].split(","), lineterminator="\n")
        writer.writeheader()
        writer.writerow(row)
        (written,) = csv.DictReader(out.getvalue().splitlines())
        self.assertEqual((written["measured"], written["ambiguous"], written["timeout"], written["early"]),
                         ("2", "", "", ""))

    def test_the_latency_column_is_still_required(self):
        with self.assertRaisesRegex(CaptureError, "^CAPTURE_SCHEMA: missing latency_ms"):
            self.ingest("scenario,frame_ms\nsearch,10\n")

    def test_negative_latencies_are_refused(self):
        with self.assertRaisesRegex(CaptureError, "^CAPTURE_SCHEMA: invalid latency_ms"):
            self.ingest("latency_ms\n-1\n")


if __name__ == "__main__":
    unittest.main()
