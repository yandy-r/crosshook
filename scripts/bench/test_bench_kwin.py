"""KWin control: gdbus argv, strict a{sv} parsing, window matching and script lifecycle; stdlib only."""
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_kwin
import bench_session
from bench_capture import CaptureError
from bench_kwin import KWinScripts, WindowInfo, find_window, parse_window_info, require_kwin

UUID_A = "{0b1c2d3e-4f50-6172-8394-a5b6c7d8e9f0}"
UUID_B = "{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee}"
MATCH = ("([('0_%s', 'CrossHook', 'icon', 1, 1.0, {}), ('0_%s', 'Other', 'icon', 1, 1.0, {})],)"
         % (UUID_A, UUID_B))
# E1 sample: the caption holds a quote and a brace that must not confuse the parser.
INFO = ("({'pid': <109076>, 'x': <0.0>, 'y': <-12.5>, 'width': <357.14285714285722>, 'height': <400>, "
        "'minimized': <false>, 'caption': <'it\\'s a } \"title\", <odd>'>, 'resourceClass': <'x'>, "
        "'layer': <uint32 2>},)")


def info_text(pid: int, width: float = 800.0, height: float = 600.0) -> str:
    return ("({'pid': <%d>, 'x': <10.0>, 'y': <20.0>, 'width': <%s>, 'height': <%s>, "
            "'minimized': <false>, 'caption': <'t'>},)" % (pid, width, height))


def reply(stdout: str = "", returncode: int = 0, stderr: str = ""):
    return SimpleNamespace(returncode=returncode, stdout=stdout + "\n", stderr=stderr)


class FakeBus:
    """Fake `_run`: answers by gdbus method name and records every argv."""

    def __init__(self, answers: dict):
        self.answers, self.calls = answers, []

    def __call__(self, argv, timeout):
        self.calls.append(list(argv))
        answer = self.answers[argv[argv.index("--method") + 1]]
        if isinstance(answer, list):
            answer = answer.pop(0) if len(answer) > 1 else answer[0]
        if isinstance(answer, BaseException):
            raise answer
        return answer

    def methods(self) -> list[str]:
        return [call[call.index("--method") + 1] for call in self.calls]


class FakeClock:
    def __init__(self):
        self.now = 0.0

    def monotonic(self) -> float:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.now += seconds


class ParseTest(unittest.TestCase):
    def test_real_sample(self):
        win = parse_window_info(UUID_A, INFO)
        self.assertEqual(win, WindowInfo(UUID_A, 109076, 0.0, -12.5, 357.14285714285722, 400.0, False))

    def test_missing_or_malformed_keys(self):
        for text in ("({'x': <1.0>},)", "{}", "({'pid': <'7'>, 'x': <1.0>, 'y': <1.0>, 'width': <1.0>, "
                     "'height': <1.0>, 'minimized': <false>},)",
                     "({'pid': <7>, 'x': <1.0>, 'y': <1.0>, 'width': <1.0>, 'height': <1.0>, 'minimized': <1>},)",
                     "({'pid': <7>, 'caption': <'unterminated>},)"):
            with self.subTest(text=text), self.assertRaisesRegex(CaptureError, "^KWIN_PROTOCOL"):
                parse_window_info(UUID_A, text)


class RequireKWinTest(unittest.TestCase):
    def check(self, which="/usr/bin/gdbus", owner=True):
        with patch("shutil.which", return_value=which), \
                patch.object(bench_kwin, "name_has_owner", return_value=owner) as probe:
            require_kwin()
        return probe

    def test_present(self):
        self.check().assert_called_once_with("org.kde.KWin")

    def test_failures(self):
        cases = [({"which": None}, "MISSING_TOOL: gdbus"),
                 ({"owner": None}, "UNSUPPORTED_COMPOSITOR: KWin is required for G4/G9 input drivers"),
                 ({"owner": False}, "UNSUPPORTED_COMPOSITOR")]
        for kwargs, message in cases:
            with self.subTest(kwargs=kwargs), self.assertRaisesRegex(CaptureError, "^" + message):
                self.check(**kwargs)

    def test_probe_goes_through_the_session_bus_guard(self):
        with patch("shutil.which", return_value="/usr/bin/gdbus"), \
                patch.object(bench_session, "_session_bus_available", return_value=False), \
                patch.object(bench_session, "_probe", side_effect=AssertionError("probed without a bus")), \
                self.assertRaisesRegex(CaptureError, "^UNSUPPORTED_COMPOSITOR"):
            require_kwin()


