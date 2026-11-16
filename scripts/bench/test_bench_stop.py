"""bench_capture.stop() is signal-safe: a signal during the teardown waits until the app group is gone."""
import os
import signal
import subprocess
import sys
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_capture

NUMBERS = (signal.SIGTERM, signal.SIGHUP, signal.SIGINT)


def unexpected(signum, _frame):
    raise AssertionError(f"unexpected {signal.Signals(signum).name} outside a handler the test installed")


class StopSignalTests(unittest.TestCase):
    def setUp(self):
        before = {number: signal.signal(number, unexpected) for number in NUMBERS}
        self.addCleanup(lambda: [signal.signal(number, handler) for number, handler in before.items()])
        self.proc = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"],
                                     start_new_session=True, stderr=subprocess.PIPE)
        self.addCleanup(self.proc.stderr.close)
        self.addCleanup(self.cleanup_group)

    def cleanup_group(self):
        try:
            os.killpg(self.proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        self.proc.wait()

    def stop_with_signals_sent_during_the_kill(self, numbers):
        """Run stop() with `numbers` sent to this process right after the group's first kill signal."""
        real_killpg = os.killpg
        sent = []

        def killpg(pid, sig):
            real_killpg(pid, sig)
            if not sent:
                sent.append(sig)
                for number in numbers:
                    os.kill(os.getpid(), number)
        with patch.object(bench_capture.os, "killpg", killpg):
            bench_capture.stop(self.proc)

    def test_signals_arriving_during_the_teardown_are_delivered_after_the_group_is_gone(self):
        seen = []

        def recorder(signum, _frame):
            seen.append((signal.Signals(signum).name, self.proc.poll() is not None))
        for number in NUMBERS:
            signal.signal(number, recorder)
        self.stop_with_signals_sent_during_the_kill([signal.SIGTERM])
        self.assertEqual(seen, [("SIGTERM", True)])  # delivered once, when the app was already reaped
        self.assertIsNotNone(self.proc.returncode)

    def test_a_raising_handler_cannot_abort_the_teardown(self):
        class Aborted(BaseException):
            pass

        def abort(signum, _frame):
            raise Aborted(signal.Signals(signum).name)
        for number in NUMBERS:
            signal.signal(number, abort)
        with self.assertRaises(Aborted):  # raised only once stop() had finished its work
            self.stop_with_signals_sent_during_the_kill([signal.SIGTERM, signal.SIGHUP, signal.SIGINT])
        self.assertIsNotNone(self.proc.poll())
        with self.assertRaises(ProcessLookupError):
            os.killpg(self.proc.pid, 0)

    def test_only_the_first_signal_is_redelivered_and_handlers_are_restored(self):
        seen = []
        for number in NUMBERS:
            signal.signal(number, lambda signum, _frame: seen.append(signum))
        installed = {number: signal.getsignal(number) for number in NUMBERS}
        self.stop_with_signals_sent_during_the_kill([signal.SIGHUP, signal.SIGTERM])
        self.assertEqual(seen, [signal.SIGHUP])
        self.assertEqual({number: signal.getsignal(number) for number in NUMBERS}, installed)

    def test_ignored_signals_stay_ignored(self):
        for number in NUMBERS:
            signal.signal(number, signal.SIG_IGN)
        self.stop_with_signals_sent_during_the_kill([signal.SIGTERM])
        self.assertEqual({signal.getsignal(number) for number in NUMBERS}, {signal.SIG_IGN})

    def test_a_quiet_teardown_leaves_the_handlers_alone(self):
        before = {number: signal.getsignal(number) for number in NUMBERS}
        bench_capture.stop(self.proc)
        self.assertEqual({number: signal.getsignal(number) for number in NUMBERS}, before)
        self.assertIsNotNone(self.proc.returncode)

    def test_stop_works_off_the_main_thread_where_handlers_cannot_be_installed(self):
        errors = []

        def run():
            try:
                bench_capture.stop(self.proc)
            except BaseException as error:  # noqa: BLE001 - reported to the test thread
                errors.append(error)
        thread = threading.Thread(target=run)
        thread.start()
        thread.join(15)
        self.assertEqual(errors, [])
        self.assertIsNotNone(self.proc.returncode)


if __name__ == "__main__":
    unittest.main()
