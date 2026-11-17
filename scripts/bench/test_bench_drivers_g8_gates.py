"""G8 idle gates (the 3 s settle gate, the 1 s baseline gate, `_wait_idle`'s idle time) and driver helper
parameters; the G8 fakes come from test_bench_drivers_g8."""
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_drivers as bd
import bench_drivers_g8 as g8
from bench_capture import CaptureError
from test_bench_drivers_g8 import MS, START, WIN, FakeProc, G8TestCase


class SettleGateTests(G8TestCase):
    def test_the_settle_gate_needs_three_seconds_without_a_frame(self):
        self.assertEqual(g8.SETTLE_IDLE_S, 3.0)
        w = self.world
        w.gap_frames_left = 3  # frames 2 s apart after `done`: a 1 s gate would call the window idle between them
        result = self.drive()
        self.assertEqual(w.gap_frames_left, 0)
        self.assertEqual(result.settled_ns, w.last_frame_ns)  # the last of the three gapped frames
        self.assertGreaterEqual(w.now - w.last_frame_ns, 3 * 10**9)
        self.assertEqual(self.sampler.phases, ["baseline", "replay", "settle", "tail"])

    def test_the_baseline_gate_keeps_one_second_of_quiet(self):
        self.world.frame_at(START - 2_000_000_000)  # the last frame is 2 s old: idle at 1 s, not at 3 s
        calls = []
        real = g8._wait_idle

        def spy(*args, **kwargs):
            calls.append((args[5:], kwargs))
            return real(*args, **kwargs)
        with patch.object(g8, "_wait_idle", spy):
            self.drive()
        self.assertEqual(calls[0], (("BASELINE_NOT_IDLE", bd.IDLE_S), {}))
        self.assertEqual(calls[1][1].get("idle_s"), g8.SETTLE_IDLE_S)

    def test_a_window_that_never_goes_quiet_for_three_seconds_does_not_settle(self):
        self.world.gap_frames_left = 10**6
        self.world.frame_gap_ns = 2_000_000_000
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_SETTLED: the console kept presenting for 25 s"):
            self.drive()
        self.assert_clean_exit()


class WaitIdleTests(G8TestCase):
    def test_idle_after_one_second_by_default_as_for_g4(self):
        w = self.world
        self.assertEqual(bd.IDLE_S, 1.0)
        w.frame_at(START - 2_000_000_000)
        last = bd._wait_idle(self.proc, self.log_dir, w.clock, w.sleep)
        self.assertEqual((last, w.now), (START - 2_000_000_000, START))

    def test_a_longer_idle_time_keeps_waiting_for_the_frame_to_age(self):
        w = self.world
        w.frame_at(START - 2_000_000_000)
        last = bd._wait_idle(self.proc, self.log_dir, w.clock, w.sleep, idle_s=3.0)
        self.assertEqual(last, START - 2_000_000_000)
        self.assertGreaterEqual(w.now - last, 3 * 10**9)
        self.assertLess(w.now - last, 3 * 10**9 + 400 * MS)

    def test_a_window_that_keeps_presenting_times_out_whatever_the_idle_time(self):
        w = self.world
        w.keep_presenting = True
        w.frame_at(START)
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_SETTLED: the console kept presenting for 2 s"):
            bd._wait_idle(self.proc, self.log_dir, w.clock, w.sleep, 2.0, idle_s=3.0)


class HelperParameterTests(unittest.TestCase):
    def test_replay_bound_is_four_times_nominal_plus_thirty_seconds(self):
        self.assertEqual(g8.replay_bound_s(50000), 70.0)

    def test_the_g4_window_helper_keeps_its_default_size(self):
        placed = []

        class Kwin:
            def place(self, win, width, height, dx, dy):
                placed.append((width, height, dx, dy))
                return win

        with patch.object(bd, "find_window", lambda pids, *a: WIN), \
                patch.object(bd, "process_tree", lambda pid: {pid}):
            bd._window(FakeProc.__new__(FakeProc), Kwin(), 640, 450)
            bd._window(FakeProc.__new__(FakeProc), Kwin(), 800, 860, 1600, 1000)
        self.assertEqual(placed, [(1280, 900, 640, 450), (1600, 1000, 800, 860)])


if __name__ == "__main__":
    unittest.main()
