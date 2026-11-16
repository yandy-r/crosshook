"""FocusTail checks: incremental K/M focus, parser parity, incomplete-log refusal; stdlib only."""
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_present
from bench_capture import CaptureError
from bench_present import FocusTail, parse_log

NAME = "wlpresent-1.log"


class FocusTailTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.dir = Path(self.temp.name)
        self.path = self.dir / NAME
        self.tail = FocusTail(self.dir)

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
        with patch.object(bench_present.os, "pread",
                          lambda fd, n, offset: reads.append((n, offset)) or original(fd, n, offset)):
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
        with self.assertRaisesRegex(CaptureError, rf"^PRESENT_LOG_MALFORMED: {NAME}:2"):
            self.tail.focused("keyboard")
        link = self.dir / "wlpresent-9.log"
        link.symlink_to(self.path)
        fresh = FocusTail(self.dir)
        self.path.write_text("S\t1\t7\ttoplevel\n")
        with self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_UNREADABLE: wlpresent-9.log"):
            fresh.focused("keyboard")

    def test_oversized_log_is_refused(self):
        self.append("S\t1\t7\ttoplevel\n")
        with patch.object(bench_present, "MAX_LOG_BYTES", 4), \
                self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_TOO_LARGE"):
            self.tail.focused("keyboard")

    def test_size_is_the_total_bytes_of_every_log_at_the_last_read(self):
        self.assertEqual(self.tail.size, 0)
        self.append("S\t1\t7\ttoplevel\n")
        self.tail.focused("keyboard")
        first = self.path.stat().st_size
        self.assertEqual(self.tail.size, first)
        self.append("K\t10\tent")  # a partial record still counts as the log growing
        self.tail.focused("keyboard")
        self.assertEqual(self.tail.size, self.path.stat().st_size)
        other = self.dir / "wlpresent-2.log"
        self.append("S\t1\t3\ttoplevel\n", other)
        self.tail.focused("keyboard")
        self.assertEqual(self.tail.size, self.path.stat().st_size + other.stat().st_size)


class IncompleteLogTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.dir = Path(self.temp.name)
        self.path = self.dir / NAME
        self.tail = FocusTail(self.dir)

    def test_every_condition_that_ends_the_witness_is_refused(self):
        for reason in ("log_cap", "bind_failed", "wrap_failed", "no_presentation_global"):
            with self.subTest(reason=reason):
                path = self.dir / f"wlpresent-{reason}.log"
                path.write_text(f"S\t1\t7\ttoplevel\nK\t10\tenter\t7\nX\t11\t{reason}\n")
                with self.assertRaisesRegex(CaptureError, f"^PRESENT_LOG_INCOMPLETE: {reason}$"):
                    FocusTail(self.dir).focused("keyboard")
                path.unlink()

    def test_a_condition_appended_later_is_refused_on_the_next_read(self):
        self.path.write_text("S\t1\t7\ttoplevel\nK\t10\tenter\t7\n")
        self.assertTrue(self.tail.focused("keyboard"))
        with self.path.open("a") as stream:
            stream.write("X\t20\tlog_cap\n")
        with self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_INCOMPLETE: log_cap"):
            self.tail.focused("keyboard")

    def test_an_unterminated_condition_record_is_not_acted_on(self):
        self.path.write_text("S\t1\t7\ttoplevel\nK\t10\tenter\t7\nX\t20\tlog_cap")
        self.assertTrue(self.tail.focused("keyboard"))

    def test_other_conditions_and_malformed_condition_records(self):
        self.path.write_text("S\t1\t7\ttoplevel\nX\t5\tsomething_else\n")
        self.assertFalse(self.tail.focused("keyboard"))
        self.path.write_text("S\t1\t7\ttoplevel\nX\tnot-a-number\tlog_cap\n")
        with self.assertRaisesRegex(CaptureError, rf"^PRESENT_LOG_MALFORMED: {NAME}:"):
            FocusTail(self.dir).focused("keyboard")


def final_focus(log, device):
    """The full parser's answer: the latest event of `device` (log order breaks ties) is an enter on a toplevel."""
    latest = None
    for event in log.focus:
        if event.device == device and (latest is None or event.ns >= latest.ns):
            latest = event
    return latest is not None and latest.entered and latest.toplevel


