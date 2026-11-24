"""Unit tests for Flatpak sandbox/subsandbox cleanup (stdlib only; uses faked flatpak ps listings)."""
from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_capture
import bench_flatpak
import bench_flatpak_io
from bench_capture import CaptureError
from bench_flatpak import (
    FlatpakInstance,
    FlatpakSandboxScope,
    InstanceInfo,
    ProcInfo,
    extract_app_id,
    is_valid_instance_id,
    kill_instance,
    list_instances,
    parse_proc_stat,
    parse_ps,
    read_instance_info,
    select_instances_to_kill,
)
from bench_guard import Originals

APP_ID = "dev.crosshook.CrossHook"
UID = 1000


class ParseTests(unittest.TestCase):
    def test_is_valid_instance_id(self):
        self.assertTrue(is_valid_instance_id("2207604640"))
        self.assertTrue(is_valid_instance_id("1"))
        for bad in ("", APP_ID, "-1", "--help", "123abc", "123\n456", "²"):
            with self.subTest(bad=bad):
                self.assertFalse(is_valid_instance_id(bad))

    def test_parse_ps_parses_valid_rows_and_skips_malformed_or_non_numeric(self):
        listing = "\n".join([
            "Instance\tPID\tChild PID\tApplication",
            "2207604640\t753359\t753366\tdev.crosshook.CrossHook",
            "785233123\t753289\t-\tdev.crosshook.CrossHook",
            "111222333\t500\t0\tcom.plexamp.Plexamp",
            "dev.crosshook.CrossHook\t900\t901\tdev.crosshook.CrossHook",
            "999999999\t0\t12\tdev.crosshook.CrossHook",
            "888888888\tnotpid\t12\tdev.crosshook.CrossHook",
            "only-two\tfields",
            "",
        ])
        parsed = parse_ps(listing)
        self.assertEqual(parsed, [
            FlatpakInstance("2207604640", 753359, 753366, APP_ID),
            FlatpakInstance("785233123", 753289, None, APP_ID),
            FlatpakInstance("111222333", 500, None, "com.plexamp.Plexamp"),
        ])

    def test_parse_proc_stat_handles_spaces_and_nested_parens_in_comm(self):
        tail = " ".join(["S", "753354"] + ["0"] * 17 + ["2235517", "0"])
        self.assertEqual(parse_proc_stat(f"753359 (glycin-svg (1) loader) {tail}"), (753354, 2235517))
        with self.assertRaises(ValueError):
            parse_proc_stat("malformed without parens")
        with self.assertRaises(ValueError):
            parse_proc_stat("10 (x) S -1 " + "0 " * 17 + "100")

    def test_extract_app_id(self):
        cmd = ["flatpak", "run", "--branch=master", "--env=FOO=1", APP_ID, "--arg"]
        self.assertEqual(extract_app_id(cmd), APP_ID)
        self.assertEqual(
            extract_app_id(["flatpak", "run", "--branch", "master", "--command", "sh", f"{APP_ID}//master"]),
            APP_ID,
        )
        self.assertEqual(extract_app_id(["flatpak", "run", "--", APP_ID]), APP_ID)
        self.assertEqual(extract_app_id(["flatpak", "run", "--user", "-p", APP_ID]), APP_ID)
        self.assertEqual(extract_app_id(["flatpak", "run"]), APP_ID)

    def test_now_boot_ticks_uses_integer_floor(self):
        with patch.object(bench_flatpak_io.time, "clock_gettime_ns", return_value=86400_009_999_999), \
                patch.object(bench_flatpak_io.os, "sysconf", return_value=100):
            self.assertEqual(bench_flatpak.now_boot_ticks(), 8640000)

    def test_path_within_rejects_relative_paths(self):
        self.assertFalse(bench_flatpak._path_within("relative/path", Path("/tmp")))

    def test_read_instance_info_reads_valid_file_and_rejects_symlinks_or_oversize(self):
        with tempfile.TemporaryDirectory() as tmp:
            rt = Path(tmp)
            inst_dir = rt / ".flatpak" / "2207604640"
            inst_dir.mkdir(parents=True)
            (inst_dir / "info").write_text(
                "[Application]\nname=dev.crosshook.CrossHook\n"
                "[Instance]\ninstance-id=2207604640\nsandbox=true\n"
            )
            info = read_instance_info(rt, "2207604640", os.getuid())
            self.assertEqual(info, InstanceInfo(instance_path=None, is_subsandbox=True))

            main_dir = rt / ".flatpak" / "785233123"
            main_dir.mkdir(parents=True)
            (main_dir / "info").write_text(
                "[Instance]\ninstance-id=785233123\n"
                f"instance-path={tmp}/bench/home/.var/app/{APP_ID}\n"
            )
            main_info = read_instance_info(rt, "785233123", os.getuid())
            self.assertEqual(
                main_info,
                InstanceInfo(instance_path=f"{tmp}/bench/home/.var/app/{APP_ID}", is_subsandbox=False),
            )

            sym_dir = rt / ".flatpak" / "333333333"
            sym_dir.mkdir(parents=True)
            (sym_dir / "info").symlink_to(main_dir / "info")
            self.assertIsNone(read_instance_info(rt, "333333333", os.getuid()))

            big_dir = rt / ".flatpak" / "444444444"
            big_dir.mkdir(parents=True)
            (big_dir / "info").write_text("x" * (bench_flatpak.INFO_MAX_BYTES + 10))
            self.assertIsNone(read_instance_info(rt, "444444444", os.getuid()))

            self.assertIsNone(read_instance_info(rt, "999999999", os.getuid()))
            self.assertIsNone(read_instance_info(rt, "../escape", os.getuid()))


class SelectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "crosshook-bench-test"
        self.root.mkdir()
        self.isolated_app = self.root / "home/.var/app" / APP_ID
        self.isolated_app.mkdir(parents=True)
        self.user_app = Path(self.temp.name) / "real-home/.var/app" / APP_ID
        self.user_app.mkdir(parents=True)

    def _run_select(
        self,
        ps_text: str,
        procs: dict[int, ProcInfo],
        *,
        baseline: frozenset[str] = frozenset(),
        baseline_known: bool = True,
        launch_pid: int | None = 5000,
        launch_start_ticks: int = 10_000,
        tracked_tree_pids: frozenset[int] = frozenset(),
        inst_infos: dict[str, InstanceInfo] | None = None,
    ) -> list[str]:
        instances = parse_ps(ps_text)
        infos = inst_infos or {}
        selected = select_instances_to_kill(
            instances,
            app_id=APP_ID,
            baseline_instances=baseline,
            baseline_known=baseline_known,
            launch_pid=launch_pid,
            launch_start_ticks=launch_start_ticks,
            tracked_tree_pids=tracked_tree_pids,
            isolated_root=self.root,
            uid=UID,
            proc_info_fn=lambda pid: procs.get(pid),
            instance_info_fn=lambda iid: infos.get(iid),
            subsandbox_in_root_fn=lambda *_a: False,
        )
        return [inst.instance for inst in selected]

    def test_selects_portal_subsandbox_and_surviving_main_when_no_foreign_instance(self):
        ps_text = "\n".join([
            f"1002\t2101\t2102\t{APP_ID}",        # started before launch_start_ticks (not in baseline)
            f"1003\t2201\t2202\tcom.plexamp.Plexamp",  # unrelated Flatpak app started after launch
            f"1004\t2301\t2302\t{APP_ID}",        # foreign UID instance started after launch
            f"1006\t2501\t2502\t{APP_ID}",        # surviving bench main sandbox (ancestry + isolated path)
            f"1007\t2601\t2602\t{APP_ID}",        # reparented glycin-svg portal subsandbox (ppid=1, sandbox=true)
        ])
        procs = {
            5000: ProcInfo(5000, 4000, 10_000, UID),
            2101: ProcInfo(2101, 1, 9_999, UID),      # started before launch
            2201: ProcInfo(2201, 1, 10_100, UID),     # wrong app_id
            2301: ProcInfo(2301, 1, 10_100, UID + 1), # foreign UID
            2501: ProcInfo(2501, 5000, 10_010, UID),  # child of launch_pid
            2601: ProcInfo(2601, 1800, 10_050, UID),  # reparented to flatpak-portal / systemd --user
            1800: ProcInfo(1800, 1, 500, UID),
        }
        infos = {
            "1006": InstanceInfo(str(self.isolated_app), is_subsandbox=False),
            "1007": InstanceInfo(None, is_subsandbox=True),
        }
        chosen = self._run_select(ps_text, procs, baseline=frozenset(), inst_infos=infos)
        self.assertEqual(chosen, ["1006", "1007"])

    def test_fails_closed_on_unattributed_subsandbox_when_baseline_or_concurrent_user_exists(self):
        ps_text = "\n".join([
            f"1001\t2001\t2002\t{APP_ID}",  # pre-existing user CrossHook in baseline
            f"1005\t2401\t2402\t{APP_ID}",  # concurrent real user CrossHook (outside isolated_root)
            f"1006\t2501\t2502\t{APP_ID}",  # bench main sandbox (ancestry + isolated path)
            f"1007\t2601\t2602\t{APP_ID}",  # unattributed portal subsandbox spawned during run -> must spare
            f"1008\t2701\t2702\t{APP_ID}",  # portal subsandbox proved via isolated child env
        ])
        procs = {
            5000: ProcInfo(5000, 4000, 10_000, UID),
            2001: ProcInfo(2001, 1, 500, UID),
            2401: ProcInfo(2401, 1, 10_100, UID),
            2501: ProcInfo(2501, 5000, 10_010, UID),
            2601: ProcInfo(2601, 1800, 10_050, UID),
            2701: ProcInfo(2701, 1800, 10_060, UID),
        }
        infos = {
            "1005": InstanceInfo(str(self.user_app), is_subsandbox=False),
            "1006": InstanceInfo(str(self.isolated_app), is_subsandbox=False),
            "1007": InstanceInfo(None, is_subsandbox=True),
            "1008": InstanceInfo(None, is_subsandbox=True),
        }
        # Both pre-launch baseline and concurrent non-isolated instance fail closed on 1007, while 1006 + 1008 match.
        for baseline in (frozenset({"1001"}), frozenset()):
            selected = select_instances_to_kill(
                parse_ps(ps_text),
                app_id=APP_ID,
                baseline_instances=baseline,
                baseline_known=True,
                launch_pid=5000,
                launch_start_ticks=10_000,
                isolated_root=self.root,
                uid=UID,
                proc_info_fn=lambda pid: procs.get(pid),
                instance_info_fn=lambda iid: infos.get(iid),
                subsandbox_in_root_fn=lambda inst, *_a: inst.instance == "1008",
            )
            self.assertEqual([inst.instance for inst in selected], ["1006", "1008"])

    def test_subsandbox_env_in_root_checks_isolated_keys_uid_and_pid_reuse(self):
        inst = FlatpakInstance("1008", 900, 901, APP_ID)
        procs = {
            901: ProcInfo(901, 1, 10_010, UID),
            902: ProcInfo(902, 901, 10_020, UID),
        }
        iso_env = b"PATH=/usr/bin\x00FLATPAK_SANDBOX_DIR=" + os.fsencode(str(self.isolated_app / "sandbox"))
        with patch.object(bench_flatpak, "_read_direct_children", return_value=[902]), \
                patch.object(bench_flatpak, "_read_small_bytes", side_effect=[None, iso_env]):
            self.assertTrue(
                bench_flatpak._subsandbox_env_in_root(inst, self.root, UID, 10_000, lambda p: procs.get(p))
            )
        # Foreign env path or UID mismatch must fail closed.
        foreign_env = b"HOME=" + os.fsencode(str(self.user_app))
        with patch.object(bench_flatpak, "_read_direct_children", return_value=[902]), \
                patch.object(bench_flatpak, "_read_small_bytes", side_effect=[None, foreign_env]):
            self.assertFalse(
                bench_flatpak._subsandbox_env_in_root(inst, self.root, UID, 10_000, lambda p: procs.get(p))
            )
        # PID reuse between before/after proc reads fails closed.
        seq_902 = iter([ProcInfo(902, 901, 10_020, UID), ProcInfo(902, 901, 99_999, UID)])
        with patch.object(bench_flatpak, "_read_direct_children", return_value=[902]), \
                patch.object(bench_flatpak, "_read_small_bytes", side_effect=[None, iso_env]):
            self.assertFalse(
                bench_flatpak._subsandbox_env_in_root(
                    inst,
                    self.root,
                    UID,
                    10_000,
                    lambda p: procs.get(p) if p != 902 else next(seq_902),
                )
            )

    def test_tracked_tree_pids_and_isolated_path_match_when_baseline_unknown(self):
        ps_text = "\n".join([
            f"2001\t3101\t3102\t{APP_ID}",  # in tracked_tree_pids, launch_pid reaped
            f"2002\t3201\t3202\t{APP_ID}",  # isolated instance_path proves ownership even if baseline_known=False
            f"2003\t3301\t3302\t{APP_ID}",  # reparented subsandbox without baseline_known -> fail closed
        ])
        procs = {
            3101: ProcInfo(3101, 1, 10_010, UID),
            3201: ProcInfo(3201, 1, 10_020, UID),
            3301: ProcInfo(3301, 1, 10_030, UID),
        }
        infos = {
            "2002": InstanceInfo(str(self.isolated_app), is_subsandbox=False),
            "2003": InstanceInfo(None, is_subsandbox=True),
        }
        chosen = self._run_select(
            ps_text,
            procs,
            baseline_known=False,
            launch_pid=5000,  # already reaped
            tracked_tree_pids=frozenset({3101}),
            inst_infos=infos,
        )
        self.assertEqual(chosen, ["2001", "2002"])

    def test_ancestry_guards_against_pid_reuse_and_ppid_cycles(self):
        ps_text = "\n".join([
            f"3001\t6001\t-\t{APP_ID}",  # ppid chain hits reused launch_pid with wrong start_time
            f"3002\t7001\t-\t{APP_ID}",  # cyclic ppid chain
        ])
        procs = {
            5000: ProcInfo(5000, 1, 99_999, UID),  # PID 5000 reused by newer unrelated process
            6001: ProcInfo(6001, 5000, 10_020, UID),
            7001: ProcInfo(7001, 7002, 10_020, UID),
            7002: ProcInfo(7002, 7001, 9_000, UID),
        }
        chosen = self._run_select(
            ps_text,
            procs,
            baseline_known=False,
            launch_pid=5000,
            launch_start_ticks=10_000,
        )
        self.assertEqual(chosen, [])

    def test_falls_back_to_child_pid_when_wrapper_pid_already_exited(self):
        ps_text = f"4001\t8001\t8002\t{APP_ID}\n"
        procs = {8002: ProcInfo(8002, 1, 10_050, UID)}
        chosen = self._run_select(ps_text, procs, baseline_known=True, launch_start_ticks=10_000)
        self.assertEqual(chosen, ["4001"])


