"""G4 pointer witness: arming before the priming notch, an incomplete or stalled shim log, `_last_frame_ns`."""
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_drivers as bd
from bench_capture import CaptureError
from test_bench_drivers import WIN, DriverTestCase


class G4ArmTests(DriverTestCase):
    def test_the_window_is_armed_once_before_the_priming_notch(self):
        self.g4()
        self.assertEqual(self.kwin.armed, [(WIN, 0, 0)])  # the placed window, no input sent yet

    def test_arm_runs_after_the_settings_keys_and_the_idle_wait(self):
        self.g4("settings")
        self.assertEqual(self.kwin.armed, [(WIN, 0, 10)])  # Ctrl+K, eight letters and Enter came first
        self.world.events.clear()
        self.kwin.armed.clear()
        self.world.wheels = 0
        self.last_frame = lambda: self.world.now  # the console replay is still presenting
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_SETTLED"):
            self.g4("console")
        self.assertEqual(self.kwin.armed, [])  # never reached the priming notch

    def test_an_unarmed_window_sends_no_wheel_at_all(self):
        self.kwin.arm_error = CaptureError(
            "POINTER_NOT_OVER_WINDOW: the bench window is not active under the cursor")
        with self.assertRaisesRegex(CaptureError, "^POINTER_NOT_OVER_WINDOW: the bench window is not active"):
            self.g4()
        self.assertEqual(self.wheels(), [])

    def test_a_locked_session_or_exited_app_is_refused_before_arming(self):
        self.world.locked = True
        with self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED"):
            self.g4()
        self.assertEqual(self.kwin.armed, [])

    def test_g9_never_arms(self):
        self.g9()
        self.assertEqual(self.kwin.armed, [])


class G4LogWitnessTests(DriverTestCase):
    def test_a_log_that_stopped_recording_ends_the_scroll_at_once(self):
        for reason in ("log_cap", "bind_failed", "wrap_failed", "no_presentation_global"):
            with self.subTest(reason=reason):
                self.world.events.clear()
                self.world.wheels = 0
                self.world.log_path.write_text("S\t1\t1\ttoplevel\n")
                original = self.dev.wheel

                def wheel(notches, original=original, reason=reason):
                    stamp = original(notches)
                    if self.world.wheels == 5:
                        self.world.append_log(f"X\t{self.world.now}\t{reason}\n")
                    return stamp
                self.dev.wheel = wheel
                with self.assertRaisesRegex(CaptureError, f"^PRESENT_LOG_INCOMPLETE: {reason}$"):
                    self.g4()
                self.assertEqual(len(self.wheels()), 5)  # the very next notch is never sent
                self.dev.wheel = original

    def test_a_condition_logged_before_the_priming_notch_is_refused(self):
        self.world.append_log("X\t1\tlog_cap\n")
        with self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_INCOMPLETE: log_cap"):
            self.g4()
        self.assertLessEqual(len(self.wheels()), 1)

    def test_a_log_that_stops_growing_mid_scroll_is_focus_lost(self):
        self.world.log_stops_after = 10
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: presentation log stopped growing$"):
            self.g4()
        notches = len(self.wheels())
        stall_notches = bd.LOG_STALL_S / bd.NOTCH_S
        self.assertGreaterEqual(notches, 10 + stall_notches - 2)
        self.assertLessEqual(notches, 10 + stall_notches + 3)

    def test_a_slow_but_growing_log_is_trusted(self):
        self.world.grow_every = 50  # one commit per 1.65 s, below the 2 s limit
        result = self.g4()
        self.assertEqual(result.injected, 2 * bd.SCROLL_NOTCHES)

    def test_idle_waits_before_the_scroll_are_not_stall_checked(self):
        busy_until = self.world.now + 8_000_000_000  # a console replay far longer than the stall limit
        self.last_frame = lambda: min(self.world.now, busy_until)
        self.world.grow_every = 1
        result = self.g4("console")
        self.assertEqual(result.injected, 2 * bd.SCROLL_NOTCHES)
        for scenario in ("library", "settings"):
            self.g4(scenario)  # the settings sequence and its pauses sleep over 1.5 s with no commits logged


class LastFrameNsTests(unittest.TestCase):
    """`_last_frame_ns` against real log files: only "no presented frame yet" means 0."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.dir = Path(self.temp.name)

    def write(self, text, name="wlpresent-1.log"):
        (self.dir / name).write_text(text)

    def test_no_log_or_no_presented_toplevel_frame_yet_is_zero(self):
        self.assertEqual(bd._last_frame_ns(self.dir), 0)
        self.write("H\tclock_id=1\nS\t1\t7\ttoplevel\nC\t5\t7\t1\t1\n")  # committed, not presented
        self.assertEqual(bd._last_frame_ns(self.dir), 0)

    def test_returns_the_last_presented_toplevel_frame_across_logs(self):
        self.write("S\t1\t7\ttoplevel\nC\t5\t7\t1\t1\nP\t6\t1\t600\t8\t1\t0\n")
        self.write("S\t1\t7\ttoplevel\nC\t5\t7\t1\t1\nP\t6\t1\t900\t8\t1\t0\n", "wlpresent-2.log")
        self.assertEqual(bd._last_frame_ns(self.dir), 900)

    def test_a_malformed_log_propagates(self):
        self.write("S\t1\t7\ttoplevel\nC\tnot-a-number\t7\t1\t1\n")
        with self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_MALFORMED: wlpresent-1.log:2"):
            bd._last_frame_ns(self.dir)

    def test_an_oversized_or_unreadable_log_propagates(self):
        self.write("S\t1\t7\ttoplevel\n")
        with patch("bench_present.MAX_LOG_BYTES", 4), self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_TOO_LARGE"):
            bd._last_frame_ns(self.dir)
        (self.dir / "wlpresent-9.log").symlink_to(self.dir / "wlpresent-1.log")
        with self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_UNREADABLE: wlpresent-9.log"):
            bd._last_frame_ns(self.dir)


if __name__ == "__main__":
    unittest.main()
