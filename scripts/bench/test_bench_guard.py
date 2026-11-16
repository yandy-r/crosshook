"""Loader-env scrubbing and Flatpak user-installation checks for bench_guard; stdlib only."""
from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_guard

APP_ID = bench_guard.FLATPAK_APP_ID


class ScrubTests(unittest.TestCase):
    def test_loader_and_module_variables_are_scrubbed(self):
        names = ("LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT", "WAYLAND_DEBUG", "LD_DEBUG", "LD_DEBUG_OUTPUT",
                 "LD_PROFILE", "LD_PROFILE_OUTPUT", "GLIBC_TUNABLES", "GTK_MODULES", "GTK3_MODULES",
                 "GIO_EXTRA_MODULES", "GST_PLUGIN_PATH")
        for name in names:
            self.assertIn(name, bench_guard.SCRUBBED)
        env = bench_guard.build_env(Path("/nonexistent-bench-root"), {**{name: "x" for name in names},
                                                                      "PATH": "/usr/bin"})
        for name in names:
            self.assertNotIn(name, env)
        self.assertEqual(env["PATH"], "/usr/bin")


class FlatpakDirTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_real_owned_directory_is_shared(self):
        user = self.root / "flatpak"
        user.mkdir()
        self.assertEqual(bench_guard._user_flatpak_dir({"FLATPAK_USER_DIR": str(user)}), str(user))
        self.assertEqual(bench_guard._user_flatpak_dir({"XDG_DATA_HOME": str(self.root)}), str(user))

    def test_symlinked_or_missing_user_dir_is_rejected(self):
        target = self.root / "target"
        target.mkdir()
        (self.root / "flatpak").symlink_to(target, target_is_directory=True)
        self.assertIsNone(bench_guard._user_flatpak_dir({"FLATPAK_USER_DIR": str(self.root / "flatpak")}))
        self.assertIsNone(bench_guard._user_flatpak_dir({"XDG_DATA_HOME": str(self.root)}))
        self.assertIsNone(bench_guard._user_flatpak_dir({"FLATPAK_USER_DIR": str(self.root / "absent")}))

    def test_file_instead_of_directory_is_rejected(self):
        (self.root / "flatpak").write_text("")
        self.assertIsNone(bench_guard._user_flatpak_dir({"XDG_DATA_HOME": str(self.root)}))

    def test_build_env_only_shares_a_valid_installation(self):
        target = self.root / "target"
        target.mkdir()
        (self.root / "flatpak").symlink_to(target, target_is_directory=True)
        env = bench_guard.build_env(self.root / "run", {"XDG_DATA_HOME": str(self.root)}, flatpak=True)
        self.assertNotIn("FLATPAK_USER_DIR", env)


class OverrideTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.user = Path(self.temp.name)
        (self.user / "overrides").mkdir()

    def test_hashes_global_and_app_overrides(self):
        (self.user / "overrides/global").write_bytes(b"[Context]\nshared=network\n")
        (self.user / "overrides" / APP_ID).write_bytes(b"[Context]\nfilesystems=home\n")
        (self.user / "overrides/other.app").write_bytes(b"ignored")
        self.assertEqual(bench_guard.flatpak_overrides(str(self.user)), {
            "overrides/global": hashlib.sha256(b"[Context]\nshared=network\n").hexdigest(),
            f"overrides/{APP_ID}": hashlib.sha256(b"[Context]\nfilesystems=home\n").hexdigest(),
        })

    def test_missing_files_and_no_dir(self):
        self.assertEqual(bench_guard.flatpak_overrides(str(self.user)), {})
        self.assertEqual(bench_guard.flatpak_overrides(None), {})

    def test_only_regular_files_are_read(self):
        secret = self.user / "secret"
        secret.write_bytes(b"data")
        (self.user / "overrides/global").symlink_to(secret)
        (self.user / "overrides" / APP_ID).mkdir()
        self.assertEqual(bench_guard.flatpak_overrides(str(self.user)), {})


if __name__ == "__main__":
    unittest.main()
