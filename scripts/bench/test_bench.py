"""Focused benchmark guard/capture regression checks; stdlib only."""
import csv
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_capture
import bench_provenance
import bench_session
from bench_capture import CaptureError, over_interval_percent, ready, stats, stop
from bench_guard import GuardError, Originals, build_env, check_path, preflight


class GuardTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "real-home"
        self.home.mkdir()
        self.original = Originals(str(self.home), str(self.home), {}, None)

    def test_protected_paths_and_ancestors(self):
        for path in ("", "/", self.home, self.home / ".config",
                     self.home / ".config/crosshook", self.home / ".local/share/crosshook/sub",
                     self.home / ".cache/crosshook", self.home / ".var/app/dev.crosshook.CrossHook"):
            with self.subTest(path=path), self.assertRaises(GuardError):
                check_path(path, "output", self.original)
        self.assertEqual(check_path(self.root / "safe", "output", self.original), self.root / "safe")

    def test_symlink_ancestor(self):
        store = self.home / ".config/crosshook"
        store.mkdir(parents=True)
        link = self.root / "link"
        link.symlink_to(store, target_is_directory=True)
        with self.assertRaises(GuardError):
            check_path(link / "uncreated/sub", "output", self.original)
        self.assertFalse((store / "uncreated").exists())

    def test_custom_profiles_directory(self):
        config = self.home / ".config/crosshook"
        config.mkdir(parents=True)
        custom = self.root / "custom-profiles"
        (config / "settings.toml").write_text(f'profiles_directory = "{custom}"\n')
        with self.assertRaises(GuardError):
            preflight(self.original, output=custom / "new")
        self.assertFalse(custom.exists())

    def test_inherited_xdg_refused_before_write(self):
        original = Originals(str(self.home), str(self.home), {"XDG_CONFIG_HOME": str(self.home / ".config")}, None)
        output = self.root / "unused"
        with self.assertRaises(GuardError):
            preflight(original, output=output)
        self.assertFalse(output.exists())

    def test_isolated_environment(self):
        root = self.root / "crosshook-bench-owned"
        env = build_env(root, {"HOME": str(self.home), "HOST_XDG_DATA_HOME": "/real-data",
                               "CROSSHOOK_BENCH_LOG": "/secret", "XDG_RUNTIME_DIR": str(self.root),
                               "LD_PRELOAD": "/x.so", "WAYLAND_DEBUG": "client", "LD_LIBRARY_PATH": "/l"})
        self.assertEqual(env["HOME"], str(root / "home"))
        self.assertEqual(env["CROSSHOOK_FLATPAK_HOST_XDG"], "0")
        self.assertNotIn("CROSSHOOK_BENCH_LOG", env)
        self.assertNotIn("HOST_XDG_DATA_HOME", env)
        for key in ("LD_PRELOAD", "WAYLAND_DEBUG", "LD_LIBRARY_PATH"):
            self.assertNotIn(key, env)

    def test_flatpak_user_installation_located(self):
        (self.home / ".local/share/flatpak").mkdir(parents=True)
        root = self.root / "crosshook-bench-owned"
        env = build_env(root, {"HOME": str(self.home)}, flatpak=True)
        self.assertEqual(env["FLATPAK_USER_DIR"], str(self.home / ".local/share/flatpak"))
        self.assertEqual(env["HOME"], str(root / "home"))
        self.assertNotIn("FLATPAK_USER_DIR", build_env(root, {"HOME": str(self.home)}))

    def test_flatpak_launcher_recipe(self):
        args = bench.arguments(["--variant", "A", "--env", "E1", "--flatpak", "--metric", "G2"])
        root = self.root / "crosshook-bench-owned"
        env = build_env(root, {}, flatpak=True)
        command = bench.launch_command(args, env)
        self.assertIn("--nofilesystem=host:reset", command)
        self.assertEqual(env["HOME"], str(root / "home"))
        with self.assertRaises(GuardError):
            bench.launch_command(args, {"HOME": str(self.home)})

    def test_flatpak_forwards_shim_variables_via_env_flags(self):
        args = bench.arguments(["--variant", "A", "--env", "E1", "--flatpak", "--metric", "G2"])
        env = {**build_env(self.root / "crosshook-bench-owned", {}, flatpak=True),
               "LD_PRELOAD": "/shim.so", "CROSSHOOK_BENCH_PRESENT_DIR": "/p/iter-1"}
        command = bench.launch_command(args, env)
        self.assertIn("--env=LD_PRELOAD=/shim.so", command)
        self.assertIn("--env=CROSSHOOK_BENCH_PRESENT_DIR=/p/iter-1", command)
        self.assertEqual(command[-1], bench.FLATPAK_APP_ID)

    def test_stale_csv_is_deleted_not_sealed(self):
        import bench_shim
        fake = bench_shim.ShimBuild(self.root / "libwlpresent.so", "0" * 64, "0" * 64, "host")
        output = self.root / "results"
        target = output / "A/E1"
        target.mkdir(parents=True)
        (target / "G2.csv").write_text("stale\n")
        outside = self.root / "outside.csv"
        outside.write_text("keep\n")
        (target / "G1.csv").symlink_to(outside)
        with patch.object(bench.Originals, "capture", return_value=self.original), \
                patch.object(bench_shim, "build", return_value=fake), \
                patch.object(bench, "capture", side_effect=CaptureError("NO_READY: x")), \
                patch.object(bench, "launch_command", return_value=["app"]), \
                patch.object(bench, "make_dirs"):
            code = bench.main(["--variant", "A", "--env", "E1", "--metric", "G1", "--metric", "G2",
                               "--cold-cache-ack", "--binary", sys.executable, "--collect", "presentation",
                               "--output", str(output), "--tmp-root", str(self.root / "runs")])
        self.assertEqual(code, 1)
        self.assertFalse((target / "G2.csv").exists())
        self.assertFalse((target / "G1.csv").is_symlink())
        self.assertEqual(outside.read_text(), "keep\n")
        meta = json.loads((target / "meta.json").read_text())
        self.assertEqual(meta["status"], "failed")
        self.assertIn("dirty", meta)
        (raw,) = (output / "raw").iterdir()
        self.assertFalse(any(path.suffix == ".csv" for path in raw.iterdir()))
        self.assertEqual(json.loads((raw / "provenance.json").read_text())["status"], "failed")

    def test_busy_output_refused_before_any_write(self):
        output = self.root / "shared"
        target = output / "A/E1"
        target.mkdir(parents=True)
        (target / "G2.csv").write_text("other run\n")
        fd = bench_provenance.lock_output(target)
        self.addCleanup(os.close, fd)
        with patch.object(bench.Originals, "capture", return_value=self.original), \
                patch("sys.stderr") as stderr:
            code = bench.main(["--variant", "A", "--env", "E1", "--metric", "G2", "--binary", sys.executable,
                               "--output", str(output), "--tmp-root", str(self.root / "runs")])
        self.assertEqual(code, 1)
        written = "".join(call.args[0] for call in stderr.write.call_args_list)
        self.assertIn("OUTPUT_BUSY: another bench run is writing", written)
        self.assertEqual((target / "G2.csv").read_text(), "other run\n")
        self.assertFalse((target / "meta.json").exists())

    def test_binary_hashed_before_launch_and_change_fails_run(self):
        import hashlib
        import bench_shim
        binary = self.root / "app"
        binary.write_bytes(b"first")
        before = hashlib.sha256(b"first").hexdigest()
        fake = bench_shim.ShimBuild(self.root / "libwlpresent.so", "0" * 64, "0" * 64, "host")

        def run_and_replace(*_args, **_kwargs):
            binary.write_bytes(b"second")
            return [{"iteration": "1", "status": "ok"}]
        for mutate, expect_code, expect_status in ((True, 1, "failed"), (False, 0, "captured")):
            binary.write_bytes(b"first")
            output = self.root / f"out-{mutate}"
            with self.subTest(mutate=mutate), patch.object(bench.Originals, "capture", return_value=self.original), \
                    patch.object(bench_shim, "build", return_value=fake), \
                    patch.object(bench, "capture", side_effect=run_and_replace if mutate
                                 else lambda *a, **k: [{"iteration": "1", "status": "ok"}]), \
                    patch.object(bench, "launch_command", return_value=["app"]), \
                    patch.object(bench, "make_dirs"), patch("sys.stderr") as stderr:
                code = bench.main(["--variant", "A", "--env", "E1", "--metric", "G2", "--cold-cache-ack",
                                   "--binary", str(binary), "--collect", "presentation",
                                   "--output", str(output), "--tmp-root", str(self.root / "runs")])
            self.assertEqual(code, expect_code)
            (raw,) = (output / "raw").iterdir()
            manifest = json.loads((raw / "provenance.json").read_text())
            meta = json.loads((output / "A/E1/meta.json").read_text())
            self.assertEqual(manifest["binary_sha256"], before)
            self.assertEqual((manifest["status"], meta["status"]), (expect_status, expect_status))
            if mutate:
                self.assertTrue(manifest["error"].startswith("BINARY_CHANGED: "))
                self.assertTrue(meta["error"].startswith("BINARY_CHANGED: "))
                written = "".join(call.args[0] for call in stderr.write.call_args_list)
                self.assertIn("BINARY_CHANGED: ", written)
            else:
                self.assertNotIn("error", manifest)

    def test_guard_error_marks_meta_failed(self):
        output = self.root / "guarded"
        with patch.object(bench.Originals, "capture", return_value=self.original), \
                patch.object(bench, "launch_command", side_effect=GuardError("nope")), \
                patch.object(bench, "make_dirs"):
            code = bench.main(["--variant", "A", "--env", "E1", "--metric", "G2", "--binary", sys.executable,
                               "--output", str(output), "--tmp-root", str(self.root / "runs")])
        self.assertEqual(code, 10)
        self.assertEqual(json.loads((output / "A/E1/meta.json").read_text())["status"], "failed")

    def test_dry_run_all_schemas(self):
        output = self.root / "results"
        with patch.object(bench.Originals, "capture", return_value=self.original):
            result = bench.main(["--variant", "A", "--env", "E1", "--dry-run", "--output", str(output),
                                 "--tmp-root", str(self.root / "runs")])
        self.assertEqual(result, 0)
        for metric, schema in bench.SCHEMAS.items():
            with (output / "A/E1" / f"{metric}.csv").open() as stream:
                rows = list(csv.DictReader(stream))
            self.assertEqual(list(rows[0]), schema.split(","))
            self.assertEqual(rows[0]["status"], "dry-run")
        self.assertTrue(json.loads((output / "A/E1/meta.json").read_text())["synthetic"])

    def test_real_runs_fail_before_writes_without_instrumentation(self):
        output = self.root / "real"
        cases = [(["--metric", "G1"], 20), (["--metric", "G4"], 1), (["--metric", "G8"], 1), (["--metric", "G7"], 30)]
        for extra, code in cases:
            with self.subTest(extra=extra), patch.object(bench.Originals, "capture", return_value=self.original), \
                    patch.object(bench.shutil, "which", return_value=None):
                self.assertEqual(bench.main(["--variant", "A", "--env", "E1", "--output", str(output),
                                                "--tmp-root", str(self.root / "runs"), *extra]), code)
        self.assertFalse(output.exists())

    def test_refusal_writes_nothing(self):
        with patch.object(bench.Originals, "capture", return_value=self.original):
            self.assertEqual(bench.main(["--variant", "A", "--env", "E1", "--dry-run",
                                         "--output", str(self.home / ".config/crosshook")]), 10)
        self.assertFalse((self.home / ".config").exists())


