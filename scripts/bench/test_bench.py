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
from bench_capture import CaptureError, ready, stats, stop
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
                               "CROSSHOOK_BENCH_LOG": "/secret", "XDG_RUNTIME_DIR": str(self.root)})
        self.assertEqual(env["HOME"], str(root / "home"))
        self.assertEqual(env["CROSSHOOK_FLATPAK_HOST_XDG"], "0")
        self.assertNotIn("CROSSHOOK_BENCH_LOG", env)
        self.assertNotIn("HOST_XDG_DATA_HOME", env)

    def test_flatpak_launcher_recipe(self):
        args = bench.arguments(["--variant", "A", "--env", "E1", "--flatpak", "--metric", "G2"])
        root = self.root / "crosshook-bench-owned"
        env = build_env(root, {}, flatpak=True)
        command = bench.launch_command(args, env)
        self.assertIn("--nofilesystem=host:reset", command)
        self.assertEqual(env["HOME"], str(root / "home"))
        with self.assertRaises(GuardError):
            bench.launch_command(args, {"HOME": str(self.home)})

    def test_dry_run_all_schemas(self):
        output = self.root / "results"
        with patch.object(bench.Originals, "capture", return_value=self.original):
            result = bench.main(["--variant", "A", "--env", "E1", "--dry-run", "--output", str(output)])
        self.assertEqual(result, 0)
        for metric, schema in bench.SCHEMAS.items():
            with (output / "A/E1" / f"{metric}.csv").open() as stream:
                rows = list(csv.DictReader(stream))
            self.assertEqual(list(rows[0]), schema.split(","))
            self.assertEqual(rows[0]["status"], "dry-run")
        self.assertTrue(json.loads((output / "A/E1/meta.json").read_text())["synthetic"])

    def test_real_runs_fail_before_writes_without_instrumentation(self):
        output = self.root / "real"
        cases = [(["--metric", "G1"], 20), (["--metric", "G4"], 30)]
        for extra, code in cases:
            with self.subTest(extra=extra), patch.object(bench.Originals, "capture", return_value=self.original), \
                    patch.object(bench.shutil, "which", return_value=None):
                self.assertEqual(bench.main(["--variant", "A", "--env", "E1", "--output", str(output), *extra]), code)
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
                patch.object(instruments, "output", return_value="10, 12\n11, 5\n99, 50\n"):
            self.assertEqual(instruments.gpu_memory_mb({10, 11}), 17)
        with patch.object(instruments.shutil, "which", return_value=None):
            self.assertEqual(instruments.gpu_memory_mb({10}), "unavailable")
        with self.assertRaises(SystemExit):
            bench.arguments(["--variant", "A", "--env", "E1", "--metrics", "G2,G6"])
        stderr = "300,,sched:sched_wakeup,1000,100.00,,\n"
        with patch.object(instruments.subprocess, "run",
                          return_value=instruments.subprocess.CompletedProcess([], 0, "", stderr)):
            self.assertEqual(instruments.wakeup_events({10}, 10), 30)
        denied = instruments.subprocess.CompletedProcess([], 255, "", "<not counted>,,sched:sched_wakeup,,,,\n")
        with patch.object(instruments.subprocess, "run", return_value=denied), \
                self.assertRaises(instruments.InstrumentError):
            instruments.wakeup_events({10}, 1)


class CaptureTests(unittest.TestCase):
    def test_percentiles(self):
        self.assertEqual(stats([1, 2, 3])["p50_ms"], 2)
        self.assertAlmostEqual(stats([1, 2, 3])["p95_ms"], 2.9)

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