HEADER = "H\tversion=1\tpid=42\tclock_id=1\n"
CORPORA = {
    "enter_then_leave": HEADER + "S\t1\t7\ttoplevel\nK\t10\tenter\t7\nM\t11\tenter\t7\nM\t12\tleave\t7\n",
    "role_changes_in_log_order": HEADER + "K\t5\tenter\t7\nS\t6\t7\ttoplevel\nK\t10\tenter\t7\nS\t11\t7\tdestroyed\n"
                                 "M\t12\tenter\t7\nS\t13\t7\ttoplevel\n",
    "cursor_surface": HEADER + "S\t1\t7\ttoplevel\nS\t2\t8\tcursor\nK\t10\tenter\t8\nM\t11\tenter\t8\n",
    "out_of_order_stamps": HEADER + "S\t1\t7\ttoplevel\nK\t30\tenter\t7\nK\t20\tleave\t7\n"
                           "M\t5\tleave\t7\nM\t4\tenter\t7\n",
    "interleaved_records": HEADER + "S\t1\t7\ttoplevel\nC\t2\t7\t1\t1\nK\t10\tenter\t7\nP\t11\t1\t11\t8\t1\t0\n"
                         "G\t12\t7\t1280\t900\t1\nX\t13\tsomething_else\nD\t14\t2\nM\t15\tenter\t7\n",
    "unterminated_last_record": HEADER + "S\t1\t7\ttoplevel\nK\t10\tenter\t7\nK\t20\tleave\t7",
    "no_focus_records": HEADER + "S\t1\t7\ttoplevel\nC\t2\t7\t1\t1\n",
}


class ParityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.dir = Path(self.temp.name)
        self.path = self.dir / NAME

    def test_both_parsers_agree_on_the_final_focus_state(self):
        for name, text in CORPORA.items():
            with self.subTest(corpus=name):
                self.path.write_text(text)
                log = parse_log(self.path)
                tail = FocusTail(self.dir)
                for device in ("keyboard", "pointer"):
                    self.assertEqual(tail.focused(device), final_focus(log, device), device)

    def test_an_incrementally_fed_log_ends_in_the_same_state_as_the_one_shot_parse(self):
        for name, text in CORPORA.items():
            with self.subTest(corpus=name):
                self.path.write_text("")
                tail = FocusTail(self.dir)
                for start in range(0, len(text), 7):  # arbitrary chunks, many ending mid-record
                    with self.path.open("a") as stream:
                        stream.write(text[start:start + 7])
                    tail.focused("keyboard")
                log = parse_log(self.path)
                for device in ("keyboard", "pointer"):
                    self.assertEqual(tail.focused(device), final_focus(log, device), device)

    def test_both_reject_the_same_malformed_records_at_the_same_line(self):
        for line in ("K\t10\tenter", "K\tx\tenter\t7", "M\t10\tmaybe\t7", "K\t10\tenter\t7\x1c0",
                     "S\t1\tx\ttoplevel"):
            with self.subTest(line=line):
                self.path.write_text(f"{HEADER}S\t1\t7\ttoplevel\n{line}\n")
                errors = []
                for parse in (lambda: parse_log(self.path), lambda: FocusTail(self.dir).focused("keyboard")):
                    with self.assertRaises(CaptureError) as caught:
                        parse()
                    errors.append(str(caught.exception))
                self.assertEqual(errors[0], errors[1])
                self.assertRegex(errors[0], rf"^PRESENT_LOG_MALFORMED: {NAME}:3$")

    def test_lines_split_on_newline_only(self):
        # str.splitlines() would also split at \x0b, \x0c, \x1c-\x1e, \x85 and \u2028; the shim never writes them,
        # so a role carrying one is a different role, not a second line.
        self.path.write_text(f"{HEADER}S\t1\t7\ttoplevel\x0c\nK\t10\tenter\t7\n")
        self.assertFalse(final_focus(parse_log(self.path), "keyboard"))
        self.assertFalse(FocusTail(self.dir).focused("keyboard"))

if __name__ == "__main__":
    unittest.main()