class InstrumentTests(unittest.TestCase):
    def test_parsers(self):
        import bench_instruments as instruments
        self.assertAlmostEqual(instruments.size_mb("1.0 MB"), 1e6 / 1024**2)
        with patch.object(instruments, "output", side_effect=["/battery_BAT0\n", "energy-rate: 7.5 W\n"]):
            self.assertEqual(instruments.battery_power_w(), 7.5)
        with patch.object(instruments.shutil, "which", return_value="/usr/bin/nvidia-smi"), \
                patch.object(instruments, "output", return_value=(
                    "# gpu pid type fb ccpm command\n# Idx # C/G MB MB name\n"
                    "0 10 G 12 0 app\n1 10 C 3 0 app\n0 11 C+G 5 0 web\n0 12 G - - x\n0 99 G 50 0 other\n")):
            self.assertEqual(instruments.gpu_memory_mb({10, 11, 12}), 20)
            self.assertEqual(instruments.gpu_memory_mb({42}), "unavailable")
        with patch.object(instruments.shutil, "which", return_value="/usr/bin/nvidia-smi"), \
                patch.object(instruments, "output", return_value="0 10 G nope 0 app\n"), \
                self.assertRaises(instruments.InstrumentError):
            instruments.gpu_memory_mb({10})
        with patch.object(instruments.shutil, "which", return_value=None):
            self.assertEqual(instruments.gpu_memory_mb({10}), "unavailable")
        with self.assertRaises(SystemExit):
            bench.arguments(["--variant", "A", "--env", "E1", "--metrics", "G2,G6"])
        self.assertEqual(bench.arguments(["--variant", "A", "--env", "E1", "--metric", "G2"]).tmp_root, tempfile.gettempdir())
        stderr = "300,,sched:sched_wakeup,1000,100.00,,\n"
        with patch.object(instruments, "_run_cancellable", return_value=(0, stderr)):
            self.assertEqual(instruments.wakeup_events({10}, 10), 30)
        denied = (255, "<not counted>,,sched:sched_wakeup,,,,\n")
        with patch.object(instruments, "_run_cancellable", return_value=denied), \
                self.assertRaises(instruments.InstrumentError):
            instruments.wakeup_events({10}, 1)


