"""G4/G9 driver checks with a fake device, KWin, clock and shim log; stdlib only."""
import io
import os
import stat
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_drivers as bd
import bench_input as bi
from bench_capture import CaptureError
from bench_kwin import WindowInfo
from bench_present import Focus, Geometry, PresentLog

WIN = WindowInfo("{u}", 4242, 0.0, 0.0, 1280.0, 900.0, False)


class World:
    """One shared fake clock plus the shim log state the drivers observe."""

    def __init__(self):
        self.now = 1_000_000_000
        self.focus: list[Focus] = []
        self.geometry: list[Geometry] = []
        self.sleeps: list[float] = []
        self.events: list[tuple] = []
        self.locked = False
        self.exit_code = None
        self.pointer_on_notch: int | None = 1   # pointer enter arrives with this wheel call
        self.pointer_leaves_at: int | None = None
        self.keyboard_enter = True
        self.keyboard_leaves_after_letters: int | None = None
        self.lock_calls: list[int] = []
        self.log_path: Path | None = None
        self.wheels = 0

    def clock(self) -> int:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.sleeps.append(seconds)
        self.now += int(seconds * 1e9)

    def set_focus(self, device: str, entered: bool) -> None:
        self.focus.append(Focus(self.now, device, entered, 1, True))
        if self.log_path is not None:  # the drivers read focus from the shim log on disk
            with self.log_path.open("a") as stream:
                stream.write(f"{'K' if device == 'keyboard' else 'M'}\t{self.now}\t"
                             f"{'enter' if entered else 'leave'}\t1\n")

    def log(self) -> PresentLog:
        return PresentLog({}, {}, {}, {}, set(), [], list(self.focus), list(self.geometry))


class FakeProc:
    pid = 4242

    def __init__(self, world):
        self.world = world

    def poll(self):
        return self.world.exit_code


class FakeDev:
    def __init__(self, world):
        self.world = world

    def event_node(self):
        return "event9"

    def wheel(self, notches):
        w = self.world
        w.wheels += 1
        stamp = w.now
        w.events.append(("wheel", notches, stamp))
        if w.pointer_on_notch == w.wheels:
            w.set_focus("pointer", True)
        if w.pointer_leaves_at == w.wheels:
            w.set_focus("pointer", False)
        return stamp

    def _record(self, name, *args):
        self.world.events.append((name, *args))

    def chord(self, *codes):
        self._record("chord", codes)

    def key(self, code):
        self._record("key", code)

    def type_text(self, text, before_each=None):
        for index, char in enumerate(text):
            if before_each is not None:
                before_each()
            self._record("letter", char)
            if self.world.keyboard_leaves_after_letters == index + 1:
                self.world.set_focus("keyboard", False)


class FakeKwin:
    def __init__(self, world, show_resizes=True):
        self.world, self.show_resizes, self.placed, self.resizes = world, show_resizes, [], []

    step_cost_s = 0.0

    def place(self, win, width, height, dx, dy):
        self.placed.append((width, height, dx, dy))
        if self.world.keyboard_enter:
            self.world.set_focus("keyboard", True)
        return win

    def resize_unverified(self, win, width, height):
        self.resizes.append((width, height))
        self.world.now += int(self.step_cost_s * 1e9)
        if self.show_resizes:
            self.world.geometry.append(Geometry(self.world.now, 1, width, height, True, True))


