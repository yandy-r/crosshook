"""bench_events.EventReader parses the app's BENCH_* stderr markers strictly."""
import io
import os
import sys
import threading
import time
import unittest
from pathlib import Path
from typing import ClassVar

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_capture
import bench_events
from bench_capture import CaptureError
from bench_events import (HEALTH_MIN_ROWS, HEALTH_WAIT_S, LINE_CAP, Begin, ClockSample, EventReader, Mark,
                          require_probe_ok)

LAUNCH = 1_000
NOW = 5_000


def reader_for(data: bytes) -> EventReader:
    reader = EventReader(io.BytesIO(data), LAUNCH, clock=lambda: NOW)
    reader._thread.join(5)
    return reader


def _close(fd: int) -> None:
    try:
        os.close(fd)
    except OSError:
        pass


def pipe_reader(case: unittest.TestCase) -> tuple[EventReader, int]:
    read_fd, write_fd = os.pipe()
    stream = os.fdopen(read_fd, "rb")
    reader = EventReader(stream, LAUNCH, clock=lambda: NOW)
    case.addCleanup(stream.close)
    case.addCleanup(_close, write_fd)
    case.addCleanup(reader.close)
    return reader, write_fd


class FocusLostTests(unittest.TestCase):
    def test_focus_lost_counts_only_marks_at_or_after_the_cutoff(self):
        reader = reader_for(b"BENCH_MARK 2000 search 0 focus_lost 1\nBENCH_MARK 3000 search 4 focus_lost 2\n"
                            b"BENCH_MARK 3500 dpad 1 focus_lost 3\nBENCH_MARK 3600 search 4 armed 4\n")
        self.assertEqual(reader.focus_lost("search", 0), Mark(2000, "search", 0, "focus_lost", 1))
        self.assertEqual(reader.focus_lost("search", 2500), Mark(3000, "search", 4, "focus_lost", 2))
        self.assertIsNone(reader.focus_lost("search", 3001))
        self.assertIsNone(reader.focus_lost("route_switch", 0))
        self.assertIsNone(reader_for(b"BENCH_MARK 3600 search 4 armed 4\n").focus_lost("search", 0))

    def test_a_later_blur_with_the_same_seq_is_not_hidden_by_an_earlier_one(self):
        reader = reader_for(b"BENCH_MARK 2000 search 3 focus_lost 1\nBENCH_MARK 3000 search 3 focus_lost 2\n")
        self.assertEqual(reader.focus_lost("search", 0), Mark(2000, "search", 3, "focus_lost", 1))
        # The cutoff falls between the two: the early blur is ignored, the later one still counts.
        self.assertEqual(reader.focus_lost("search", 2500), Mark(3000, "search", 3, "focus_lost", 2))
        self.assertIsNone(reader.focus_lost("search", 3001))

    def test_focus_lost_marks_do_not_answer_a_mark_wait(self):
        reader = reader_for(b"BENCH_MARK 2000 search 3 focus_lost 1\n")
        with self.assertRaisesRegex(CaptureError, "^EVENT_STREAM_ENDED: MARK_TIMEOUT"):
            reader.wait_mark("search", 3, "focus_lost", 0.05)


class RecordTests(unittest.TestCase):
    def test_begin_mark_and_clock_records(self):
        reader = reader_for(b"noise\nBENCH_BEGIN 2000 search\n"
                            b"BENCH_MARK 3000 search 4 render 777\n"
                            b"BENCH_CLOCK 4000 0 123\nBENCH_CLOCK 4500 1 456\n")
        self.assertEqual(reader.wait_begin(1), Begin(2000, "search"))
        self.assertEqual(reader.wait_mark("search", 4, "render", 1), Mark(3000, "search", 4, "render", 777))
        self.assertEqual(reader.clock_samples(), [ClockSample(4000, 0, 123), ClockSample(4500, 1, 456)])
        reader.check()

    def test_records_are_frozen(self):
        with self.assertRaises(AttributeError):
            Begin(1, "search").ns = 2

    def test_every_scenario_and_phase_is_accepted(self):
        lines = b"".join(f"BENCH_MARK 2000 {s} 0 {p} 1\n".encode()
                          for s in ("search", "toggle_favorite", "toggle_optimization", "dpad", "route_switch")
                          for p in ("armed", "render", "focus_lost"))
        reader = reader_for(lines)
        reader.check()
        self.assertEqual(reader.wait_mark("route_switch", 0, "armed", 1).t_perf_us, 1)
        self.assertEqual(reader.wait_mark("route_switch", 0, "render", 1).t_perf_us, 1)
        self.assertEqual(reader.focus_lost("route_switch", 0).t_perf_us, 1)

    def test_other_lines_are_ignored(self):
        reader = reader_for(b"READY 2000\nBENCH_UNKNOWN x\nBENCH_MARKX 1\n\n\xff\xfe junk\nBENCH_BEGIN 2000 dpad")
        reader.check()
        self.assertEqual(reader.wait_begin(1), Begin(2000, "dpad"))  # unterminated final line at EOF

    def test_crlf_is_tolerated(self):
        self.assertEqual(reader_for(b"BENCH_BEGIN 2000 dpad\r\n").wait_begin(1).scenario, "dpad")

    def test_bounds_are_inclusive(self):
        reader = reader_for(f"BENCH_CLOCK {LAUNCH} 0 0\nBENCH_CLOCK {NOW} 1 0\n".encode())
        reader.check()
        self.assertEqual(len(reader.clock_samples()), 2)


