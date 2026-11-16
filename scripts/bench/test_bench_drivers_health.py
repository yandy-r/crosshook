"""Health drivers: `await_health_page` as the G4 `before_prime` hook, the `health` scenario prep and the
G5 `health_table` first-armed budget and too-few-rows witness; fakes for the clock, app and shim log."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_drivers as bd
import bench_drivers_g5 as g5
import bench_drivers_health as health
import bench_events as be
import bench_input as bi
from bench_capture import CaptureError
from bench_events import Begin, Mark
from test_bench_drivers import DriverTestCase
from test_bench_drivers_g5 import G5TestCase
from test_bench_scenario import ScenarioTestCase

SCENARIO = "health_scroll"


class HealthEvents:
    """The app's side of the health contract: BEGIN after `begin_after` slices, marks as configured."""

    def __init__(self, world, phase: str | None = "health_ready", begin_after: int = 0,
                 phase_after: int = 0):
        self.world, self.phase = world, phase
        self.begin_after, self.phase_after = begin_after, phase_after
        self.begin_calls: list[float] = []

    def wait_begin(self, timeout_s):
        self.begin_calls.append(timeout_s)
        if len(self.begin_calls) <= self.begin_after:
            self.world.sleep(timeout_s)
            raise CaptureError("BEGIN_TIMEOUT: no BENCH_BEGIN")
        self.world.events.append(("begin",))
        return Begin(1, SCENARIO)

    def status_mark(self, scenario):
        if self.phase is None or len(self.begin_calls) < self.phase_after:
            return None
        return Mark(1, scenario, 0, self.phase, 0)


class AwaitHealthPageTests(DriverTestCase):
    def hook(self, events, flatpak=False, mark_timeout=1.0):
        return lambda: health.await_health_page(self.proc, events, self.log_dir, mark_timeout, flatpak,
                                                self.world.clock, self.world.sleep)

    def g4_health(self, events, **kwargs):
        return bd.drive_g4(self.proc, "health", self.log_dir, self.kwin, self.dev, self.world.clock,
                           self.world.sleep, before_prime=self.hook(events, **kwargs))

    def test_the_hook_runs_after_placement_and_before_the_priming_notch_in_order(self):
        steps = []
        self.last_frame = lambda: steps.append(("idle",)) or max(self.world.now - 2_000_000_000, 1)
        events = HealthEvents(self.world)
        self.kwin.arm_under_cursor = lambda win: steps.append(("prime-arm", len(self.wheels())))
        real_begin = events.wait_begin
        events.wait_begin = lambda t: steps.append(("begin", len(self.kwin.placed))) or real_begin(t)
        original_status = events.status_mark
        events.status_mark = lambda s: steps.append(("ready",)) or original_status(s)
        self.g4_health(events)
        names = [step[0] for step in steps]
        self.assertEqual(names[:4], ["begin", "ready", "ready", "idle"])
        self.assertLess(names.index("idle"), names.index("prime-arm"))
        self.assertEqual(steps[0], ("begin", 1))           # the window was placed first
        self.assertEqual(steps[names.index("prime-arm")], ("prime-arm", 0))  # before any notch
        self.assertEqual(self.kwin.placed, [(*bd.WINDOW_SIZE["health"], *bd.CURSOR_OFFSET["health"])])

    def test_begin_is_awaited_in_slices_up_to_the_row_wait_plus_slack(self):
        events = HealthEvents(self.world, begin_after=3)
        self.hook(events)()
        self.assertEqual(events.begin_calls, [bd.MARK_SLICE_S] * 4)
        events = HealthEvents(self.world, begin_after=10**6)
        with self.assertRaisesRegex(CaptureError, "^BEGIN_TIMEOUT: not seen within 66 s while waiting for the health page"):
            self.hook(events)()
        self.assertEqual(len(events.begin_calls), int((1.0 + be.HEALTH_WAIT_S + 5) / bd.MARK_SLICE_S))

    def test_too_few_rows_before_begin_fails_without_waiting_the_full_timeout(self):
        events = HealthEvents(self.world, phase="health_rows_too_few", begin_after=10**6, phase_after=3)
        start = self.world.now
        with self.assertRaisesRegex(CaptureError, "^HEALTH_ROWS_TOO_FEW"):
            self.hook(events)()
        self.assertLessEqual(len(events.begin_calls), 4)
        self.assertLess(self.world.now - start, 2 * 10**9)

    def test_too_few_rows_with_begin_fails_and_the_scroll_never_starts(self):
        events = HealthEvents(self.world, phase="health_rows_too_few")
        with self.assertRaisesRegex(CaptureError, "^HEALTH_ROWS_TOO_FEW"):
            self.g4_health(events)
        self.assertEqual(self.wheels(), [])
        self.assertEqual(self.kwin.armed, [])

    def test_begin_without_a_mark_is_a_missing_mark(self):
        with self.assertRaisesRegex(CaptureError, "^HEALTH_MARK_MISSING"):
            self.hook(HealthEvents(self.world, phase=None))()

    def test_a_non_health_status_mark_is_a_missing_mark(self):
        with self.assertRaisesRegex(CaptureError, "^HEALTH_MARK_MISSING"):
            self.hook(HealthEvents(self.world, phase="follow_on"))()

    def test_a_page_that_keeps_presenting_is_not_settled(self):
        self.last_frame = lambda: self.world.now
        with self.assertRaisesRegex(CaptureError, "^HEALTH_NOT_SETTLED: the health page kept presenting for 25 s"):
            self.hook(HealthEvents(self.world))()

    def test_flatpak_scales_the_idle_wait(self):
        self.last_frame = lambda: self.world.now
        with self.assertRaisesRegex(CaptureError, "^HEALTH_NOT_SETTLED: the health page kept presenting for 100 s"):
            self.hook(HealthEvents(self.world), flatpak=True)()

    def test_an_exited_app_is_reported_while_waiting_for_the_page(self):
        self.world.exit_code = 139
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT: the app exited with status 139"):
            self.hook(HealthEvents(self.world, begin_after=2))()

    def test_a_locked_session_stops_the_wait(self):
        self.world.locked = True
        with self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED"):
            self.hook(HealthEvents(self.world, begin_after=2))()

    def test_the_idle_error_for_the_console_still_says_console(self):
        self.last_frame = lambda: self.world.now
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_SETTLED: the console kept presenting"):
            bd._wait_idle(self.proc, self.log_dir, self.world.clock, self.world.sleep)