class DriverTestCase(unittest.TestCase):
    def setUp(self):
        self.world = World()
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.log_dir = Path(self.temp.name)
        self.world.log_path = self.log_dir / "wlpresent-1.log"
        self.world.log_path.write_text("S\t1\t1\ttoplevel\n")
        bd._noticed = False
        stderr = patch.object(sys, "stderr", io.StringIO())
        stderr.start()
        self.addCleanup(stderr.stop)
        for target, value in (
            ("find_window", lambda pids, *a: WIN),
            ("process_tree", lambda pid: {pid, pid + 1}),
            ("session_locked", self.session_locked),
            ("parse_log", lambda path: self.world.log()),
            ("input_device_sysnames", lambda: "(<['event3', 'event9']>,)"),
            ("_last_frame_ns", lambda log_dir: self.last_frame()),
        ):
            patcher = patch.object(bd, target, value)
            patcher.start()
            self.addCleanup(patcher.stop)
        self.proc, self.dev, self.kwin = FakeProc(self.world), FakeDev(self.world), FakeKwin(self.world)
        self.last_frame = lambda: max(self.world.now - 2_000_000_000, 1)  # idle unless a test says otherwise

    def session_locked(self):
        self.world.lock_calls.append(self.world.now)
        return self.world.locked

    def g4(self, scenario="library"):
        return bd.drive_g4(self.proc, scenario, self.log_dir, self.kwin, self.dev,
                           self.world.clock, self.world.sleep)

    def g9(self):
        return bd.drive_g9(self.proc, self.log_dir, self.kwin, self.world.clock, self.world.sleep)

    def wheels(self):
        return [event for event in self.world.events if event[0] == "wheel"]