class MalformedTests(unittest.TestCase):
    CASES: ClassVar[dict[str, str]] = {
        "begin too few": "BENCH_BEGIN 2000",
        "begin too many": "BENCH_BEGIN 2000 search extra",
        "mark too few": "BENCH_MARK 2000 search 1 render",
        "mark too many": "BENCH_MARK 2000 search 1 render 5 6",
        "clock too many": "BENCH_CLOCK 2000 1 2 3",
        "non-int ns": "BENCH_BEGIN abc search",
        "float ns": "BENCH_BEGIN 2000.5 search",
        "non-int seq": "BENCH_MARK 2000 search x render 5",
        "underscore int": "BENCH_MARK 2000 search 1_0 render 5",
        "plus-signed int": "BENCH_CLOCK 2000 +1 5",
        "negative seq": "BENCH_MARK 2000 search -1 render 5",
        "negative t_perf": "BENCH_MARK 2000 search 1 render -5",
        "negative clock seq": "BENCH_CLOCK 2000 -1 5",
        "negative clock t_perf": "BENCH_CLOCK 2000 1 -5",
        "unknown phase": "BENCH_MARK 2000 search 1 paint 5",
        "unknown scenario": "BENCH_MARK 2000 bogus 1 render 5",
        "unknown begin scenario": "BENCH_BEGIN 2000 bogus",
        "ns before launch": "BENCH_BEGIN 999 search",
        "ns after now": "BENCH_MARK 5001 search 1 render 5",
        "clock ns after now": "BENCH_CLOCK 5001 1 5",
    }

    def test_each_malformed_line_records_an_error(self):
        for name, line in self.CASES.items():
            with self.subTest(name):
                reader = reader_for((line + "\n").encode())
                with self.assertRaises(CaptureError) as caught:
                    reader.check()
                self.assertEqual(str(caught.exception), "BENCH_EVENT_MALFORMED: " + line[:80])

    def test_malformed_text_is_cut_at_80_chars(self):
        line = "BENCH_BEGIN 2000 " + "x" * 200
        with self.assertRaises(CaptureError) as caught:
            reader_for((line + "\n").encode()).check()
        self.assertEqual(str(caught.exception), "BENCH_EVENT_MALFORMED: " + line[:80])
        self.assertEqual(len(str(caught.exception)), len("BENCH_EVENT_MALFORMED: ") + 80)

    def test_malformed_line_fails_waiting_even_when_the_mark_exists(self):
        reader = reader_for(b"BENCH_MARK 2000 search 1 render 5\nBENCH_BEGIN nope\n")
        with self.assertRaisesRegex(CaptureError, "BENCH_EVENT_MALFORMED"):
            reader.wait_mark("search", 1, "render", 1)