class SessionTests(unittest.TestCase):
    def setUp(self):
        bus = patch.object(bench_session, "_session_bus_available", return_value=True)
        bus.start()
        self.addCleanup(bus.stop)

    def probes(self, logind, screensaver):
        if logind is not None and "=" not in logind:
            logind = f"Type=wayland\nLockedHint={logind}"
        replies = {tuple(bench_session.LOCKED_HINT): logind, tuple(bench_session.SCREENSAVER_ACTIVE): screensaver,
                   tuple(bench_session.SCREENSAVER_OWNED): None if screensaver is None else "(true,)"}

        def run(command, **_):
            reply = replies[tuple(command)]
            if reply is None:
                raise FileNotFoundError(command[0])
            return bench_session.subprocess.CompletedProcess(command, 0, reply + "\n", "")
        return patch.object(bench_session.subprocess, "run", side_effect=run)

    def test_lock_state(self):
        cases = [("yes", "(false,)", True), ("no", "(true,)", True), ("no", "(false,)", False),
                 ("no", None, False), (None, "(true,)", True), (None, None, None), ("garbage", None, None)]
        for logind, screensaver, expected in cases:
            with self.subTest(logind=logind, screensaver=screensaver), self.probes(logind, screensaver):
                self.assertIs(bench_session.session_locked(), expected)

    def test_non_graphical_session_is_unknown(self):
        with self.probes("Type=tty\nLockedHint=no", None):
            self.assertIsNone(bench_session.session_locked())

    def test_no_session_bus_skips_gdbus(self):
        def run(command, **_):
            if command[0] == "gdbus":
                raise AssertionError("gdbus called without a session bus")
            return bench_session.subprocess.CompletedProcess(command, 0, "Type=x11\nLockedHint=no\n", "")
        with patch.object(bench_session, "_session_bus_available", return_value=False), \
                patch.object(bench_session.subprocess, "run", side_effect=run):
            self.assertIs(bench_session.session_locked(), False)

    def test_unowned_screensaver_is_not_called(self):
        def run(command, **_):
            if command == bench_session.SCREENSAVER_ACTIVE:
                raise AssertionError("GetActive called on an unowned name")
            reply = "(false,)" if command == bench_session.SCREENSAVER_OWNED else "Type=wayland\nLockedHint=no"
            return bench_session.subprocess.CompletedProcess(command, 0, reply + "\n", "")
        with patch.object(bench_session.subprocess, "run", side_effect=run):
            self.assertIs(bench_session.session_locked(), False)

    def test_hung_probe_is_unknown(self):
        hung = bench_session.subprocess.TimeoutExpired(["loginctl"], 2)
        with patch.object(bench_session.subprocess, "run", side_effect=hung):
            self.assertIsNone(bench_session.session_locked())

    def test_failed_probe_is_unknown(self):
        failed = bench_session.subprocess.CompletedProcess([], 1, "yes\n", "")
        with patch.object(bench_session.subprocess, "run", return_value=failed):
            self.assertIsNone(bench_session.session_locked())


