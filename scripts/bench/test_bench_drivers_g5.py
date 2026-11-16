"""G5 driver checks with a fake device, KWin, event reader, clock and a real shim-format log; stdlib only."""
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_drivers as bd
import bench_drivers_g5 as g5
import bench_input as bi
from bench_capture import CaptureError
from bench_events import Begin, ClockSample, Mark
from bench_g5select import AMBIGUOUS, EARLY, MEASURED, TIMEOUT
from bench_kwin import WindowInfo

WIN = WindowInfo("{u}", 4242, 0.0, 0.0, 1280.0, 900.0, False)
MS = 1_000_000
OFFSET = 5_000_000_000_000   # CLOCK_MONOTONIC ns = performance.now() ns + OFFSET
START = 10_000_000_000


def clock_samples(offset_ns: int = OFFSET, rtt_us: int = 2000, count: int = 20) -> list[ClockSample]:
    """Samples as the app sends them: sequential, each stamped by Rust midway through its round trip."""
    return [ClockSample(offset_ns + (1_000_000 + i * rtt_us) * 1000 + rtt_us * 500, i, 1_000_000 + i * rtt_us)
            for i in range(count)]


class World:
    """One shared fake clock, the shim log the driver tails, and the app's markers."""

    def __init__(self, directory: Path):
        self.now = START
        self.trace: list[tuple] = []
        self.locked = False
        self.exit_code = None
        self.keyboard_enter = True
        self.keyboard_leaves_after_keys: int | None = None
        self.focus_lost_at: int | None = None     # a focus_lost mark exists from this clock value on
        self.no_armed_seq: int | None = None
        self.no_render = False
        self.commit_delay_ns = 40 * MS            # change frame commit after the key
        self.render_delay_ns = 30 * MS            # the render mark after the key (host time)
        self.present_delay_ns = 20 * MS
        self.suppress_commit = False
        self.busy_gaps_ns: list[int] = []         # background commits written when a trial arms
        self.samples = clock_samples()
        self.marks: dict[tuple[str, int, str], Mark] = {}
        self.log = directory / "wlpresent-1.log"
        self.log.write_text("H\tclock_id=1\nS\t1\t1\ttoplevel\n")
        self.frame = 0
        self.keys = 0
        self.scenario = "search"

    def clock(self) -> int:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.now += int(seconds * 1e9)

    def append(self, text: str) -> None:
        with self.log.open("a") as stream:
            stream.write(text)

    def frame_at(self, commit_ns: int, present_ns: int | None = None) -> None:
        self.frame += 1
        shown = commit_ns + self.present_delay_ns if present_ns is None else present_ns
        self.append(f"C\t{commit_ns}\t1\t{self.frame}\t1\nP\t{shown}\t{self.frame}\t{shown}\t8333333\t1\t0\n")

    def set_keyboard(self, entered: bool) -> None:
        self.append(f"K\t{self.now}\t{'enter' if entered else 'leave'}\t1\n")


class FakeProc:
    pid = 4242

    def __init__(self, world):
        self.world = world

    def poll(self):
        return self.world.exit_code


