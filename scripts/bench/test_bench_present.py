"""Focused wlpresent log parser checks; stdlib only."""
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_present
from bench_capture import CaptureError

HEADER = "H\tversion=1\tpid=42\tcomm=game\tclock_id=1\tpres_version=1\n"
SURFACES = "S\t100\t1\ttoplevel\nS\t101\t2\tcursor\n"
HAPPY = (HEADER + SURFACES
         + "C\t1000\t1\t10\t1\nC\t2000\t1\t11\t1\nC\t2500\t2\t12\t1\n"
         + "P\t9100\t11\t8000000\t8341675\t2\t1\n"
         + "P\t9200\t10\t5000000\t8341675\t1\t1\n"
         + "P\t9300\t12\t6000000\t8341675\t3\t1\n")


class PresentLogTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def write(self, text, name="wlpresent-1.log"):
        path = self.root / name
        path.write_text(text)
        return path

    def logs(self, text):
        self.write(text)
        return bench_present.parse_dir(self.root)

    def test_happy_path_sorts_frames_and_ignores_cursor(self):
        logs = self.logs(HAPPY)
        bench_present.require_clock(logs)
        frames = bench_present.toplevel_frames(logs)
        self.assertEqual([f.presented_ns for f in frames], [5000000, 8000000])
        self.assertEqual(bench_present.first_present_ns(logs), 5000000)
        self.assertEqual(logs[0].roles, {1: "toplevel", 2: "cursor"})

    def test_missing_clock_or_incomplete_log_refused(self):
        with self.assertRaisesRegex(CaptureError, r"^PRESENT_LOG_MALFORMED: header without clock_id"):
            bench_present.require_clock(self.logs(HAPPY.replace("\tclock_id=1", "")))
        for condition in ("log_cap", "bind_failed", "wrap_failed"):
            with self.subTest(condition=condition), \
                    self.assertRaisesRegex(CaptureError, f"^PRESENT_LOG_INCOMPLETE: {condition}"):
                bench_present.require_clock(self.logs(HAPPY + f"X\t9\t{condition}\n"))

    def test_wrong_clock(self):
        logs = self.logs(HAPPY.replace("clock_id=1", "clock_id=7"))
        with self.assertRaisesRegex(CaptureError, r"^PRESENTATION_CLOCK: clock_id=7, expected 1"):
            bench_present.require_clock(logs)

    def test_no_presentation_global(self):
        # What the shim really writes: the header carries clock_id=-1 next to the X record.
        logs = self.logs(HEADER.replace("clock_id=1", "clock_id=-1").replace("pres_version=1", "pres_version=0")
                         + "X\t5\tno_presentation_global\n")
        with self.assertRaisesRegex(CaptureError, "^NO_PRESENTATION_GLOBAL"):
            bench_present.require_clock(logs)

    def test_unknown_clock_without_condition_is_still_no_global(self):
        logs = self.logs(HEADER.replace("clock_id=1", "clock_id=-1"))
        with self.assertRaisesRegex(CaptureError, "^NO_PRESENTATION_GLOBAL"):
            bench_present.require_clock(logs)

    def test_process_without_global_does_not_hide_wrong_clock_elsewhere(self):
        self.write(HEADER.replace("clock_id=1", "clock_id=-1") + "X\t5\tno_presentation_global\n", "wlpresent-1.log")
        self.write(HAPPY.replace("clock_id=1", "clock_id=7"), "wlpresent-2.log")
        with self.assertRaisesRegex(CaptureError, "^PRESENTATION_CLOCK: clock_id=7"):
            bench_present.require_clock(bench_present.parse_dir(self.root))

    def test_surface_id_reuse_follows_log_order(self):
        text = (HEADER + "S\t1\t1\ttoplevel\nC\t2\t1\t1\t1\nS\t3\t1\tdestroyed\n"
                + "C\t4\t1\t2\t1\nS\t5\t1\tcursor\nC\t6\t1\t3\t1\n"
                + "S\t7\t1\ttoplevel\nC\t8\t1\t4\t1\n"
                + "P\t9\t1\t100\t8341675\t1\t1\nP\t9\t2\t200\t8341675\t1\t1\n"
                + "P\t9\t3\t300\t8341675\t1\t1\nP\t9\t4\t400\t8341675\t1\t1\n")
        logs = self.logs(text)
        self.assertEqual([c.toplevel for _, c in sorted(logs[0].commits.items())], [True, False, False, True])
        self.assertEqual([f.presented_ns for f in bench_present.toplevel_frames(logs)], [100, 400])
        self.assertEqual(logs[0].roles, {1: "toplevel"})

    def test_cursor_only_has_no_toplevel_commits(self):
        logs = self.logs(HEADER + SURFACES + "C\t1\t2\t5\t1\nP\t9\t5\t7\t8341675\t1\t1\n")
        with self.assertRaisesRegex(CaptureError, "^NO_TOPLEVEL_COMMITS"):
            bench_present.toplevel_frames(logs)

    def test_commit_without_buffer_excluded(self):
        logs = self.logs(HEADER + SURFACES + "C\t1\t1\t5\t0\nP\t9\t5\t7\t8341675\t1\t1\n")
        with self.assertRaisesRegex(CaptureError, "^NO_TOPLEVEL_COMMITS"):
            bench_present.toplevel_frames(logs)

    def test_truncated_tail_ignored(self):
        logs = self.logs(HAPPY + "P\t9400\t99\t77")
        self.assertEqual(len(logs[0].presented), 3)

    def test_malformed_line_raises(self):
        for bad in ("C\t1\t1\t5\n", "Q\t1\n", "P\t1\t2\tx\t4\t5\t6\n"):
            with self.subTest(bad=bad):
                self.write(HEADER + bad + "S\t1\t1\ttoplevel\n")
                with self.assertRaisesRegex(CaptureError,
                                            r"^PRESENT_LOG_MALFORMED: wlpresent-1\.log:2$"):
                    bench_present.parse_log(self.root / "wlpresent-1.log")

    def test_oversize_and_symlink_refused(self):
        path = self.write(HEADER)
        with unittest.mock.patch.object(bench_present, "MAX_LOG_BYTES", 4):
            with self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_TOO_LARGE"):
                bench_present.parse_log(path)
        link = self.root / "link.log"
        link.symlink_to(path)
        with self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_UNREADABLE"):
            bench_present.parse_log(link)

    def test_refresh_match_and_mismatch(self):
        frames = bench_present.toplevel_frames(self.logs(HAPPY))
        self.assertAlmostEqual(bench_present.check_refresh(frames, 119.88), 119.88, places=1)
        slow = bench_present.toplevel_frames(self.logs(HAPPY.replace("8341675", "5553611")))
        with self.assertRaisesRegex(CaptureError, r"^REFRESH_MISMATCH: compositor refresh 180\.06 Hz"):
            bench_present.check_refresh(slow, 119.88)
        unknown = bench_present.toplevel_frames(self.logs(HAPPY.replace("8341675", "0")))
        self.assertIsNone(bench_present.check_refresh(unknown, 119.88))

    def test_discard_rate(self):
        text = (HEADER + SURFACES + "C\t1\t1\t1\t1\nC\t2\t1\t2\t1\nC\t3\t1\t3\t1\nC\t4\t2\t4\t1\n"
                + "P\t9\t1\t10\t8341675\t1\t1\nD\t9\t2\nD\t9\t3\nD\t9\t4\n")
        self.assertAlmostEqual(bench_present.discard_rate(self.logs(text)), 2 / 3)
        self.assertEqual(bench_present.discard_rate(self.logs(HEADER)), 0.0)

    def test_discard_rate_window_keeps_commits_inside_it(self):
        text = (HEADER + SURFACES + "C\t1\t1\t1\t1\nC\t2\t1\t2\t1\nC\t3\t1\t3\t1\nC\t4\t1\t4\t1\nC\t5\t2\t5\t1\n"
                + "P\t9\t1\t10\t8341675\t1\t1\nP\t9\t4\t11\t8341675\t1\t1\nD\t9\t2\nD\t9\t3\nD\t9\t5\n")
        logs = self.logs(text)
        rate = bench_present.discard_rate
        self.assertAlmostEqual(rate(logs), 2 / 4)                  # the cursor commit never counts
        self.assertAlmostEqual(rate(logs, 2, 3), 1.0)             # both commits in the window were dropped
        self.assertAlmostEqual(rate(logs, 1, 1), 0.0)             # the only commit in it was shown
        self.assertAlmostEqual(rate(logs, 3, 4), 1 / 2)           # bounds are inclusive
        self.assertAlmostEqual(rate(logs, start_ns=3), 1 / 2)
        self.assertAlmostEqual(rate(logs, end_ns=2), 1 / 2)
        self.assertEqual(rate(logs, 100, 200), 0.0)

    def test_interval_windowing(self):
        frames = [bench_present.Presented(0, i, ns, 0, 0, 0)
                  for i, ns in enumerate([0, 10_000_000, 25_000_000, 45_000_000])]
        self.assertEqual(bench_present.frame_intervals_ms(frames), [10.0, 15.0, 20.0])
        self.assertEqual(bench_present.frame_intervals_ms(frames, 10_000_000, 25_000_000), [15.0])
        self.assertEqual(bench_present.frame_intervals_ms(frames, end_ns=10_000_000), [10.0])
        self.assertEqual(bench_present.frame_intervals_ms(frames, start_ns=45_000_000), [])

    def test_empty_dir_is_shim_not_loaded(self):
        with self.assertRaisesRegex(CaptureError, "^SHIM_NOT_LOADED: no wlpresent log in "):
            bench_present.parse_dir(self.root)

    def test_focus_and_geometry_parsed_in_log_order(self):
        logs = self.logs(HEADER + SURFACES + "K\t10\tenter\t1\nM\t11\tenter\t2\nK\t12\tleave\t1\n"
                         + "G\t13\t1\t800\t600\t1\nG\t14\t1\t810\t600\t0\n")
        self.assertEqual(logs[0].focus, [bench_present.Focus(10, "keyboard", True, 1, True),
                                         bench_present.Focus(11, "pointer", True, 2, False),
                                         bench_present.Focus(12, "keyboard", False, 1, True)])
        self.assertEqual([(g.ns, g.w, g.h, g.activated) for g in logs[0].geometry],
                         [(13, 800, 600, True), (14, 810, 600, False)])

    def test_distinct_sizes_windowing_and_toplevel_only(self):
        logs = self.logs(HEADER + SURFACES + "G\t10\t1\t800\t600\t1\nG\t20\t1\t900\t600\t1\n"
                         + "G\t30\t1\t800\t600\t1\nG\t40\t1\t1000\t700\t1\nG\t50\t2\t5\t5\t0\n")
        self.assertEqual(bench_present.distinct_sizes(logs), 3)
        self.assertEqual(bench_present.distinct_sizes(logs, 20, 30), 2)
        self.assertEqual(bench_present.distinct_sizes(logs, start_ns=31), 1)
        self.assertEqual(bench_present.distinct_sizes(logs, end_ns=9), 0)

    def test_malformed_focus_and_geometry_lines(self):
        for bad in ("K\t1\tenter\n", "K\t1\tfocus\t1\n", "K\tx\tenter\t1\n", "M\t1\tleave\tz\n",
                    "M\t1\tenter\t1\t2\n", "G\t1\t1\t800\t600\n", "G\t1\t1\t800\tx\t1\n",
                    "G\t1\t1\t800\t600\t2\n"):
            with self.subTest(bad=bad):
                self.write(HEADER + bad + "S\t1\t1\ttoplevel\n")
                with self.assertRaisesRegex(CaptureError,
                                            r"^PRESENT_LOG_MALFORMED: wlpresent-1\.log:2$"):
                    bench_present.parse_log(self.root / "wlpresent-1.log")

    def test_truncated_focus_and_geometry_tail_ignored(self):
        for tail in ("K\t9\tent", "G\t9\t1\t800"):
            with self.subTest(tail=tail):
                logs = self.logs(HEADER + SURFACES + "K\t5\tenter\t1\n" + tail)
                self.assertEqual(len(logs[0].focus), 1)
                self.assertEqual(logs[0].geometry, [])

    def test_parse_log_of_a_growing_file_ignores_the_partial_line_and_sees_growth(self):
        path = self.write(HEADER + SURFACES + "K\t10\tenter\t1\nG\t11\t1\t80")
        live = bench_present.parse_log(path)
        self.assertEqual(len(live.focus), 1)
        self.assertEqual(live.geometry, [])
        with path.open("a") as handle:
            handle.write("0\t600\t1\n")
        self.assertEqual(bench_present.parse_log(path).geometry,
                         [bench_present.Geometry(11, 1, 800, 600, True, True)])


if __name__ == "__main__":
    unittest.main()
