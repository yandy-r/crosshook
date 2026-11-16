from __future__ import annotations

import os
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

import bench_gpu
from bench_gpu import FdinfoScan, GpuSample, GpuToolError

# Real `nvidia-smi pmon -s m -c 1` output captured on E1 (trailing blanks kept).
PMON = (
    "# gpu         pid   type     fb   ccpm    command \n"
    "# Idx           #    C/G     MB     MB    name \n"
    "    0     108538   C+G    154      0    kwin_wayland   \n"
    "    0     108568     G    136      0    plasmashell    \n"
    "    0     108716     G    365      0    plasma-keyboard\n"
    "    0     108770     G      3      0    Xwayland       \n"
    "    0     109076     G      2      0    xwaylandvideobr\n"
    "    0     109492   C+G     12      0    .config        \n"
    "    0     110247   C+G    593      0    t3code-v2 --gpu\n"
    "    0     118964   C+G    120      0    Grok Bo        \n"
    "    0     122929     G     11      0    krunner        \n"
    "    0     249714   C+G    190      0    renderD128 --cr\n"
    "    0     481656   C+G    288      0    herdr-app      \n"
)
PMON_EMPTY = (
    "# gpu         pid   type     fb   ccpm    command \n"
    "# Idx           #    C/G     MB     MB    name \n"
)

AMDGPU = """pos:\t0
flags:\t02100002
mnt_id:\t26
ino:\t1234
drm-driver:\tamdgpu
drm-client-id:\t42
drm-pdev:\t0000:03:00.0
pasid:\t32770
drm-total-cpu:\t0
drm-shared-cpu:\t0
drm-resident-cpu:\t0
drm-total-gtt:\t65536 KiB
drm-shared-gtt:\t0
drm-resident-gtt:\t4096 KiB
drm-total-vram:\t1048576 KiB
drm-shared-vram:\t0
drm-resident-vram:\t524288 KiB
drm-memory-vram:\t524288 KiB
amd-evicted-vram:\t0
"""
# 65536 KiB + 1048576 KiB = 1088 MiB
AMDGPU_MB = 1088.0

I915 = """pos:\t0
flags:\t02100002
drm-driver:\ti915
drm-pdev:\t0000:00:02.0
drm-client-id:\t7
drm-engine-render:\t123456 ns
drm-total-system0:\t4096 KiB
drm-shared-system0:\t0
drm-resident-system0:\t2048 KiB
drm-total-local0:\t2 MiB
"""
I915_MB = 6.0

XE = """pos:\t0
drm-driver:\txe
drm-client-id:\t9
drm-pdev:\t0000:04:00.0
drm-total-gtt:\t8388608
drm-resident-gtt:\t4194304
drm-total-vram0:\t16 MiB
drm-resident-vram0:\t8 MiB
drm-cycles-rcs:\t100
"""
XE_MB = 8.0 + 16.0

RESIDENT_ONLY = "drm-driver:\tpanfrost\ndrm-client-id:\t3\ndrm-resident-memory:\t3 MiB\n"
NO_KEYS = "pos:\t0\nflags:\t0100002\nmnt_id:\t41\nino:\t1304\n"
NO_MEMORY_KEYS = "drm-driver:\tx\ndrm-client-id:\t5\ndrm-engine-gfx:\t1 ns\n"


def scans(mapping: dict[int, tuple[list[str], int]]):
    def reader(pid: int) -> FdinfoScan:
        texts, denied = mapping.get(pid, ([], 0))
        return FdinfoScan(tuple(texts), denied)
    return reader


def pmon_runner(stdout: str, returncode: int = 0):
    calls: list[list[str]] = []

    def runner(command, timeout, cancel):
        calls.append(command)
        return returncode, stdout
    runner.calls = calls  # type: ignore[attr-defined]
    return runner


def failing_runner(error: BaseException):
    def runner(command, timeout, cancel):
        raise error
    return runner


def no_fdinfo(_pid: int) -> FdinfoScan:
    return FdinfoScan(())


