"""Presentation collection for G1/G2: per-metric dirs, host env isolation, refresh checks; stdlib only."""
import sys
import tempfile
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_collect
from bench_capture import CaptureError
from bench_collect import presentation_rows
from bench_present import Presented, check_refresh


def log_text(commit_ns: int, presented_ns: int, refresh_ns: int = 8333333, clock: str = "1") -> str:
    return (f"H\tclock_id={clock}\nS\t1\t7\ttoplevel\n"
            f"C\t{commit_ns}\t7\t1\t1\nP\t{presented_ns}\t1\t{presented_ns}\t{refresh_ns}\t1\t0\n")


class FakeReady:
    """Stands in for bench_capture.ready: records calls and writes a present log like the shim."""

    def __init__(self, offset_ns: int = 5_000_000, clock: str = "1", refresh_ns: int = 8333333):
        self.calls: list[tuple[list[str], dict[str, str]]] = []
        self.offset_ns, self.clock, self.refresh_ns = offset_ns, clock, refresh_ns

    def __call__(self, command, env, timeout, exit_after):
        self.calls.append((list(command), dict(env)))
        launch = time.monotonic_ns()
        flag = "--env=CROSSHOOK_BENCH_PRESENT_DIR="  # Flatpak gets the directory only as a flag
        present = Path(env.get("CROSSHOOK_BENCH_PRESENT_DIR") or
                       next(arg[len(flag):] for arg in command if arg.startswith(flag)))
        (present / "wlpresent-1.log").write_text(
            log_text(launch, launch + self.offset_ns, self.refresh_ns, self.clock))
        return SimpleNamespace(launch_ns=launch, stderr=None), 12.5


class CollectTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "crosshook-bench-x"
        (self.root / "home").mkdir(parents=True)
        self.env = {"HOME": str(self.root / "home"), "LD_PRELOAD": "/shim.so", "PATH": "/usr/bin"}
        self.args = SimpleNamespace(iterations=2, timeout=5, refresh_hz=120)
        self.fake = FakeReady()
        for target, replacement in (("ready", self.fake), ("stop", lambda proc: None)):
            patcher = patch.object(bench_collect, target, replacement)
            patcher.start()
            self.addCleanup(patcher.stop)

    def rows(self, metric="G2", command_for=None):
        return presentation_rows(self.args, command_for or (lambda env: ["app"]), self.env, metric)

    def test_rows_and_summary(self):
        rows = self.rows()
        measured = [row for row in rows if row["iteration"] != "summary"]
        self.assertEqual([row["iteration"] for row in measured], [1, 2])
        for row in measured:
            self.assertEqual(row["ready_ms"], 12.5)
            self.assertGreaterEqual(row["first_present_ms"], 5.0)
            self.assertEqual(row["status"], "measured")
        for key in ("p50_ms", "p95_ms", "first_present_p50_ms", "first_present_p95_ms"):
            self.assertIn(key, rows[-1])

    def test_dirs_are_per_metric(self):
        self.rows("G1")
        self.rows("G2")
        base = self.root / "present"
        for metric in ("G1", "G2"):
            for number in (1, 2):
                self.assertEqual(len(list((base / metric / f"iter-{number}").glob("wlpresent-*.log"))), 1)
        self.assertEqual(oct((base / "G1").stat().st_mode & 0o777), "0o700")
        self.assertEqual(oct((base / "G1/iter-1").stat().st_mode & 0o777), "0o700")

    def test_existing_dir_refused(self):
        (self.root / "present/G2/iter-1").mkdir(parents=True)
        with self.assertRaisesRegex(CaptureError, "^PRESENT_DIR_EXISTS: .*iter-1"):
            self.rows("G2")
        self.assertEqual(self.fake.calls, [])

    def test_present_before_launch_is_invalid(self):
        self.fake.offset_ns = -50_000_000
        with self.assertRaisesRegex(CaptureError, "^INVALID_PRESENT_CLOCK: "):
            self.rows()

    def test_wrong_clock_refused(self):
        self.fake.clock = "0"
        with self.assertRaisesRegex(CaptureError, "^PRESENTATION_CLOCK: "):
            self.rows()

    def test_refresh_checked_with_one_frame(self):
        self.fake.refresh_ns = 16_666_666  # 60 Hz against the default 120 Hz
        with self.assertRaisesRegex(CaptureError, "^REFRESH_MISMATCH: "):
            self.rows()
        frame = Presented(1, 1, 2, 16_666_666, 1, 0)
        with self.assertRaises(CaptureError):
            check_refresh([frame], 120)

    def test_native_keeps_preload_in_env(self):
        self.rows()
        command, env = self.fake.calls[0]
        self.assertEqual(env["LD_PRELOAD"], "/shim.so")
        self.assertIn("CROSSHOOK_BENCH_PRESENT_DIR", env)

    def test_flatpak_host_env_loses_shim_variables(self):
        args = SimpleNamespace(flatpak=True, branch="master", binary=None)
        self.rows(command_for=lambda env: bench.launch_command(args, env))
        command, env = self.fake.calls[0]
        present = self.root / "present/G2/iter-1"
        self.assertIn("--env=LD_PRELOAD=/shim.so", command)
        self.assertIn(f"--env=CROSSHOOK_BENCH_PRESENT_DIR={present}", command)
        self.assertNotIn("LD_PRELOAD", env)
        self.assertNotIn("CROSSHOOK_BENCH_PRESENT_DIR", env)
        self.assertEqual(env["PATH"], "/usr/bin")


if __name__ == "__main__":
    unittest.main()
