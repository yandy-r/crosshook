"""bench_events parses the G8 `BENCH_REPLAY` markers and replay phase marks strictly."""
import io
import sys
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_cli
import bench_events
from bench_capture import CaptureError
from bench_events import EventReader, Mark, ReplayEvent

LAUNCH = 1_000
NOW = 5_000


def reader_for(data: bytes) -> EventReader:
    reader = EventReader(io.BytesIO(data), LAUNCH, clock=lambda: NOW)
    reader._thread.join(5)
    return reader


class WaitReplayTests(unittest.TestCase):
    def test_start_and_done_are_returned_with_their_line_counts(self):
        reader = reader_for(b"BENCH_REPLAY 2000 start 0\nBENCH_REPLAY 3000 done 50000\n")
        self.assertEqual(reader.wait_replay("start", 1), ReplayEvent(2000, 0))
        self.assertEqual(reader.wait_replay("done", 1), ReplayEvent(3000, 50000))

    def test_a_missing_start_or_done_times_out_with_its_own_code(self):
        reader = reader_for(b"")
        reader._eof = False  # a live stream: the wait runs to its timeout
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_STARTED"):
            reader.wait_replay("start", .05)
        with self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_DONE"):
            reader.wait_replay("done", .05)

    def test_an_ended_stream_names_the_replay_code(self):
        with self.assertRaisesRegex(CaptureError, "^EVENT_STREAM_ENDED: REPLAY_NOT_DONE"):
            reader_for(b"BENCH_REPLAY 2000 start 0\n").wait_replay("done", 1)

    def test_an_unknown_kind_is_a_programming_error(self):
        with self.assertRaises(ValueError):
            reader_for(b"").wait_replay("middle", .01)

    def test_the_wait_wakes_when_the_line_arrives_late(self):
        import os
        import threading
        read_fd, write_fd = os.pipe()
        stream = os.fdopen(read_fd, "rb")
        reader = EventReader(stream, LAUNCH, clock=lambda: NOW)
        self.addCleanup(stream.close)
        self.addCleanup(os.close, write_fd)
        self.addCleanup(reader.close)
        threading.Timer(.05, os.write, (write_fd, b"BENCH_REPLAY 2000 start 7\n")).start()
        started = time.monotonic()
        self.assertEqual(reader.wait_replay("start", 5), ReplayEvent(2000, 7))
        self.assertLess(time.monotonic() - started, 3)


class StrictParseTests(unittest.TestCase):
    def assertMalformed(self, data: bytes) -> None:
        reader = reader_for(data)
        with self.assertRaisesRegex(CaptureError, "^BENCH_EVENT_MALFORMED"):
            reader.check()
        with self.assertRaisesRegex(CaptureError, "^BENCH_EVENT_MALFORMED"):
            reader.wait_replay("start", .05)

    def test_field_count_must_be_exactly_four(self):
        self.assertMalformed(b"BENCH_REPLAY 2000 start\n")
        self.assertMalformed(b"BENCH_REPLAY 2000 start 0 extra\n")

    def test_kind_must_be_start_or_done(self):
        for kind in (b"begin", b"START", b"stop", b"1"):
            self.assertMalformed(b"BENCH_REPLAY 2000 " + kind + b" 0\n")

    def test_lines_are_an_integer_from_zero_to_the_cap(self):
        for lines in (b"-1", b"50001", b"1.5", b"abc", b"0x10", b"+5", b"", b"9" * 5000):
            self.assertMalformed(b"BENCH_REPLAY 2000 start " + lines + b"\n")
        self.assertEqual(reader_for(b"BENCH_REPLAY 2000 start 50000\n").wait_replay("start", 1).lines, 50000)

    def test_ns_must_be_an_integer_inside_the_launch_window(self):
        for ns in (b"999", b"5001", b"-3", b"x", b"1e3"):
            self.assertMalformed(b"BENCH_REPLAY " + ns + b" start 0\n")
        self.assertEqual(reader_for(b"BENCH_REPLAY 1000 start 0\n").wait_replay("start", 1).ns, 1000)
        self.assertEqual(reader_for(b"BENCH_REPLAY 5000 start 0\n").wait_replay("start", 1).ns, 5000)

    def test_start_is_accepted_once(self):
        self.assertMalformed(b"BENCH_REPLAY 2000 start 0\nBENCH_REPLAY 2100 start 0\n")

    def test_done_is_accepted_once(self):
        self.assertMalformed(b"BENCH_REPLAY 2000 start 0\nBENCH_REPLAY 3000 done 10\nBENCH_REPLAY 3100 done 10\n")

    def test_done_must_follow_a_start(self):
        self.assertMalformed(b"BENCH_REPLAY 3000 done 10\n")
        self.assertMalformed(b"BENCH_REPLAY 3000 done 10\nBENCH_REPLAY 2000 start 0\n")

    def test_done_must_be_later_than_start(self):
        self.assertMalformed(b"BENCH_REPLAY 3000 start 0\nBENCH_REPLAY 2000 done 10\n")
        self.assertMalformed(b"BENCH_REPLAY 3000 start 0\nBENCH_REPLAY 3000 done 10\n")

    def test_a_rejected_lookalike_does_not_replace_the_real_event(self):
        reader = reader_for(b"BENCH_REPLAY 2000 start 0\nBENCH_REPLAY 2500 start 99\n")
        self.assertEqual(reader._replay["start"], ReplayEvent(2000, 0))

    def test_other_lines_that_merely_mention_replay_are_discarded(self):
        reader = reader_for(b"BENCH_REPLAYX 2000 start 0\nreplay start\n  \nBENCH_REPLAY\n")
        # A bare `BENCH_REPLAY` is a claimed marker with the wrong field count; the rest are noise.
        with self.assertRaisesRegex(CaptureError, "^BENCH_EVENT_MALFORMED"):
            reader.check()
        self.assertEqual(len(reader._errors), 1)