class ParseFdinfoTests(unittest.TestCase):
    def test_drivers(self):
        for name, text, mb in (("amdgpu", AMDGPU, AMDGPU_MB), ("i915", I915, I915_MB), ("xe", XE, XE_MB)):
            with self.subTest(name):
                parsed = bench_gpu.parse_fdinfo(text)
                assert parsed is not None
                self.assertAlmostEqual(parsed[1], mb)

    def test_key_is_pdev_and_client(self):
        parsed = bench_gpu.parse_fdinfo(AMDGPU)
        assert parsed is not None
        self.assertEqual(parsed[0], ("0000:03:00.0", "42"))

    def test_resident_fallback_and_bytes_default(self):
        parsed = bench_gpu.parse_fdinfo(RESIDENT_ONLY)
        assert parsed is not None
        self.assertEqual(parsed, (("", "3"), 3.0))

    def test_total_wins_over_resident(self):
        text = "drm-client-id:\t1\ndrm-total-memory:\t2 MiB\ndrm-resident-memory:\t9 MiB\n"
        parsed = bench_gpu.parse_fdinfo(text)
        assert parsed is not None
        self.assertEqual(parsed[1], 2.0)

    def test_absent_or_malformed_is_none_not_zero(self):
        for text in (NO_KEYS, NO_MEMORY_KEYS, "", "drm-total-vram:\t5 MiB\n",
                     "drm-client-id:\t1\ndrm-total-vram:\tlots\n",
                     "drm-client-id:\t1\ndrm-total-vram:\t-5 KiB\n",
                     "drm-client-id:\t1\ndrm-total-vram:\t5 TiB\n"):
            with self.subTest(text=text):
                self.assertIsNone(bench_gpu.parse_fdinfo(text))

    def test_malformed_region_is_skipped_but_others_count(self):
        text = "drm-client-id:\t1\ndrm-total-gtt:\tbad\ndrm-total-vram:\t4 MiB\n"
        parsed = bench_gpu.parse_fdinfo(text)
        assert parsed is not None
        self.assertEqual(parsed[1], 4.0)

    def test_first_duplicate_key_wins(self):
        text = "drm-client-id:\t1\ndrm-total-vram:\t4 MiB\ndrm-total-vram:\t400 MiB\n"
        parsed = bench_gpu.parse_fdinfo(text)
        assert parsed is not None
        self.assertEqual(parsed[1], 4.0)


class ParsePmonTests(unittest.TestCase):
    def test_real_output_filters_by_pid(self):
        totals = bench_gpu.parse_pmon(PMON, {110247, 249714})
        self.assertEqual(totals, bench_gpu.PmonTotals(2, 2, 783.0))

    def test_empty_pmon_lists_nothing(self):
        self.assertEqual(bench_gpu.parse_pmon(PMON_EMPTY, {1}), bench_gpu.PmonTotals(0, 0, 0.0))

    def test_dash_is_listed_but_not_numeric(self):
        totals = bench_gpu.parse_pmon("0 12 G - - x\n0 10 G 5 0 y\n", {10, 12})
        self.assertEqual(totals, bench_gpu.PmonTotals(2, 1, 5.0))

    def test_multi_gpu_rows_sum(self):
        self.assertEqual(bench_gpu.parse_pmon("0 10 G 12 0 a\n1 10 C 3 0 a\n", {10}).mb, 15.0)

    def test_malformed_wanted_row_raises(self):
        for row in ("0 10 G nope 0 app\n", "0 10 G -4 0 app\n", "0 10 G nan 0 app\n", "0 10 G inf 0 app\n"):
            with self.subTest(row=row), self.assertRaises(ValueError):
                bench_gpu.parse_pmon(row, {10})

    def test_malformed_unwanted_or_short_rows_are_ignored(self):
        text = "garbage\n0 x G 5 0 a\n0 99 G nope 0 a\n0 10 G\n0 10 G 7 0 ok\n"
        self.assertEqual(bench_gpu.parse_pmon(text, {10}), bench_gpu.PmonTotals(1, 1, 7.0))

    def test_row_cap(self):
        text = "0 99 G 1 0 x\n" * bench_gpu.MAX_PMON_ROWS + "0 10 G 7 0 late\n"
        self.assertEqual(bench_gpu.parse_pmon(text, {10}).listed, 0)


