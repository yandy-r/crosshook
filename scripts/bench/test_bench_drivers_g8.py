"""G8 driver checks with a fake device, KWin, event reader, sampler, stdin and clock over a real shim-format log."""
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_drivers as bd
import bench_drivers_g8 as g8
from bench_capture import CaptureError
from bench_events import Begin, Mark, ReplayEvent
from bench_kwin import WindowInfo
from bench_memory import MemoryBenchError, MemoryRow

WIN = WindowInfo("{u}", 4242, 0.0, 0.0, 1600.0, 1000.0, False)
MS = 1_000_000
START = 10_000_000_000
LINES = 50000


class World:
    """One shared fake clock, the shim log the driver tails, and the app's side of the contract."""

    def __init__(self, directory: Path):
        self.now = START
        self.trace: list[tuple] = []
        self.locked = False
        self.exit_code = None
        self.phase_mark: str | None = "follow_on"   # the replay mark the probe sent (None: none yet)
        self.begin = True
        self.no_start = False
        self.done_at_start = False                  # `done` is already there when the scroll begins
        self.done_after_notches: int | None = 20    # notches during the replay before `done` (None: never)
        self.keep_presenting = False                # a frame is committed during every sleep
        self.presenting_after_done = False
        self.gap_frames_left = 0                    # frames committed after `done`, one per `frame_gap_ns`
        self.frame_gap_ns = 2_000_000_000
        self.stream_ends_after_notches: int | None = None   # the event reader reports the app's stderr closed
        self.stream_end_exit_code: int | None = None        # ... and the process exit it comes with (None: alive)
        self.pointer_leaves_at: int | None = None
        self.focus_lost_at: int | None = None
        self.stdin_broken = False
        self.go_ns: int | None = None
        self.done_ns: int | None = None
        self.wheels = 0
        self.replay_notches = 0
        self.log = directory / "wlpresent-1.log"
        self.log.write_text("H\tclock_id=1\nS\t1\t1\ttoplevel\n")
        self.frame = 0
        self.last_frame_ns = 0
        self.frame_at(START - 5_000_000_000)    # the app is idle when the driver starts

    def clock(self) -> int:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.now += int(seconds * 1e9)
        if self.keep_presenting or (self.presenting_after_done and self.done_ns is not None):
            self.frame_at(self.now)
        elif self.gap_frames_left and self.done_ns is not None and self.now - self.last_frame_ns >= self.frame_gap_ns:
            self.gap_frames_left -= 1
            self.frame_at(self.now)

    def append(self, text: str) -> None:
        with self.log.open("a") as stream:
            stream.write(text)

    def frame_at(self, ns: int) -> None:
        self.frame += 1
        self.last_frame_ns = max(self.last_frame_ns, ns)
        self.append(f"C\t{ns}\t1\t{self.frame}\t1\nP\t{ns}\t{self.frame}\t{ns}\t8333333\t1\t0\n")

    def set_pointer(self, entered: bool) -> None:
        self.append(f"M\t{self.now}\t{'enter' if entered else 'leave'}\t1\n")

    @property
    def replaying(self) -> bool:
        return self.go_ns is not None and self.done_ns is None

    def step(self, name: str, *args) -> None:
        self.trace.append((name, *args))

    def names(self) -> list[str]:
        return [item[0] for item in self.trace]


class FakeStdin:
    def __init__(self, world):
        self.world, self.closed, self.written = world, False, b""

    def write(self, data: bytes) -> None:
        if self.world.stdin_broken:
            raise BrokenPipeError(32, "Broken pipe")
        self.written += data
        self.world.go_ns = self.world.now
        self.world.step("go", data)

    def flush(self) -> None:
        self.world.step("flush")

    def close(self) -> None:
        self.closed = True


class FakeProc:
    pid = 4242

    def __init__(self, world, stdin=True):
        self.world = world
        self.stdin = FakeStdin(world) if stdin else None

    def poll(self):
        return self.world.exit_code


class FakeDev:
    def __init__(self, world):
        self.world = world

    def wheel(self, notches):
        w = self.world
        w.wheels += 1
        stamp = w.now
        w.step("wheel", notches)
        if w.wheels == 1:
            w.set_pointer(True)
        if w.replaying:
            w.replay_notches += 1
            w.frame_at(stamp)
            if w.pointer_leaves_at == w.replay_notches:
                w.set_pointer(False)
            if w.done_after_notches is not None and w.replay_notches >= w.done_after_notches:
                w.done_ns = stamp + 1
        return stamp


class FakeKwin:
    def __init__(self, world):
        self.world = world

    def place(self, win, width, height, dx, dy):
        self.world.step("place", width, height, dx, dy)
        return win

    def arm_under_cursor(self, win):
        self.world.step("arm")
        return win


