"""G8 collection: launch env, session binding, schema, observations and provenance evidence; fake app and driver."""
import csv
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from typing import ClassVar
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench
import bench_capture
import bench_collect
import bench_drivers_g5
import bench_g8session
from bench_capture import CaptureError
from bench_collect import driven_rows
from bench_drivers_g8 import G8Result
from bench_gpu import GpuSample
from bench_memory import MemoryRow

BASE_NS = 1_000_000_000_000
PERIOD_NS = 8_333_333
RUN_ID = "a" * 32
PID = 4242
WINDOW_START, DONE, WINDOW_END = 5_000_000_000, 15_000_000_000, 15_300_000_000  # offsets from the launch


class FakeDevice:
    instances: ClassVar[list["FakeDevice"]] = []

    def __init__(self):
        self.closed = False
        FakeDevice.instances.append(self)

    def __enter__(self):
        return self

    def __exit__(self, *_exc):
        self.closed = True


class FakeKWin:
    def __init__(self, tools_dir, run_tag):
        self.tools_dir, self.run_tag = tools_dir, run_tag


class FakeEvents:
    closed = False

    def close(self):
        self.closed = True


class FakeStdin:
    closed = False

    def close(self):
        self.closed = True


class FakeReady:
    """Stands in for bench_capture.ready; each launch gets a later `launch_ns`."""

    def __init__(self):
        self.calls: list[dict] = []
        self.procs: list[SimpleNamespace] = []

    def __call__(self, command, env, timeout, exit_after, events=False, stdin=False):
        self.calls.append({"command": list(command), "env": dict(env), "exit_after": exit_after,
                           "events": events, "stdin": stdin})
        proc = SimpleNamespace(pid=PID, launch_ns=BASE_NS + (len(self.calls) - 1) * 100_000_000_000,
                               events=FakeEvents(), stdin=FakeStdin(),
                               stderr=SimpleNamespace(close=lambda: None))
        self.procs.append(proc)
        return proc, 10.0


def memory_rows(launch_ns: int, *, gpu: bool = True, tail: bool = True, shift_ns: int = 0) -> list[MemoryRow]:
    """30 baseline, 100 replay and 30 tail samples spanning the frame window."""
    rows = []
    for phase, first_ms, count, pss, mb in (("baseline", 1000, 30, 100.0, 500.0), ("replay", 5000, 100, 120.0, 505.0),
                                           ("settle", 15000, 10, 118.0, 505.0), ("tail", 16000, 30, 110.0, 510.0)):
        if phase == "tail" and not tail:
            continue
        for index in range(count):
            gpu_seq = (first_ms + index * 100) // 1000 + 1
            source = "fdinfo" if gpu else "unavailable:no_matching_pid"
            rows.append(MemoryRow(launch_ns + (first_ms + index * 100) * 1_000_000 + shift_ns, phase,
                                  pss + (index % 3) * 0.1, pss / 2, mb if gpu else None, source, gpu_seq,
                                  gpu_t_ns=launch_ns + (gpu_seq - 1) * 1_000_000_000 + shift_ns))
    return rows


def present_log(launch_ns: int, first_offset_ns: int = WINDOW_START - 100_000_000, refresh_ns: int = PERIOD_NS,
                pid: int = PID) -> str:
    lines = [f"H\tversion=1\tpid={pid}\tcomm=crosshook\tclock_id=1", "S\t1\t7\ttoplevel"]
    time_ns, seq = launch_ns + first_offset_ns, 1
    while time_ns <= launch_ns + WINDOW_END + 100_000_000:
        lines += [f"C\t{time_ns - 1000}\t7\t{seq}\t1", f"P\t{time_ns}\t{seq}\t{time_ns}\t{refresh_ns}\t{seq}\t0"]
        time_ns, seq = time_ns + PERIOD_NS, seq + 1
    return "\n".join(lines) + "\n"


