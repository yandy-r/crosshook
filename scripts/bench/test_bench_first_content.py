"""G1/G2 `first_content_ms`: the first toplevel frame presented at or after READY; stdlib only."""
import csv
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_capture
import bench_cli
import bench_collect
import bench_present
from bench_capture import CaptureError
from bench_present import first_present_at_or_after
from test_bench_collect import CollectTests


def frames_log(*presented_ns: int) -> str:
    """A log with one toplevel frame per timestamp (seq 1, 2, ...)."""
    text = "H\tclock_id=1\nS\t1\t7\ttoplevel\n"
    for seq, ns in enumerate(presented_ns, 1):
        text += f"C\t{ns}\t7\t{seq}\t1\nP\t{ns}\t{seq}\t{ns}\t8333333\t1\t0\n"
    return text


class FirstPresentAtOrAfterTests(unittest.TestCase):
    def logs(self, *presented_ns: int):
        return [bench_present._parse_text(frames_log(*presented_ns), "wlpresent-1.log")]

    def test_picks_first_frame_at_or_after(self):
        logs = self.logs(100, 400, 900)
        self.assertEqual(first_present_at_or_after(logs, 300), 400)
        self.assertEqual(first_present_at_or_after(logs, 400), 400)
        self.assertEqual(first_present_at_or_after(logs, 0), 100)

    def test_none_when_every_frame_precedes(self):
        self.assertIsNone(first_present_at_or_after(self.logs(100, 400), 401))

    def test_no_toplevel_frame_is_an_error(self):
        logs = [bench_present._parse_text("H\tclock_id=1\n", "wlpresent-1.log")]
        with self.assertRaisesRegex(CaptureError, "^NO_TOPLEVEL_COMMITS"):
            first_present_at_or_after(logs, 0)


class FirstContentTests(unittest.TestCase):
    """Reuses the collect fixture (FakeReady) without inheriting its tests."""

    setUp = CollectTests.setUp
    rows = CollectTests.rows

    def test_frame_after_ready_is_measured(self):
        for row in self.rows()[:-1]:
            # present at launch+5 ms, READY at launch+12.5 ms, content frame 20 ms after READY
            self.assertAlmostEqual(row["first_present_ms"], 5.0, places=3)
            self.assertAlmostEqual(row["first_content_ms"], 32.5, places=3)

    def test_content_below_present_is_invalid(self):
        # Unreachable from sorted frames; guards a selector that returns something earlier.
        with patch.object(bench_collect, "first_present_at_or_after", return_value=1), \
                self.assertRaisesRegex(CaptureError, "^INVALID_PRESENT_CLOCK: first content precedes"):
            self.rows()

    def test_ready_exactly_on_a_frame_counts(self):
        self.fake.offset_ns = 12_500_000
        self.fake.content_gap_ns = None  # the pre-READY frame is presented exactly at READY
        for row in self.rows()[:-1]:
            self.assertEqual(row["first_content_ms"], row["first_present_ms"])

    def test_only_pre_ready_frames_give_no_content_frame(self):
        self.fake.content_gap_ns = None
        with patch.object(bench_collect, "PRESENT_POLL_S", 0.1), \
                self.assertRaisesRegex(CaptureError, "^NO_CONTENT_FRAME: no toplevel frame was presented "
                                       "within 0.1 s after READY$"):
            self.rows()

    def test_summary_has_content_percentiles(self):
        summary = self.rows()[-1]
        self.assertEqual(summary["iteration"], "summary")
        self.assertAlmostEqual(summary["first_content_p50_ms"], 32.5, places=3)
        self.assertAlmostEqual(summary["first_content_p95_ms"], 32.5, places=3)


class SchemaTests(unittest.TestCase):
    def test_g1_g2_header(self):
        expected = ("iteration,ready_ms,first_present_ms,first_content_ms,p50_ms,p95_ms,"
                    "first_present_p50_ms,first_present_p95_ms,first_content_p50_ms,"
                    "first_content_p95_ms,status")
        self.assertEqual(bench_cli.SCHEMAS["G1"], expected)
        self.assertEqual(bench_cli.SCHEMAS["G2"], expected)

    def test_csv_mode_capture_leaves_content_columns_blank_in_the_written_file(self):
        calls = []

        def fake_ready(command, env, timeout, exit_after, events=False):
            calls.append((command, exit_after))
            return SimpleNamespace(stderr=None), 900.0

        args = SimpleNamespace(collect="csv", iterations=2, timeout=5, cold_cache_ack=True)
        for metric in ("G1", "G2"):
            with self.subTest(metric=metric), tempfile.TemporaryDirectory() as temp, \
                    patch.object(bench_capture, "ready", fake_ready), patch.object(bench_capture, "stop"):
                rows = bench_capture.capture(metric, args, ["app"], {})
                path = Path(temp) / f"{metric}.csv"
                bench.write_result(path, metric, rows)
                with path.open(newline="") as stream:
                    written = list(csv.DictReader(stream))
                self.assertEqual([row["iteration"] for row in written], ["1", "2", "summary"])
                self.assertEqual([row["ready_ms"] for row in written[:2]], ["900.0", "900.0"])
                for row in written:
                    for column in ("first_present_ms", "first_content_ms", "first_present_p50_ms",
                                   "first_present_p95_ms", "first_content_p50_ms", "first_content_p95_ms"):
                        self.assertIn(column, row)
                        self.assertEqual(row[column], "", f"{metric} {column}")
        self.assertEqual(len(calls), 4)


if __name__ == "__main__":
    unittest.main()