class ReplayMarkTests(unittest.TestCase):
    def test_the_first_replay_phase_mark_is_returned(self):
        reader = reader_for(b"BENCH_MARK 2000 replay_scroll 0 follow_on 10\nBENCH_MARK 2100 replay_scroll 1 follow_off 20\n")
        self.assertEqual(reader.status_mark("replay_scroll"), Mark(2000, "replay_scroll", 0, "follow_on", 10))

    def test_each_replay_phase_is_accepted(self):
        for phase in ("console_hidden", "follow_on", "follow_off"):
            line = f"BENCH_MARK 2000 replay_scroll 0 {phase} 10\n".encode()
            self.assertEqual(reader_for(line).status_mark("replay_scroll").phase, phase)

    def test_the_earliest_stamp_wins_over_arrival_order(self):
        reader = reader_for(b"BENCH_MARK 3000 replay_scroll 1 follow_off 1\nBENCH_MARK 2000 replay_scroll 0 follow_on 2\n")
        self.assertEqual(reader.status_mark("replay_scroll").phase, "follow_on")

    def test_no_mark_is_none(self):
        self.assertIsNone(reader_for(b"").status_mark("replay_scroll"))
        self.assertIsNone(reader_for(b"BENCH_MARK 2000 replay_scroll 0 armed 1\n").status_mark("replay_scroll"))

    def test_other_scenarios_are_ignored(self):
        self.assertIsNone(reader_for(b"BENCH_MARK 2000 search 0 follow_on 1\n").status_mark("replay_scroll"))

    def test_status_marks_do_not_answer_a_mark_wait_or_a_focus_query(self):
        reader = reader_for(b"BENCH_MARK 2000 replay_scroll 0 console_hidden 1\n")
        self.assertIsNone(reader.focus_lost("replay_scroll", 0))
        with self.assertRaisesRegex(CaptureError, "^EVENT_STREAM_ENDED: MARK_TIMEOUT"):
            reader.wait_mark("replay_scroll", 0, "console_hidden", .05)

    def test_an_unknown_phase_is_still_malformed(self):
        with self.assertRaisesRegex(CaptureError, "^BENCH_EVENT_MALFORMED"):
            reader_for(b"BENCH_MARK 2000 replay_scroll 0 follow_maybe 1\n").check()

    def test_replay_scroll_begin_is_accepted(self):
        self.assertEqual(reader_for(b"BENCH_BEGIN 2000 replay_scroll\n").wait_begin(1).scenario, "replay_scroll")


class ScenarioSetTests(unittest.TestCase):
    def test_g5_keeps_its_own_set_and_the_one_shot_probes_are_the_only_extras(self):
        self.assertEqual(bench_events.G5_SCENARIOS,
                         set(bench_cli.SCENARIOS["G5"] + bench_cli.SUPPLEMENTAL_SCENARIOS["G5"]))
        self.assertEqual(bench_events.SCENARIOS - bench_events.G5_SCENARIOS, {"replay_scroll", "health_scroll"})

    def test_g5_marks_still_parse_unchanged(self):
        reader = reader_for(b"BENCH_MARK 2000 search 0 armed 1\nBENCH_MARK 3000 search 0 render 2\n")
        self.assertEqual(reader.wait_mark("search", 0, "render", 1), Mark(3000, "search", 0, "render", 2))


if __name__ == "__main__":
    unittest.main()
