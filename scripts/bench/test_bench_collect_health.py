"""G4 `health` collection: events launch, the before_prime hook, Flatpak env forwarding and cleanup."""
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_collect
import bench_guard
from bench_capture import CaptureError
from bench_collect import driven_rows
from bench_drivers import DriveResult
from test_bench_collect import WINDOW, FakeDevice, FakeDrivenReady, FakeKWin


class FakeEvents:
    def __init__(self):
        self.closed = False

    def close(self):
        self.closed = True


class EventsReady(FakeDrivenReady):
    """Like FakeDrivenReady, with the `events` keyword and a proc that carries a reader."""

    def __init__(self):
        super().__init__()
        self.kwargs: list[dict] = []
        self.procs: list[SimpleNamespace] = []

    def __call__(self, command, env, timeout, exit_after, events=False):
        proc, ready_ms = super().__call__(command, env, timeout, exit_after)
        self.kwargs.append({"events": events})
        proc.events = FakeEvents() if events else None
        self.procs.append(proc)
        return proc, ready_ms


class HealthCollectTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "crosshook-bench-x"
        for name in ("home", "config"):
            (self.root / name).mkdir(parents=True)
        (self.root / "bench-tools").mkdir(mode=0o700)
        self.env = {"HOME": str(self.root / "home"), "XDG_CONFIG_HOME": str(self.root / "config"),
                    "CROSSHOOK_BENCH_ROOT": str(self.root), "LD_PRELOAD": "/shim.so", "PATH": "/usr/bin",
                    "CROSSHOOK_BENCH_LOG": "/x", "CROSSHOOK_BENCH_LOG_RATE": "5"}
        self.args = SimpleNamespace(iterations=2, timeout=60, refresh_hz=120.0, flatpak=False,
                                    branch="master", binary=None, scenarios={"G4": ["health"]})
        self.fake = EventsReady()
        self.stopped: list[object] = []
        self.drives: list[dict] = []
        self.awaited: list[tuple] = []
        self.hook_error: BaseException | None = None
        FakeDevice.instances, FakeKWin.instances = [], []

        def drive_g4(proc, scenario, log_dir, kwin, dev, before_prime=None):
            self.drives.append({"scenario": scenario, "log_dir": log_dir, "before_prime": before_prime})
            if before_prime is not None:
                before_prime()
            times = self.fake.times()
            return DriveResult(times[WINDOW[0]], times[WINDOW[1]], 90)

        def await_health_page(*call):
            self.awaited.append(call)
            if self.hook_error is not None:
                raise self.hook_error

        for target, replacement in (
                ("ready", self.fake), ("stop", self.stopped.append), ("InputDevice", FakeDevice),
                ("KWinScripts", FakeKWin), ("drive_g4", drive_g4), ("settle_device", lambda dev: None),
                ("require_kwin", lambda: None), ("require_no_instance", lambda: None),
                ("await_health_page", await_health_page)):
            patcher = patch.object(bench_collect, target, replacement)
            patcher.start()
            self.addCleanup(patcher.stop)

    def rows(self, command_for=None):
        return driven_rows(self.args, "G4", command_for or (lambda env: ["app"]), self.env)

    def test_health_launches_with_events_and_the_health_scroll_app_scenario(self):
        rows = self.rows()
        self.assertEqual([(row["scenario"], row["frames"], row["status"]) for row in rows],
                         [("health", 60, "captured")])
        (row,) = rows
        self.assertEqual(set(row), set(bench.SCHEMAS["G4"].split(",")))
        for column in ("p50_ms", "p95_ms", "p99_ms", "stddev_ms", "over_interval_percent"):
            self.assertIsInstance(row[column], float, column)
        self.assertEqual([call["events"] for call in self.fake.kwargs], [True, True])
        for _command, env, _exit in self.fake.calls:
            self.assertEqual(env[bench_guard.ENV_SCENARIO], "health_scroll")
            self.assertNotIn("CROSSHOOK_BENCH_LOG", env)  # the console replay never runs here
            self.assertNotIn("CROSSHOOK_BENCH_LOG_RATE", env)

    def test_the_before_prime_hook_waits_for_the_health_page_of_this_launch(self):
        self.rows()
        self.assertEqual(len(self.awaited), 2)
        for number, (proc, events, log_dir, mark_timeout, flatpak) in enumerate(self.awaited, 1):
            self.assertIs(proc, self.fake.procs[number - 1])
            self.assertIs(events, proc.events)
            self.assertEqual(log_dir, self.root / f"present/G4/health/iter-{number}")
            self.assertEqual((mark_timeout, flatpak), (10.0, False))  # max(10 s native, 60 s / 6)
        self.assertEqual([drive["log_dir"] for drive in self.drives],
                         [call[2] for call in self.awaited])

    def test_the_app_is_stopped_and_its_event_reader_closed(self):
        self.rows()
        self.assertEqual(self.stopped, self.fake.procs)
        for proc in self.fake.procs:
            self.assertTrue(proc.events.closed)
        self.assertTrue(self.fake.stderr_closed)

    def test_a_failing_health_page_stops_the_app_and_fails_the_capture(self):
        self.hook_error = CaptureError("HEALTH_ROWS_TOO_FEW: the live health table showed fewer than 300 rows")
        with self.assertRaisesRegex(CaptureError, "^HEALTH_ROWS_TOO_FEW: "):
            self.rows()
        self.assertEqual(len(self.fake.calls), 1)  # no later iteration starts
        self.assertEqual(self.stopped, self.fake.procs)
        self.assertTrue(self.fake.procs[0].events.closed)

    def test_other_g4_scenarios_launch_as_before_without_events_or_a_hook(self):
        self.args.scenarios = {"G4": ["library"]}
        self.rows()
        self.assertEqual([call["events"] for call in self.fake.kwargs], [False, False])
        self.assertEqual([drive["before_prime"] for drive in self.drives], [None, None])
        self.assertEqual(self.awaited, [])
        for _command, env, _exit in self.fake.calls:
            self.assertNotIn(bench_guard.ENV_SCENARIO, env)

    def test_flatpak_receives_the_app_scenario_only_through_env_and_a_scaled_mark_timeout(self):
        self.args.flatpak = True
        self.rows(command_for=lambda env: bench.launch_command(self.args, env))
        command, env, _exit = self.fake.calls[0]
        self.assertIn("--env=CROSSHOOK_BENCH_SCENARIO=health_scroll", command)
        self.assertIn("--env=LD_PRELOAD=/shim.so", command)
        self.assertNotIn(bench_guard.ENV_SCENARIO, env)
        self.assertNotIn("LD_PRELOAD", env)
        self.assertEqual({call[3] for call in self.awaited}, {20.0})  # 4x the native base
        self.assertEqual({call[4] for call in self.awaited}, {True})


if __name__ == "__main__":
    unittest.main()