class FakeDev:
    def __init__(self, world):
        self.world = world

    def key_stamped(self, code: int) -> int:
        w = self.world
        inject = w.now
        w.trace.append(("key", code))
        w.keys += 1
        if not w.suppress_commit:
            w.frame_at(inject + w.commit_delay_ns)
        seq = max(seq for (_, seq, phase) in w.marks if phase == "armed")
        if not w.no_render:
            w.marks[(w.scenario, seq, "render")] = Mark(
                inject, w.scenario, seq, "render", (inject + w.render_delay_ns - OFFSET) // 1000)
        if w.keyboard_leaves_after_keys == w.keys:
            w.set_keyboard(False)
        return inject


class FakeKwin:
    def __init__(self, world):
        self.world = world

    def place(self, win, width, height, dx, dy):
        self.world.trace.append(("place", width, height, dx, dy))
        if self.world.keyboard_enter:
            self.world.set_keyboard(True)
        return win

    def arm_under_cursor(self, win):
        self.world.trace.append(("arm",))
        return win


class FakeEvents:
    """The app's side: arms trial N when asked for it and answers `render` after a key."""

    def __init__(self, world, scenario="search"):
        self.world, self.scenario = world, scenario

    def wait_begin(self, timeout_s):
        self.world.trace.append(("begin",))
        return Begin(1, self.scenario)

    def clock_samples(self):
        return list(self.world.samples)

    def focus_lost(self, scenario, since_ns):
        w = self.world
        if w.focus_lost_at is not None and w.now >= w.focus_lost_at:
            return Mark(w.focus_lost_at, scenario, 0, "focus_lost", 0)
        return None

    def wait_mark(self, scenario, seq, phase, timeout_s):
        w = self.world
        if phase == "armed":
            if w.no_armed_seq == seq:
                raise CaptureError(f"MARK_TIMEOUT: {scenario} seq {seq} armed")
            w.trace.append(("armed", seq))
            t = w.now
            for gap in w.busy_gaps_ns:
                t += gap
                w.frame_at(t)
            w.marks[(scenario, seq, "armed")] = Mark(w.now, scenario, seq, "armed", 0)
            return w.marks[(scenario, seq, "armed")]
        found = w.marks.get((scenario, seq, phase))
        if found is None:
            raise CaptureError(f"MARK_TIMEOUT: {scenario} seq {seq} {phase}")
        w.trace.append((phase, seq))
        return found

    def check(self):
        self.world.trace.append(("check",))


class G5TestCase(unittest.TestCase):
    scenario = "search"

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.log_dir = Path(self.temp.name)
        self.world = World(self.log_dir)
        bd._noticed = False
        stderr = patch.object(sys, "stderr", io.StringIO())
        stderr.start()
        self.addCleanup(stderr.stop)
        for target, value in (("find_window", lambda pids, *a: WIN),
                              ("process_tree", lambda pid: {pid, pid + 1}),
                              ("session_locked", lambda: self.world.locked)):
            patcher = patch.object(bd, target, value)
            patcher.start()
            self.addCleanup(patcher.stop)
        self.proc, self.dev = FakeProc(self.world), FakeDev(self.world)
        self.kwin = FakeKwin(self.world)

    def drive(self, scenario=None, trials=4, mark_timeout=1.0, events=None):
        scenario = self.scenario if scenario is None else scenario
        self.world.scenario = scenario
        return g5.drive_g5(self.proc, scenario, self.log_dir, self.kwin, self.dev,
                           events or FakeEvents(self.world, scenario), trials, mark_timeout,
                           self.world.clock, self.world.sleep)

    def key_trace(self):
        return [item[1] for item in self.world.trace if item[0] == "key"]


class EstimateOffsetTests(unittest.TestCase):
    def test_offset_is_the_median_of_midpoint_offsets_and_uncertainty_uses_the_best_round_trip(self):
        result = g5.estimate_offset(clock_samples())
        self.assertEqual(result.offset_ns, OFFSET)
        self.assertEqual(result.uncertainty_ns, 1000 * 1000 + 1 * MS)  # best rtt 2 ms: 1 ms + 1 ms floor

    def test_a_slow_outlier_does_not_move_the_median_and_the_fastest_pair_sets_the_uncertainty(self):
        samples = clock_samples(rtt_us=1000)
        # One slow call: everything after sample 5 happens 3 ms later on both clocks.
        samples = [s if s.seq <= 5 else ClockSample(s.ns + 3 * MS, s.seq, s.t_perf_us + 3000) for s in samples]
        result = g5.estimate_offset(samples)
        self.assertEqual(result.offset_ns, OFFSET)  # the one biased pair (1.5 ms low) is not the median
        self.assertEqual(result.uncertainty_ns, 500_000 + 1 * MS)

    def test_zero_round_trips_from_the_one_millisecond_floor_still_give_the_floor_uncertainty(self):
        self.assertEqual(g5.estimate_offset(clock_samples(rtt_us=0)).uncertainty_ns, 1 * MS)

    def test_pairs_taken_while_the_app_was_busy_are_ignored(self):
        # 110 ms round trips during startup, then a quiet stretch: only the fast tail sets the offset.
        busy = clock_samples(offset_ns=OFFSET + 55 * MS, rtt_us=110_000, count=15)
        start = busy[-1].t_perf_us + 110_000
        quiet = [ClockSample(OFFSET + (start + i * 1000) * 1000 + 500_000, 15 + i, start + i * 1000) for i in range(8)]
        result = g5.estimate_offset(busy + quiet)
        self.assertEqual(result.offset_ns, OFFSET)
        self.assertEqual(result.uncertainty_ns, 500_000 + 1 * MS)

    def test_too_few_fast_pairs_fail(self):
        busy = clock_samples(rtt_us=3000, count=12)
        start = busy[-1].t_perf_us + 1000
        fast = [ClockSample(OFFSET + (start + i * 1000) * 1000, 12 + i, start + i * 1000) for i in range(3)]
        with self.assertRaisesRegex(CaptureError, "^CLOCK_SYNC_FAILED: 3 clock sample pairs within 1 ms"):
            g5.estimate_offset(busy + fast)

    def test_samples_are_used_in_sequence_order(self):
        self.assertEqual(g5.estimate_offset(list(reversed(clock_samples()))).offset_ns, OFFSET)

    def test_a_best_round_trip_over_the_limit_fails(self):
        with self.assertRaisesRegex(CaptureError, "^CLOCK_SYNC_FAILED: best round trip 4.50 ms"):
            g5.estimate_offset(clock_samples(rtt_us=4500))

    def test_the_limit_itself_passes(self):
        g5.estimate_offset(clock_samples(rtt_us=4000))

    def test_too_few_pairs_fail(self):
        with self.assertRaisesRegex(CaptureError, "^CLOCK_SYNC_FAILED: 4 usable"):
            g5.estimate_offset(clock_samples(count=5))
        with self.assertRaisesRegex(CaptureError, "^CLOCK_SYNC_FAILED: 0 usable"):
            g5.estimate_offset([])

    def test_gaps_in_the_sequence_do_not_form_pairs(self):
        samples = [s for s in clock_samples(count=22) if s.seq % 2 == 0]
        with self.assertRaisesRegex(CaptureError, "^CLOCK_SYNC_FAILED: 0 usable"):
            g5.estimate_offset(samples)

    def test_backwards_stamps_fail(self):
        samples = clock_samples()
        samples[7] = ClockSample(samples[7].ns, 7, samples[6].t_perf_us - 1)
        with self.assertRaisesRegex(CaptureError, "^CLOCK_SYNC_FAILED: performance.now"):
            g5.estimate_offset(samples)


class MarkTimeoutTests(unittest.TestCase):
    def test_scaled_for_flatpak_and_raised_by_a_larger_timeout(self):
        self.assertEqual(g5.mark_timeout_s(60, False), 10)
        self.assertEqual(g5.mark_timeout_s(30, False), 5)
        self.assertEqual(g5.mark_timeout_s(5, True), 20)
        self.assertEqual(g5.mark_timeout_s(240, True), 40)
        self.assertEqual(g5.mark_timeout_s(600, True), 100)


class TrialTests(G5TestCase):
    def test_trial_order_is_arm_quiet_focus_key_render(self):
        self.drive(trials=2)
        self.assertEqual([item[0] for item in self.world.trace],
                         ["place", "arm", "begin", "armed", "key", "render", "armed", "key", "render", "check"])

    def test_window_is_placed_and_armed_before_the_first_mark_is_awaited(self):
        self.drive(trials=1)
        self.assertEqual(self.world.trace[:3], [("place", 1280, 900, 640, 450), ("arm",), ("begin",)])

    def test_search_alternates_a_and_backspace(self):
        self.drive("search", trials=4)
        self.assertEqual(self.key_trace(), [bi.LETTER_KEYS["q"], bi.KEY_BACKSPACE] * 2)

    def test_dpad_alternates_right_and_left(self):
        self.drive("dpad", trials=4)
        self.assertEqual(self.key_trace(), [bi.KEY_RIGHT, bi.KEY_LEFT] * 2)

    def test_enter_scenarios_press_enter_every_trial(self):
        for scenario in ("toggle_favorite", "toggle_optimization", "route_switch"):
            with self.subTest(scenario=scenario):
                self.world.trace.clear()
                self.world.marks.clear()
                self.drive(scenario, trials=2)
                self.assertEqual(self.key_trace(), [bi.KEY_ENTER] * 2)

    def test_every_scenario_has_an_allowlisted_key(self):
        for scenario, keys in g5.G5_KEYS.items():
            self.assertTrue(set(keys) <= bi.ALLOWED_KEYS, scenario)

    def test_unknown_scenario_is_refused_before_anything_happens(self):
        for scenario in ("library", "", "x" * 50):
            with self.assertRaisesRegex(CaptureError, "^UNKNOWN_SCENARIO: "):
                self.drive(scenario)
        self.assertEqual(self.world.trace, [])

    def test_a_measured_trial_carries_the_selected_frame_and_the_press_to_photon_latency(self):
        result = self.drive(trials=1)
        (record,) = result.records
        self.assertEqual(record.trial.status, MEASURED)
        inject = record.inject_ns
        self.assertEqual(record.trial.commit_ns, inject + 40 * MS)
        self.assertEqual(record.trial.presented_ns, inject + 60 * MS)
        self.assertEqual(record.trial.latency_ms, 60.0)
        self.assertEqual(record.render_ns, inject + 30 * MS)  # the mark mapped onto CLOCK_MONOTONIC
        self.assertEqual(result.clock.offset_ns, OFFSET)

    def test_the_clock_offset_moves_selection_but_never_the_duration(self):
        shifted = []
        for shift_ms in (0, 8, -8):
            self.setUp()
            self.world.samples = clock_samples(OFFSET + shift_ms * MS)
            (record,) = self.drive(trials=1).records
            shifted.append(record)
            self.assertEqual(record.trial.status, MEASURED)
            self.assertEqual(record.trial.latency_ms, 60.0)  # presented - inject, whatever the offset
            self.assertEqual(record.render_ns, record.inject_ns + (30 + shift_ms) * MS)
        # A large shift puts the render mark after the change frame: another selection, never another sum.
        self.setUp()
        self.world.samples = clock_samples(OFFSET + 60 * MS)
        (late,) = self.drive(trials=1).records
        self.assertNotEqual(late.trial.status, MEASURED)
        self.assertIsNone(late.trial.latency_ms)

    def test_a_commit_inside_the_render_window_is_ambiguous_and_claims_no_latency(self):
        self.world.commit_delay_ns = self.world.render_delay_ns  # lands on the mark
        (record,) = self.drive(trials=1).records
        self.assertEqual(record.trial.status, AMBIGUOUS)
        self.assertIsNone(record.trial.latency_ms)

    def test_no_frame_before_the_deadline_is_a_timeout_trial_not_an_error(self):
        self.world.suppress_commit = True
        (record,) = self.drive(trials=1, mark_timeout=2.0).records
        self.assertEqual(record.trial.status, TIMEOUT)
        self.assertGreaterEqual(self.world.now, record.inject_ns + 2_000_000_000)

    def test_a_render_mark_before_the_key_is_an_early_trial_that_is_not_measured(self):
        self.world.render_delay_ns = -20 * MS  # the mark lands 20 ms before the key, beyond the 2 ms uncertainty
        (record,) = self.drive(trials=1).records
        self.assertEqual(record.trial.status, EARLY)
        self.assertIsNone(record.trial.latency_ms)
        self.assertIsNone(record.trial.presented_ns)
        self.assertEqual(record.render_ns, record.inject_ns - 20 * MS)

    def test_a_render_mark_within_the_uncertainty_before_the_key_is_still_selected(self):
        self.world.render_delay_ns = -1 * MS  # inside the 2 ms uncertainty (1 ms floor + 1 ms half round trip)
        (record,) = self.drive(trials=1).records
        self.assertNotEqual(record.trial.status, EARLY)

    def test_early_trials_do_not_stop_the_run_and_the_notice_is_printed_once(self):
        self.world.render_delay_ns = -20 * MS
        result = self.drive(trials=4)
        self.assertEqual([record.trial.status for record in result.records], [EARLY] * 4)
        self.assertEqual(len(self.key_trace()), 4)
        notice = sys.stderr.getvalue()
        self.assertEqual(notice.count("BENCH_NOTICE: G5 search seq 0"), 1)
        self.assertIn("`early`", notice)

    def test_the_run_ends_by_surfacing_a_recorded_event_error(self):
        events = FakeEvents(self.world)
        events.check = lambda: (_ for _ in ()).throw(CaptureError("BENCH_EVENT_MALFORMED: BENCH_MARK x"))
        with self.assertRaisesRegex(CaptureError, "^BENCH_EVENT_MALFORMED"):
            self.drive(trials=1, events=events)


class QuietGateTests(G5TestCase):
    def test_the_key_waits_for_a_quiet_toplevel_of_at_least_250_ms(self):
        self.world.busy_gaps_ns = [100 * MS] * 5  # commits until arm + 500 ms
        armed_at = self.world.now
        (record,) = self.drive(trials=1).records
        self.assertGreaterEqual(record.inject_ns, armed_at + 500 * MS + g5.QUIET_MIN_NS)

    def test_the_gate_is_twice_the_p50_frame_when_that_is_longer(self):
        self.world.busy_gaps_ns = [300 * MS] * 4  # p50 frame 300 ms: 600 ms of quiet
        armed_at = self.world.now
        (record,) = self.drive(trials=1).records
        self.assertGreaterEqual(record.inject_ns, armed_at + 1200 * MS + 600 * MS)

    def test_an_already_quiet_window_is_not_delayed(self):
        (record,) = self.drive(trials=1).records
        self.assertLess(record.inject_ns - START, 5 * MS)

    def test_a_window_that_never_goes_quiet_fails(self):
        self.world.busy_gaps_ns = [100 * MS] * 200  # 20 s of commits
        with self.assertRaisesRegex(CaptureError, "^NOT_QUIET: .* before G5 trial 0"):
            self.drive(trials=1)
        self.assertEqual(self.key_trace(), [])


    def test_the_quiet_wait_scales_with_the_mark_timeout(self):
        self.world.busy_gaps_ns = [100 * MS] * 150  # 15 s of commits: past the 10 s floor, inside 2 x 10 s
        (record,) = self.drive(trials=1, mark_timeout=10.0).records
        self.assertGreater(record.inject_ns - START, 15_000 * MS)

class WitnessTests(G5TestCase):
    def test_no_key_without_keyboard_focus_on_the_toplevel(self):
        self.world.keyboard_enter = False
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: keyboard focus is not on the app window"):
            self.drive()
        self.assertEqual(self.key_trace(), [])

    def test_keyboard_focus_lost_mid_run_stops_before_the_next_key(self):
        self.world.keyboard_leaves_after_keys = 2
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: keyboard focus is not on the app window"):
            self.drive(trials=6)
        self.assertEqual(len(self.key_trace()), 2)

    def test_a_focus_lost_mark_stops_before_the_next_key(self):
        self.world.focus_lost_at = START
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: the app reported a lost window focus"):
            self.drive()
        self.assertEqual(self.key_trace(), [])

    def test_a_locked_session_stops_before_the_next_key(self):
        self.world.locked = True
        with self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED"):
            self.drive()
        self.assertEqual(self.key_trace(), [])

    def test_an_exited_app_stops_before_the_next_key(self):
        self.world.exit_code = 139
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT: the app exited with status 139"):
            self.drive()
        self.assertEqual(self.key_trace(), [])

    def test_armed_never_sent_is_a_mark_timeout(self):
        self.world.no_armed_seq = 0
        with self.assertRaisesRegex(CaptureError, "^MARK_TIMEOUT: search seq 0 armed not seen within 31 s"):
            self.drive()  # the first arm waits mark_timeout + 30 s: BENCH_BEGIN precedes prepare()
        self.assertEqual(self.key_trace(), [])

    def test_later_armed_waits_use_the_plain_mark_timeout(self):
        self.world.no_armed_seq = 1
        with self.assertRaisesRegex(CaptureError, "^MARK_TIMEOUT: search seq 1 armed not seen within 1 s"):
            self.drive(trials=2)
        self.assertEqual(len(self.key_trace()), 1)

    def test_the_first_armed_wait_is_budgeted_for_prepare_in_slices(self):
        slices = []
        events = FakeEvents(self.world)
        wait = events.wait_mark

        def counting(scenario, seq, phase, timeout_s):
            if phase == "armed":
                slices.append(seq)
            return wait(scenario, seq, phase, timeout_s)

        events.wait_mark = counting
        self.world.no_armed_seq = 0
        with self.assertRaises(CaptureError):
            self.drive(events=events, mark_timeout=1.0)
        self.assertEqual(len(slices), int((1.0 + g5.FIRST_ARMED_EXTRA_S) / g5.MARK_SLICE_S))

    def test_render_never_sent_is_a_mark_timeout_naming_the_trial(self):
        self.world.no_render = True
        with self.assertRaisesRegex(CaptureError, "^MARK_TIMEOUT: search seq 0 render not seen"):
            self.drive()
        self.assertEqual(len(self.key_trace()), 1)

    def test_a_mark_wait_is_sliced_so_a_lost_focus_ends_it_early(self):
        self.world.no_armed_seq = 0
        self.world.focus_lost_at = START
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: the app reported a lost window focus"):
            self.drive(mark_timeout=60)

    def test_clock_sync_failure_sends_no_key(self):
        self.world.samples = clock_samples(rtt_us=9000)
        with self.assertRaisesRegex(CaptureError, "^CLOCK_SYNC_FAILED"):
            self.drive()
        self.assertEqual(self.key_trace(), [])


if __name__ == "__main__":
    unittest.main()
