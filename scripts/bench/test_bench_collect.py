"""Presentation collection for G1/G2: per-metric dirs, host env isolation, refresh checks; stdlib only."""
import csv
import shutil
import sys
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from typing import ClassVar
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_capture
import bench_collect
from bench_capture import CaptureError
from bench_collect import driven_rows, presentation_rows
from bench_drivers import DriveResult
from bench_guard import GuardError
from bench_present import Presented, check_refresh


def log_text(commit_ns: int, presented_ns: int, refresh_ns: int = 8333333, clock: str = "1") -> str:
    return (f"H\tclock_id={clock}\nS\t1\t7\ttoplevel\n"
            f"C\t{commit_ns}\t7\t1\t1\nP\t{presented_ns}\t1\t{presented_ns}\t{refresh_ns}\t1\t0\n")


class FakeReady:
    """Stands in for bench_capture.ready: records calls and writes a present log like the shim."""

    def __init__(self, offset_ns: int = 5_000_000, clock: str = "1", refresh_ns: int = 8333333):
        self.calls: list[tuple[list[str], dict[str, str]]] = []
        self.offset_ns, self.clock, self.refresh_ns = offset_ns, clock, refresh_ns

    def __call__(self, command, env, timeout, exit_after):
        self.calls.append((list(command), dict(env)))
        launch = time.monotonic_ns()
        flag = "--env=CROSSHOOK_BENCH_PRESENT_DIR="  # Flatpak gets the directory only as a flag
        present = Path(env.get("CROSSHOOK_BENCH_PRESENT_DIR") or
                       next(arg[len(flag):] for arg in command if arg.startswith(flag)))
        (present / "wlpresent-1.log").write_text(
            log_text(launch, launch + self.offset_ns, self.refresh_ns, self.clock))
        return SimpleNamespace(launch_ns=launch, stderr=None), 12.5


class CollectTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "crosshook-bench-x"
        (self.root / "home").mkdir(parents=True)
        self.env = {"HOME": str(self.root / "home"), "LD_PRELOAD": "/shim.so", "PATH": "/usr/bin"}
        self.args = SimpleNamespace(iterations=2, timeout=5, refresh_hz=120)
        self.fake = FakeReady()
        for target, replacement in (("ready", self.fake), ("stop", lambda proc: None)):
            patcher = patch.object(bench_collect, target, replacement)
            patcher.start()
            self.addCleanup(patcher.stop)

    def rows(self, metric="G2", command_for=None):
        return presentation_rows(self.args, command_for or (lambda env: ["app"]), self.env, metric)

    def test_rows_and_summary(self):
        rows = self.rows()
        measured = [row for row in rows if row["iteration"] != "summary"]
        self.assertEqual([row["iteration"] for row in measured], [1, 2])
        for row in measured:
            self.assertEqual(row["ready_ms"], 12.5)
            self.assertGreaterEqual(row["first_present_ms"], 5.0)
            self.assertEqual(row["status"], "measured")
        for key in ("p50_ms", "p95_ms", "first_present_p50_ms", "first_present_p95_ms"):
            self.assertIn(key, rows[-1])

    def test_dirs_are_per_metric(self):
        self.rows("G1")
        self.rows("G2")
        base = self.root / "present"
        for metric in ("G1", "G2"):
            for number in (1, 2):
                self.assertEqual(len(list((base / metric / f"iter-{number}").glob("wlpresent-*.log"))), 1)
        self.assertEqual(oct((base / "G1").stat().st_mode & 0o777), "0o700")
        self.assertEqual(oct((base / "G1/iter-1").stat().st_mode & 0o777), "0o700")

    def test_existing_dir_refused(self):
        (self.root / "present/G2/iter-1").mkdir(parents=True)
        with self.assertRaisesRegex(CaptureError, "^PRESENT_DIR_EXISTS: .*iter-1"):
            self.rows("G2")
        self.assertEqual(self.fake.calls, [])

    def test_present_before_launch_is_invalid(self):
        self.fake.offset_ns = -50_000_000
        with self.assertRaisesRegex(CaptureError, "^INVALID_PRESENT_CLOCK: "):
            self.rows()

    def test_wrong_clock_refused(self):
        self.fake.clock = "0"
        with self.assertRaisesRegex(CaptureError, "^PRESENTATION_CLOCK: "):
            self.rows()

    def test_refresh_checked_with_one_frame(self):
        self.fake.refresh_ns = 16_666_666  # 60 Hz against the default 120 Hz
        with self.assertRaisesRegex(CaptureError, "^REFRESH_MISMATCH: "):
            self.rows()
        frame = Presented(1, 1, 2, 16_666_666, 1, 0)
        with self.assertRaises(CaptureError):
            check_refresh([frame], 120)

    def test_native_keeps_preload_in_env(self):
        self.rows()
        command, env = self.fake.calls[0]
        self.assertEqual(env["LD_PRELOAD"], "/shim.so")
        self.assertIn("CROSSHOOK_BENCH_PRESENT_DIR", env)

    def test_flatpak_host_env_loses_shim_variables(self):
        args = SimpleNamespace(flatpak=True, branch="master", binary=None)
        self.rows(command_for=lambda env: bench.launch_command(args, env))
        command, env = self.fake.calls[0]
        present = self.root / "present/G2/iter-1"
        self.assertIn("--env=LD_PRELOAD=/shim.so", command)
        self.assertIn(f"--env=CROSSHOOK_BENCH_PRESENT_DIR={present}", command)
        self.assertNotIn("LD_PRELOAD", env)
        self.assertNotIn("CROSSHOOK_BENCH_PRESENT_DIR", env)
        self.assertEqual(env["PATH"], "/usr/bin")