class FakeEvents:
    def __init__(self, world):
        self.world = world

    def wait_begin(self, timeout_s):
        w = self.world
        if not w.begin:
            raise CaptureError("BEGIN_TIMEOUT")
        w.step("begin")
        return Begin(1, "replay_scroll")

    def replay_mark(self, scenario):
        w = self.world
        return None if w.phase_mark is None else Mark(1, scenario, 0, w.phase_mark, 0)

    def wait_replay(self, kind, timeout_s):
        w = self.world
        if kind == "start":
            if w.go_ns is None or w.no_start:
                raise CaptureError("REPLAY_NOT_STARTED")
            w.step("replay_start")
            if w.done_at_start:
                w.done_ns = w.go_ns + 6 * MS
            return ReplayEvent(w.go_ns + 5 * MS, LINES)
        if w.stream_ends_after_notches is not None and w.replay_notches >= w.stream_ends_after_notches:
            w.exit_code = w.stream_end_exit_code
            raise CaptureError("EVENT_STREAM_ENDED: the app's stderr closed while waiting for BENCH_REPLAY done")
        if w.done_ns is None:
            raise CaptureError("REPLAY_NOT_DONE")
        return ReplayEvent(w.done_ns, LINES)

    def focus_lost(self, scenario, since_ns):
        w = self.world
        if w.focus_lost_at is not None and w.replay_notches >= w.focus_lost_at:
            return Mark(w.now, scenario, 0, "focus_lost", 0)
        return None

    def check(self):
        self.world.step("check")


class FakeSampler:
    def __init__(self, world):
        self.world, self.phases, self.started, self.stops = world, [], False, 0
        self.exit_on_phase: str | None = None
        self.stop_error: MemoryBenchError | None = None

    def set_phase(self, name):
        self.phases.append(name)
        self.world.step("phase", name)
        if name == self.exit_on_phase:
            self.world.exit_code = 1

    def start(self):
        self.started = True
        self.world.step("sampler_start")

    def stop(self):
        self.stops += 1
        self.world.step("sampler_stop")
        if self.stop_error is not None:
            raise self.stop_error
        return [MemoryRow(START, phase, 100.0, 80.0, None, "unavailable") for phase in self.phases]


class G8TestCase(unittest.TestCase):
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
        self.proc = FakeProc(self.world)
        self.sampler = FakeSampler(self.world)

    def drive(self, mark_timeout=1.0, flatpak=False, proc=None):
        w = self.world
        return g8.drive_g8(proc or self.proc, self.log_dir, FakeKwin(w), FakeDev(w), FakeEvents(w),
                           self.sampler, mark_timeout, flatpak, w.clock, w.sleep)

    def assert_clean_exit(self, proc=None):
        proc = proc or self.proc
        if proc.stdin is not None:
            self.assertTrue(proc.stdin.closed)
        self.assertGreaterEqual(self.sampler.stops, 1 if self.sampler.started else 0)


class SequenceTests(G8TestCase):
    def test_steps_run_in_order_and_go_is_written_once_after_the_priming_notch(self):
        self.drive()
        names = self.world.names()
        order = ["place", "begin", "phase", "sampler_start", "arm", "wheel", "go", "flush", "replay_start"]
        positions = [names.index(name) for name in order]
        self.assertEqual(positions, sorted(positions))
        self.assertEqual(names.count("go"), 1)
        self.assertEqual(self.proc.stdin.written, b"GO\n")
        self.assertEqual(names[names.index("wheel") + 1], "go")  # the priming notch is the only wheel before GO

    def test_the_window_is_placed_at_1600_by_1000_with_the_cursor_over_the_console(self):
        self.drive()
        self.assertIn(("place", 1600, 1000, 800, 860), self.world.trace)
        self.assertEqual((g8.WINDOW_W, g8.WINDOW_H, g8.CURSOR_DX, g8.CURSOR_DY), (1600, 1000, 800, 860))

    def test_phases_are_tagged_in_order(self):
        self.drive()
        self.assertEqual(self.sampler.phases, ["baseline", "replay", "settle", "tail"])
        names = self.world.names()
        self.assertLess(names.index("sampler_start"), names.index("go"))

    def test_phase_changes_follow_the_replay_markers(self):
        self.drive()
        trace = self.world.trace
        self.assertLess(trace.index(("replay_start",)), trace.index(("phase", "replay")))
        self.assertLess(trace.index(("phase", "replay")), trace.index(("wheel", -1)))
        self.assertLess(trace.index(("phase", "settle")), trace.index(("phase", "tail")))
        self.assertLess(trace.index(("phase", "tail")), trace.index(("sampler_stop",)))

    def test_result_carries_the_marks_the_collector_needs(self):
        result = self.drive()
        w = self.world
        self.assertEqual(result.follow, "on")
        self.assertEqual(result.notches, 20)
        self.assertEqual(result.start_ns, w.go_ns + 5 * MS)
        self.assertEqual(result.done_ns, w.done_ns)
        self.assertGreater(result.done_ns, result.start_ns)
        self.assertGreaterEqual(result.settled_ns, result.done_ns)
        self.assertEqual((result.window_start_ns, result.window_end_ns),
                         (result.start_ns, result.done_ns + 300 * MS))
        self.assertEqual(result.driven_ns, 19 * 33 * MS)
        self.assertEqual([row.phase for row in result.rows], ["baseline", "replay", "settle", "tail"])
        self.assertTrue(self.proc.stdin.closed)

    def test_scroll_follows_the_g4_pattern(self):
        self.world.done_after_notches = 25
        self.drive()
        notches = [item[1] for item in self.world.trace if item[0] == "wheel"][1:]  # after the priming notch
        self.assertEqual(notches, [-1] * 10 + [1] * 10 + [-1] * 5)

    def test_follow_off_is_recorded(self):
        self.world.phase_mark = "follow_off"
        self.assertEqual(self.drive().follow, "off")

    def test_the_baseline_and_tail_hold_three_seconds_each(self):
        self.drive()
        self.assertEqual((g8.BASELINE_S, g8.TAIL_S), (3.0, 3.0))
        self.assertGreaterEqual(self.world.now - START, 6 * 10**9)

    def test_begin_waits_scale_with_the_mark_timeout(self):
        with patch.object(g8, "BEGIN_EXTRA_S", 0.5):
            self.world.begin = False
            with self.assertRaisesRegex(CaptureError, r"^BEGIN_TIMEOUT: not seen within 1\.5 s"):
                self.drive(mark_timeout=1.0)