class SubprocessAndScopeTests(unittest.TestCase):
    def test_kill_instance_refuses_non_numeric_ids_without_spawning(self):
        with patch.object(bench_flatpak_io, "run_cancellable") as runner:
            self.assertFalse(kill_instance(APP_ID))
            self.assertFalse(kill_instance("--help"))
            self.assertFalse(kill_instance(""))
        runner.assert_not_called()

    def test_list_instances_handles_missing_flatpak_and_scrubs_loader_env(self):
        with patch.object(bench_flatpak_io, "run_cancellable", side_effect=FileNotFoundError("flatpak")):
            self.assertIsNone(list_instances({"XDG_RUNTIME_DIR": "/tmp/rt"}))

        seen_env: dict[str, str] = {}

        def fake_run(cmd, timeout, cancel, *, stdout_limit=None, step=0.5, env=None):
            seen_env.update(env or {})
            return 0, f"55555\t101\t102\t{APP_ID}\n"

        with patch.dict(os.environ, {"LD_PRELOAD": "/bad/shim.so"}, clear=False), \
                patch.object(bench_flatpak_io, "run_cancellable", side_effect=fake_run):
            rows = list_instances({"XDG_RUNTIME_DIR": "/tmp/custom-rt", "LD_PRELOAD": "/other.so"})
        self.assertEqual(rows, [FlatpakInstance("55555", 101, 102, APP_ID)])
        self.assertNotIn("LD_PRELOAD", seen_env)
        self.assertEqual(seen_env["XDG_RUNTIME_DIR"], "/tmp/custom-rt")

    def test_scope_cleanup_polls_until_flatpak_ps_clears_and_rekills_after_interval(self):
        baseline_rows = [FlatpakInstance("1001", 2001, 2002, APP_ID)]
        main_draining = FlatpakInstance("3085988570", 763216, 763230, APP_ID)
        subsandbox = FlatpakInstance("2207604640", 753359, 753366, APP_ID)
        # Simulate flatpak ps still listing both: main sandbox already exited (proc_info=None)
        # and subsandbox surviving kill until the 5th snapshot, while baseline stays untouched.
        snapshots = iter([
            baseline_rows,
            [*baseline_rows, main_draining, subsandbox],
            [*baseline_rows, main_draining, subsandbox],
            [*baseline_rows, subsandbox],
            [*baseline_rows, subsandbox],  # even after subsandbox pid exits, wait until flatpak ps drops it
            baseline_rows,
        ])
        clock = iter([0.0, 0.0, 0.2, 1.3, 1.4, 1.6])
        killed: list[str] = []
        subsandbox_alive = True

        def proc_lookup(pid: int) -> ProcInfo | None:
            if pid == 2001:
                return ProcInfo(2001, 1, 500, UID)
            if pid == 753359 and subsandbox_alive:
                return ProcInfo(753359, 1, 10_050, UID)
            return None

        def do_kill(iid: str) -> bool:
            nonlocal subsandbox_alive
            killed.append(iid)
            if len(killed) == 2:
                subsandbox_alive = False
            return True

        scope = FlatpakSandboxScope.capture_before_launch(
            APP_ID,
            {},
            isolated_root=Path("/tmp/bench-isolated"),
            uid=UID,
            list_fn=lambda: next(snapshots),
            kill_fn=do_kill,
            proc_info_fn=proc_lookup,
            instance_info_fn=lambda iid: InstanceInfo(None, is_subsandbox=True),
            subsandbox_in_root_fn=lambda inst, *_a: inst.instance == "2207604640",
            clock_ticks_fn=lambda: 10_000,
            sleep_fn=lambda _dt: None,
            monotonic_fn=lambda: next(clock),
        )
        scope.launch_pid = 763216
        targeted = scope.cleanup(wait_timeout=5.0, poll_step=0.1, rekill_interval=1.0)
        self.assertEqual(targeted, ["2207604640"])
        self.assertEqual(killed, ["2207604640", "2207604640"])

    def test_scope_cleanup_warns_without_raising_if_instance_lingers(self):
        subsandbox = FlatpakInstance("2207604640", 753359, 753366, APP_ID)
        clock = iter([0.0, 0.0, 0.5, 0.6])
        procs = {753359: ProcInfo(753359, 1, 10_050, UID)}
        scope = FlatpakSandboxScope(
            app_id=APP_ID,
            launch_env={},
            isolated_root=None,
            baseline_instances=frozenset(),
            baseline_known=True,
            launch_start_ticks=10_000,
            uid=UID,
            list_fn=lambda: [subsandbox],
            kill_fn=lambda _iid: True,
            proc_info_fn=lambda pid: procs.get(pid),
            instance_info_fn=lambda _iid: None,
            sleep_fn=lambda _dt: None,
            monotonic_fn=lambda: next(clock),
        )
        with patch("sys.stderr") as stderr:
            targeted = scope.cleanup(wait_timeout=0.5, poll_step=0.1, rekill_interval=1.0)
        self.assertEqual(targeted, ["2207604640"])
        written = "".join(call.args[0] for call in stderr.write.call_args_list)
        self.assertIn("FLATPAK_CLEANUP_INCOMPLETE: remaining instances: 2207604640", written)

    def test_scope_cleanup_retries_and_warns_when_flatpak_ps_fails_mid_cleanup(self):
        subsandbox = FlatpakInstance("2207604640", 753359, 753366, APP_ID)
        snapshots = iter([[subsandbox], None, None])
        clock = iter([0.0, 0.0, 0.2, 0.6])
        procs = {753359: ProcInfo(753359, 1, 10_050, UID)}
        scope = FlatpakSandboxScope(
            app_id=APP_ID,
            launch_env={},
            isolated_root=None,
            baseline_instances=frozenset(),
            baseline_known=True,
            launch_start_ticks=10_000,
            uid=UID,
            list_fn=lambda: next(snapshots),
            kill_fn=lambda _iid: True,
            proc_info_fn=lambda pid: procs.get(pid),
            instance_info_fn=lambda _iid: None,
            sleep_fn=lambda _dt: None,
            monotonic_fn=lambda: next(clock),
        )
        with patch("sys.stderr") as stderr:
            targeted = scope.cleanup(wait_timeout=0.5, poll_step=0.1, rekill_interval=1.0)
        self.assertEqual(targeted, ["2207604640"])
        written = "".join(call.args[0] for call in stderr.write.call_args_list)
        self.assertIn("FLATPAK_CLEANUP_INCOMPLETE: remaining instances: 2207604640", written)

    def test_scope_observe_live_retains_draining_ownership_for_pre_cleanup_exits_with_foreign_baseline(self):
        baseline_rows = [FlatpakInstance("1001", 2001, 2002, APP_ID)]
        owned_sub = FlatpakInstance("2207604640", 753359, 753366, APP_ID)
        foreign_dead = FlatpakInstance("3333333333", 853359, None, APP_ID)
        snapshots = iter([
            baseline_rows,                                     # baseline capture
            [*baseline_rows, owned_sub],                       # observe_live while running
            [*baseline_rows, owned_sub, foreign_dead],         # cleanup poll 1: owned_sub exited before cleanup
            [*baseline_rows, foreign_dead],                    # cleanup poll 2: owned_sub cleared, foreign_dead ignored
        ])
        clock = iter([0.0, 0.0, 0.2])
        sub_alive = True
        killed: list[str] = []

        def proc_lookup(pid: int) -> ProcInfo | None:
            if pid == 2001:
                return ProcInfo(2001, 1, 500, UID)
            if pid == 753359 and sub_alive:
                return ProcInfo(753359, 1, 10_050, UID)
            return None

        scope = FlatpakSandboxScope.capture_before_launch(
            APP_ID,
            {},
            isolated_root=Path("/tmp/bench-isolated"),
            uid=UID,
            list_fn=lambda: next(snapshots),
            kill_fn=lambda iid: (killed.append(iid), True)[1],
            proc_info_fn=proc_lookup,
            instance_info_fn=lambda _iid: InstanceInfo(None, is_subsandbox=True),
            subsandbox_in_root_fn=lambda inst, *_a: inst.instance == "2207604640",
            clock_ticks_fn=lambda: 10_000,
            sleep_fn=lambda _dt: None,
            monotonic_fn=lambda: next(clock),
        )
        observed = scope.observe_live()
        self.assertEqual(observed, frozenset({"2207604640"}))
        sub_alive = False  # subsandbox exited before cleanup() starts; row lingers as draining
        targeted = scope.cleanup(wait_timeout=5.0, poll_step=0.1, rekill_interval=1.0)
        self.assertEqual(targeted, [])
        self.assertEqual(killed, [])

    def test_scope_cleanup_waits_for_exited_draining_subsandbox_when_no_foreign_instance(self):
        exited_sub = FlatpakInstance("2207604640", 753359, 753366, APP_ID)
        snapshots = iter([[], [exited_sub], []])
        clock = iter([0.0, 0.0, 0.2])
        killed: list[str] = []
        scope = FlatpakSandboxScope.capture_before_launch(
            APP_ID,
            {},
            isolated_root=Path("/tmp/bench-isolated"),
            uid=UID,
            list_fn=lambda: next(snapshots),
            kill_fn=lambda iid: (killed.append(iid), True)[1],
            proc_info_fn=lambda _pid: None,
            instance_info_fn=lambda _iid: InstanceInfo(None, is_subsandbox=True),
            subsandbox_in_root_fn=lambda *_a: False,
            clock_ticks_fn=lambda: 10_000,
            sleep_fn=lambda _dt: None,
            monotonic_fn=lambda: next(clock),
        )
        targeted = scope.cleanup(wait_timeout=5.0, poll_step=0.1, rekill_interval=1.0)
        self.assertEqual(targeted, [])
        self.assertEqual(killed, [])