PERIOD_NS = 8_333_333
BASE_NS = 1_000_000_000_000
FRAMES = 100
WINDOW = (20, 50)  # frame indices the fake driver reports as the timed window: 30 intervals


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


class FakeDrivenReady:
    """Stands in for bench_capture.ready: writes a present log with `FRAMES` presented frames."""

    def __init__(self):
        self.calls: list[tuple[list[str], dict[str, str], bool]] = []
        self.steps = [PERIOD_NS] * FRAMES
        self.extra_discards = 0
        self.discard_commit_ns = BASE_NS + 30 * PERIOD_NS  # inside the timed window by default
        self.stderr_closed = False

    def times(self) -> list[int]:
        out, now = [], BASE_NS
        for step in self.steps:
            now += step
            out.append(now)
        return out

    def __call__(self, command, env, timeout, exit_after):
        self.calls.append((list(command), dict(env), exit_after))
        flag = "--env=CROSSHOOK_BENCH_PRESENT_DIR="
        present = Path(env.get("CROSSHOOK_BENCH_PRESENT_DIR") or
                       next(arg[len(flag):] for arg in command if arg.startswith(flag)))
        lines = ["H\tclock_id=1", "S\t1\t7\ttoplevel"]
        for seq, ns in enumerate(self.times(), 1):
            lines += [f"C\t{ns - 1000}\t7\t{seq}\t1", f"P\t{ns}\t{seq}\t{ns}\t{PERIOD_NS}\t{seq}\t0"]
        for seq in range(1000, 1000 + self.extra_discards):
            lines += [f"C\t{self.discard_commit_ns}\t7\t{seq}\t1", f"D\t{self.discard_commit_ns}\t{seq}"]
        (present / "wlpresent-1.log").write_text("\n".join(lines) + "\n")
        proc = SimpleNamespace(launch_ns=BASE_NS, stderr=SimpleNamespace(close=self.close_stderr))
        return proc, 10.0

    def close_stderr(self):
        self.stderr_closed = True


class DrivenTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "crosshook-bench-x"
        for name in ("home", "config"):
            (self.root / name).mkdir(parents=True)
        (self.root / "bench-tools").mkdir(mode=0o700)
        self.env = {"HOME": str(self.root / "home"), "XDG_CONFIG_HOME": str(self.root / "config"),
                    "CROSSHOOK_BENCH_ROOT": str(self.root), "LD_PRELOAD": "/shim.so", "PATH": "/usr/bin"}
        self.args = SimpleNamespace(iterations=2, timeout=5, refresh_hz=120.0, flatpak=False,
                                    branch="master", binary=None,
                                    scenarios={"G4": ["library", "console"], "G9": ["resize"]})
        self.fake = FakeDrivenReady()
        self.stopped: list[object] = []
        self.drive_calls: list[tuple] = []
        FakeDevice.instances, FakeKWin.instances = [], []
        self.settled: list[FakeDevice] = []
        self.bounds = lambda: DriveResult(self.fake.times()[WINDOW[0]], self.fake.times()[WINDOW[1]], 90)

        self.settings = self.root / "config/crosshook/settings.toml"
        self.settings_seen: list[tuple[str, bool]] = []  # (scenario, settings.toml present while driving)

        def drive_g4(proc, scenario, log_dir, kwin, dev):
            self.drive_calls.append(("G4", scenario, log_dir, kwin, dev))
            self.settings_seen.append((scenario, self.settings.is_file()))
            return self.bounds()

        def drive_g9(proc, log_dir, kwin):
            self.drive_calls.append(("G9", "resize", log_dir, kwin, None))
            self.settings_seen.append(("resize", self.settings.is_file()))
            return self.bounds()

        self.drive_g4, self.drive_g9 = drive_g4, drive_g9
        self.require_kwin_calls = 0
        self.instance_checks: list[int] = []
        for target, replacement in (
                ("ready", self.fake), ("stop", self.stopped.append), ("InputDevice", FakeDevice),
                ("KWinScripts", FakeKWin), ("drive_g4", lambda *a: self.drive_g4(*a)),
                ("drive_g9", lambda *a: self.drive_g9(*a)), ("settle_device", self.settled.append),
                ("require_kwin", self.count_kwin), ("require_no_instance", self.check_instance)):
            patcher = patch.object(bench_collect, target, replacement)
            patcher.start()
            self.addCleanup(patcher.stop)

    def check_instance(self):
        self.instance_checks.append(len(self.fake.calls))

    def count_kwin(self):
        self.require_kwin_calls += 1

    def rows(self, metric="G4", command_for=None):
        return driven_rows(self.args, metric, command_for or (lambda env: ["app"]), self.env)

    def test_one_row_per_scenario_with_pooled_stats(self):
        self.fake.steps[30] = 2 * PERIOD_NS  # one missed vblank inside the window, every iteration
        rows = self.rows()
        self.assertEqual([row["scenario"] for row in rows], ["library", "console"])
        for row in rows:
            self.assertEqual(row["frames"], 60)  # 2 iterations x 30 intervals; pre/post frames excluded
            self.assertEqual(row["status"], "captured")
            self.assertAlmostEqual(row["p50_ms"], PERIOD_NS / 1e6)
            self.assertAlmostEqual(row["p99_ms"], 2 * PERIOD_NS / 1e6, delta=0.3)
            self.assertAlmostEqual(row["over_interval_percent"], 100 * 2 / 60)
            for key in ("p95_ms", "stddev_ms"):
                self.assertIn(key, row)
        self.assertEqual(len(self.fake.calls), 4)
        self.assertTrue(all(call[2] is False for call in self.fake.calls))

    def test_g9_row(self):
        rows = self.rows("G9")
        self.assertEqual([(row["scenario"], row["frames"]) for row in rows], [("resize", 60)])
        self.assertEqual({call[0] for call in self.drive_calls}, {"G9"})

    def test_g9_never_opens_or_settles_the_input_device(self):
        self.rows("G9")
        self.assertEqual(FakeDevice.instances, [])
        self.assertEqual(self.settled, [])
        self.assertEqual(self.require_kwin_calls, 1)

    def test_frames_outside_the_window_are_excluded(self):
        # Frames before start or after end never contribute, however slow they are.
        self.fake.steps[5] = 50 * PERIOD_NS
        self.fake.steps[80] = 50 * PERIOD_NS
        for row in self.rows():
            self.assertEqual(row["frames"], 60)
            self.assertAlmostEqual(row["p99_ms"], PERIOD_NS / 1e6)
            self.assertEqual(row["over_interval_percent"], 0)

    def test_device_opened_once_and_settled_once(self):
        self.rows()
        self.assertEqual(self.require_kwin_calls, 1)
        self.assertEqual(len(FakeDevice.instances), 1)
        self.assertEqual(self.settled, FakeDevice.instances)
        self.assertTrue(FakeDevice.instances[0].closed)
        self.assertEqual(len(self.stopped), 4)
        self.assertTrue(self.fake.stderr_closed)

    def test_fresh_dirs_and_kwin_scripts_per_iteration(self):
        self.rows()
        base = self.root / "present/G4"
        for scenario in ("library", "console"):
            for number in (1, 2):
                directory = base / scenario / f"iter-{number}"
                self.assertEqual(len(list(directory.glob("wlpresent-*.log"))), 1)
                self.assertEqual(oct(directory.stat().st_mode & 0o777), "0o700")
        self.assertEqual(oct((base / "library").stat().st_mode & 0o777), "0o700")
        self.assertEqual([call[2] for call in self.drive_calls],
                         [base / s / f"iter-{n}" for s in ("library", "console") for n in (1, 2)])
        kwins = FakeKWin.instances
        self.assertEqual(len(kwins), 4)
        self.assertEqual({kwin.tools_dir for kwin in kwins}, {self.root / "bench-tools"})
        self.assertEqual(len({kwin.run_tag for kwin in kwins}), 4)
        for kwin in kwins:
            self.assertRegex(kwin.run_tag, r"^[a-z0-9]{1,16}$")
        self.assertTrue(all(call[4] is FakeDevice.instances[0] for call in self.drive_calls))

    def test_existing_iteration_dir_refused(self):
        (self.root / "present/G4/library/iter-1").mkdir(parents=True)
        with self.assertRaisesRegex(CaptureError, "^PRESENT_DIR_EXISTS: .*library/iter-1"):
            self.rows()
        self.assertEqual(self.fake.calls, [])
        self.assertTrue(FakeDevice.instances[0].closed)

    def test_console_scenario_sets_log_env(self):
        self.rows()
        library, console = self.fake.calls[0][1], self.fake.calls[2][1]
        self.assertNotIn("CROSSHOOK_BENCH_LOG", library)
        self.assertEqual(console["CROSSHOOK_BENCH_LOG_RATE"], "5000")
        self.assertTrue(console["CROSSHOOK_BENCH_LOG"].startswith(str(self.root)))
        self.assertEqual(self.settings_seen, [("library", False), ("library", False),
                                              ("console", True), ("console", True)])
        self.assertFalse(self.settings.exists())  # removed once the metric finished

    def test_input_not_delivered(self):
        self.bounds = lambda: DriveResult(self.fake.times()[-1] + 1, self.fake.times()[-1] + 10**9, 90)
        with self.assertRaisesRegex(CaptureError, "^INPUT_NOT_DELIVERED: library"):
            self.rows()
        self.assertTrue(FakeDevice.instances[0].closed)

    def test_g9_has_no_input_delivery_gate(self):
        self.bounds = lambda: DriveResult(self.fake.times()[-1] + 1, self.fake.times()[-1] + 10**9, 90)
        with self.assertRaisesRegex(CaptureError, "^CAPTURE_TOO_SHORT: resize iteration 1: 0 frame intervals, need 5"):
            self.rows("G9")

    def test_discard_rate(self):
        self.fake.extra_discards = 2  # 2 of 32 frames committed inside the window, above 5 %
        with self.assertRaisesRegex(CaptureError, "^DISCARD_RATE: "):
            self.rows()

    def test_discard_rate_below_threshold_passes(self):
        self.fake.extra_discards = 1  # 1 of 31
        self.assertEqual(len(self.rows()), 2)

    def test_discards_outside_the_timed_window_do_not_count(self):
        self.fake.extra_discards = 40  # a settle-time burst before the first notch
        self.fake.discard_commit_ns = BASE_NS + 5 * PERIOD_NS
        self.assertEqual(len(self.rows()), 2)
        self.fake.discard_commit_ns = BASE_NS + 90 * PERIOD_NS  # and after the last one
        self.assertEqual(len(self.rows("G9")), 1)

    def test_each_iteration_needs_a_minimum_of_intervals(self):
        calls = []

        def bounds():
            calls.append(1)
            window = (20, 50) if len(calls) == 1 else (20, 24)  # the second iteration yields 4 intervals
            return DriveResult(self.fake.times()[window[0]], self.fake.times()[window[1]], 90)
        self.bounds = bounds
        with self.assertRaisesRegex(
                CaptureError, "^CAPTURE_TOO_SHORT: library iteration 2: 4 frame intervals, need 5$"):
            self.rows()
        self.assertTrue(FakeDevice.instances[0].closed)
        self.assertEqual(len(self.fake.calls), 2)

    def test_five_intervals_in_one_iteration_is_enough_for_that_iteration(self):
        windows = iter([(20, 50), (20, 25)])
        self.bounds = lambda: DriveResult(self.fake.times()[(w := next(windows))[0]], self.fake.times()[w[1]], 90)
        self.args.scenarios = {"G4": ["library"], "G9": ["resize"]}
        (row,) = self.rows()
        self.assertEqual(row["frames"], 35)

    def test_capture_too_short(self):
        self.args.iterations = 1
        with patch.object(bench_collect, "MIN_INTERVALS", 31), \
                self.assertRaisesRegex(CaptureError, "^CAPTURE_TOO_SHORT: library: 30 frame intervals, need 31"):
            self.rows()
        self.assertTrue(FakeDevice.instances[0].closed)

    def test_refresh_mismatch_is_checked(self):
        self.args.refresh_hz = 60.0
        with self.assertRaisesRegex(CaptureError, "^REFRESH_MISMATCH: "):
            self.rows()

    def test_driver_failure_stops_app_and_closes_device(self):
        def failing(*_args):
            raise CaptureError("FOCUS_LOST: pointer")
        self.drive_g4 = failing
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: "):
            self.rows()
        self.assertEqual(len(self.stopped), 1)
        self.assertTrue(self.fake.stderr_closed)
        self.assertEqual(len(self.fake.calls), 1)
        self.assertTrue(FakeDevice.instances[0].closed)

    def test_keyboard_interrupt_stops_app_and_closes_device(self):
        def interrupted(*_args):
            raise KeyboardInterrupt
        self.drive_g4 = interrupted
        with self.assertRaises(KeyboardInterrupt):
            self.rows()
        self.assertEqual(len(self.stopped), 1)
        self.assertTrue(self.fake.stderr_closed)
        self.assertEqual(len(self.fake.calls), 1)
        self.assertTrue(FakeDevice.instances[0].closed)

    def test_unregistered_device_fails_before_any_launch_and_closes_it(self):
        def unregistered(_dev):
            raise CaptureError("INPUT_NOT_DELIVERED: KWin did not register the bench input device")
        with patch.object(bench_collect, "settle_device", unregistered), \
                self.assertRaisesRegex(CaptureError, "^INPUT_NOT_DELIVERED: KWin did not register"):
            self.rows()
        self.assertEqual(self.fake.calls, [])
        self.assertTrue(FakeDevice.instances[0].closed)

    def test_unavailable_kwin_opens_no_device(self):
        def refuse():
            raise CaptureError("UNSUPPORTED_COMPOSITOR: KWin is required")
        with patch.object(bench_collect, "require_kwin", refuse), \
                self.assertRaisesRegex(CaptureError, "^UNSUPPORTED_COMPOSITOR"):
            self.rows()
        self.assertEqual(FakeDevice.instances, [])
        self.assertEqual(self.fake.calls, [])

    def test_instance_is_rechecked_immediately_before_every_launch(self):
        self.rows()
        self.assertEqual(self.instance_checks, [0, 1, 2, 3])  # app launches seen so far at each check
        self.instance_checks.clear()
        FakeDevice.instances.clear()
        self.rows("G9")
        self.assertEqual(len(self.instance_checks), 2)

    def test_an_instance_that_appears_mid_run_refuses_before_the_next_launch(self):
        def check():
            self.instance_checks.append(len(self.fake.calls))
            if len(self.instance_checks) == 3:
                raise GuardError("INSTANCE_RUNNING: quit CrossHook before driving input; ...")
        with patch.object(bench_collect, "require_no_instance", check), \
                self.assertRaisesRegex(GuardError, "^INSTANCE_RUNNING"):
            self.rows()
        self.assertEqual(len(self.fake.calls), 2)  # iterations 1 and 2 launched, the third never
        self.assertTrue(FakeDevice.instances[0].closed)

    def test_driven_rows_record_the_drive_per_iteration(self):
        evidence: dict = {}
        self.bounds = lambda: DriveResult(self.fake.times()[20], self.fake.times()[50], 150, 5_010_000_000)
        driven_rows(self.args, "G9", lambda env: ["app"], self.env, evidence)
        self.assertEqual(evidence, {"G9": {"resize": [
            {"iteration": 1, "injected": 150, "driven_s": 5.01},
            {"iteration": 2, "injected": 150, "driven_s": 5.01}]}})

    def test_native_keeps_shim_env(self):
        self.rows()
        env = self.fake.calls[0][1]
        self.assertEqual(env["LD_PRELOAD"], "/shim.so")
        self.assertIn("CROSSHOOK_BENCH_PRESENT_DIR", env)

    def test_flatpak_host_env_loses_shim_variables(self):
        self.args.flatpak = True
        self.rows(command_for=lambda env: bench.launch_command(self.args, env))
        library, console = self.fake.calls[0], self.fake.calls[2]  # scenario order: library x2, console x2
        present = self.root / "present/G4/library/iter-1"
        self.assertIn("--env=LD_PRELOAD=/shim.so", library[0])
        self.assertIn(f"--env=CROSSHOOK_BENCH_PRESENT_DIR={present}", library[0])
        self.assertFalse([arg for arg in library[0] if arg.startswith("--env=CROSSHOOK_BENCH_LOG")])
        self.assertTrue([arg for arg in console[0] if arg.startswith("--env=CROSSHOOK_BENCH_LOG=")])
        self.assertIn("--env=CROSSHOOK_BENCH_LOG_RATE=5000", console[0])
        for _command, env, _exit in (library, console):
            for key in ("LD_PRELOAD", "CROSSHOOK_BENCH_PRESENT_DIR", "CROSSHOOK_BENCH_LOG",
                        "CROSSHOOK_BENCH_LOG_RATE"):
                self.assertNotIn(key, env)
            self.assertEqual(env["PATH"], "/usr/bin")
        flatpak_settings = self.root / "home/.var/app/dev.crosshook.CrossHook/config/crosshook/settings.toml"
        self.assertFalse(flatpak_settings.exists())  # removed once the metric finished

    def test_flatpak_console_settings_exist_while_driving_and_are_removed_after(self):
        self.args.flatpak = True
        flatpak_settings = self.root / "home/.var/app/dev.crosshook.CrossHook/config/crosshook/settings.toml"
        seen = []
        self.drive_g4 = lambda proc, scenario, *_rest: (seen.append((scenario, flatpak_settings.is_file())),
                                                        self.bounds())[1]
        self.rows(command_for=lambda env: bench.launch_command(self.args, env))
        self.assertEqual(seen, [("library", False), ("library", False), ("console", True), ("console", True)])
        self.assertFalse(flatpak_settings.exists())

    def test_console_settings_do_not_outlive_the_metric(self):
        failures = {"a capture error": CaptureError("FOCUS_LOST: pointer"), "an interrupt": KeyboardInterrupt()}
        for name, failure in failures.items():
            with self.subTest(failure=name):
                self.args.scenarios = {"G4": ["console"], "G9": ["resize"]}
                self.args.iterations = 1
                FakeDevice.instances.clear()
                self.fake.calls.clear()
                shutil.rmtree(self.root / "present", ignore_errors=True)

                def failing(*_args, failure=failure):
                    self.assertTrue(self.settings.is_file())  # the console iteration really had it
                    raise failure
                self.drive_g4 = failing
                with self.assertRaises(type(failure)):
                    self.rows()
                self.assertFalse(self.settings.exists())

    def test_a_failure_in_a_later_check_still_removes_the_console_settings(self):
        self.args.scenarios = {"G4": ["console"], "G9": ["resize"]}
        self.fake.extra_discards = 2  # DISCARD_RATE fails after the drive returned
        with self.assertRaisesRegex(CaptureError, "^DISCARD_RATE: "):
            self.rows()
        self.assertFalse(self.settings.exists())

    def test_g9_starts_without_a_console_settings_file_left_by_an_earlier_metric(self):
        self.settings.parent.mkdir(parents=True)
        self.settings.write_text("console_drawer_collapsed_default = false\n")
        self.rows("G9")
        self.assertEqual(self.settings_seen, [("resize", False), ("resize", False)])
        self.assertFalse(self.settings.exists())

    def test_g4_then_g9_in_one_run_do_not_leak_the_console_file(self):
        self.args.scenarios = {"G4": ["library", "console"], "G9": ["resize"]}
        self.rows("G4")
        self.rows("G9")
        self.assertEqual([seen for scenario, seen in self.settings_seen if scenario == "resize"], [False, False])

    def test_settings_outside_the_bench_root_are_refused_and_left_alone(self):
        outside = Path(self.temp.name) / "real-config/crosshook"
        outside.mkdir(parents=True)
        victim = outside / "settings.toml"
        victim.write_text("keep = true\n")
        self.env["XDG_CONFIG_HOME"] = str(outside.parent)
        for metric in ("G4", "G9"):
            with self.subTest(metric=metric), self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE"):
                self.rows(metric)
        self.assertEqual(victim.read_text(), "keep = true\n")
        self.assertEqual(self.fake.calls, [])  # nothing was launched


