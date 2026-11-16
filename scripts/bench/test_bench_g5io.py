"""G5 toggle-state snapshot/restore containment and the observation CSV; stdlib only."""
import csv
import io
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_g5io as io5
from bench_capture import CaptureError
from bench_drivers_g5 import TrialRecord
from bench_g5select import AMBIGUOUS, MEASURED, TIMEOUT, Trial
from bench_guard import FLATPAK_APP_ID


class StateTestCase(unittest.TestCase):
    flatpak = False

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve() / "crosshook-bench-x"
        if self.flatpak:
            base = self.root / "home/.var/app" / FLATPAK_APP_ID
            self.config, self.data = base / "config/crosshook", base / "data/crosshook"
            self.env = {"HOME": str(self.root / "home"), "CROSSHOOK_BENCH_ROOT": str(base)}
        else:
            self.config, self.data = self.root / "config/crosshook", self.root / "data/crosshook"
            self.env = {"HOME": str(self.root / "home"), "XDG_CONFIG_HOME": str(self.root / "config"),
                        "XDG_DATA_HOME": str(self.root / "data"), "CROSSHOOK_BENCH_ROOT": str(self.root)}
        (self.config / "profiles/sub").mkdir(parents=True)
        self.data.mkdir(parents=True)
        (self.root / "home").mkdir(exist_ok=True)
        (self.config / "profiles/a.toml").write_text("favorite = false\n")
        (self.config / "profiles/sub/b.toml").write_text("b\n")
        (self.data / "metadata.db").write_bytes(b"db-original")
        (self.data / "metadata.db-wal").write_bytes(b"wal-original")
        (self.config / "settings.toml").write_text("keep = true\n")  # not part of the toggle state

    def state(self):
        return io5.ToggleState(self.env, self.flatpak)

    def mutate(self):
        (self.config / "profiles/a.toml").write_text("favorite = true\n")
        (self.config / "profiles/new.toml").write_text("new\n")
        (self.config / "profiles/sub/b.toml").unlink()
        (self.data / "metadata.db").write_bytes(b"db-changed")
        (self.data / "metadata.db-shm").write_bytes(b"shm-created")
        (self.data / "metadata.db-wal").unlink(missing_ok=True)

    def assert_original(self):
        self.assertEqual((self.config / "profiles/a.toml").read_text(), "favorite = false\n")
        self.assertEqual((self.config / "profiles/sub/b.toml").read_text(), "b\n")
        self.assertFalse((self.config / "profiles/new.toml").exists())
        self.assertEqual((self.data / "metadata.db").read_bytes(), b"db-original")
        self.assertEqual((self.data / "metadata.db-wal").read_bytes(), b"wal-original")
        self.assertFalse((self.data / "metadata.db-shm").exists())
        self.assertEqual((self.config / "settings.toml").read_text(), "keep = true\n")


class ToggleStateTests(StateTestCase):
    def test_restore_returns_profiles_and_database_files_to_the_snapshot(self):
        state = self.state()
        state.capture()
        self.mutate()
        state.restore()
        self.assert_original()

    def test_restore_can_run_after_every_iteration(self):
        state = self.state()
        state.capture()
        for _ in range(3):
            self.mutate()
            state.restore()
            self.assert_original()

    def test_files_absent_at_capture_are_removed_on_restore(self):
        (self.data / "metadata.db").unlink()
        (self.data / "metadata.db-wal").unlink(missing_ok=True)
        state = self.state()
        state.capture()
        self.mutate()
        state.restore()
        self.assertFalse((self.data / "metadata.db").exists())
        self.assertFalse((self.data / "metadata.db-wal").exists())
        self.assertFalse((self.data / "metadata.db-shm").exists())

    def test_a_missing_profiles_directory_is_restored_as_absent(self):
        for path in sorted((self.config / "profiles").rglob("*"), reverse=True):
            path.rmdir() if path.is_dir() else path.unlink()
        (self.config / "profiles").rmdir()
        state = self.state()
        state.capture()
        (self.config / "profiles").mkdir()
        (self.config / "profiles/x.toml").write_text("x")
        state.restore()
        self.assertFalse((self.config / "profiles").exists())

    def test_discard_removes_the_snapshot_and_restore_then_does_nothing_harmful(self):
        state = self.state()
        state.capture()
        store = self.root / io5.STATE_DIR
        self.assertTrue(store.is_dir())
        state.discard()
        self.assertFalse(store.exists())

    def test_restore_before_capture_touches_nothing(self):
        state = self.state()
        self.mutate()
        state.restore()
        self.assertEqual((self.data / "metadata.db").read_bytes(), b"db-changed")

    def test_a_snapshot_that_already_exists_is_refused(self):
        (self.root / io5.STATE_DIR).mkdir()
        with self.assertRaisesRegex(CaptureError, "^STATE_UNSAFE: g5-state already exists"):
            self.state().capture()

    def test_a_symlink_in_the_state_is_refused_not_followed(self):
        outside = Path(self.temp.name) / "outside.toml"
        outside.write_text("secret")
        (self.config / "profiles/link.toml").symlink_to(outside)
        state = self.state()
        with self.assertRaisesRegex(CaptureError, "^STATE_UNSAFE: link.toml is not a regular file"):
            state.capture()
        state.restore()  # a failed capture restores nothing
        self.assertEqual(outside.read_text(), "secret")

    def test_restore_unlinks_a_symlink_the_app_planted_without_following_it(self):
        outside = Path(self.temp.name) / "outside-dir"
        outside.mkdir()
        (outside / "victim").write_text("victim")
        state = self.state()
        state.capture()
        (self.config / "profiles/sub/b.toml").unlink()
        (self.config / "profiles/evil").symlink_to(outside)
        state.restore()
        self.assertTrue((outside / "victim").exists())
        self.assertFalse(os.path.lexists(self.config / "profiles/evil"))
        self.assert_original()

    def test_a_config_directory_that_resolves_outside_the_bench_root_is_refused(self):
        outside = Path(self.temp.name) / "elsewhere"
        (outside / "profiles").mkdir(parents=True)
        (outside / "profiles/keep.toml").write_text("keep")
        os.rename(self.config, self.root / "config-old")
        self.config.symlink_to(outside)
        with self.assertRaisesRegex(CaptureError, "^STATE_UNSAFE: profiles resolves outside the bench root"):
            self.state()
        self.assertEqual((outside / "profiles/keep.toml").read_text(), "keep")

    def test_a_root_outside_any_bench_directory_is_refused(self):
        env = {**self.env, "CROSSHOOK_BENCH_ROOT": self.temp.name}
        with self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE: "):
            io5.ToggleState(env, False)

    def test_an_incomplete_environment_is_refused(self):
        with self.assertRaisesRegex(CaptureError, "^BENCH_ENV_INCOMPLETE: XDG_DATA_HOME"):
            io5.ToggleState({k: v for k, v in self.env.items() if k != "XDG_DATA_HOME"}, False)