class G4ScrollTests(DriverTestCase):
    def test_notch_count_cadence_and_bounds_exclude_the_priming_notch(self):
        result = self.g4()
        priming, *timed = self.wheels()
        self.assertEqual(priming[1], 1)
        leg = [-1] * bd.SCROLL_LEG + [1] * bd.SCROLL_LEG
        self.assertEqual([event[1] for event in timed], leg * (bd.SCROLL_NOTCHES // bd.SCROLL_LEG))
        stamps = [event[2] for event in timed]
        gaps = {later - earlier for earlier, later in zip(stamps, stamps[1:])}
        self.assertEqual(gaps, {int(bd.NOTCH_S * 1e9)})
        self.assertEqual(result.start_ns, stamps[0])
        self.assertGreaterEqual(result.start_ns, priming[2])
        self.assertEqual(result.end_ns, stamps[-1] + bd.TAIL_NS)
        self.assertEqual(result.injected, 2 * bd.SCROLL_NOTCHES)
        self.assertGreaterEqual(self.world.now, result.end_ns)

    def test_window_is_placed_per_scenario(self):
        self.g4("library")
        self.g4("console")
        self.assertEqual(self.kwin.placed, [(1280, 900, 640, 450), (1280, 900, 640, 780)])

    def test_pointer_never_entering_raises_and_starts_no_timed_scroll(self):
        self.world.pointer_on_notch = None
        with self.assertRaisesRegex(CaptureError, "^POINTER_NOT_OVER_WINDOW"):
            self.g4()
        self.assertEqual(len(self.wheels()), 1)

    def test_focus_lost_after_any_notch_stops_the_very_next_one(self):
        for lost_after in (1, 7, 100):
            with self.subTest(lost_after=lost_after):
                self.world.events.clear()
                self.world.wheels = 0
                self.world.pointer_leaves_at = 1 + lost_after  # priming + timed notches
                with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST"):
                    self.g4()
                self.assertEqual(len(self.wheels()), 1 + lost_after)

    def test_session_lock_aborts_scroll(self):
        self.world.locked = False
        original = self.session_locked

        def locked_from_the_third_probe():
            original()
            return len(self.world.lock_calls) > 2

        patcher = patch.object(bd, "session_locked", locked_from_the_third_probe)
        patcher.start()
        self.addCleanup(patcher.stop)
        with self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED"):
            self.g4()
        self.assertLess(len(self.wheels()), 2 * bd.SCROLL_NOTCHES)

    def test_lock_probe_is_time_throttled_not_per_notch(self):
        self.g4()
        calls = self.world.lock_calls
        self.assertGreaterEqual(len(calls), 10)
        self.assertLessEqual(len(calls), 2 + int(2 * bd.SCROLL_NOTCHES * bd.NOTCH_S / bd.LOCK_CHECK_S) + 1)
        # the first two probes are the priming notch's and the first timed notch's; the rest are spaced
        for earlier, later in zip(calls[1:], calls[2:]):
            self.assertGreaterEqual(later - earlier, int(bd.LOCK_CHECK_S * 1e9))

    def test_locked_session_or_exited_app_stops_before_the_priming_notch(self):
        self.world.locked = True
        with self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED"):
            self.g4()
        self.assertEqual(self.wheels(), [])
        self.world.locked, self.world.exit_code = False, 1
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT"):
            self.g4()
        self.assertEqual(self.wheels(), [])

    def test_app_exit_aborts_scroll(self):
        original = self.dev.wheel

        def wheel(notches):
            stamp = original(notches)
            if self.world.wheels == 6:
                self.world.exit_code = -11
            return stamp

        self.dev.wheel = wheel
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT"):
            self.g4()
        self.assertEqual(len(self.wheels()), 6)

    def test_unknown_scenario_is_refused_before_any_input(self):
        with self.assertRaisesRegex(CaptureError, "^UNKNOWN_SCENARIO"):
            self.g4("nope")
        self.assertEqual(self.world.events, [])

    def test_g4_never_settles_the_device_itself(self):
        with patch.object(bd, "input_device_sysnames", side_effect=AssertionError("settled in drive_g4")):
            self.g4()
        self.assertEqual(self.world.sleeps.count(bd.DEVICE_POLL_S), 0)


class SettleDeviceTests(DriverTestCase):
    def settle(self):
        bd.settle_device(self.dev, self.world.clock, self.world.sleep)

    def test_returns_as_soon_as_kwin_lists_the_node(self):
        self.settle()
        self.assertEqual(self.world.sleeps, [])

    def test_polls_until_the_node_appears(self):
        replies = ["(<['event3']>,)"] * 3 + ["(<['event3', 'event9']>,)"]
        with patch.object(bd, "input_device_sysnames", side_effect=replies):
            self.settle()
        self.assertEqual(self.world.sleeps, [bd.DEVICE_POLL_S] * 3)

    def test_unlisted_device_fails_after_the_settle_timeout(self):
        with patch.object(bd, "input_device_sysnames", lambda: "(<['event3']>,)"):
            with self.assertRaisesRegex(
                    CaptureError, "^INPUT_NOT_DELIVERED: KWin did not register the bench input device$"):
                self.settle()
        self.assertEqual(self.world.sleeps.count(bd.DEVICE_POLL_S), round(bd.DEVICE_SETTLE_S / bd.DEVICE_POLL_S))

    def test_a_node_name_that_is_only_a_prefix_does_not_match(self):
        with patch.object(bd, "input_device_sysnames", lambda: "(<['event90']>,)"):
            with self.assertRaisesRegex(CaptureError, "^INPUT_NOT_DELIVERED"):
                self.settle()


class G4ConsoleIdleTests(DriverTestCase):
    def test_console_waits_for_the_replay_to_stop_presenting_before_scrolling(self):
        busy_until = self.world.now + 5_000_000_000
        self.last_frame = lambda: min(self.world.now, busy_until)
        result = self.g4("console")
        self.assertGreaterEqual(result.start_ns, busy_until + 1_000_000_000)

    def test_console_that_never_settles_fails_before_any_timed_notch(self):
        self.last_frame = lambda: self.world.now
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_SETTLED"):
            self.g4("console")
        self.assertEqual(self.wheels(), [])

    def test_library_does_not_wait_for_idle(self):
        self.last_frame = lambda: self.world.now
        self.g4("library")


class G4SettingsTests(DriverTestCase):
    def test_keys_follow_the_documented_sequence(self):
        self.g4("settings")
        keys = [event for event in self.world.events if event[0] != "wheel"]
        self.assertEqual(keys, [("chord", (bi.KEY_LEFTCTRL, bi.KEY_K)),
                                *(("letter", char) for char in "settings"), ("key", bi.KEY_ENTER)])
        self.assertEqual([event[0] for event in self.world.events[:3]], ["chord", "letter", "letter"])
        self.assertEqual(self.world.events[9][0], "key")
        self.assertEqual(self.world.events[10][0], "wheel")
        for pause in (bd.PALETTE_S, bd.TYPED_S, bd.ENTER_S):
            self.assertIn(pause, self.world.sleeps)

    def test_no_keys_are_sent_without_keyboard_focus(self):
        self.world.keyboard_enter = False
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST"):
            self.g4("settings")
        self.assertEqual(self.world.events, [])

    def test_keyboard_focus_lost_between_keys_stops_the_sequence(self):
        original = self.dev.chord

        def chord(*codes):
            original(*codes)
            self.world.set_focus("keyboard", False)

        self.dev.chord = chord
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST"):
            self.g4("settings")
        self.assertEqual([event[0] for event in self.world.events], ["chord"])

    def test_keyboard_focus_is_checked_before_every_letter(self):
        self.world.keyboard_leaves_after_letters = 3
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST: keyboard"):
            self.g4("settings")
        self.assertEqual([event[1] for event in self.world.events if event[0] == "letter"], list("set"))

    def test_app_exit_between_letters_stops_typing(self):
        original = self.dev.type_text

        def type_text(text, before_each=None):
            def hook():
                if len([e for e in self.world.events if e[0] == "letter"]) == 2:
                    self.world.exit_code = 1
                before_each()
            original(text, hook)

        self.dev.type_text = type_text
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT"):
            self.g4("settings")
        self.assertEqual(len([e for e in self.world.events if e[0] == "letter"]), 2)

    def test_non_settings_scenarios_never_send_keys(self):
        self.world.keyboard_enter = False
        self.g4("library")
        self.assertEqual({event[0] for event in self.world.events}, {"wheel"})


class G9Tests(DriverTestCase):
    def test_resizes_for_resize_s_inside_the_breakpoints(self):
        result = self.g9()
        self.assertEqual(self.kwin.placed, [(bd.WINDOW_W, bd.WINDOW_H, bd.WINDOW_W // 2, bd.WINDOW_H // 2)])
        self.assertAlmostEqual(result.injected, bd.RESIZE_S * bd.RESIZE_HZ, delta=1)
        self.assertEqual(len(self.kwin.resizes), result.injected)
        self.assertTrue(all(1100 < w < 1440 and 720 < h for w, h in self.kwin.resizes))
        self.assertGreaterEqual(len(set(self.kwin.resizes)), bd.MIN_DISTINCT_SIZES)
        self.assertAlmostEqual((result.end_ns - result.start_ns) / 1e9, bd.RESIZE_S + bd.TAIL_NS / 1e9, delta=0.2)

    def test_slow_kwin_is_bounded_by_wall_time_not_step_count(self):
        self.kwin.step_cost_s = 0.1  # every step is a slow round trip
        result = self.g9()
        self.assertLess(result.injected, bd.RESIZE_S * bd.RESIZE_HZ / 2)
        self.assertEqual(len(self.kwin.resizes), result.injected)
        self.assertAlmostEqual(result.driven_ns / 1e9, bd.RESIZE_S, delta=0.2)
        self.assertEqual(result.end_ns, result.start_ns + result.driven_ns + bd.TAIL_NS)

    def test_result_records_the_achieved_duration(self):
        result = self.g9()
        self.assertAlmostEqual(result.driven_ns / 1e9, bd.RESIZE_S, delta=0.1)
        self.assertAlmostEqual((result.end_ns - result.start_ns) / 1e9, bd.RESIZE_S + bd.TAIL_NS / 1e9, delta=0.1)

    def test_g4_result_records_the_driven_span(self):
        result = self.g4()
        notch_stamps = [event[2] for event in self.wheels()[1:]]
        self.assertEqual(result.driven_ns, notch_stamps[-1] - notch_stamps[0])

    def test_unobserved_resizes_raise(self):
        self.kwin.show_resizes = False
        with self.assertRaisesRegex(CaptureError, "^RESIZE_NOT_OBSERVED"):
            self.g9()

    def test_session_lock_and_app_exit_abort(self):
        self.world.locked = True
        with self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED"):
            self.g9()
        self.assertLess(len(self.kwin.resizes), int(bd.RESIZE_S * bd.RESIZE_HZ))
        self.world.locked, self.world.exit_code = False, 1
        self.kwin.resizes.clear()
        with self.assertRaisesRegex(CaptureError, "^APP_EXIT"):
            self.g9()
        self.assertEqual(self.kwin.resizes, [])

    def test_g9_never_touches_the_device(self):
        with patch.object(bd, "input_device_sysnames", side_effect=AssertionError("settled in drive_g9")):
            self.g9()
        self.assertEqual(self.world.events, [])


class NoticeTests(DriverTestCase):
    def test_notice_is_printed_once_before_input(self):
        stream = io.StringIO()
        with redirect_stderr(stream):
            self.g4()
            self.g4()
            self.g9()
        self.assertEqual(stream.getvalue().count(bd.NOTICE), 1)
        self.assertTrue(stream.getvalue().startswith("BENCH_NOTICE: injecting input"))


class FocusTailTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.dir = Path(self.temp.name)
        self.path = self.dir / "wlpresent-1.log"
        self.tail = bd.FocusTail(self.dir)

    def append(self, text, path=None):
        with (path or self.path).open("a") as stream:
            stream.write(text)

    def test_tracks_the_latest_enter_or_leave_on_a_toplevel(self):
        self.append("H\tclock_id=1\nS\t1\t7\ttoplevel\n")
        self.assertFalse(self.tail.focused("keyboard"))
        self.append("K\t10\tenter\t7\nM\t11\tenter\t7\n")
        self.assertTrue(self.tail.focused("keyboard"))
        self.assertTrue(self.tail.focused("pointer"))
        self.append("M\t12\tleave\t7\n")
        self.assertTrue(self.tail.focused("keyboard"))
        self.assertFalse(self.tail.focused("pointer"))

    def test_a_non_toplevel_or_destroyed_surface_is_not_focus(self):
        self.append("S\t1\t7\ttoplevel\nK\t10\tenter\t8\n")  # surface 8 has no role
        self.assertFalse(self.tail.focused("keyboard"))
        self.append("S\t2\t7\tdestroyed\nK\t11\tenter\t7\n")
        self.assertFalse(self.tail.focused("keyboard"))

    def test_reads_only_new_complete_lines(self):
        reads = []
        original = os.pread
        self.append("S\t1\t7\ttoplevel\nK\t10\tenter\t7\nC\t5\t7\t1\t1\nP\t6\t1\t6\t8\t1\t0\n")
        size = self.path.stat().st_size
        with patch.object(bd.os, "pread", lambda fd, n, offset: reads.append((n, offset)) or original(fd, n, offset)):
            self.assertTrue(self.tail.focused("keyboard"))
            self.assertTrue(self.tail.focused("keyboard"))  # nothing new: no read
            self.append("K\t11\tleave\t7\nK\t12\tent")  # the last record is incomplete
            self.assertFalse(self.tail.focused("keyboard"))
            self.append("er\t7\n")
            self.assertTrue(self.tail.focused("keyboard"))
        self.assertEqual(reads[0], (size, 0))
        self.assertEqual(len(reads), 3)
        self.assertEqual(reads[1][1], size)  # the second read starts where the first stopped
        self.assertEqual(reads[2][1], size + len("K\t11\tleave\t7\n"))  # only the partial record is read again

    def test_latest_event_across_logs_wins_and_new_logs_are_picked_up(self):
        self.append("S\t1\t7\ttoplevel\nK\t10\tenter\t7\n")
        self.assertTrue(self.tail.focused("keyboard"))
        other = self.dir / "wlpresent-2.log"
        self.append("S\t1\t3\ttoplevel\nK\t20\tleave\t3\n", other)
        self.assertFalse(self.tail.focused("keyboard"))
        self.append("K\t30\tenter\t7\n")
        self.assertTrue(self.tail.focused("keyboard"))

    def test_malformed_records_and_symlinks_fail_like_the_parser(self):
        self.append("S\t1\t7\ttoplevel\nK\t10\tenter\n")
        with self.assertRaisesRegex(CaptureError, r"^PRESENT_LOG_MALFORMED: wlpresent-1.log:2"):
            self.tail.focused("keyboard")
        link = self.dir / "wlpresent-9.log"
        link.symlink_to(self.path)
        fresh = bd.FocusTail(self.dir)
        self.path.write_text("S\t1\t7\ttoplevel\n")
        with self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_UNREADABLE: wlpresent-9.log"):
            fresh.focused("keyboard")

    def test_oversized_log_is_refused(self):
        self.append("S\t1\t7\ttoplevel\n")
        with patch.object(bd, "MAX_LOG_BYTES", 4), self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_TOO_LARGE"):
            self.tail.focused("keyboard")


class PrepareScenarioTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def env(self, flatpak):
        home = self.root / "home"
        env = {"HOME": str(home), "XDG_CONFIG_HOME": str(self.root / "config"),
               "CROSSHOOK_BENCH_LOG": "/stale", "KEEP": "1"}
        env["CROSSHOOK_BENCH_ROOT"] = str(home / ".var/app/dev.crosshook.CrossHook" if flatpak else self.root)
        return env

    def check_console(self, flatpak, config):
        env = self.env(flatpak)
        updated = bd.prepare_scenario("console", env, flatpak)
        path = config / "settings.toml"
        self.assertEqual(path.read_text(), "console_drawer_collapsed_default = false\n")
        self.assertEqual(stat.S_IMODE(os.stat(path).st_mode), 0o600)
        self.assertEqual(updated["CROSSHOOK_BENCH_LOG"],
                         str(Path(env["CROSSHOOK_BENCH_ROOT"]) / "logs/launch-50k.log"))
        self.assertEqual(updated["CROSSHOOK_BENCH_LOG_RATE"], "5000")
        self.assertEqual(updated["KEEP"], "1")
        self.assertEqual(env["CROSSHOOK_BENCH_LOG"], "/stale")  # the input is not mutated
        bd.prepare_scenario("console", env, flatpak)  # a second iteration rewrites the file
        self.assertEqual(path.read_text(), "console_drawer_collapsed_default = false\n")

    def test_console_native_writes_settings_under_xdg_config_home(self):
        self.check_console(False, self.root / "config/crosshook")

    def test_console_flatpak_writes_settings_under_the_app_sandbox_config(self):
        self.check_console(True, self.root / "home/.var/app/dev.crosshook.CrossHook/config/crosshook")

    def test_other_scenarios_write_nothing_and_drop_the_replay_env(self):
        for scenario in ("library", "settings"):
            updated = bd.prepare_scenario(scenario, self.env(False), False)
            self.assertNotIn("CROSSHOOK_BENCH_LOG", updated)
        self.assertFalse((self.root / "config").exists())

    def test_console_settings_do_not_leak_into_later_scenarios(self):
        for flatpak, config in ((False, self.root / "config/crosshook"),
                                (True, self.root / "home/.var/app/dev.crosshook.CrossHook/config/crosshook")):
            path = config / "settings.toml"
            for later in ("settings", "library"):
                with self.subTest(flatpak=flatpak, later=later):
                    env = self.env(flatpak)
                    bd.prepare_scenario("console", env, flatpak)
                    self.assertTrue(path.is_file())
                    bd.prepare_scenario(later, env, flatpak)
                    self.assertFalse(os.path.lexists(path))
                    bd.prepare_scenario(later, env, flatpak)  # nothing left to remove is fine

    def test_a_symlinked_settings_file_is_refused_and_left_alone(self):
        config = self.root / "config/crosshook"
        config.mkdir(parents=True)
        target = self.root / "elsewhere.toml"
        target.write_text("keep = true\n")
        (config / "settings.toml").symlink_to(target)
        for scenario in ("library", "settings", "console"):
            with self.subTest(scenario=scenario), self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE"):
                bd.prepare_scenario(scenario, self.env(False), False)
        self.assertEqual(target.read_text(), "keep = true\n")
        self.assertTrue((config / "settings.toml").is_symlink())

    def test_incomplete_env_and_unknown_scenario_fail_by_name(self):
        with self.assertRaisesRegex(CaptureError, "^BENCH_ENV_INCOMPLETE"):
            bd.prepare_scenario("console", {"HOME": str(self.root)}, False)
        with self.assertRaisesRegex(CaptureError, "^UNKNOWN_SCENARIO"):
            bd.prepare_scenario("x", {}, False)


if __name__ == "__main__":
    unittest.main()
