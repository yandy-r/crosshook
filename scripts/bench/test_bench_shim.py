"""Presentation shim build regression checks; stdlib only."""
from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_shim
from bench_capture import CaptureError


def done(argv, code=0, stdout="", stderr=""):
    return subprocess.CompletedProcess(argv, code, stdout, stderr)


class Runner:
    """Fake subprocess.run: records argv, fakes pkg-config, and writes the .so on cc/flatpak run."""

    def __init__(self, out: Path, fail=None):
        self.out, self.fail, self.calls = out, fail, []

    def __call__(self, argv, **kwargs):
        self.calls.append((list(argv), kwargs))
        if self.fail:
            result = self.fail(argv)
            if result is not None:
                return result
        if argv[0] == "pkg-config":
            return done(argv, stdout="-I/usr/include/x -lwayland-client\n")
        if argv[0] in ("cc", "flatpak") and argv[1] != "info":
            (self.out / "libwlpresent.so").write_bytes(b"\x7fELF")
        return done(argv)


class BuildTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.out = Path(self.temp.name)

    def build(self, target, runner, which=lambda tool: f"/usr/bin/{tool}"):
        with patch.object(bench_shim.subprocess, "run", runner), patch.object(bench_shim.shutil, "which", which):
            return bench_shim.build(self.out, target)

    def test_native_argv(self):
        runner = Runner(self.out)
        result = self.build("native", runner)
        argv = [call[0] for call in runner.calls]
        self.assertEqual(argv[0], ["pkg-config", "--atleast-version=1.24", "wayland-client"])
        self.assertEqual(argv[1][:2], ["wayland-scanner", "client-header"])
        self.assertEqual(argv[2][:2], ["wayland-scanner", "private-code"])
        self.assertEqual(argv[3], ["pkg-config", "--cflags", "--libs", "wayland-client"])
        cc = argv[4]
        self.assertEqual(cc[0], "cc")
        for flag in ("-shared", "-fPIC", "-pthread", f"-I{self.out}", "-I/usr/include/x", "-lwayland-client",
                     *bench_shim.CFLAGS):
            self.assertIn(flag, cc)
        self.assertNotIn("-Werror", cc)  # a new compiler warning must not break a measurement
        self.assertEqual(cc[-1], "-ldl")
        self.assertTrue(all(call[1]["timeout"] == 120 for call in runner.calls))
        self.assertEqual((result.built_in, result.path), ("host", self.out / "libwlpresent.so"))

    def test_flatpak_argv(self):
        runner = Runner(self.out)
        result = self.build("flatpak", runner)
        argv = [call[0] for call in runner.calls]
        self.assertEqual(argv[0], ["flatpak", "info", bench_shim.SDK])
        self.assertEqual(len(argv), 2)
        run = argv[1]
        self.assertEqual(run[:3], ["flatpak", "run", "--command=sh"])
        self.assertIn(f"--filesystem={self.out}", run)
        self.assertIn(f"--filesystem={bench_shim.SOURCE_DIR}:ro", run)
        self.assertEqual(run[run.index("-c") + 1], bench_shim.SCRIPT)
        self.assertEqual(run[run.index("sh", run.index("-c")):],
                         ["sh", str(bench_shim.PROTOCOL_XML), str(bench_shim.SOURCE), str(self.out),
                          *bench_shim.CFLAGS])
        self.assertNotIn("-Werror", run)
        self.assertNotIn(str(self.out), bench_shim.SCRIPT)
        self.assertEqual(result.built_in, "org.gnome.Sdk//50")

    def test_unsupported_out_dir_refused_before_any_command(self):
        for name in ("a:b", "a,b"):
            with self.subTest(name=name):
                runner = Runner(self.out)
                with patch.object(bench_shim.subprocess, "run", runner):
                    with self.assertRaisesRegex(CaptureError, "^SHIM_BUILD_FAILED: unsupported path$"):
                        bench_shim.build(self.out / name, "flatpak")
                self.assertEqual(runner.calls, [])

    def test_check_keeps_werror_with_the_shared_flags(self):
        runner = Runner(self.out)
        with patch.object(bench_shim.subprocess, "run", runner), \
                patch.object(bench_shim.shutil, "which", lambda tool: f"/usr/bin/{tool}"):
            self.assertEqual(bench_shim.check(), 0)
        cc = next(call[0] for call in runner.calls if call[0][0] == "cc")
        for flag in ("-fsyntax-only", "-Werror", *bench_shim.CFLAGS):
            self.assertIn(flag, cc)

    def test_missing_tool(self):
        with self.assertRaisesRegex(CaptureError, "^MISSING_TOOL: cc$"):
            self.build("native", Runner(self.out), which=lambda tool: None if tool == "cc" else "/x")

    def test_old_libwayland_is_refused_before_compiling(self):
        def old(argv):
            return done(argv, 1) if "--atleast-version=1.24" in argv else None
        runner = Runner(self.out, fail=old)
        with self.assertRaisesRegex(CaptureError, r"^MISSING_TOOL: libwayland-client >= 1\.24$"):
            self.build("native", runner)
        self.assertEqual([call[0][0] for call in runner.calls], ["pkg-config"])
        runner = Runner(self.out, fail=old)
        with patch.object(bench_shim.subprocess, "run", runner), \
                patch.object(bench_shim.shutil, "which", lambda tool: f"/usr/bin/{tool}"), \
                patch("sys.stderr") as stderr:
            self.assertEqual(bench_shim.check(), 1)
        written = "".join(call.args[0] for call in stderr.write.call_args_list)
        self.assertIn("MISSING_TOOL: libwayland-client >= 1.24", written)

    def test_flatpak_script_checks_libwayland_and_reports_it(self):
        self.assertIn("pkg-config --atleast-version=1.24 wayland-client", bench_shim.SCRIPT)
        runner = Runner(self.out, fail=lambda argv: done(
            argv, 1, stderr="noise\nMISSING_TOOL: libwayland-client >= 1.24\n") if argv[1] == "run" else None)
        with self.assertRaisesRegex(CaptureError, r"^MISSING_TOOL: libwayland-client >= 1\.24$"):
            self.build("flatpak", runner)

    def test_missing_sdk(self):
        runner = Runner(self.out, fail=lambda argv: done(argv, 1, stderr="not installed") if argv[1] == "info" else None)
        with self.assertRaisesRegex(CaptureError, r"^MISSING_TOOL: org\.gnome\.Sdk//50$"):
            self.build("flatpak", runner)
        self.assertEqual(len(runner.calls), 1)

    def test_compile_failure_uses_last_stderr_line(self):
        def fail(argv):
            return done(argv, 1, stderr="first\nerror: bad thing\n\n") if argv[0] == "cc" else None
        with self.assertRaisesRegex(CaptureError, "^SHIM_BUILD_FAILED: error: bad thing$"):
            self.build("native", Runner(self.out, fail=fail))

    def test_timeout(self):
        def fail(argv):
            if argv[0] == "cc":
                raise subprocess.TimeoutExpired(argv, 120)
        with self.assertRaisesRegex(CaptureError, "^SHIM_BUILD_FAILED: timeout$"):
            self.build("native", Runner(self.out, fail=fail))


