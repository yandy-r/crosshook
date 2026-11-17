"""bench_proc.run_cancellable: stderr capture (G7), bounded stdout capture (G8 GPU), kill on timeout, cancel and overflow."""
import os
import sys
import threading
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_proc
from bench_proc import CommandFailed, run_cancellable

PY = sys.executable


class StderrModeTests(unittest.TestCase):
    def test_returns_the_code_and_stderr_and_discards_stdout(self):
        code, text = run_cancellable([PY, "-c", "import sys;print('out');print('row',file=sys.stderr);sys.exit(3)"],
                                     10, None)
        self.assertEqual((code, text), (3, "row\n"))

    def test_timeout_kills_the_group(self):
        with self.assertRaises(CommandFailed) as caught:
            run_cancellable([PY, "-c", "import time;time.sleep(30)"], 0.3, None, step=.1)
        self.assertEqual(caught.exception.kind, bench_proc.TIMEOUT)

    def test_cancel_kills_the_group_quickly(self):
        cancel = threading.Event()
        threading.Timer(.2, cancel.set).start()
        began = time.monotonic()
        with self.assertRaises(CommandFailed) as caught:
            run_cancellable([PY, "-c", "import time;time.sleep(30)"], 60, cancel, step=.1)
        self.assertEqual(caught.exception.kind, bench_proc.CANCELLED)
        self.assertLess(time.monotonic() - began, 5)

    def test_stdin_is_closed(self):
        code, _ = run_cancellable([PY, "-c", "import sys;sys.exit(0 if sys.stdin.read() == '' else 1)"], 10, None)
        self.assertEqual(code, 0)


class StdoutModeTests(unittest.TestCase):
    def run_py(self, code, limit=1000, timeout=10, cancel=None):
        return run_cancellable([PY, "-c", code], timeout, cancel, stdout_limit=limit, step=.05)

    def test_captures_stdout_with_replacement_decoding_and_discards_stderr(self):
        code, text = self.run_py("import sys;sys.stdout.buffer.write(b'ok \\xff');sys.stderr.write('noise')")
        self.assertEqual((code, text), (0, "ok �"))

    def test_the_exit_code_is_returned(self):
        self.assertEqual(self.run_py("raise SystemExit(4)")[0], 4)

    def test_exactly_the_limit_is_accepted_and_one_more_byte_is_an_overflow(self):
        self.assertEqual(len(self.run_py("import sys;sys.stdout.write('x'*1000)")[1]), 1000)
        with self.assertRaises(CommandFailed) as caught:
            self.run_py("import sys;sys.stdout.write('x'*1001)")
        self.assertEqual(caught.exception.kind, bench_proc.OVERFLOW)

    def test_an_overflowing_child_that_keeps_running_is_killed_not_awaited(self):
        began = time.monotonic()
        with self.assertRaises(CommandFailed) as caught:
            self.run_py("import sys,time;sys.stdout.write('x'*50000);sys.stdout.flush();time.sleep(30)")
        self.assertEqual(caught.exception.kind, bench_proc.OVERFLOW)
        self.assertLess(time.monotonic() - began, 5)

    def test_never_reads_more_than_the_limit_plus_one_byte(self):
        reads = []
        real_read = os.read

        def spy(fd, size):
            data = real_read(fd, size)
            if sys._getframe(1).f_globals["__name__"] == "bench_proc":  # not subprocess's own error pipe
                reads.append(len(data))
            return data
        bench_proc.os.read = spy
        try:
            with self.assertRaises(CommandFailed):
                self.run_py("import sys;sys.stdout.write('x'*500000)", limit=100)
        finally:
            bench_proc.os.read = real_read
        self.assertTrue(reads)
        self.assertLessEqual(sum(reads), 101)

    def test_timeout_and_cancel_kill_a_silent_child(self):
        with self.assertRaises(CommandFailed) as caught:
            self.run_py("import time;time.sleep(30)", timeout=0.3)
        self.assertEqual(caught.exception.kind, bench_proc.TIMEOUT)
        cancel = threading.Event()
        cancel.set()
        with self.assertRaises(CommandFailed) as caught:
            self.run_py("import time;time.sleep(30)", cancel=cancel)
        self.assertEqual(caught.exception.kind, bench_proc.CANCELLED)

    def test_a_child_that_closes_stdout_but_keeps_running_is_waited_for(self):
        code, text = self.run_py("import os,time;os.close(1);time.sleep(.3);raise SystemExit(5)")
        self.assertEqual((code, text), (5, ""))

    def test_a_missing_binary_raises_oserror(self):
        with self.assertRaises(OSError):
            run_cancellable(["/nonexistent/tool"], 1, None, stdout_limit=10)


if __name__ == "__main__":
    unittest.main()
