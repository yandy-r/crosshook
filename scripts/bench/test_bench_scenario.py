"""G4 scenario preparation: the console settings file, its removal, and containment in the bench root."""
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_drivers as bd
from bench_capture import CaptureError

FLATPAK_CONFIG = "home/.var/app/dev.crosshook.CrossHook/config/crosshook"


class ScenarioTestCase(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.outer = Path(self.temp.name)
        self.root = self.outer / "crosshook-bench-x"
        self.root.mkdir()

    def env(self, flatpak, root=None):
        home = self.root / "home"
        env = {"HOME": str(home), "XDG_CONFIG_HOME": str(self.root / "config"),
               "CROSSHOOK_BENCH_LOG": "/stale", "KEEP": "1"}
        env["CROSSHOOK_BENCH_ROOT"] = str(root or (home / ".var/app/dev.crosshook.CrossHook" if flatpak
                                                    else self.root))
        return env


class PrepareScenarioTests(ScenarioTestCase):
    def check_console(self, flatpak, config):
        env = self.env(flatpak)
        updated = bd.prepare_scenario("console", env, flatpak)
        path = config / "settings.toml"
        self.assertEqual(path.read_text(), "console_drawer_collapsed_default = false\n")
        self.assertEqual(stat.S_IMODE(os.stat(path).st_mode), 0o600)
        self.assertEqual(updated["CROSSHOOK_BENCH_LOG"],
                         str(Path(env["CROSSHOOK_BENCH_ROOT"]) / "logs/launch-50k.log"))
        self.assertEqual(updated["CROSSHOOK_BENCH_LOG_RATE"], "5000")
        self.assertEqual(updated["KEEP"], "1")
        self.assertEqual(env["CROSSHOOK_BENCH_LOG"], "/stale")  # the input is not mutated
        bd.prepare_scenario("console", env, flatpak)  # a second iteration rewrites the file
        self.assertEqual(path.read_text(), "console_drawer_collapsed_default = false\n")

    def test_console_native_writes_settings_under_xdg_config_home(self):
        self.check_console(False, self.root / "config/crosshook")

    def test_console_flatpak_writes_settings_under_the_app_sandbox_config(self):
        self.check_console(True, self.root / FLATPAK_CONFIG)

    def test_other_scenarios_write_nothing_and_drop_the_replay_env(self):
        for scenario in ("library", "settings"):
            updated = bd.prepare_scenario(scenario, self.env(False), False)
            self.assertNotIn("CROSSHOOK_BENCH_LOG", updated)
        self.assertFalse((self.root / "config").exists())

    def test_console_settings_do_not_leak_into_later_scenarios(self):
        for flatpak, config in ((False, self.root / "config/crosshook"), (True, self.root / FLATPAK_CONFIG)):
            path = config / "settings.toml"
            for later in ("settings", "library"):
                with self.subTest(flatpak=flatpak, later=later):
                    env = self.env(flatpak)
                    bd.prepare_scenario("console", env, flatpak)
                    self.assertTrue(path.is_file())
                    bd.prepare_scenario(later, env, flatpak)
                    self.assertFalse(os.path.lexists(path))
                    bd.prepare_scenario(later, env, flatpak)  # nothing left to remove is fine

    def test_a_symlinked_settings_file_is_refused_and_left_alone(self):
        config = self.root / "config/crosshook"
        config.mkdir(parents=True)
        target = self.root / "elsewhere.toml"
        target.write_text("keep = true\n")
        (config / "settings.toml").symlink_to(target)
        for scenario in ("library", "settings", "console"):
            with self.subTest(scenario=scenario), self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE"):
                bd.prepare_scenario(scenario, self.env(False), False)
        self.assertEqual(target.read_text(), "keep = true\n")
        self.assertTrue((config / "settings.toml").is_symlink())

    def test_incomplete_env_and_unknown_scenario_fail_by_name(self):
        with self.assertRaisesRegex(CaptureError, "^BENCH_ENV_INCOMPLETE"):
            bd.prepare_scenario("console", {"HOME": str(self.root)}, False)
        with self.assertRaisesRegex(CaptureError, "^UNKNOWN_SCENARIO"):
            bd.prepare_scenario("x", {}, False)
        env = self.env(False)
        del env["CROSSHOOK_BENCH_ROOT"]
        for scenario in ("library", "console"):
            with self.subTest(scenario=scenario), \
                    self.assertRaisesRegex(CaptureError, "^BENCH_ENV_INCOMPLETE: CROSSHOOK_BENCH_ROOT"):
                bd.prepare_scenario(scenario, env, False)


class ContainmentTests(ScenarioTestCase):
    def outside_config(self):
        """A settings.toml somewhere real users keep theirs: outside every crosshook-bench-* root."""
        config = self.outer / "real-config"
        (config / "crosshook").mkdir(parents=True)
        victim = config / "crosshook/settings.toml"
        victim.write_text("keep = true\n")
        return config, victim

    def test_a_config_home_outside_the_bench_root_is_refused_and_untouched(self):
        config, victim = self.outside_config()
        env = {**self.env(False), "XDG_CONFIG_HOME": str(config)}
        for scenario in ("library", "settings", "console"):
            with self.subTest(scenario=scenario), self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE"):
                bd.prepare_scenario(scenario, env, False)
        with self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE"):
            bd.remove_scenario_settings(env, False)
        self.assertEqual(victim.read_text(), "keep = true\n")

    def test_a_config_dir_symlinked_out_of_the_root_is_refused_and_untouched(self):
        config, victim = self.outside_config()
        (self.root / "config").symlink_to(config)
        for call in (lambda: bd.prepare_scenario("console", self.env(False), False),
                     lambda: bd.remove_scenario_settings(self.env(False), False)):
            with self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE"):
                call()
        self.assertEqual(victim.read_text(), "keep = true\n")

    def test_a_bench_root_that_is_not_a_crosshook_bench_directory_is_refused(self):
        plain = self.outer / "plain"
        (plain / "config/crosshook").mkdir(parents=True)
        (plain / "config/crosshook/settings.toml").write_text("keep = true\n")
        env = {**self.env(False, root=plain), "XDG_CONFIG_HOME": str(plain / "config")}
        with self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE: CROSSHOOK_BENCH_ROOT"):
            bd.remove_scenario_settings(env, False)
        self.assertTrue((plain / "config/crosshook/settings.toml").exists())

    def test_the_root_may_be_the_flatpak_sandbox_dir_inside_the_bench_root(self):
        path = self.root / FLATPAK_CONFIG / "settings.toml"
        path.parent.mkdir(parents=True)
        path.write_text("x\n")
        bd.remove_scenario_settings(self.env(True), True)
        self.assertFalse(path.exists())

    def test_remove_deletes_a_regular_file_and_tolerates_none(self):
        path = self.root / "config/crosshook/settings.toml"
        path.parent.mkdir(parents=True)
        path.write_text("x\n")
        bd.remove_scenario_settings(self.env(False), False)
        self.assertFalse(path.exists())
        bd.remove_scenario_settings(self.env(False), False)  # nothing left is fine

    def test_remove_refuses_a_non_regular_file_and_incomplete_env(self):
        path = self.root / "config/crosshook/settings.toml"
        path.mkdir(parents=True)
        with self.assertRaisesRegex(CaptureError, "^SETTINGS_UNSAFE"):
            bd.remove_scenario_settings(self.env(False), False)
        self.assertTrue(path.is_dir())
        with self.assertRaisesRegex(CaptureError, "^BENCH_ENV_INCOMPLETE"):
            bd.remove_scenario_settings({"HOME": str(self.root)}, False)


if __name__ == "__main__":
    unittest.main()