class ErrorAndTimeoutTests(unittest.TestCase):
    def test_bench_error_is_recorded(self):
        reader = reader_for(b"BENCH_ERROR: unsupported scenario\n")
        with self.assertRaisesRegex(CaptureError, "^BENCH_ERROR: unsupported scenario$"):
            reader.check()
        with self.assertRaisesRegex(CaptureError, "BENCH_ERROR"):
            reader.wait_begin(1)

    def test_wait_mark_raises_immediately_on_error_from_a_live_stream(self):
        reader, write_fd = pipe_reader(self)
        threading.Timer(.1, os.write, (write_fd, b"BENCH_ERROR: boom\n")).start()
        started = time.monotonic()
        with self.assertRaisesRegex(CaptureError, "BENCH_ERROR: boom"):
            reader.wait_mark("search", 0, "render", 10)
        self.assertLess(time.monotonic() - started, 5)

    def test_mark_timeout_names_the_wait(self):
        reader, _ = pipe_reader(self)
        with self.assertRaises(CaptureError) as caught:
            reader.wait_mark("toggle_favorite", 3, "armed", .1)
        self.assertEqual(str(caught.exception), "MARK_TIMEOUT: toggle_favorite seq 3 armed")

    def test_begin_timeout(self):
        reader, _ = pipe_reader(self)
        with self.assertRaisesRegex(CaptureError, "^BEGIN_TIMEOUT$"):
            reader.wait_begin(.05)

    def test_eof_while_waiting_raises_immediately(self):
        reader = reader_for(b"BENCH_BEGIN 2000 search\n")
        started = time.monotonic()
        with self.assertRaisesRegex(CaptureError, "EVENT_STREAM_ENDED: MARK_TIMEOUT: search seq 0 render"):
            reader.wait_mark("search", 0, "render", 30)
        self.assertLess(time.monotonic() - started, 5)

    def test_eof_while_blocked_wakes_the_waiter(self):
        reader, write_fd = pipe_reader(self)
        threading.Timer(.1, _close, (write_fd,)).start()
        started = time.monotonic()
        with self.assertRaisesRegex(CaptureError, "EVENT_STREAM_ENDED"):
            reader.wait_mark("dpad", 1, "render", 30)
        self.assertLess(time.monotonic() - started, 5)

    def test_mark_seen_before_eof_is_still_returned(self):
        reader = reader_for(b"BENCH_MARK 2000 dpad 1 render 5\n")
        self.assertEqual(reader.wait_mark("dpad", 1, "render", 1).t_perf_us, 5)

    def test_mark_arriving_while_waiting_is_returned(self):
        reader, write_fd = pipe_reader(self)
        threading.Timer(.1, os.write, (write_fd, b"BENCH_MARK 2000 dpad 2 armed 9\n")).start()
        self.assertEqual(reader.wait_mark("dpad", 2, "armed", 10).seq, 2)

    def test_close_wakes_a_waiter(self):
        reader, _ = pipe_reader(self)
        threading.Timer(.1, reader.close).start()
        with self.assertRaisesRegex(CaptureError, "EVENT_STREAM_ENDED"):
            reader.wait_begin(30)


class LineCapTests(unittest.TestCase):
    def test_overlong_line_is_an_error_and_later_lines_still_parse(self):
        data = b"x" * 70000 + b"\nBENCH_BEGIN 2000 search\n"
        reader = reader_for(data)
        with self.assertRaisesRegex(CaptureError, "UNBOUNDED_STDERR_LINE"):
            reader.check()
        self.assertEqual(reader._begin, Begin(2000, "search"))

    def test_overlong_unterminated_line_is_an_error(self):
        with self.assertRaisesRegex(CaptureError, "UNBOUNDED_STDERR_LINE"):
            reader_for(b"y" * 70000).check()

    def test_line_at_the_cap_is_not_an_error(self):
        reader_for(b"z" * LINE_CAP + b"\nBENCH_BEGIN 2000 search\n").check()


class HandoffTests(unittest.TestCase):
    def test_initial_bytes_are_read_before_the_stream(self):
        reader = EventReader(io.BytesIO(b"ed 7\nBENCH_CLOCK 2100 0 1\n"), LAUNCH, clock=lambda: NOW,
                             initial=b"BENCH_BEGIN 2000 search\nBENCH_MARK 2050 search 0 arm")
        reader._thread.join(5)
        reader.check()
        self.assertEqual(reader.wait_begin(0), Begin(2000, "search"))  # a partial initial line joins the stream
        self.assertEqual(reader.wait_mark("search", 0, "armed", 0).t_perf_us, 7)
        self.assertEqual(reader.clock_samples(), [ClockSample(2100, 0, 1)])

    def test_ready_with_events_keeps_lines_buffered_after_ready(self):
        code = ("import sys,time\n"
                "t=time.clock_gettime_ns(time.CLOCK_MONOTONIC)\n"
                "sys.stderr.write('READY %d\\nBENCH_BEGIN %d dpad\\nBENCH_MARK %d dpad 0 armed 5\\n' % (t, t, t))\n"
                "sys.stderr.flush()\n"
                "time.sleep(.3)\n"
                "t=time.clock_gettime_ns(time.CLOCK_MONOTONIC)\n"
                "sys.stderr.write('BENCH_MARK %d dpad 0 render 6\\n' % t)\n"
                "sys.stderr.flush()\n"
                "time.sleep(30)\n")
        proc, _ = bench_capture.ready([sys.executable, "-c", code], dict(os.environ), 5, False, events=True)
        self.addCleanup(proc.stderr.close)
        self.addCleanup(bench_capture.stop, proc)
        self.addCleanup(proc.events.close)
        self.assertEqual(proc.events.wait_begin(5).scenario, "dpad")
        self.assertEqual(proc.events.wait_mark("dpad", 0, "armed", 5).t_perf_us, 5)
        self.assertEqual(proc.events.wait_mark("dpad", 0, "render", 5).t_perf_us, 6)

    def test_ready_without_events_keeps_the_drain(self):
        code = ("import sys,time;t=time.clock_gettime_ns(time.CLOCK_MONOTONIC);"
                "sys.stderr.write('READY %d\\n'%t);sys.stderr.flush()")
        proc, _ = bench_capture.ready([sys.executable, "-c", code], dict(os.environ), 5, True)
        self.addCleanup(proc.stderr.close)
        self.assertFalse(hasattr(proc, "events"))


