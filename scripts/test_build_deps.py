"""Regression tests for dependency setup; never install real system packages."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
LIB = ROOT / "scripts/lib/native-build-deps.sh"


class BuildDependenciesTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.log = self.root / "commands"
        self.release = self.root / "os-release"
        self.release.write_text('ID=fedora\n')
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}",
                        CROSSHOOK_OS_RELEASE_FILE=str(self.release),
                        DEPS_TEST_LOG=str(self.log))
        for tool in ("apt-get", "dnf", "pacman", "zypper", "sudo"):
            self.stub(tool, 'printf "%s %s\\n" "${0##*/}" "$*" >> "$DEPS_TEST_LOG"')
        self.stub("sudo", 'exec "$@"')
        for tool in ("cargo", "rustc", "node", "npm", "cc", "c++", "make",
                     "pkg-config", "curl", "file", "flatpak", "flatpak-builder",
                     "rsvg-convert", "desktop-file-validate", "appstreamcli", "magick"):
            self.stub(tool, "exit 0")

    def stub(self, tool, body):
        path = self.bin / tool
        path.write_text("#!/usr/bin/env bash\n" + body + "\n")
        path.chmod(0o755)

    def run_bash(self, body):
        return subprocess.run(["bash", "-c", f'set -euo pipefail; source "{LIB}"; {body}'],
                              env=self.env, text=True, capture_output=True)

    def test_distribution_and_derivative_selection(self):
        for release, expected in (("ID=fedora", "dnf"), ("ID=ubuntu", "apt-get"),
                                  ("ID=arch", "pacman"), ("ID=opensuse-tumbleweed", "zypper"),
                                  ('ID=linuxmint\nID_LIKE="ubuntu debian"', "apt-get"),
                                  ('ID=custom\nID_LIKE="fedora"', "dnf"),
                                  ('ID=ubuntu\nID_LIKE="arch"', "apt-get")):
            with self.subTest(release=release):
                self.release.write_text(release + "\n")
                result = self.run_bash("native_build_deps_manager")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), expected)

    def test_unknown_distribution_does_not_install(self):
        self.release.write_text("ID=unknown\n")
        result = self.run_bash("native_build_deps_install 1 1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unsupported Linux distribution", result.stderr)
        self.assertFalse(self.log.exists())

    def test_all_installers_include_native_and_packaging_packages(self):
        for distro, native, assets, yes in (("fedora", "webkit2gtk4.1-devel", "librsvg2-tools", "-y"),
                                           ("debian", "libwebkit2gtk-4.1-dev", "librsvg2-bin", "-y"),
                                           ("arch", "webkit2gtk-4.1", "librsvg", "--noconfirm"),
                                           ("opensuse", "webkit2gtk3-devel", "rsvg-convert", "--non-interactive")):
            with self.subTest(distro=distro):
                self.release.write_text(f"ID={distro}\n")
                self.log.unlink(missing_ok=True)
                result = self.run_bash("native_build_deps_install 1 1")
                self.assertEqual(result.returncode, 0, result.stderr)
                commands = self.log.read_text()
                for package in (native, assets, "flatpak-builder", "desktop-file-utils", yes):
                    self.assertIn(package, commands)
                # Existing toolchains must not be replaced with distro versions.
                self.assertNotIn(" cargo", commands)
                self.assertNotIn(" nodejs", commands)

    def test_interactive_native_install_omits_packaging(self):
        result = self.run_bash("native_build_deps_install 0 0")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.log.read_text()
        self.assertIn("gtk3-devel", commands)
        self.assertNotIn("flatpak", commands)
        self.assertNotIn("-y", commands)

    def test_apt_update_failure_stops_install(self):
        self.release.write_text("ID=debian\n")
        self.stub("apt-get", 'printf "%s\\n" "$*" >> "$DEPS_TEST_LOG"; exit 1')
        result = self.run_bash("native_build_deps_install 1 1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.log.read_text().strip(), "update")

    def test_missing_libraries_reported_together(self):
        self.stub("pkg-config", 'case "$*" in *cairo*|*glib-2.0*) exit 1;; esac')
        result = self.run_bash("native_build_deps_check 1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("library: cairo", result.stderr)
        self.assertIn("library: glib-2.0", result.stderr)
        self.assertIn("PKG_CONFIG_PATH", result.stderr)

    def test_check_only_never_installs(self):
        result = subprocess.run([str(ROOT / "scripts/install-native-build-deps.sh"),
                                 "--check", "--flatpak"], env=self.env,
                                text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.log.exists())

    def test_install_failure_stops_before_success(self):
        self.stub("dnf", "exit 19")
        result = subprocess.run([str(ROOT / "scripts/install-native-build-deps.sh"), "--yes"],
                                env=self.env, text=True, capture_output=True)
        self.assertEqual(result.returncode, 19)
        self.assertNotIn("are ready", result.stdout)

    def test_flatpak_setup_installs_native_dependencies_before_build(self):
        repo = self.root / "repo"
        (repo / "scripts/lib").mkdir(parents=True)
        for name in ("build-flatpak.sh", "install-native-build-deps.sh"):
            shutil.copy2(ROOT / "scripts" / name, repo / "scripts" / name)
        for name in ("native-build-deps.sh", "build-paths.sh"):
            shutil.copy2(ROOT / "scripts/lib" / name, repo / "scripts/lib" / name)
        build = repo / "scripts/build-release-binary.sh"
        build.write_text('#!/usr/bin/env bash\nprintf "binary-build\\n" >> "$DEPS_TEST_LOG"\nexit 23\n')
        build.chmod(0o755)
        packaging = repo / "packaging/flatpak"
        packaging.mkdir(parents=True)
        for suffix in ("yml", "desktop", "metainfo.xml"):
            (packaging / f"dev.crosshook.CrossHook.{suffix}").touch()
        self.stub("flatpak", 'printf "flatpak %s\\n" "$*" >> "$DEPS_TEST_LOG"')
        env = dict(self.env, DIST_DIR=str(self.root / "dist"),
                   CARGO_TARGET_DIR=str(self.root / "target"))
        command = json.loads((ROOT / "package.json").read_text())["scripts"]["flatpak:setup"].split()
        result = subprocess.run([str(repo / command[0]), *command[1:], "--yes"],
                                env=env, text=True, capture_output=True)
        self.assertEqual(result.returncode, 23, result.stderr)
        commands = self.log.read_text()
        self.assertIn("webkit2gtk4.1-devel", commands)
        self.assertIn("flatpak-builder", commands)
        self.assertIn("--arch=x86_64", commands)
        self.assertLess(commands.index("dnf"), commands.index("flatpak install"))
        self.assertLess(commands.index("flatpak install"), commands.index("binary-build"))


if __name__ == "__main__":
    unittest.main()