def _native_toolchain() -> bool:
    return bool(shutil.which("cc") and shutil.which("wayland-scanner") and shutil.which("pkg-config")
                and subprocess.run(["pkg-config", "--atleast-version=1.24", "wayland-client"]).returncode == 0)


@unittest.skipUnless(_native_toolchain(), "native toolchain with libwayland-client >= 1.24 unavailable")
class RealBuildTests(unittest.TestCase):
    def test_native_build(self):
        # build() has no -Werror, so warnings from a newer compiler never fail this test.
        with tempfile.TemporaryDirectory() as tmp:
            result = bench_shim.build(Path(tmp), "native")
            self.assertTrue(result.path.is_file())
            self.assertRegex(result.so_sha256, r"^[0-9a-f]{64}$")
            self.assertRegex(result.source_sha256, r"^[0-9a-f]{64}$")
            self.assertEqual(result.built_in, "host")


class ShippingGuardTests(unittest.TestCase):
    def test_nothing_that_ships_references_the_shim(self):
        root = Path(__file__).resolve().parents[2]
        shipped = [path for path in (root / "packaging").rglob("*") if path.is_file()]
        shipped += (root / "scripts").glob("build-*.sh")
        shipped.append(root / "scripts" / "prepare-release.sh")
        self.assertTrue(any(path.parent.name == "scripts" for path in shipped))
        for path in shipped:
            if not path.is_file():
                continue
            text = path.read_text(errors="replace")
            for needle in ("wlpresent", "scripts/bench/presentation"):
                with self.subTest(path=str(path.relative_to(root)), needle=needle):
                    self.assertNotIn(needle, text)


if __name__ == "__main__":
    unittest.main()