class LifecycleIntegrationTests(unittest.TestCase):
    def test_ready_and_stop_trigger_flatpak_cleanup_on_success_and_failure(self):
        class FakeScope:
            def __init__(self):
                self.bound: list[int] = []
                self.observed: list[frozenset[int]] = []
                self.cleanups: list[frozenset[int]] = []

            def bind_launch(self, pid: int) -> None:
                self.bound.append(pid)

            def observe_live(self, *, tree_pids=()) -> frozenset[str]:
                self.observed.append(frozenset(tree_pids))
                return frozenset()

            def cleanup(self, *, tree_pids=(), **_kwargs):
                self.cleanups.append(frozenset(tree_pids))
                return []

        code_ok = "import time,sys;print('READY',time.monotonic_ns(),file=sys.stderr,flush=True);time.sleep(30)"
        scope_ok = FakeScope()
        flatpak_cmd = ["flatpak", "run", "--branch=master", APP_ID]
        real_popen = bench_capture.subprocess.Popen

        def fake_popen(cmd, env=None, **_kwargs):
            # Substitute an actual Python process while keeping command[0] == "flatpak" for ready().
            return real_popen(
                [sys.executable, "-c", code_ok],
                env=env,
                start_new_session=True,
                stderr=bench_capture.subprocess.PIPE,
                stdout=bench_capture.subprocess.DEVNULL,
            )

        with patch.object(bench_capture, "session_locked", return_value=False), \
                patch.object(bench_capture.FlatpakSandboxScope, "capture_before_launch", return_value=scope_ok) as cap, \
                patch.object(bench_capture.subprocess, "Popen", side_effect=fake_popen):
            proc, _ = bench_capture.ready(flatpak_cmd, {"HOME": "/tmp/crosshook-bench-x/home"}, 5, False)
            try:
                self.assertEqual(scope_ok.bound, [proc.pid])
            finally:
                bench_capture.stop(proc)
                proc.stderr.close()
        cap.assert_called_once()
        self.assertEqual(len(scope_ok.observed), 1)
        self.assertIn(proc.pid, scope_ok.observed[0])
        self.assertEqual(len(scope_ok.cleanups), 1)
        self.assertIn(proc.pid, scope_ok.cleanups[0])

        # Second stop is a no-op for the scope.
        bench_capture.stop(proc)
        self.assertEqual(len(scope_ok.cleanups), 1)

        # Failure inside ready() (NO_READY) still runs cleanup via stop(proc).
        scope_fail = FakeScope()

        def fail_popen(cmd, env=None, **_kwargs):
            return real_popen(
                [sys.executable, "-c", "pass"],
                env=env,
                start_new_session=True,
                stderr=bench_capture.subprocess.PIPE,
                stdout=bench_capture.subprocess.DEVNULL,
            )

        with patch.object(bench_capture, "session_locked", return_value=False), \
                patch.object(bench_capture.FlatpakSandboxScope, "capture_before_launch", return_value=scope_fail), \
                patch.object(bench_capture.subprocess, "Popen", side_effect=fail_popen), \
                self.assertRaises(CaptureError):
            bench_capture.ready(flatpak_cmd, {}, 2, False)
        self.assertEqual(len(scope_fail.cleanups), 1)

        # exit_after=True (G1/G2 CSV mode): process has exited before stop(proc), tree_pids is empty,
        # and cleanup still runs.
        scope_exit = FakeScope()

        def exit_popen(cmd, env=None, **_kwargs):
            return real_popen(
                [sys.executable, "-c", "import time,sys;print('READY',time.monotonic_ns(),file=sys.stderr)"],
                env=env,
                start_new_session=True,
                stderr=bench_capture.subprocess.PIPE,
                stdout=bench_capture.subprocess.DEVNULL,
            )

        with patch.object(bench_capture, "session_locked", return_value=False), \
                patch.object(bench_capture.FlatpakSandboxScope, "capture_before_launch", return_value=scope_exit), \
                patch.object(bench_capture.subprocess, "Popen", side_effect=exit_popen):
            eproc, _ = bench_capture.ready(flatpak_cmd, {}, 5, True)
            try:
                self.assertEqual(scope_exit.bound, [eproc.pid])
            finally:
                bench_capture.stop(eproc)
                eproc.stderr.close()
        self.assertEqual(scope_exit.cleanups, [frozenset()])

        # Aborting during bind_launch() inside ready() still triggers stop(proc) and closes pipes.
        class AbortOnBindScope(FakeScope):
            def bind_launch(self, pid: int) -> None:
                super().bind_launch(pid)
                raise bench.BenchAborted("SIGTERM")

        scope_abort = AbortOnBindScope()
        with patch.object(bench_capture, "session_locked", return_value=False), \
                patch.object(bench_capture.FlatpakSandboxScope, "capture_before_launch", return_value=scope_abort), \
                patch.object(bench_capture.subprocess, "Popen", side_effect=fake_popen), \
                self.assertRaises(bench.BenchAborted):
            bench_capture.ready(flatpak_cmd, {}, 5, False)
        self.assertEqual(len(scope_abort.cleanups), 1)

        # stop() isolates cleanup failures so termination and caller pipe close never abort.
        class ExplodingScope(FakeScope):
            def cleanup(self, *, tree_pids=(), **_kwargs):
                raise RuntimeError("boom")

        scope_boom = ExplodingScope()
        with patch.object(bench_capture, "session_locked", return_value=False), \
                patch.object(bench_capture.FlatpakSandboxScope, "capture_before_launch", return_value=scope_boom), \
                patch.object(bench_capture.subprocess, "Popen", side_effect=fake_popen), \
                patch("sys.stderr") as stderr:
            bproc, _ = bench_capture.ready(flatpak_cmd, {}, 5, False)
            bench_capture.stop(bproc)
            bproc.stderr.close()
        self.assertIsNotNone(bproc.poll())
        written = "".join(call.args[0] for call in stderr.write.call_args_list)
        self.assertIn("FLATPAK_CLEANUP_FAILED: boom", written)

        # Native launches never invoke FlatpakSandboxScope.
        with patch.object(bench_capture, "session_locked", return_value=False), \
                patch.object(bench_capture.FlatpakSandboxScope, "capture_before_launch") as native_cap:
            nproc, _ = bench_capture.ready(
                [sys.executable, "-c", "import time,sys;print('READY',time.monotonic_ns(),file=sys.stderr)"],
                dict(os.environ),
                5,
                True,
            )
            bench_capture.stop(nproc)
            nproc.stderr.close()
        native_cap.assert_not_called()

    def test_bench_main_runs_flatpak_cleanup_on_capture_and_failure_only_when_flatpak(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            original = Originals("/nonexistent-home", "/nonexistent-home", {}, None)

            for outcome, expect_code in (("ok", 0), ("fail", 1), ("abort", 130)):
                calls: list[str] = []

                class RunScope:
                    def cleanup(self):
                        calls.append("cleanup")
                        return []

                def fake_capture(*_a, **_k):
                    if outcome == "fail":
                        raise CaptureError("NO_READY: failed")
                    if outcome == "abort":
                        raise bench.BenchAborted("SIGTERM")
                    return [{"iteration": 1, "ready_ms": 10.0, "status": "measured"}]

                out_dir = root / f"out-{outcome}"
                with self.subTest(outcome=outcome), \
                        patch.object(bench.Originals, "capture", return_value=original), \
                        patch.object(bench.FlatpakSandboxScope, "capture_before_launch", return_value=RunScope()) as cap, \
                        patch.object(bench, "capture", side_effect=fake_capture):
                    code = bench.main([
                        "--variant", "B", "--env", "E1", "--flatpak", "--metric", "G2",
                        "--output", str(out_dir), "--tmp-root", str(root / "runs"),
                    ])
                self.assertEqual(code, expect_code)
                cap.assert_called_once()
                self.assertEqual(calls, ["cleanup"])

            # --dry-run never touches FlatpakSandboxScope.
            with patch.object(bench.Originals, "capture", return_value=original), \
                    patch.object(bench.FlatpakSandboxScope, "capture_before_launch") as dry_cap:
                code = bench.main([
                    "--variant", "B", "--env", "E1", "--flatpak", "--dry-run", "--metric", "G2",
                    "--output", str(root / "out-dry"), "--tmp-root", str(root / "runs"),
                ])
            self.assertEqual(code, 0)
            dry_cap.assert_not_called()


if __name__ == "__main__":
    unittest.main()