class SampleFdinfoTests(unittest.TestCase):
    def sample(self, mapping, **kwargs):
        runner = kwargs.setdefault("runner", pmon_runner(PMON))
        kwargs.setdefault("which", lambda _name: "/usr/bin/nvidia-smi")
        return bench_gpu.sample(set(mapping) or {1}, fdinfo_reader=scans(mapping), **kwargs), runner

    def test_sums_across_drivers_and_skips_tool(self):
        result, runner = self.sample({1: ([AMDGPU, NO_KEYS], 0), 2: ([I915, XE], 0)})
        self.assertEqual(result.source, "fdinfo")
        self.assertEqual(result.reason, "")
        self.assertAlmostEqual(result.mb or 0, AMDGPU_MB + I915_MB + XE_MB)
        self.assertEqual(runner.calls, [])
        self.assertIsNone(result.tool_path)

    def test_deduplicates_shared_client_across_fds_and_pids(self):
        result, _ = self.sample({1: ([AMDGPU, AMDGPU], 0), 2: ([AMDGPU], 0)})
        self.assertEqual(result.mb, AMDGPU_MB)

    def test_same_client_id_on_other_device_is_distinct(self):
        other = AMDGPU.replace("0000:03:00.0", "0000:0a:00.0")
        result, _ = self.sample({1: ([AMDGPU, other], 0)})
        self.assertEqual(result.mb, AMDGPU_MB * 2)

    def test_denied_is_counted_and_reported(self):
        result, _ = self.sample({1: ([AMDGPU], 3), 2: ([], 1)})
        self.assertEqual(result.denied, 4)
        self.assertEqual(result.mb, AMDGPU_MB)

    def test_zero_is_a_real_fdinfo_reading(self):
        text = "drm-client-id:\t1\ndrm-total-vram:\t0\n"
        result, _ = self.sample({1: ([text], 0)})
        self.assertEqual(result, GpuSample(0.0, "fdinfo", "", 0, None))


