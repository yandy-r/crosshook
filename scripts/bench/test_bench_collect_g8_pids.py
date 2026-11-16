"""G8 collection with more than one shim log in a launch: only a log that presented a toplevel frame vouches for the pid.

Uses real shim-log text through the real `toplevel_pids`; nothing about the pid binding is patched.
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bench_capture import CaptureError
from test_bench_collect_g8 import BASE_NS, PID, WINDOW_START, G8CollectTestCase, present_log

CHILD_PID = PID + 500


def idle_child_log(pid: int) -> str:
    """What a child process that loaded the shim but never presented a frame writes."""
    return f"H\tversion=1\tpid={pid}\tcomm=child\tclock_id=1\nS\t1\t7\ttoplevel\nC\t1000\t7\t1\t1\n"


class G8MultipleShimLogTests(G8CollectTestCase):
    def test_a_non_presenting_child_log_under_another_pid_passes(self):
        self.args.iterations = 1
        self.extra_logs = {"wlpresent-2.log": idle_child_log(CHILD_PID)}
        (row,) = self.rows()
        self.assertEqual(row["status"], "captured")

    def test_a_presenting_log_under_a_foreign_pid_is_another_session(self):
        self.args.iterations = 1
        self.extra_logs = {"wlpresent-2.log": present_log(BASE_NS, WINDOW_START - 100_000_000, pid=CHILD_PID)}
        with self.assertRaisesRegex(CaptureError,
                                    r"^SESSION_MISMATCH: the frames come from another session \(pid differ"):
            self.rows()


if __name__ == "__main__":
    unittest.main()
