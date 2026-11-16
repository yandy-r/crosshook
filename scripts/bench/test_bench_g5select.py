"""G5 frame selection checks over inline wlpresent logs; stdlib only."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_g5select as g5
import bench_present
from bench_capture import CaptureError

MS = 1_000_000
HEADER = "H\tversion=1\tpid=42\tcomm=app\tclock_id=1\tpres_version=1\n"
SURFACES = "S\t100\t1\ttoplevel\nS\t101\t2\tcursor\n"
INJECT, RENDER, UNC, DEADLINE = 1000 * MS, 1100 * MS, 2 * MS, 2000 * MS


def commit(ms, seq, surface=1, buffer=1):
    return f"C\t{ms * MS}\t{surface}\t{seq}\t{buffer}\n"


def present(ms, seq):
    return f"P\t{ms * MS}\t{seq}\t{ms * MS}\t8341675\t1\t1\n"


def log(*records):
    return bench_present._parse_text(HEADER + SURFACES + "".join(records), "test.log")


def select(logs, render=RENDER, unc=UNC):
    return g5.select_trial([logs], INJECT, render, unc, DEADLINE)


class SelectTrialTests(unittest.TestCase):
    def test_normal_case_picks_first_commit_after_window(self):
        trial = select(log(commit(1101, 1), commit(1120, 2), present(1130, 1), present(1150, 2)))
        # 1101 sits inside [1098, 1102): ambiguous. Move it clear of the window for the normal case.
        self.assertEqual(trial.status, g5.AMBIGUOUS)
        trial = select(log(commit(1110, 1), commit(1120, 2), present(1130, 1), present(1150, 2)))
        self.assertEqual(trial, g5.Trial(g5.MEASURED, 1110 * MS, 1130 * MS, 130.0))

    def test_stale_in_flight_commit_before_render_is_skipped(self):
        trial = select(log(commit(1050, 1), commit(1110, 2), present(1060, 1), present(1130, 2)))
        self.assertEqual((trial.status, trial.commit_ns, trial.presented_ns),
                         (g5.MEASURED, 1110 * MS, 1130 * MS))

    def test_caret_blink_before_render_mark_is_not_selected(self):
        trial = select(log(commit(1090, 1), commit(1112, 2), present(1095, 1), present(1140, 2)))
        self.assertEqual((trial.status, trial.commit_ns, trial.latency_ms), (g5.MEASURED, 1112 * MS, 140.0))

    def test_commit_inside_window_is_ambiguous_but_records_next_presented(self):
        for ms in (1098, 1100, 1101):  # lower bound inclusive, upper bound exclusive
            with self.subTest(commit_ms=ms):
                trial = select(log(commit(ms, 1), commit(1120, 2), present(ms + 20, 1), present(1150, 2)))
                self.assertEqual((trial.status, trial.commit_ns, trial.presented_ns, trial.latency_ms),
                                 (g5.AMBIGUOUS, ms * MS, (ms + 20) * MS, None))
        edge = select(log(commit(1102, 1), present(1130, 1)))  # exactly render + uncertainty: selectable
        self.assertEqual(edge.status, g5.MEASURED)

    def test_commit_before_inject_never_counts(self):
        trial = select(log(commit(999, 1), commit(1110, 2), present(1000, 1), present(1130, 2)))
        self.assertEqual((trial.status, trial.commit_ns), (g5.MEASURED, 1110 * MS))
        # Even inside the render window, a commit older than the inject cannot make the trial ambiguous.
        trial = g5.select_trial([log(commit(1099, 1), commit(1110, 2), present(1120, 1), present(1130, 2))],
                                1100 * MS, RENDER, UNC, DEADLINE)
        self.assertEqual((trial.status, trial.commit_ns), (g5.MEASURED, 1110 * MS))

    def test_discarded_chosen_commit_moves_to_next(self):
        trial = select(log(commit(1110, 1), commit(1120, 2), "D\t9\t1\n", present(1150, 2)))
        self.assertEqual((trial.status, trial.commit_ns, trial.latency_ms), (g5.MEASURED, 1120 * MS, 150.0))

    def test_non_toplevel_and_bufferless_commits_ignored(self):
        trial = select(log(commit(1100, 1, surface=2), commit(1101, 2, buffer=0), commit(1110, 3),
                           present(1105, 1), present(1106, 2), present(1130, 3)))
        self.assertEqual((trial.status, trial.commit_ns), (g5.MEASURED, 1110 * MS))

    def test_timeout_when_nothing_presented_by_deadline(self):
        self.assertEqual(select(log(commit(1110, 1))), g5.Trial(g5.TIMEOUT))
        self.assertEqual(select(log(commit(1110, 1), present(2001, 1))), g5.Trial(g5.TIMEOUT))
        self.assertEqual(select(log()), g5.Trial(g5.TIMEOUT))

    def test_latency_depends_only_on_inject_and_presented(self):
        logs = log(commit(1110, 1), commit(1120, 2), present(1130, 1), present(1150, 2))
        base = select(logs)
        for render, unc in ((RENDER + 5 * MS, UNC), (RENDER - 3 * MS, UNC + MS), (RENDER, 0), (RENDER - 7 * MS, MS)):
            with self.subTest(render_ms=render / MS, unc_ms=unc / MS):
                shifted = select(logs, render, unc)
                self.assertEqual(shifted.commit_ns, base.commit_ns)
                self.assertEqual(shifted.latency_ms, base.latency_ms)
        moved = select(logs, RENDER + 15 * MS, UNC)  # a later mark selects the next commit
        self.assertEqual((moved.commit_ns, moved.latency_ms), (1120 * MS, 150.0))
        inject_moved = g5.select_trial([logs], INJECT + 4 * MS, RENDER, UNC, DEADLINE)
        self.assertEqual(inject_moved.latency_ms, 126.0)


class SummarizeTests(unittest.TestCase):
    @staticmethod
    def measured(count, base=100.0):
        return [g5.Trial(g5.MEASURED, 1, 2, base + i) for i in range(count)]

    def test_captured_row(self):
        row = g5.summarize("search", self.measured(20) + [g5.Trial(g5.TIMEOUT)])
        self.assertEqual(row["scenario"], "search")
        self.assertEqual((row["measured"], row["ambiguous"], row["timeout"], row["early"], row["status"]),
                         (20, 0, 1, 0, "captured"))
        self.assertAlmostEqual(row["p50_ms"], 109.5)
        self.assertAlmostEqual(row["p95_ms"], 118.05)
        self.assertAlmostEqual(row["p99_ms"], 118.81)

    def test_ambiguity_above_ten_percent_is_partial(self):
        ambiguous = g5.Trial(g5.AMBIGUOUS, 1, 2)
        self.assertEqual(g5.summarize("dpad", self.measured(18) + [ambiguous] * 2)["status"], "captured")
        row = g5.summarize("dpad", self.measured(17) + [ambiguous] * 3)
        self.assertEqual((row["status"], row["measured"], row["ambiguous"]), ("partial", 17, 3))

    def test_timeouts_count_toward_the_partial_rule_over_all_trials(self):
        timeout = g5.Trial(g5.TIMEOUT)
        self.assertEqual(g5.summarize("dpad", self.measured(18) + [timeout] * 2)["status"], "captured")
        row = g5.summarize("dpad", self.measured(17) + [timeout] * 3)
        self.assertEqual((row["status"], row["measured"], row["timeout"], row["ambiguous"]),
                         ("partial", 17, 3, 0))

    def test_early_trials_count_toward_the_partial_rule_and_are_not_measured(self):
        early = g5.Trial(g5.EARLY)
        self.assertEqual(g5.summarize("dpad", self.measured(18) + [early] * 2)["status"], "captured")
        row = g5.summarize("dpad", self.measured(17) + [early] * 3)
        self.assertEqual((row["status"], row["measured"], row["early"]), ("partial", 17, 3))
        self.assertAlmostEqual(row["p50_ms"], 108.0)  # only the 17 measured latencies

    def test_ambiguous_timeout_and_early_are_pooled_for_the_partial_rule(self):
        mixed = [g5.Trial(g5.AMBIGUOUS, 1, 2), g5.Trial(g5.TIMEOUT), g5.Trial(g5.EARLY)]
        row = g5.summarize("search", self.measured(27) + mixed)  # 3 of 30 is exactly the limit
        self.assertEqual((row["status"], row["ambiguous"], row["timeout"], row["early"]),
                         ("captured", 1, 1, 1))
        row = g5.summarize("search", self.measured(26) + mixed + [g5.Trial(g5.TIMEOUT)])  # 4 of 30
        self.assertEqual((row["status"], row["timeout"]), ("partial", 2))

    def test_the_row_keys_match_the_g5_csv_schema(self):
        from bench_cli import SCHEMAS
        row = g5.summarize("search", self.measured(10))
        self.assertEqual(list(row), SCHEMAS["G5"].split(","))

    def test_too_few_measured_trials(self):
        trials = self.measured(9) + [g5.Trial(g5.TIMEOUT)] * 5
        with self.assertRaisesRegex(CaptureError, r"^CAPTURE_TOO_SHORT: route: 9 measured trials, need 10$"):
            g5.summarize("route", trials)
        self.assertEqual(g5.summarize("route", self.measured(10))["measured"], 10)


if __name__ == "__main__":
    unittest.main()