class SampleNvidiaTests(unittest.TestCase):
    def sample(self, pids, runner, which=lambda _name: "/usr/bin/nvidia-smi", reader=no_fdinfo):
        return bench_gpu.sample(pids, fdinfo_reader=reader, runner=runner, which=which)

    def test_falls_through_to_pmon_with_real_output(self):
        runner = pmon_runner(PMON)
        result = self.sample({110247, 249714}, runner)
        self.assertEqual(result, GpuSample(783.0, "nvidia-smi", "", 0, "/usr/bin/nvidia-smi"))
        self.assertEqual(runner.calls, [["/usr/bin/nvidia-smi", "pmon", "-s", "m", "-c", "1"]])

    def test_nvidia_drm_without_keys_falls_through(self):
        reader = scans({110247: ([NO_KEYS, NO_MEMORY_KEYS], 0)})
        self.assertEqual(self.sample({110247}, pmon_runner(PMON), reader=reader).mb, 593.0)

    def test_listed_pids_whose_fb_is_all_dash_are_unavailable_not_zero(self):
        result = self.sample({12}, pmon_runner("0 12 G - - x\n"))
        self.assertEqual(result, GpuSample(None, "unavailable", "no_numeric_fb", 0, "/usr/bin/nvidia-smi"))
        both = self.sample({12, 13}, pmon_runner("0 12 G - - x\n0 13 C - - y\n"))
        self.assertEqual((both.mb, both.source, both.reason), (None, "unavailable", "no_numeric_fb"))

    def test_a_numeric_fb_of_zero_is_a_real_zero(self):
        result = self.sample({12}, pmon_runner("0 12 G 0 0 x\n"))
        self.assertEqual((result.mb, result.source, result.reason), (0.0, "nvidia-smi", ""))

    def test_one_numeric_fb_next_to_a_dash_is_summed(self):
        result = self.sample({12, 13}, pmon_runner("0 12 G - - x\n0 13 C 2 0 y\n"))
        self.assertEqual((result.mb, result.source), (2.0, "nvidia-smi"))

    def test_the_two_mib_figure_seen_on_e1_is_a_measured_value(self):
        result = self.sample({12}, pmon_runner("0 12 G 2 1 x\n"))
        self.assertEqual(result, GpuSample(2.0, "nvidia-smi", "", 0, "/usr/bin/nvidia-smi"))

    def test_empty_pmon_is_no_matching_pid(self):
        result = self.sample({110247}, pmon_runner(PMON_EMPTY))
        self.assertEqual(result, GpuSample(None, "unavailable", "no_matching_pid", 0, "/usr/bin/nvidia-smi"))

    def test_unlisted_pid_is_no_matching_pid(self):
        self.assertEqual(self.sample({42}, pmon_runner(PMON)).reason, "no_matching_pid")

    def test_empty_pid_set_is_no_matching_pid(self):
        self.assertEqual(self.sample(set(), pmon_runner(PMON)).reason, "no_matching_pid")

    def test_missing_tool_without_readable_fdinfo_is_no_tool(self):
        runner = pmon_runner(PMON)
        result = self.sample({1}, runner, which=lambda _name: None)
        self.assertEqual(result, GpuSample(None, "unavailable", "no_tool", 0, None))
        self.assertEqual(runner.calls, [])

    def test_missing_tool_with_readable_keyless_fdinfo_is_no_drm_keys(self):
        reader = scans({1: ([NO_KEYS], 0)})
        result = self.sample({1}, pmon_runner(PMON), which=lambda _name: None, reader=reader)
        self.assertEqual((result.mb, result.source, result.reason), (None, "unavailable", "no_drm_keys"))

    def test_malformed_row_is_tool_failed(self):
        result = self.sample({10}, pmon_runner("0 10 G nope 0 app\n"))
        self.assertEqual((result.mb, result.reason), (None, "tool_failed"))

    def test_nonzero_exit_is_tool_failed(self):
        self.assertEqual(self.sample({110247}, pmon_runner(PMON, returncode=9)).reason, "tool_failed")

    def test_runner_errors_are_tool_failed(self):
        for error in (GpuToolError("TIMEOUT"), OSError("boom"), subprocess.SubprocessError("x")):
            with self.subTest(error=type(error).__name__):
                result = self.sample({110247}, failing_runner(error))
                self.assertEqual((result.mb, result.source, result.reason),
                                 (None, "unavailable", "tool_failed"))
                self.assertEqual(result.tool_path, "/usr/bin/nvidia-smi")

    def test_never_zero_for_unavailable(self):
        for result in (self.sample({1}, pmon_runner(PMON_EMPTY)),
                       self.sample({1}, pmon_runner("0 1 G - - x\n")),
                       self.sample({1}, pmon_runner(PMON), which=lambda _name: None),
                       self.sample({1}, pmon_runner("", returncode=1))):
            self.assertIsNone(result.mb)
            self.assertEqual(result.source, "unavailable")
            self.assertTrue(result.reason)

    def test_timeout_and_cancel_are_forwarded(self):
        seen = {}

        def runner(command, timeout, cancel):
            seen.update(timeout=timeout, cancel=cancel)
            return 0, PMON
        cancel = threading.Event()
        bench_gpu.sample({110247}, fdinfo_reader=no_fdinfo, runner=runner,
                         which=lambda _name: "/x/nvidia-smi", cancel=cancel)
        self.assertEqual(seen, {"timeout": 3.0, "cancel": cancel})