class G8CollectTestCase(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve() / "crosshook-bench-x"
        for name in ("home", "bench-tools", "config/crosshook"):
            (self.root / name).mkdir(parents=True)
        self.env = {"HOME": str(self.root / "home"), "XDG_CONFIG_HOME": str(self.root / "config"),
                    "XDG_DATA_HOME": str(self.root / "data"), "CROSSHOOK_BENCH_ROOT": str(self.root),
                    "LD_PRELOAD": "/shim.so", "PATH": "/usr/bin"}
        self.args = SimpleNamespace(iterations=2, timeout=60, refresh_hz=120.0, flatpak=False, trials=10,
                                    branch="master", binary=None, run_id=RUN_ID, scenarios={"G8": ["replay_scroll"]})
        self.fake = FakeReady()
        self.stopped: list[object] = []
        self.drives: list[dict] = []
        self.settings_seen: list[str | None] = []
        self.drive_error: BaseException | None = None
        self.gpu = True
        self.tail = True
        self.refresh_ns = PERIOD_NS
        self.log_offset = WINDOW_START - 100_000_000
        self.row_shift_ns = 0
        self.root_pid = PID
        self.shim_pid = PID  # the `pid=` the shim wrote into its log header
        self.sampler_args: list[tuple] = []
        FakeDevice.instances = []
        self.settings = self.root / "config/crosshook/settings.toml"
        self.sample = lambda pids: GpuSample(None, "unavailable", "no_matching_pid", 0, "/usr/bin/nvidia-smi")

        def drive(proc, present_dir, kwin, dev, events, sampler, mark_timeout, flatpak):
            self.drives.append({"proc": proc, "dir": present_dir, "kwin": kwin, "dev": dev, "events": events,
                                "sampler": sampler, "mark_timeout": mark_timeout, "flatpak": flatpak})
            self.settings_seen.append(self.settings.read_text() if self.settings.exists() else None)
            sampler.pids_fn()
            sampler.gpu_fn({PID})
            if self.drive_error is not None:
                raise self.drive_error
            (present_dir / "wlpresent-1.log").write_text(
                present_log(proc.launch_ns, self.log_offset, self.refresh_ns, self.shim_pid))
            rows = memory_rows(proc.launch_ns, gpu=self.gpu, tail=self.tail, shift_ns=self.row_shift_ns)
            return G8Result(proc.launch_ns + WINDOW_START, proc.launch_ns + DONE, proc.launch_ns + DONE + 1_200_000_000,
                            "on", rows, 300, 9_900_000_000, proc.launch_ns + WINDOW_START,
                            proc.launch_ns + WINDOW_END)

        class FakeSampler:
            def __init__(inner, pids_fn, gpu_fn):
                inner.pids_fn, inner.gpu_fn = pids_fn, gpu_fn
                self.sampler_args.append((pids_fn, gpu_fn))

        def recorder(pid, flatpak):
            return bench_g8session.PidRecorder(
                self.root_pid, flatpak, pids_fn=lambda: {pid, pid + 1},
                read=lambda p, name: (f"Name:\t{'WebKitWebProces' if p != pid else 'crosshook'}\n"
                                      f"NSpid:\t{p}\t{p - 4000}\n" if name == "status"
                                      else ("/usr/lib/WebKitWebProcess\0" if p != pid else "/usr/bin/crosshook\0")))

        for target, replacement in (
                ("ready", self.fake), ("stop", self.stopped.append), ("InputDevice", FakeDevice),
                ("KWinScripts", FakeKWin), ("drive_g8", drive), ("settle_device", lambda dev: None),
                ("require_kwin", lambda: None), ("require_no_instance", lambda: None),
                ("MemorySampler", FakeSampler), ("PidRecorder", recorder),
                ("GpuProbe", lambda: bench_g8session.GpuProbe(lambda pids: self.sample(pids)))):
            patcher = patch.object(bench_collect, target, replacement)
            patcher.start()
            self.addCleanup(patcher.stop)

    def rows(self, command_for=None, evidence=None):
        return driven_rows(self.args, "G8", command_for or (lambda env: ["app"]), self.env, evidence)

    def reset_launch_state(self):
        """Forget the first run's present directory so the same iteration can launch again."""
        shutil.rmtree(self.root / "present", ignore_errors=True)

    def observations(self, suffix):
        path = self.root / f"observations/G8-replay_scroll-{suffix}.csv"
        return list(csv.DictReader(path.read_text().splitlines()))


class G8RowTests(G8CollectTestCase):
    def test_one_row_in_the_g8_schema_pooling_every_iteration(self):
        (row,) = self.rows()
        self.assertEqual(sorted(row), sorted(bench.SCHEMAS["G8"].split(",")))
        self.assertEqual((row["scenario"], row["follow"], row["status"]), ("replay_scroll", "on", "captured"))
        self.assertGreater(row["frames"], 2 * 1000)
        self.assertAlmostEqual(row["p50_ms"], 8.333, places=2)
        self.assertEqual(len(self.fake.calls), 2)

    def test_memory_columns_come_from_the_baseline_and_tail_medians(self):
        (row,) = self.rows()
        self.assertAlmostEqual(row["memory_growth_mb"], 10.0, places=1)  # median tail 110.1 - median baseline 100.1
        self.assertAlmostEqual(row["peak_pss_mb"], 120.2, places=1)
        self.assertAlmostEqual(row["gpu_growth_mb"], 10.0)
        self.assertEqual(row["gpu_source"], "fdinfo")

    def test_no_gpu_leaves_growth_blank_and_names_the_reason(self):
        self.gpu = False
        (row,) = self.rows()
        self.assertIsNone(row["gpu_growth_mb"])
        self.assertEqual(row["gpu_source"], "unavailable:no_matching_pid")
        self.assertEqual(row["status"], "captured")
        self.assertEqual({sample["gpu_mb"] for sample in self.observations("memory")}, {""})
        self.assertEqual({sample["gpu_source"] for sample in self.observations("memory")},
                         {"unavailable:no_matching_pid"})

    def test_a_missing_tail_is_memory_unavailable(self):
        self.tail = False
        with self.assertRaisesRegex(CaptureError, "^MEMORY_UNAVAILABLE: .*'tail'"):
            self.rows()

    def test_refresh_is_checked(self):
        self.refresh_ns = 16_666_666
        with self.assertRaisesRegex(CaptureError, "^REFRESH_MISMATCH: "):
            self.rows()

    def test_an_iteration_that_barely_presented_fails(self):
        self.args.iterations = 1
        with patch.object(bench_collect, "frame_intervals_ms", return_value=[8.3] * 4), \
                self.assertRaisesRegex(CaptureError, "^CAPTURE_TOO_SHORT: replay_scroll iteration 1: 4 frame "):
            self.rows()

    def test_the_pooled_interval_minimum_applies(self):
        self.args.iterations = 1
        with patch.object(bench_collect, "frame_intervals_ms", return_value=[8.3] * 20), \
                self.assertRaisesRegex(CaptureError, "^CAPTURE_TOO_SHORT: replay_scroll: 20 frame intervals, need 30"):
            self.rows()

    def test_discarded_frames_over_the_limit_fail(self):
        with patch.object(bench_collect, "discard_rate", return_value=0.5), \
                self.assertRaisesRegex(CaptureError, "^DISCARD_RATE: replay_scroll"):
            self.rows()


class G8SessionBindingTests(G8CollectTestCase):
    def test_memory_sampled_from_another_launch_is_rejected(self):
        self.root_pid = PID + 1  # the sampler tracked a different process
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: the memory come from another session \\(pid"):
            self.rows()

    def test_frames_from_before_the_launch_are_rejected(self):
        self.log_offset = -2_000_000_000
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: a presented frame precedes this launch"):
            self.rows()

    def test_memory_rows_from_before_the_launch_are_rejected(self):
        self.row_shift_ns = -2_000_000_000
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: a memory sample precedes this launch"):
            self.rows()

    def test_a_missing_run_id_is_refused_before_any_launch(self):
        del self.args.run_id
        with self.assertRaisesRegex(CaptureError, "^RUN_ID_MISSING"):
            self.rows()
        self.assertEqual(self.fake.calls, [])

    def test_a_tampered_shim_log_pid_is_another_session(self):
        for shim_pid in (PID + 1, 1, PID - 4000):
            with self.subTest(shim_pid=shim_pid):
                self.shim_pid = shim_pid
                self.reset_launch_state()
                with self.assertRaisesRegex(CaptureError,
                                            "^SESSION_MISMATCH: the frames come from another session \\(pid differ"):
                    self.rows()

    def test_a_shim_log_without_a_pid_is_malformed(self):
        real = bench_collect.toplevel_pids

        def headerless(logs):
            for log in logs:
                log.header.pop("pid", None)
            return real(logs)
        with patch.object(bench_collect, "toplevel_pids", headerless), \
                self.assertRaisesRegex(CaptureError, "^PRESENT_LOG_MALFORMED: header without an integer pid"):
            self.rows()

    def test_flatpak_accepts_the_sandbox_pid_of_a_sampled_process_only(self):
        flatpak_root = self.root / "home/.var/app/dev.crosshook.CrossHook"
        (flatpak_root / "config/crosshook").mkdir(parents=True)
        self.env["CROSSHOOK_BENCH_ROOT"] = str(flatpak_root)
        self.args.flatpak, self.args.iterations = True, 1
        self.shim_pid = PID - 4000  # the innermost NSpid of the sampled app process
        self.assertEqual(len(self.rows()), 1)
        self.shim_pid = 77  # no sampled process has this pid, in any namespace
        self.reset_launch_state()
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: the frames come from another session "
                                                  "\\(pid differ"):
            self.rows()

    def test_the_session_is_tagged_with_run_iteration_pid_and_launch_time(self):
        evidence: dict = {}
        self.rows(evidence=evidence)
        sessions = [entry["session"] for entry in evidence["G8"]["replay_scroll"]]
        self.assertEqual(sessions, [{"run_id": RUN_ID, "pid": PID, "launch_ns": proc.launch_ns}
                                    for proc in self.fake.procs])
        self.assertEqual([entry["iteration"] for entry in evidence["G8"]["replay_scroll"]], [1, 2])


class G8LaunchTests(G8CollectTestCase):
    def test_every_launch_reads_events_and_holds_stdin_with_the_replay_env_and_scenario(self):
        self.rows()
        for call in self.fake.calls:
            self.assertFalse(call["exit_after"])
            self.assertTrue(call["events"] and call["stdin"])
            env = call["env"]
            self.assertEqual(env["CROSSHOOK_BENCH_SCENARIO"], "replay_scroll")
            self.assertEqual(env["CROSSHOOK_BENCH_LOG_RATE"], "5000")
            self.assertEqual(env["CROSSHOOK_BENCH_LOG"], str(self.root / "logs/launch-50k.log"))
            self.assertEqual(env["LD_PRELOAD"], "/shim.so")
        self.assertNotIn("CROSSHOOK_BENCH_SCENARIO", self.env)  # the caller's env is not modified

    def test_the_drawer_is_expanded_during_the_launch_and_the_settings_removed_after(self):
        self.rows()
        self.assertEqual(self.settings_seen, ["console_drawer_collapsed_default = false\n"] * 2)
        self.assertFalse(self.settings.exists())

    def test_the_settings_are_removed_when_the_drive_fails(self):
        self.drive_error = CaptureError("CONSOLE_NOT_VISIBLE: the console log is not visible")
        with self.assertRaisesRegex(CaptureError, "^CONSOLE_NOT_VISIBLE"):
            self.rows()
        self.assertFalse(self.settings.exists())
        self.assertEqual(len(self.stopped), 1)

    def test_the_app_is_stopped_and_every_pipe_closed_each_iteration(self):
        self.rows()
        self.assertEqual(self.stopped, self.fake.procs)
        self.assertTrue(all(proc.events.closed and proc.stdin.closed for proc in self.fake.procs))

    def test_the_driver_gets_the_device_kwin_events_and_the_mark_timeout(self):
        self.rows()
        (device,) = FakeDevice.instances
        self.assertTrue(device.closed)
        for drive, proc in zip(self.drives, self.fake.procs, strict=True):
            self.assertIs(drive["dev"], device)
            self.assertIs(drive["events"], proc.events)
            self.assertEqual(drive["mark_timeout"], bench_drivers_g5.mark_timeout_s(60, False))
            self.assertEqual(drive["dir"], self.root / f"present/G8/replay_scroll/iter-{self.fake.procs.index(proc) + 1}")
        self.assertEqual(len({drive["kwin"].run_tag for drive in self.drives}), 2)

    def test_the_sampler_walks_the_launched_process_tree(self):
        self.rows()
        self.assertEqual(len(self.sampler_args), 2)
        self.assertIsInstance(self.sampler_args[0][0], bench_g8session.PidRecorder)
        self.assertIsInstance(self.sampler_args[0][1], bench_g8session.GpuProbe)

    def test_flatpak_forwards_the_scenario_and_replay_env_only_through_env_flags(self):
        flatpak_root = self.root / "home/.var/app/dev.crosshook.CrossHook"
        (flatpak_root / "config/crosshook").mkdir(parents=True)
        self.env["CROSSHOOK_BENCH_ROOT"] = str(flatpak_root)
        self.args.flatpak, self.args.iterations = True, 1
        launch = SimpleNamespace(flatpak=True, branch="master", binary=None)
        self.rows(command_for=lambda env: bench.launch_command(launch, env))
        call = self.fake.calls[0]
        self.assertIn("--env=CROSSHOOK_BENCH_SCENARIO=replay_scroll", call["command"])
        self.assertIn("--env=CROSSHOOK_BENCH_LOG_RATE=5000", call["command"])
        for key in ("CROSSHOOK_BENCH_SCENARIO", "CROSSHOOK_BENCH_LOG", "CROSSHOOK_BENCH_LOG_RATE", "LD_PRELOAD"):
            self.assertNotIn(key, call["env"])
        self.assertTrue(self.drives[0]["flatpak"])
        self.assertEqual(self.drives[0]["mark_timeout"], bench_drivers_g5.mark_timeout_s(60, True))


class G8ObservationTests(G8CollectTestCase):
    def test_memory_and_frame_csvs_pool_both_launches_with_the_schema_columns(self):
        self.rows()
        memory, frames = self.observations("memory"), self.observations("frames")
        self.assertEqual(list(memory[0]),
                         ["t_ns", "phase", "pss_mb", "uss_mb", "gpu_mb", "gpu_source", "failed_pids"])
        self.assertEqual({row["failed_pids"] for row in memory}, {"0"})
        self.assertEqual(list(frames[0]), ["presented_ns", "frame_ms", "in_window"])
        self.assertEqual(len(memory), 2 * 170)
        self.assertEqual([int(row["t_ns"]) for row in memory], sorted(int(row["t_ns"]) for row in memory))
        self.assertEqual({row["phase"] for row in memory}, {"baseline", "replay", "settle", "tail"})
        self.assertEqual(frames[0]["frame_ms"], "")
        self.assertEqual({row["in_window"] for row in frames}, {"0", "1"})

    def test_observations_survive_a_later_failing_iteration(self):
        self.args.iterations = 3
        original = bench_collect._g8_iteration

        def second_fails(args, command_for, env, number, dev):
            if number == 2:
                raise CaptureError("REPLAY_NOT_DONE: x")
            return original(args, command_for, env, number, dev)
        with patch.object(bench_collect, "_g8_iteration", second_fails), \
                self.assertRaisesRegex(CaptureError, "^REPLAY_NOT_DONE"):
            self.rows()
        self.assertEqual(len(self.observations("memory")), 170)

    def test_evidence_records_timing_gpu_tool_and_the_sampled_processes(self):
        evidence: dict = {}
        self.rows(evidence=evidence)
        entry = evidence["G8"]["replay_scroll"][0]
        self.assertEqual((entry["start_to_done_s"], entry["settle_s"], entry["notches"], entry["follow"]),
                         (10.0, 1.2, 300, "on"))
        self.assertEqual(entry["gpu_source"], "fdinfo")
        self.assertEqual(entry["nvidia_smi_path"], "/usr/bin/nvidia-smi")
        self.assertTrue(entry["web_process_sampled"])
        self.assertEqual([item["pid"] for item in entry["sampled_pids"]], [PID, PID + 1])
        self.assertNotIn("nspid", entry["sampled_pids"][0])  # NSpid is only recorded for Flatpak

    def test_flatpak_evidence_carries_nspid(self):
        flatpak_root = self.root / "home/.var/app/dev.crosshook.CrossHook"
        (flatpak_root / "config/crosshook").mkdir(parents=True)
        self.env["CROSSHOOK_BENCH_ROOT"] = str(flatpak_root)
        self.args.flatpak, self.args.iterations = True, 1
        evidence: dict = {}
        launch = SimpleNamespace(flatpak=True, branch="master", binary=None)
        self.rows(command_for=lambda env: bench.launch_command(launch, env), evidence=evidence)
        self.assertEqual(evidence["G8"]["replay_scroll"][0]["sampled_pids"][0]["nspid"], [PID, PID - 4000])


class DispatchAndIngestTests(unittest.TestCase):
    def test_presentation_g8_routes_to_driven_rows(self):
        args = SimpleNamespace(collect="presentation")
        with patch.object(bench_collect, "driven_rows", return_value=[{"status": "captured"}]) as driven:
            factory = lambda env: ["app"]
            rows = bench_capture.capture("G8", args, ["app"], {"A": "1"}, command_for=factory)
        self.assertEqual(rows, [{"status": "captured"}])
        driven.assert_called_once_with(args, "G8", factory, {"A": "1"}, None)

    def test_csv_g8_ingest_keeps_working_and_fits_the_new_schema_with_blanks(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "frames.csv"
            path.write_text("frame_ms\n8.0\n8.5\n9.0\n16.0\n")
            args = SimpleNamespace(collect="csv", capture_csv=str(path), refresh_hz=120.0, timeout=5)
            proc = SimpleNamespace(pid=1, stderr=None)
            with patch.object(bench_capture, "ready", return_value=(proc, 1.0)), \
                    patch.object(bench_capture, "memory", return_value=(100.0, 50.0)), \
                    patch.object(bench_capture, "stop"), \
                    patch.object(bench_capture.time, "sleep", lambda _s: None), \
                    patch.object(bench_capture.time, "monotonic", side_effect=[0.0, 1.0, 20.0]), \
                    patch.object(proc, "poll", create=True, return_value=None):
                (row,) = bench_capture.capture("G8", args, ["app"], {})
        self.assertEqual((row["scenario"], row["frames"], row["status"]), ("external", 4, "measured"))
        self.assertLessEqual(set(row), set(bench.SCHEMAS["G8"].split(",")))
        for blank in ("peak_pss_mb", "gpu_growth_mb", "gpu_source", "follow"):
            self.assertNotIn(blank, row)  # the CSV writer leaves these empty


if __name__ == "__main__":
    unittest.main()