class HealthProbeTests(unittest.TestCase):
    def test_health_scenarios_and_phases_are_accepted(self):
        reader = reader_for(b"BENCH_BEGIN 2000 health_scroll\nBENCH_BEGIN 2100 health_table\n"
                            b"BENCH_MARK 2200 health_table 0 armed 1\nBENCH_MARK 2300 health_table 0 render 2\n")
        reader.check()
        self.assertEqual(reader.wait_begin(1).scenario, "health_scroll")
        self.assertEqual(reader.wait_mark("health_table", 0, "render", 1).ns, 2300)

    def test_health_table_is_a_g5_scenario_and_health_scroll_is_not(self):
        self.assertIn("health_table", bench_events.G5_SCENARIOS)
        self.assertNotIn("health_scroll", bench_events.G5_SCENARIOS)
        self.assertIn("health_scroll", bench_events.SCENARIOS)

    def test_status_mark_returns_the_health_phase(self):
        for phase in ("health_ready", "health_rows_too_few"):
            reader = reader_for(f"BENCH_MARK 2000 health_scroll 0 {phase} 1\n".encode())
            reader.check()
            self.assertEqual(reader.status_mark("health_scroll").phase, phase)

    def test_health_marks_do_not_answer_a_mark_wait_or_a_focus_query(self):
        reader = reader_for(b"BENCH_MARK 2000 health_table 0 health_ready 1\n")
        self.assertIsNone(reader.focus_lost("health_table", 0))
        with self.assertRaisesRegex(CaptureError, "^EVENT_STREAM_ENDED: MARK_TIMEOUT"):
            reader.wait_mark("health_table", 0, "health_ready", .05)

    def test_a_health_phase_on_a_non_health_scenario_is_well_formed_and_ignored(self):
        reader = reader_for(b"BENCH_MARK 2000 search 0 health_rows_too_few 1\n")
        reader.check()
        require_probe_ok(reader, "health_table")

    def test_an_unknown_health_phase_is_malformed(self):
        with self.assertRaisesRegex(CaptureError, "^BENCH_EVENT_MALFORMED"):
            reader_for(b"BENCH_MARK 2000 health_scroll 0 health_maybe 1\n").check()

    def test_probe_ok_passes_without_a_mark_or_with_ready(self):
        require_probe_ok(reader_for(b""), "health_scroll")
        require_probe_ok(reader_for(b"BENCH_MARK 2000 health_scroll 0 health_ready 1\n"), "health_scroll")

    def test_too_few_rows_fails_with_the_fixture_hint(self):
        reader = reader_for(b"BENCH_MARK 2000 health_scroll 0 health_rows_too_few 1\n")
        with self.assertRaisesRegex(CaptureError, "^HEALTH_ROWS_TOO_FEW: .*fewer than 300 rows.*within 60 s; "
                                                  "generate the 500-profile fixtures$"):
            require_probe_ok(reader, "health_scroll")

    def test_the_failure_is_per_scenario(self):
        reader = reader_for(b"BENCH_MARK 2000 health_scroll 0 health_rows_too_few 1\n")
        require_probe_ok(reader, "health_table")

    def test_the_constants_mirror_the_app(self):
        self.assertEqual((HEALTH_MIN_ROWS, HEALTH_WAIT_S), (300, 60))


if __name__ == "__main__":
    unittest.main()