class ConsoleVisibilityTests(G8TestCase):
    def test_a_hidden_console_stops_the_run_before_go(self):
        self.world.phase_mark = "console_hidden"
        with self.assertRaisesRegex(CaptureError, "^CONSOLE_NOT_VISIBLE: the console log is not visible "
                                    r"\(drawer collapsed or status-bar layout\)"):
            self.drive()
        self.assertNotIn("go", self.world.names())
        self.assertEqual(self.proc.stdin.written, b"")
        self.assertNotIn("sampler_start", self.world.names())
        self.assert_clean_exit()

    def test_a_hidden_console_is_reported_without_waiting_for_a_begin_that_never_comes(self):
        self.world.phase_mark = "console_hidden"
        self.world.begin = False   # the probe sends console_hidden and stops: no bench_begin
        with self.assertRaisesRegex(CaptureError, "^CONSOLE_NOT_VISIBLE"):
            self.drive(mark_timeout=100.0)
        self.assertNotIn("go", self.world.names())

    def test_a_begin_without_a_follow_mark_fails(self):
        self.world.phase_mark = None
        with self.assertRaisesRegex(CaptureError, "^REPLAY_MARK_MISSING"):
            self.drive()
        self.assertNotIn("go", self.world.names())


class FailureTests(G8TestCase):
    def test_no_begin_times_out(self):
        self.world.begin = False
        with self.assertRaisesRegex(CaptureError, "^BEGIN_TIMEOUT"):
            self.drive()
        self.assert_clean_exit()

    def test_app_exit_before_go_is_reported_and_nothing_is_written(self):
        self.sampler.exit_on_phase = "baseline"
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT: the app exited with status 1"):
            self.drive()
        self.assertNotIn("go", self.world.names())
        self.assertEqual(self.proc.stdin.written, b"")
        self.assert_clean_exit()

    def test_app_exit_while_waiting_for_begin_is_reported(self):
        self.world.begin = False
        self.world.exit_code = 3
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT: the app exited with status 3"):
            self.drive()
        self.assertTrue(self.proc.stdin.closed)

    def test_a_closed_stdin_at_go_is_an_app_exit(self):
        self.world.stdin_broken = True
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT: the app closed its stdin before the GO signal"):
            self.drive()
        self.assert_clean_exit()

    def test_a_missing_stdin_pipe_fails_before_the_replay(self):
        proc = FakeProc(self.world, stdin=False)
        with self.assertRaisesRegex(CaptureError, "^GO_STDIN_MISSING"):
            self.drive(proc=proc)
        self.assertNotIn("replay_start", self.world.names())

    def test_a_locked_session_stops_the_run_before_go(self):
        self.world.locked = True
        with self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED"):
            self.drive()
        self.assertNotIn("go", self.world.names())
        self.assertTrue(self.proc.stdin.closed)

    def test_pointer_never_entering_the_window_fails_before_go(self):
        with patch.object(FakeDev, "wheel", lambda self, notches: self.world.now), \
                self.assertRaisesRegex(CaptureError, "^POINTER_NOT_OVER_WINDOW"):
            self.drive()
        self.assertNotIn("go", self.world.names())
        self.assert_clean_exit()

    def test_the_baseline_gate_fails_when_the_app_keeps_presenting(self):
        self.world.keep_presenting = True
        self.world.frame_at(START)   # the app is presenting when the driver starts
        with self.assertRaisesRegex(CaptureError, "^BASELINE_NOT_IDLE: the console kept presenting for 25 s"):
            self.drive()
        self.assertNotIn("go", self.world.names())
        self.assertGreaterEqual(self.world.now - START, 25 * 10**9)

    def test_the_idle_bounds_are_four_times_longer_for_flatpak(self):
        self.world.keep_presenting = True
        self.world.frame_at(START)   # the app is presenting when the driver starts
        with self.assertRaisesRegex(CaptureError, "^BASELINE_NOT_IDLE: the console kept presenting for 100 s"):
            self.drive(flatpak=True)
        self.assertGreaterEqual(self.world.now - START, 100 * 10**9)

    def test_replay_not_started(self):
        self.world.no_start = True
        with self.assertRaisesRegex(CaptureError, r"^REPLAY_NOT_STARTED: not seen within 1 s"):
            self.drive()
        self.assertEqual(self.proc.stdin.written, b"GO\n")
        self.assert_clean_exit()

    def test_replay_not_done_within_the_bound(self):
        self.world.done_after_notches = None
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_DONE: no replay done within 70 s"):
            self.drive()
        self.assertGreaterEqual(self.world.now - START, 70 * 10**9)
        self.assert_clean_exit()

    def test_replay_not_settled(self):
        self.world.presenting_after_done = True
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_SETTLED: the console kept presenting for 25 s"):
            self.drive()
        self.assertEqual(self.sampler.phases, ["baseline", "replay", "settle"])
        self.assert_clean_exit()

    def test_pointer_focus_loss_during_the_replay(self):
        self.world.pointer_leaves_at = 5
        self.world.done_after_notches = 50
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: pointer focus is not on the app window "
                                    "during the replay"):
            self.drive()
        self.assertEqual(self.world.replay_notches, 5)   # the next notch is never sent
        self.assert_clean_exit()

    def test_a_focus_lost_mark_from_the_app_stops_the_scroll(self):
        self.world.focus_lost_at = 3
        self.world.done_after_notches = 50
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: the app reported a lost window focus"):
            self.drive()
        self.assertEqual(self.world.replay_notches, 3)

    def test_app_exit_during_the_replay(self):
        self.world.done_after_notches = 50
        real_wheel = FakeDev.wheel

        def wheel(dev, notches):
            stamp = real_wheel(dev, notches)
            if dev.world.replay_notches == 4:
                dev.world.exit_code = -9
            return stamp

        with patch.object(FakeDev, "wheel", wheel), \
                self.assertRaisesRegex(CaptureError, "^APP_EXIT: the app exited with status -9 during the replay"):
            self.drive()
        self.assert_clean_exit()

    def test_an_ended_event_stream_is_the_apps_exit_when_the_process_confirms_it(self):
        self.world.done_after_notches = None
        self.world.stream_ends_after_notches, self.world.stream_end_exit_code = 4, -11
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT: the app exited with status -11 during the replay"):
            self.drive()
        self.assertEqual(self.world.replay_notches, 4)
        self.assert_clean_exit()

    def test_an_ended_event_stream_with_the_app_still_running_is_reported_as_it_is(self):
        self.world.done_after_notches = None
        self.world.stream_ends_after_notches = 4
        with self.assertRaisesRegex(CaptureError, "^EVENT_STREAM_ENDED: the app's stderr closed"):
            self.drive()
        self.assert_clean_exit()

    def test_a_replay_that_ends_before_the_first_notch_is_not_a_measurement(self):
        self.world.done_at_start = True
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_SCROLLED"):
            self.drive()
        self.assert_clean_exit()

    def test_a_sampler_failure_is_a_capture_error_with_its_code(self):
        self.sampler.stop_error = MemoryBenchError("MEMORY_SAMPLES_EXCEEDED: more than 5 samples")
        with self.assertRaisesRegex(CaptureError, "^MEMORY_SAMPLES_EXCEEDED"):
            self.drive()
        self.assertTrue(self.proc.stdin.closed)

    def test_the_sampler_is_stopped_when_the_scroll_fails(self):
        self.world.pointer_leaves_at = 2
        self.world.done_after_notches = 50
        with self.assertRaises(CaptureError):
            self.drive()
        self.assertGreaterEqual(self.sampler.stops, 1)


if __name__ == "__main__":
    unittest.main()