class GdbusTest(unittest.TestCase):
    def test_timeout_and_spawn_errors_are_call_failures(self):
        for error in (subprocess.TimeoutExpired(["gdbus"], bench_kwin.CALL_TIMEOUT_S), FileNotFoundError("gdbus")):
            fake = FakeBus({"org.kde.KWin.getWindowInfo": error})
            with self.subTest(error=type(error).__name__), patch.object(bench_kwin, "_run", fake), \
                    self.assertRaisesRegex(CaptureError, "^KWIN_CALL_FAILED: org.kde.KWin.getWindowInfo: "):
                bench_kwin.window_info(UUID_A)

    def test_call_uses_the_call_timeout(self):
        timeouts = []

        def run(argv, timeout):
            timeouts.append(timeout)
            return reply("ok")
        with patch.object(bench_kwin, "_run", run):
            self.assertEqual(bench_kwin._gdbus(["gdbus", "--method", "m"]), "ok")
        self.assertEqual(timeouts, [bench_kwin.CALL_TIMEOUT_S])

    def test_input_device_sysnames_reads_the_kwin_property(self):
        fake = FakeBus({"org.freedesktop.DBus.Properties.Get": reply("(<['event3', 'event9']>,)")})
        with patch.object(bench_kwin, "_run", fake):
            self.assertEqual(bench_kwin.input_device_sysnames(), "(<['event3', 'event9']>,)")
        self.assertEqual(fake.calls, [bench_kwin.DEVICES_PROPERTY])
        failing = FakeBus({"org.freedesktop.DBus.Properties.Get": reply(returncode=1, stderr="no such object")})
        with patch.object(bench_kwin, "_run", failing), self.assertRaisesRegex(CaptureError, "^KWIN_CALL_FAILED"):
            bench_kwin.input_device_sysnames()


class FindWindowTest(unittest.TestCase):
    def find(self, infos: dict, timeout_s=0, pids=None):
        fake = FakeBus({"org.kde.krunner1.Match": reply(MATCH),
                        "org.kde.KWin.getWindowInfo": [reply(text) for text in infos.values()]})
        clock = FakeClock()
        with patch.object(bench_kwin, "_run", fake), patch.object(bench_kwin, "_clock", clock.monotonic), \
                patch.object(bench_kwin, "_sleep", clock.sleep):
            return find_window(pids or {7}, timeout_s), fake

    def test_argv(self):
        win, fake = self.find({UUID_A: info_text(7), UUID_B: info_text(8)})
        self.assertEqual(win.uuid, UUID_A)
        self.assertEqual(fake.calls[0], ["gdbus", "call", "--session", "--dest", "org.kde.KWin",
                                         "--object-path", "/WindowsRunner",
                                         "--method", "org.kde.krunner1.Match", ""])
        self.assertEqual(fake.calls[1], ["gdbus", "call", "--session", "--dest", "org.kde.KWin",
                                         "--object-path", "/KWin", "--method", "org.kde.KWin.getWindowInfo",
                                         "'%s'" % UUID_A])

    def test_zero_matches(self):
        with self.assertRaisesRegex(CaptureError, "^WINDOW_NOT_FOUND"):
            self.find({UUID_A: info_text(1), UUID_B: info_text(2)})

    def test_two_matches(self):
        with self.assertRaisesRegex(CaptureError, "^WINDOW_AMBIGUOUS"):
            self.find({UUID_A: info_text(7), UUID_B: info_text(8)}, pids={7, 8})

    def test_polls_until_window_appears(self):
        fake = FakeBus({"org.kde.krunner1.Match": [reply("([],)"), reply(MATCH)],
                        "org.kde.KWin.getWindowInfo": [reply(info_text(7)), reply(info_text(1))]})
        clock = FakeClock()
        with patch.object(bench_kwin, "_run", fake), patch.object(bench_kwin, "_clock", clock.monotonic), \
                patch.object(bench_kwin, "_sleep", clock.sleep):
            self.assertEqual(find_window({7}, 5).pid, 7)
        self.assertEqual(clock.now, bench_kwin.POLL_S)


class ScriptsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.tools = Path(self.tmp.name) / "bench-tools"
        self.tools.mkdir(mode=0o700)
        self.clock = FakeClock()
        for name, value in (("_clock", self.clock.monotonic), ("_sleep", self.clock.sleep)):
            patcher = patch.object(bench_kwin, name, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def bus(self, loaded="(false,)", load="(7,)", run=None, info=None):
        fake = FakeBus({"org.kde.kwin.Scripting.isScriptLoaded": reply(loaded),
                        "org.kde.kwin.Scripting.loadScript": reply(load),
                        "org.kde.kwin.Script.run": run or reply(),
                        "org.kde.kwin.Scripting.unloadScript": reply("(true,)"),
                        "org.kde.KWin.getWindowInfo": reply(info or info_text(7))})
        self.scripts: list[str] = []  # script text as KWin would read it at loadScript time

        def recording(argv, timeout):
            if argv[-1].startswith("'crosshook-bench-") and argv[-2].endswith(".js'"):
                self.scripts.append(Path(argv[-2].strip("'")).read_text())
            return fake(argv, timeout)
        recording.calls, recording.methods = fake.calls, fake.methods
        patcher = patch.object(bench_kwin, "_run", recording)
        patcher.start()
        self.addCleanup(patcher.stop)
        return recording

    def win(self):
        return WindowInfo(UUID_A, 7, 10.0, 20.0, 400.0, 300.0, False)

    def test_run_lifecycle_and_argv(self):
        fake = self.bus()
        KWinScripts(self.tools, "abc123").run("1;")
        plugin = "crosshook-bench-abc123-1"
        path = self.tools / f"{plugin}.js"
        self.assertEqual(self.scripts, ["1;"])
        self.assertFalse(path.exists())  # removed after the unload
        base = ["gdbus", "call", "--session", "--dest", "org.kde.KWin", "--object-path"]
        self.assertEqual(fake.calls, [
            [*base, "/Scripting", "--method", "org.kde.kwin.Scripting.isScriptLoaded", f"'{plugin}'"],
            [*base, "/Scripting", "--method", "org.kde.kwin.Scripting.loadScript", f"'{path}'", f"'{plugin}'"],
            [*base, "/Scripting/Script7", "--method", "org.kde.kwin.Script.run"],
            [*base, "/Scripting", "--method", "org.kde.kwin.Scripting.unloadScript", f"'{plugin}'"]])

    def test_script_is_private_while_loaded_and_removed_after_the_unload(self):
        seen = []
        fake = self.bus()
        inner = bench_kwin._run

        def spy(argv, timeout):
            method = argv[argv.index("--method") + 1]
            seen.append((method, sorted(p.name for p in self.tools.iterdir()),
                         [p.stat().st_mode & 0o777 for p in self.tools.iterdir()]))
            return inner(argv, timeout)
        with patch.object(bench_kwin, "_run", spy):
            KWinScripts(self.tools, "abc").run("1;")
        by_method = {method: (names, modes) for method, names, modes in seen}
        self.assertEqual(by_method["org.kde.kwin.Scripting.loadScript"],
                         (["crosshook-bench-abc-1.js"], [0o600]))
        self.assertEqual(by_method["org.kde.kwin.Scripting.unloadScript"][0], ["crosshook-bench-abc-1.js"])
        self.assertEqual(list(self.tools.iterdir()), [])
        self.assertEqual(fake.methods()[-1], "org.kde.kwin.Scripting.unloadScript")

    def test_script_file_is_removed_on_every_failure_after_the_write(self):
        cases = (("run fails", dict(run=reply(returncode=1, stderr="boom")), "^KWIN_CALL_FAILED"),
                 ("interrupted", dict(run=KeyboardInterrupt()), None),
                 ("bad load reply", dict(load="(true,)"), "^KWIN_PROTOCOL"),
                 ("busy on load", dict(load="(-1,)"), "^KWIN_SCRIPT_BUSY"),
                 ("already loaded", dict(loaded="(true,)"), "^KWIN_SCRIPT_BUSY"))
        for name, kwargs, pattern in cases:
            with self.subTest(case=name):
                self.bus(**kwargs)
                expected = KeyboardInterrupt if pattern is None else CaptureError
                with self.assertRaises(expected):
                    KWinScripts(self.tools, f"abc{len(name)}").run("1;")
                self.assertEqual(list(self.tools.iterdir()), [])

    def test_unload_runs_when_run_fails(self):
        fake = self.bus(run=reply(returncode=1, stderr="boom"))
        with self.assertRaisesRegex(CaptureError, "^KWIN_CALL_FAILED"):
            KWinScripts(self.tools, "abc").run("1;")
        self.assertEqual(fake.methods()[-1], "org.kde.kwin.Scripting.unloadScript")

    def test_unload_runs_when_interrupted(self):
        fake = self.bus(run=KeyboardInterrupt())
        with self.assertRaises(KeyboardInterrupt):
            KWinScripts(self.tools, "abc").run("1;")
        self.assertEqual(fake.methods()[-1], "org.kde.kwin.Scripting.unloadScript")

    def test_script_write_failure_loads_nothing(self):
        (self.tools / "crosshook-bench-abc-1.js").write_text("old")  # write_private refuses an existing file
        fake = self.bus()
        with self.assertRaisesRegex(CaptureError, "^KWIN_SCRIPT_WRITE_FAILED: crosshook-bench-abc-1.js: "):
            KWinScripts(self.tools, "abc").run("1;")
        self.assertEqual(fake.calls, [])

    def test_non_int_load_reply_is_a_protocol_error_and_unloads(self):
        for number, load in enumerate(("(true,)", "('7',)", "()", "(7, 8)")):
            fake = self.bus(load=load)
            with self.subTest(load=load), self.assertRaisesRegex(CaptureError, "^KWIN_PROTOCOL: loadScript"):
                KWinScripts(self.tools, f"abc{number}").run("1;")
            self.assertNotIn("org.kde.kwin.Script.run", fake.methods())
            self.assertEqual(fake.methods()[-1], "org.kde.kwin.Scripting.unloadScript")

    def test_busy_when_already_loaded(self):
        fake = self.bus(loaded="(true,)")
        with self.assertRaisesRegex(CaptureError, "^KWIN_SCRIPT_BUSY"):
            KWinScripts(self.tools, "abc").run("1;")
        self.assertNotIn("org.kde.kwin.Scripting.loadScript", fake.methods())

    def test_busy_when_load_returns_minus_one_leaves_plugin_alone(self):
        fake = self.bus(load="(-1,)")
        with self.assertRaisesRegex(CaptureError, "^KWIN_SCRIPT_BUSY"):
            KWinScripts(self.tools, "abc").run("1;")
        self.assertNotIn("org.kde.kwin.Scripting.unloadScript", fake.methods())

    def test_rejected_names_and_paths(self):
        for tag in ("", "ABC", "a-b", "x" * 17):
            with self.subTest(tag=tag), self.assertRaisesRegex(CaptureError, "^KWIN_PLUGIN_INVALID"):
                KWinScripts(self.tools, tag)
        for plugin in ("bench-x", "crosshook-bench-", "crosshook-bench-A", "crosshook-bench-a'b"):
            with self.subTest(plugin=plugin), self.assertRaisesRegex(CaptureError, "^KWIN_PLUGIN_INVALID"):
                bench_kwin._quoted_plugin(plugin)
        for path in ("/tmp/it's.js", "/tmp/a\\b.js", "/tmp/a\nb.js", "-rf", "relative.js"):
            with self.subTest(path=path), self.assertRaisesRegex(CaptureError, "^KWIN_SCRIPT_PATH_INVALID"):
                bench_kwin._quoted_path(Path(path))

    def test_tools_dir_must_be_owned_0700_real_directory(self):
        loose = Path(self.tmp.name) / "loose"
        loose.mkdir(mode=0o755)
        loose.chmod(0o755)
        link = Path(self.tmp.name) / "link"
        link.symlink_to(self.tools)
        for path in (loose, link, Path(self.tmp.name) / "missing"):
            with self.subTest(path=path), self.assertRaisesRegex(CaptureError, "^KWIN_TOOLS_DIR_UNSAFE"):
                KWinScripts(path, "abc")

    def test_generated_js_has_only_digits_for_numbers(self):
        self.bus(info=info_text(7, 640, 480))
        scripts = KWinScripts(self.tools, "abc")
        scripts.place(self.win(), 640, 480, 12, 34)
        scripts.resize_to(self.win(), 640, 480)
        place_js, resize_js = self.scripts
        self.assertIn("w.pid === 7", place_js)
        self.assertIn("{x: c.x - 12, y: c.y - 34, width: 640, height: 480}", place_js)
        self.assertIn("workspace.activeWindow = w", place_js)
        self.assertIn("{x: g.x, y: g.y, width: 640, height: 480}", resize_js)
        self.assertNotIn("activeWindow", resize_js)
        for js in (place_js, resize_js):
            self.assertIsNone(re.search(r"[0-9.]+\.[0-9]|\{[0-9]|NaN|undefined|None", js))
        with self.assertRaises(TypeError):
            scripts.place(self.win(), 640.0, 480, 1, 1)  # type: ignore[arg-type]
        with self.assertRaises(TypeError):
            scripts.resize_to(self.win(), 640, True)  # type: ignore[arg-type]

    def test_resize_unverified_runs_the_script_without_polling_geometry(self):
        fake = self.bus()
        self.assertIsNone(KWinScripts(self.tools, "abc").resize_unverified(self.win(), 640, 480))
        self.assertNotIn("org.kde.KWin.getWindowInfo", fake.methods())
        (script,) = self.scripts
        self.assertIn("{x: g.x, y: g.y, width: 640, height: 480}", script)
        self.assertIn("w.setMaximize(false, false)", script)
        with self.assertRaises(TypeError):
            KWinScripts(self.tools, "abd").resize_unverified(self.win(), 640, 480.0)  # type: ignore[arg-type]

    def test_resize_to_runs_the_same_script_then_verifies(self):
        self.bus(info=info_text(7, 640, 480))
        scripts = KWinScripts(self.tools, "abc")
        scripts.resize_unverified(self.win(), 640, 480)
        scripts.resize_to(self.win(), 640, 480)
        unverified, verified = self.scripts
        self.assertEqual(unverified, verified)

    def test_geometry_within_tolerance_is_applied(self):
        self.bus(info=info_text(7, 641.4, 478.6))
        self.assertEqual(round(KWinScripts(self.tools, "abc").resize_to(self.win(), 640, 480).width), 641)

    def test_geometry_not_applied_times_out(self):
        self.bus(info=info_text(7, 800, 600))
        with self.assertRaisesRegex(CaptureError, "^KWIN_GEOMETRY_NOT_APPLIED"):
            KWinScripts(self.tools, "abc").resize_to(self.win(), 640, 480)
        self.assertGreaterEqual(self.clock.now, bench_kwin.GEOMETRY_TIMEOUT_S)


def kwin_live() -> bool:
    if not os.environ.get("WAYLAND_DISPLAY"):
        return False
    try:
        return bench_session.name_has_owner(bench_kwin.DEST) is True
    except (OSError, subprocess.SubprocessError):
        return False


@unittest.skipUnless(kwin_live(), "needs a live KWin session and WAYLAND_DISPLAY")
class LiveKWinTest(unittest.TestCase):
    def test_require_and_no_window_for_test_process(self):
        require_kwin()
        with self.assertRaisesRegex(CaptureError, "^WINDOW_NOT_FOUND"):
            find_window({os.getpid()}, timeout_s=0)


if __name__ == "__main__":
    unittest.main()