class DriveG4HookTests(DriverTestCase):
    def test_the_hook_runs_before_the_priming_notch_and_the_timed_scroll(self):
        calls = []
        bd.drive_g4(self.proc, "library", self.log_dir, self.kwin, self.dev, self.world.clock,
                    self.world.sleep, before_prime=lambda: calls.append(len(self.wheels())))
        self.assertEqual(calls, [0])
        self.assertEqual(len(self.wheels()), 1 + 2 * bd.SCROLL_NOTCHES)

    def test_the_hook_runs_after_the_settings_scenario_has_typed_its_way_in(self):
        calls = []
        bd.drive_g4(self.proc, "settings", self.log_dir, self.kwin, self.dev, self.world.clock,
                    self.world.sleep, before_prime=lambda: calls.append(len(self.world.events)))
        self.assertEqual(calls, [next(i for i, e in enumerate(self.world.events) if e[0] == "key") + 1])

    def test_a_failing_hook_stops_before_any_input(self):
        def hook():
            raise CaptureError("HEALTH_ROWS_TOO_FEW: x")
        with self.assertRaisesRegex(CaptureError, "^HEALTH_ROWS_TOO_FEW"):
            bd.drive_g4(self.proc, "health", self.log_dir, self.kwin, self.dev, self.world.clock,
                        self.world.sleep, before_prime=hook)
        self.assertEqual(self.wheels(), [])


class PrepareHealthScenarioTests(ScenarioTestCase):
    def test_health_is_a_g4_scenario_with_the_library_geometry(self):
        self.assertIn("health", bd.SCENARIOS)
        self.assertEqual(bd.CURSOR_OFFSET["health"], (640, 450))
        self.assertEqual(bd.WINDOW_SIZE["health"], (1280, 900))

    def test_health_strips_the_replay_env_sets_the_app_scenario_and_deletes_settings(self):
        env = self.env(False)
        settings = self.root / "config/crosshook/settings.toml"
        bd.prepare_scenario("console", env, False)
        self.assertTrue(settings.exists())
        updated = bd.prepare_scenario("health", env, False)
        self.assertFalse(settings.exists())
        self.assertEqual(updated["CROSSHOOK_BENCH_SCENARIO"], "health_scroll")
        self.assertNotIn("CROSSHOOK_BENCH_LOG", updated)
        self.assertNotIn("CROSSHOOK_BENCH_SCENARIO", env)
        self.assertEqual(updated["KEEP"], "1")

    def test_other_scenarios_do_not_set_the_app_scenario(self):
        for scenario in ("library", "settings", "console"):
            self.assertNotIn("CROSSHOOK_BENCH_SCENARIO", bd.prepare_scenario(scenario, self.env(False), False))


class HealthTableG5Tests(G5TestCase):
    scenario = "health_table"

    def test_health_table_presses_enter(self):
        self.assertEqual(g5.G5_KEYS["health_table"], (bi.KEY_ENTER,))

    def test_the_first_armed_bound_is_eighty_seconds_beyond_the_mark_timeout(self):
        self.assertEqual(g5.first_armed_extra_s("health_table"), 80.0)
        self.assertEqual(g5.first_armed_extra_s("search"), g5.FIRST_ARMED_EXTRA_S)
        self.assertEqual(g5.first_armed_extra_s("toggle_optimization"), 40.0)
        self.world.no_armed_seq = 0
        with self.assertRaisesRegex(CaptureError, "^MARK_TIMEOUT: health_table seq 0 armed not seen within 81 s"):
            self.drive()
        self.assertEqual(self.key_trace(), [])

    def test_other_scenarios_keep_the_forty_second_budget(self):
        self.world.no_armed_seq = 0
        with self.assertRaisesRegex(CaptureError, "^MARK_TIMEOUT: search seq 0 armed not seen within 41 s"):
            self.drive("search")

    def test_a_too_few_mark_during_the_first_armed_wait_stops_early(self):
        self.world.no_armed_seq = 0
        self.world.status_phase = "health_rows_too_few"
        start = self.world.now
        with self.assertRaisesRegex(CaptureError, "^HEALTH_ROWS_TOO_FEW"):
            self.drive()
        self.assertLess(self.world.now - start, 2 * 10**9)
        self.assertEqual(self.key_trace(), [])

    def test_a_ready_mark_does_not_stop_the_wait(self):
        self.world.status_phase = "health_ready"
        self.assertEqual(len(self.drive(trials=2).records), 2)

    def test_the_health_table_scenario_is_driven_like_the_others(self):
        self.assertEqual(len(self.drive(trials=3).records), 3)
        self.assertEqual(len(self.key_trace()), 3)


if __name__ == "__main__":
    unittest.main()