class DispatchAndIngestTests(unittest.TestCase):
    def test_presentation_g4_g9_route_to_driven_rows(self):
        args = SimpleNamespace(collect="presentation")
        for metric in ("G4", "G9"):
            with patch.object(bench_collect, "driven_rows", return_value=[{"status": "captured"}]) as driven:
                factory = lambda env: ["app"]
                rows = bench_capture.capture(metric, args, ["app"], {"A": "1"}, command_for=factory)
            self.assertEqual(rows, [{"status": "captured"}])
            driven.assert_called_once_with(args, metric, factory, {"A": "1"}, None)

    def test_g1_presentation_does_not_use_driven_rows(self):
        args = SimpleNamespace(collect="presentation", cold_cache_ack=True)
        with patch.object(bench_collect, "driven_rows") as driven, \
                patch.object(bench_collect, "presentation_rows", return_value=["x"]):
            self.assertEqual(bench_capture.capture("G1", args, ["app"], {}), ["x"])
        driven.assert_not_called()

    def test_csv_ingest_rows_are_external_with_frame_count(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "capture.csv"
            with path.open("w", newline="") as stream:
                writer = csv.writer(stream)
                writer.writerow(["frame_ms"])
                writer.writerows([[8.0], [8.5], [9.0], [16.0]])
            args = SimpleNamespace(collect="csv", capture_csv=str(path), refresh_hz=120.0)
            for metric in ("G4", "G9"):
                (row,) = bench_capture.capture(metric, args, ["app"], {})
                self.assertEqual((row["scenario"], row["frames"], row["status"]), ("external", 4, "measured"))
                for key in ("p50_ms", "p95_ms", "p99_ms", "stddev_ms", "over_interval_percent"):
                    self.assertIn(key, row)
                self.assertEqual(set(row), set(bench.SCHEMAS[metric].split(",")))


if __name__ == "__main__":
    unittest.main()
