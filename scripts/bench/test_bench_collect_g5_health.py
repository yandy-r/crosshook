"""G5 `health_table` collection: scenario env, observations, no toggle snapshot and sealing; fake app and driver."""
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_collect
from bench_provenance import finalize_raw
from test_bench_collect_g5 import G5CollectTestCase

SCENARIO = "health_table"


class G5HealthTableCollectTests(G5CollectTestCase):
    def setUp(self):
        super().setUp()
        self.args.scenarios = {"G5": [SCENARIO]}

    def test_the_row_is_the_health_table_scenario_in_the_g5_schema(self):
        rows = self.rows()
        self.assertEqual([row["scenario"] for row in rows], [SCENARIO])
        (row,) = rows
        self.assertEqual((row["measured"], row["status"]), (20, "captured"))
        self.assertEqual(sorted(row), sorted(bench.SCHEMAS["G5"].split(",")))

    def test_every_launch_sets_the_health_table_scenario_env(self):
        self.rows()
        self.assertEqual(len(self.fake.calls), 2)
        for _command, env, exit_after, events in self.fake.calls:
            self.assertEqual(env["CROSSHOOK_BENCH_SCENARIO"], SCENARIO)
            self.assertFalse(exit_after)
            self.assertTrue(events)
        self.assertEqual({call["scenario"] for call in self.drives}, {SCENARIO})

    def test_the_observation_csv_is_written_private_and_pooled_over_iterations(self):
        self.rows()
        path = self.root / f"observations/G5-{SCENARIO}.csv"
        self.assertEqual(oct(path.stat().st_mode & 0o777), "0o600")
        self.assertEqual([row["seq"] for row in self.observations(SCENARIO)], [str(n) for n in range(20)])

    def test_no_toggle_state_snapshot_is_taken_and_the_profile_is_left_alone(self):
        self.mutate_state = True
        with patch.object(bench_collect, "ToggleState") as toggle_state:
            self.rows()
        toggle_state.assert_not_called()
        self.assertFalse((self.root / "g5-state").exists())
        self.assertNotEqual((self.profiles / "game.toml").read_text(), "optimizations = []\nfavorite = false\n")

    def test_finalize_raw_lists_the_observation_in_sha256sums(self):
        self.rows()
        raw, output = (Path(self.temp.name) / name for name in ("raw", "out"))
        raw.mkdir()
        output.mkdir()
        finalize_raw(raw, self.root, output, [], {"status": "captured"})
        sealed = raw / f"observations/G5-{SCENARIO}.csv"
        self.assertTrue(sealed.is_file())
        self.assertEqual(sealed.stat().st_mode & 0o777, 0o600)
        listed = [line.split("  ", 1)[1] for line in (raw / "SHA256SUMS").read_text().splitlines()]
        self.assertIn(f"observations/G5-{SCENARIO}.csv", listed)


if __name__ == "__main__":
    unittest.main()