class CancellableRunTests(unittest.TestCase):
    def test_cancel_kills_running_command(self):
        import threading
        import time
        import bench_instruments as instruments
        cancel = threading.Event()
        threading.Timer(.2, cancel.set).start()
        began = time.monotonic()
        with self.assertRaisesRegex(instruments.InstrumentError, "^INSTRUMENT_CANCELLED: "):
            instruments._run_cancellable([sys.executable, "-c", "import time;time.sleep(30)"], 60, cancel)
        self.assertLess(time.monotonic() - began, 5)

    def test_completed_command_returns_stderr(self):
        import bench_instruments as instruments
        code, stderr = instruments._run_cancellable(
            [sys.executable, "-c", "import sys;print('row',file=sys.stderr);sys.exit(3)"], 10, None)
        self.assertEqual((code, stderr), (3, "row\n"))


class CaptureTests(unittest.TestCase):
    def setUp(self):
        unlocked = patch.object(bench_capture, "session_locked", return_value=False)
        unlocked.start()
        self.addCleanup(unlocked.stop)

    def test_locked_session_refused_before_launch(self):
        with patch.object(bench_capture, "session_locked", return_value=True), \
                patch.object(bench_capture.subprocess, "Popen") as popen, \
                self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED: the session is locked before launch; .*unlock"):
            ready([sys.executable, "-c", "pass"], dict(os.environ), 1, True)
        popen.assert_not_called()

    def test_lock_during_wait_reported(self):
        states = iter([False])
        with patch.object(bench_capture, "session_locked", side_effect=lambda: next(states, True)), \
                self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED: the session is locked after waiting for READY"):
            ready([sys.executable, "-c", "import time;time.sleep(30)"], dict(os.environ), .2, True)

    def test_timeout_names_presentation_requirement(self):
        code = "import sys,time;print('booted',file=sys.stderr,flush=True);time.sleep(30)"
        with self.assertRaisesRegex(CaptureError, "^READY_TIMEOUT: no READY within 0.5 s; .*minimize.*Last stderr: booted$"):
            ready([sys.executable, "-c", code], dict(os.environ), .5, True)

    def test_lock_during_sampling_stops_capture(self):
        from types import SimpleNamespace
        proc = bench_capture.subprocess.Popen([sys.executable, "-c", "import time;time.sleep(30)"],
                                              start_new_session=True, stderr=bench_capture.subprocess.PIPE)
        args = SimpleNamespace(timeout=1, duration=60)
        with patch.object(bench_capture, "ready", return_value=(proc, 1.0)), \
                patch.object(bench_capture, "session_locked", return_value=True), \
                self.assertRaisesRegex(CaptureError, "^SESSION_LOCKED: the session is locked during G3"):
            bench_capture.capture("G3", args, ["app"], {})
        self.assertIsNotNone(proc.poll())

    def test_session_locked_exits_one(self):
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        with tempfile.TemporaryDirectory() as temp, \
                patch.object(bench.Originals, "capture", return_value=original), \
                patch.object(bench, "capture", side_effect=CaptureError("SESSION_LOCKED: x")), \
                patch.object(bench, "launch_command", return_value=["app"]), \
                patch.object(bench, "make_dirs"):
            self.assertEqual(bench.main(["--variant", "A", "--env", "E1", "--metric", "G2", "--binary", sys.executable,
                                         "--output", f"{temp}/out", "--tmp-root", f"{temp}/runs"]), 1)

    def test_shim_build_failure_exits_cleanly_and_seals_raw_dir(self):
        import bench_shim
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        with tempfile.TemporaryDirectory() as temp, \
                patch.object(bench.Originals, "capture", return_value=original), \
                patch.object(bench_shim, "build", side_effect=CaptureError("MISSING_TOOL: cc")), \
                patch.object(bench, "make_dirs"):
            code = bench.main(["--variant", "A", "--env", "E1", "--metric", "G2", "--collect", "presentation",
                               "--binary", sys.executable, "--output", f"{temp}/out", "--tmp-root", f"{temp}/runs"])
            self.assertEqual(code, 30)
            manifests = list(Path(temp, "out", "raw").glob("*/provenance.json"))
            self.assertEqual(len(manifests), 1)
            self.assertEqual(json.loads(manifests[0].read_text())["status"], "failed")

    def test_presentation_arguments_validated(self):
        base = ["--variant", "A", "--env", "E1", "--collect", "presentation"]
        with self.assertRaises(SystemExit) as caught:
            bench.arguments([*base, "--metric", "G4"])
        self.assertEqual(caught.exception.code, 2)
        with self.assertRaises(SystemExit):
            bench.arguments([*base, "--metric", "G2", "--capture-csv", "x.csv"])
        self.assertEqual(bench.arguments([*base, "--metric", "G2"]).collect, "presentation")
        self.assertEqual(bench.arguments(["--variant", "A", "--env", "E1", "--metric", "G2"]).collect, "csv")

    def test_presentation_errors_exit_forty(self):
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        for message in ("REFRESH_MISMATCH: x", "NO_PRESENTATION_GLOBAL: x", "PRESENTATION_CLOCK: x"):
            with self.subTest(message=message), tempfile.TemporaryDirectory() as temp, \
                    patch.object(bench.Originals, "capture", return_value=original), \
                    patch.object(bench, "capture", side_effect=CaptureError(message)), \
                    patch.object(bench, "launch_command", return_value=["app"]), \
                    patch.object(bench, "make_dirs"):
                self.assertEqual(bench.main(["--variant", "A", "--env", "E1", "--metric", "G2", "--binary", sys.executable,
                                             "--output", f"{temp}/out", "--tmp-root", f"{temp}/runs"]), 40)

    def test_foreign_capture_is_stale(self):
        original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)
        with tempfile.TemporaryDirectory() as temp:
            foreign = Path(temp) / "foreign.csv"
            foreign.write_text("frame_ms\n8.3\n")
            with patch.object(bench.Originals, "capture", return_value=original), \
                    patch("sys.stderr") as stderr:
                code = bench.main(["--variant", "A", "--env", "E1", "--metric", "G4", "--capture-csv", str(foreign),
                                   "--output", f"{temp}/out", "--tmp-root", f"{temp}/runs"])
            self.assertEqual(code, 1)
            written = "".join(call.args[0] for call in stderr.write.call_args_list)
            self.assertTrue(written.startswith("STALE_CAPTURE"), written)

    def test_presentation_g2_measures_first_present(self):
        from types import SimpleNamespace
        app = ("import os,sys,time\n"
               "d=os.environ['CROSSHOOK_BENCH_PRESENT_DIR']\n"
               "t=time.monotonic_ns()\n"
               "open(d+'/wlpresent-1.log','w').write('H\\tclock_id=1\\nS\\t1\\t7\\ttoplevel\\n'"
               "+'C\\t%d\\t7\\t1\\t1\\n'%t+'P\\t%d\\t1\\t%d\\t8333333\\t1\\t0\\n'%(t,t))\n"
               "print('READY',time.monotonic_ns(),file=sys.stderr,flush=True)\n"
               "time.sleep(30)\n")
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp) / "home"
            home.mkdir()
            args = SimpleNamespace(collect="presentation", iterations=2, timeout=10, refresh_hz=120,
                                   cold_cache_ack=True)
            rows = bench_capture.capture("G2", args, [sys.executable, "-c", app], {**os.environ, "HOME": str(home)})
        measured = [row for row in rows if row["iteration"] != "summary"]
        self.assertEqual(len(measured), 2)
        for row in measured:
            self.assertGreaterEqual(row["first_present_ms"], 0)
        for key in ("p50_ms", "p95_ms", "first_present_p50_ms", "first_present_p95_ms"):
            self.assertIn(key, rows[-1])

    def test_percentiles(self):
        self.assertEqual(stats([1, 2, 3])["p50_ms"], 2)
        self.assertAlmostEqual(stats([1, 2, 3])["p95_ms"], 2.9)

    def test_over_interval_percent(self):
        self.assertEqual(over_interval_percent([8.342] * 4, 119.88), 0)
        self.assertEqual(over_interval_percent([16.7] * 4, 119.88), 100)
        self.assertEqual(over_interval_percent([8.342, 8.342, 16.7, 8.4], 119.88), 25)

    def test_ready_monotonic_success(self):
        command = [sys.executable, "-c", "import time,sys;print('READY',time.monotonic_ns(),file=sys.stderr)"]
        proc, ms = ready(command, dict(os.environ), 5, True)
        try:
            self.assertGreaterEqual(ms, 0)
            self.assertEqual(proc.returncode, 0)
        finally:
            stop(proc)
            proc.stderr.close()

    def test_missing_marker_and_timeout(self):
        for code in ("pass", "import time;time.sleep(30)"):
            with self.subTest(code=code), self.assertRaises(CaptureError):
                ready([sys.executable, "-c", code], dict(os.environ), .2, True)

    def test_malformed_marker(self):
        with self.assertRaisesRegex(CaptureError, "MALFORMED_READY"):
            ready([sys.executable, "-c", "import sys;print('READY nope',file=sys.stderr)"], dict(os.environ), 2, True)


if __name__ == "__main__":
    unittest.main()