class FlatpakToggleStateTests(ToggleStateTests):
    flatpak = True

    def test_a_config_directory_that_resolves_outside_the_bench_root_is_refused(self):
        outside = Path(self.temp.name) / "elsewhere"
        (outside / "profiles").mkdir(parents=True)
        os.rename(self.config, self.config.parent / "crosshook-old")
        self.config.symlink_to(outside)
        with self.assertRaisesRegex(CaptureError, "^STATE_UNSAFE: profiles resolves outside the bench root"):
            self.state()

    def test_an_incomplete_environment_is_refused(self):
        with self.assertRaisesRegex(CaptureError, "^BENCH_ENV_INCOMPLETE: HOME"):
            io5.ToggleState({"CROSSHOOK_BENCH_ROOT": self.env["CROSSHOOK_BENCH_ROOT"]}, True)

    def test_a_root_outside_any_bench_directory_is_refused(self):
        env = {**self.env, "CROSSHOOK_BENCH_ROOT": self.temp.name}
        with self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE: "):
            io5.ToggleState(env, True)


def record(inject, status, latency=None, commit=None, presented=None, uncertainty=1_500_000):
    return TrialRecord(0, inject, inject + 30_000_000, uncertainty, Trial(status, commit, presented, latency))


class ObservationTests(unittest.TestCase):
    def rows(self, records):
        return list(csv.DictReader(io.StringIO(io5.observation_text(records))))

    def test_columns_and_one_row_per_trial_with_a_running_seq(self):
        text = io5.observation_text([record(1000, MEASURED, 60.0, 41_000_000, 61_000_000)])
        self.assertEqual(text.splitlines()[0], "seq,inject_ns,render_ns,commit_ns,presented_ns,"
                                               "latency_ms,uncertainty_ms,status")
        rows = self.rows([record(1000, MEASURED, 60.0, 41_000_000, 61_000_000), record(2000, TIMEOUT),
                          record(3000, AMBIGUOUS, None, 5, 9)])
        self.assertEqual([row["seq"] for row in rows], ["0", "1", "2"])
        self.assertEqual(rows[0], {"seq": "0", "inject_ns": "1000", "render_ns": "30001000",
                                   "commit_ns": "41000000", "presented_ns": "61000000", "latency_ms": "60.000",
                                   "uncertainty_ms": "1.500", "status": "measured"})

    def test_unselected_fields_are_empty_and_an_ambiguous_trial_has_no_latency(self):
        timeout, ambiguous = self.rows([record(2000, TIMEOUT), record(3000, AMBIGUOUS, None, 5, 9)])
        self.assertEqual((timeout["commit_ns"], timeout["presented_ns"], timeout["latency_ms"]), ("", "", ""))
        self.assertEqual((ambiguous["commit_ns"], ambiguous["latency_ms"], ambiguous["status"]),
                         ("5", "", "ambiguous"))

    def test_write_observations_puts_the_file_beside_the_present_logs_and_can_rewrite_it(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "crosshook-bench-x"
            (root / "home").mkdir(parents=True)
            env = {"HOME": str(root / "home")}
            path = io5.write_observations(env, "search", [record(1, TIMEOUT)])
            self.assertEqual(path, root / "observations/G5-search.csv")
            self.assertEqual(oct(path.stat().st_mode & 0o777), "0o600")
            self.assertEqual(oct(path.parent.stat().st_mode & 0o777), "0o700")
            io5.write_observations(env, "search", [record(1, TIMEOUT), record(2, TIMEOUT)])
            self.assertEqual(len(path.read_text().splitlines()), 3)


if __name__ == "__main__":
    unittest.main()
