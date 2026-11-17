"""G8 driver failure paths: every way a session can stop, over the fakes of `test_bench_drivers_g8`."""
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bench_capture import CaptureError
from bench_memory import MemoryBenchError
from test_bench_drivers_g8 import START, FakeDev, FakeProc, G8TestCase


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