class RunCaptureTests(unittest.TestCase):
    def test_captures_stdout_with_replacement_decoding(self):
        code, out = bench_gpu.run_capture(
            ["python3", "-c", "import sys; sys.stdout.buffer.write(b'ok \\xff\\n')"], 5, None)
        self.assertEqual((code, out), (0, "ok �\n"))

    def test_nonzero_exit_is_returned(self):
        code, _ = bench_gpu.run_capture(["python3", "-c", "raise SystemExit(3)"], 5, None)
        self.assertEqual(code, 3)

    def test_timeout_kills_and_raises(self):
        with self.assertRaisesRegex(GpuToolError, "TIMEOUT"):
            bench_gpu.run_capture(["python3", "-c", "import time; time.sleep(30)"], 0.2, None)

    def test_cancel_kills_and_raises(self):
        cancel = threading.Event()
        cancel.set()
        with self.assertRaisesRegex(GpuToolError, "CANCELLED"):
            bench_gpu.run_capture(["python3", "-c", "import time; time.sleep(30)"], 30, cancel)

    def test_more_than_the_output_limit_kills_the_command_and_raises(self):
        with patch.object(bench_gpu, "MAX_TOOL_OUTPUT_BYTES", 1000), \
                self.assertRaisesRegex(GpuToolError, "^OVERFLOW: python3"):
            bench_gpu.run_capture(
                ["python3", "-c", "import sys,time; sys.stdout.write('x'*5000); sys.stdout.flush(); time.sleep(30)"],
                30, None)

    def test_exactly_the_output_limit_is_accepted(self):
        with patch.object(bench_gpu, "MAX_TOOL_OUTPUT_BYTES", 1000):
            code, out = bench_gpu.run_capture(["python3", "-c", "import sys; sys.stdout.write('x'*1000)"], 5, None)
        self.assertEqual((code, len(out)), (0, 1000))

    def test_an_overflowing_tool_is_tool_failed_through_sample(self):
        def runner(command, timeout, cancel):
            raise GpuToolError("OVERFLOW: nvidia-smi")
        result = bench_gpu.sample({1}, fdinfo_reader=no_fdinfo, runner=runner, which=lambda _n: "/x/nvidia-smi")
        self.assertEqual((result.mb, result.reason), (None, "tool_failed"))

    def test_missing_binary_raises_oserror(self):
        with self.assertRaises(OSError):
            bench_gpu.run_capture(["/nonexistent/nvidia-smi"], 1, None)


class ReadFdinfoTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.dir = self.root / "7" / "fdinfo"
        self.dir.mkdir(parents=True)

    def read(self, **kwargs):
        return bench_gpu.read_fdinfo(7, proc_root=str(self.root), **kwargs)

    def test_reads_regular_files(self):
        (self.dir / "5").write_text(AMDGPU)
        (self.dir / "6").write_text(I915)
        scan = self.read()
        self.assertEqual(sorted(scan.texts), sorted([AMDGPU, I915]))
        self.assertEqual(scan.denied, 0)

    def test_symlinks_are_not_followed(self):
        target = self.root / "secret"
        target.write_text(AMDGPU)
        os.symlink(target, self.dir / "5")
        self.assertEqual(self.read(), FdinfoScan(()))

    def test_file_is_capped_at_8_kib(self):
        (self.dir / "5").write_text("drm-client-id:\t1\n" + "x" * 20000)
        (text,) = self.read().texts
        self.assertEqual(len(text), bench_gpu.MAX_FDINFO_BYTES)

    def test_fd_count_is_capped(self):
        for index in range(bench_gpu.MAX_FDS_PER_PID + 10):
            (self.dir / str(index)).write_text("pos:\t0\n")
        self.assertEqual(len(self.read().texts), bench_gpu.MAX_FDS_PER_PID)

    def test_eacces_is_skipped_and_counted(self):
        (self.dir / "5").write_text(AMDGPU)
        (self.dir / "6").write_text(I915)
        real_open = os.open

        def deny_five(path, *args, **kwargs):
            if str(path).endswith("/5"):
                raise PermissionError(13, "denied")
            return real_open(path, *args, **kwargs)
        with patch.object(bench_gpu.os, "open", side_effect=deny_five):
            scan = self.read()
        self.assertEqual(scan, FdinfoScan((I915,), 1))

    def test_foreign_uid_pid_is_denied_without_reading(self):
        (self.dir / "5").write_text(AMDGPU)
        self.assertEqual(self.read(uid=os.getuid() + 1), FdinfoScan((), 1))

    def test_missing_pid_is_empty_not_denied(self):
        self.assertEqual(bench_gpu.read_fdinfo(999, proc_root=str(self.root)), FdinfoScan(()))

    def test_unreadable_fdinfo_dir_is_denied(self):
        with patch.object(bench_gpu.os, "scandir", side_effect=PermissionError(13, "denied")):
            self.assertEqual(self.read(), FdinfoScan((), 1))

    def test_end_to_end_through_sample(self):
        (self.dir / "5").write_text(AMDGPU)
        (self.dir / "6").write_text(AMDGPU)
        result = bench_gpu.sample(
            {7}, fdinfo_reader=lambda pid: bench_gpu.read_fdinfo(pid, proc_root=str(self.root)))
        self.assertEqual(result, GpuSample(AMDGPU_MB, "fdinfo", "", 0, None))


if __name__ == "__main__":
    unittest.main()
