"""G8 session binding (pure), GPU labelling, pid recording and row summary."""
import dataclasses
import os
import sys
import tempfile
import unittest
from pathlib import Path
from typing import ClassVar
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bench_capture import CaptureError
from bench_g8session import (
    G8Iteration,
    G8Session,
    GpuProbe,
    PidRecorder,
    SampledPid,
    SessionFrames,
    SessionMemory,
    check_session,
    frames_pid,
    frames_session,
    iteration_gpu_source,
    summarize,
)
import bench_g8session
from bench_gpu import GpuSample
from bench_memory import Growth, MemoryRow

LAUNCH = 1_000_000_000
SESSION = G8Session("run-a", 1, 100, LAUNCH)


def rows(start_ns: int = LAUNCH + 100, end_ns: int = LAUNCH + 9_000) -> tuple[MemoryRow, ...]:
    return (MemoryRow(start_ns, "baseline", 1.0, 1.0, None, "unavailable"),
            MemoryRow(end_ns, "tail", 1.0, 1.0, None, "unavailable"))


def check(**changes):
    frames = SessionFrames(changes.pop("frames_session", SESSION), tuple(changes.pop("times", (LAUNCH + 500,))))
    memory = SessionMemory(changes.pop("memory_session", SESSION), changes.pop("memory_rows", rows()))
    check_session(changes.pop("expected", SESSION), frames, memory, LAUNCH + 200, LAUNCH + 8_000)


class CheckSessionTests(unittest.TestCase):
    def test_matching_session_passes(self):
        check()

    def test_each_identity_field_is_enforced_for_frames_and_for_memory(self):
        for field, other in (("run_id", "run-b"), ("iteration", 2), ("pid", 101), ("launch_ns", LAUNCH + 1)):
            for label in ("frames", "memory"):
                with self.subTest(field=field, label=label):
                    wrong = dataclasses.replace(SESSION, **{field: other})
                    with self.assertRaisesRegex(
                            CaptureError, rf"^SESSION_MISMATCH: the {label} come from another session \({field} differ"):
                        check(**{f"{label}_session": wrong})

    def test_frames_before_the_launch_are_another_session(self):
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: a presented frame precedes"):
            check(times=(LAUNCH - 1, LAUNCH + 500))

    def test_memory_before_the_launch_is_another_session(self):
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: a memory sample precedes"):
            check(memory_rows=rows(start_ns=LAUNCH - 5))

    def test_no_memory_samples_are_refused(self):
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: no memory samples"):
            check(memory_rows=())

    def test_memory_must_span_the_frame_window(self):
        for early, late in ((LAUNCH + 300, LAUNCH + 9_000), (LAUNCH + 100, LAUNCH + 7_999)):
            with self.subTest(early=early, late=late), \
                    self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: the memory samples do not span"):
                check(memory_rows=rows(early, late))

    def test_the_check_is_pure(self):
        frames, memory = SessionFrames(SESSION, (LAUNCH + 500,)), SessionMemory(SESSION, rows())
        for _ in range(2):
            check_session(SESSION, frames, memory, LAUNCH + 200, LAUNCH + 8_000)
        self.assertEqual((frames.presented_ns, memory.rows), ((LAUNCH + 500,), rows()))

    def test_frames_session_takes_the_iteration_from_the_directory_name(self):
        self.assertEqual(frames_session("r", Path("/x/present/G8/replay_scroll/iter-3"), 7, 9),
                         G8Session("r", 3, 7, 9))
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: 'elsewhere'"):
            frames_session("r", Path("/x/elsewhere"), 7, 9)


class FramesPidTests(unittest.TestCase):
    SAMPLED: ClassVar[list[SampledPid]] = [SampledPid(500, "bwrap", (500, 1), False),
                                           SampledPid(510, "crosshook", (510, 2), False),
                                           SampledPid(511, "web", (511, 3), True)]

    def test_natively_only_the_root_pid_is_the_launch(self):
        self.assertEqual(frames_pid({500}, 500, [], False), 500)
        for shim in ({501}, {2}, {500, 501}):
            with self.subTest(shim=shim):
                self.assertNotEqual(frames_pid(shim, 500, self.SAMPLED, False), 500)

    def test_natively_a_sandbox_pid_is_not_accepted_even_when_it_is_sampled(self):
        self.assertEqual(frames_pid({2}, 500, self.SAMPLED, False), 2)

    def test_flatpak_accepts_the_innermost_nspid_or_a_sampled_host_pid(self):
        for shim in ({2}, {3}, {510}, {2, 510}):
            with self.subTest(shim=shim):
                self.assertEqual(frames_pid(shim, 500, self.SAMPLED, True), 500)

    def test_flatpak_rejects_a_pid_that_matches_no_sampled_process(self):
        self.assertEqual(frames_pid({77}, 500, self.SAMPLED, True), 77)
        self.assertEqual(frames_pid({2, 77}, 500, self.SAMPLED, True), 77)

    def test_a_pid_that_is_only_an_outer_nspid_entry_is_not_accepted(self):
        self.assertEqual(frames_pid({1}, 501, [SampledPid(500, "bwrap", (500, 1, 7), False)], True), 1)

    def test_no_shim_pid_is_a_session_mismatch(self):
        with self.assertRaisesRegex(CaptureError, "^SESSION_MISMATCH: no shim log pid"):
            frames_pid(set(), 500, self.SAMPLED, False)

    def test_the_returned_pid_is_what_check_session_compares(self):
        session = G8Session("r", 1, 500, LAUNCH)
        tampered = frames_session("r", Path("/x/iter-1"), frames_pid({999}, 500, self.SAMPLED, False), LAUNCH)
        with self.assertRaisesRegex(CaptureError, r"^SESSION_MISMATCH: the frames come from another session \(pid"):
            check_session(session, SessionFrames(tampered, (LAUNCH + 500,)), SessionMemory(session, rows()),
                          LAUNCH + 200, LAUNCH + 8_000)


class GpuProbeTests(unittest.TestCase):
    def test_an_unavailable_reading_carries_its_reason_in_the_source(self):
        probe = GpuProbe(lambda pids: GpuSample(None, "unavailable", "no_matching_pid", 0, "/usr/bin/nvidia-smi"))
        reading = probe({1})
        self.assertEqual((reading.mb, reading.source), (None, "unavailable:no_matching_pid"))
        self.assertEqual(probe.tool_path, "/usr/bin/nvidia-smi")

    def test_a_reason_less_unavailable_reading_is_still_labelled(self):
        self.assertEqual(GpuProbe(lambda pids: GpuSample(None, "unavailable"))({1}).source,
                         "unavailable:no_sample")

    def test_a_real_reading_passes_through_and_zero_stays_zero(self):
        reading = GpuProbe(lambda pids: GpuSample(0.0, "nvidia-smi", "", 0, "/p"))({1})
        self.assertEqual((reading.mb, reading.source), (0.0, "nvidia-smi"))

    def test_the_highest_denied_count_is_kept(self):
        answers = iter([GpuSample(1.0, "fdinfo", "", 2), GpuSample(1.0, "fdinfo", "", 5),
                        GpuSample(None, "unavailable", "no_tool", 3)])
        probe = GpuProbe(lambda pids: next(answers))
        self.assertEqual(probe.denied_max, 0)
        for _ in range(3):
            probe({1})
        self.assertEqual(probe.denied_max, 5)

    def test_the_tool_path_is_kept_from_any_sample(self):
        answers = iter([GpuSample(None, "unavailable", "tool_failed", 0, "/p"),
                        GpuSample(None, "unavailable", "no_tool")])
        probe = GpuProbe(lambda pids: next(answers))
        probe({1})
        probe({1})
        self.assertEqual(probe.tool_path, "/p")


def reader(table: dict[tuple[int, str], str]):
    def read(pid: int, name: str) -> str:
        try:
            return table[pid, name]
        except KeyError:
            raise OSError("gone") from None
    return read


class PidRecorderTests(unittest.TestCase):
    TABLE: ClassVar[dict] = {(10, "status"): "Name:\tcrosshook\nNSpid:\t10\t2\n", (10, "cmdline"): "/app/crosshook\0",
                   (11, "status"): "Name:\tWebKitWebProces\nNSpid:\t11\t3\n",
                   (11, "cmdline"): "/usr/lib/webkit/WebKitWebProcess\0-x\0"}

    def test_it_returns_the_tree_and_describes_each_pid_once(self):
        calls = []
        read = reader(self.TABLE)
        recorder = PidRecorder(10, True, pids_fn=lambda: {10, 11}, read=lambda p, n: calls.append((p, n)) or read(p, n))
        self.assertEqual(recorder(), {10, 11})
        recorder()
        self.assertEqual([c for c in calls if c[1] != "stat"],
                         [(10, "status"), (10, "cmdline"), (11, "status"), (11, "cmdline")])  # each described once
        self.assertEqual([(e.pid, e.comm, e.nspid) for e in recorder.sampled()],
                         [(10, "crosshook", (10, 2)), (11, "WebKitWebProces", (11, 3))])
        self.assertTrue(recorder.web_process_sampled())
        self.assertEqual(recorder.root_pid, 10)

    def test_a_reused_pid_is_a_new_process_and_is_described_again(self):
        stat = {"text": stat_line(10, "old", 100)}
        status = {"Name": "old"}

        def read(pid, name):
            if name == "stat":
                return stat["text"]
            return f"Name:\t{status['Name']}\n" if name == "status" else "/x\0"
        recorder = PidRecorder(10, False, pids_fn=lambda: {10}, read=read)
        recorder()
        recorder()
        self.assertEqual([(e.pid, e.comm, e.start_time) for e in recorder.sampled()], [(10, "old", 100)])
        stat["text"], status["Name"] = stat_line(10, "new", 200), "new"
        recorder()
        self.assertEqual([(e.pid, e.comm, e.start_time) for e in recorder.sampled()],
                         [(10, "old", 100), (10, "new", 200)])

    def test_a_pid_without_a_readable_start_time_is_keyed_on_the_pid_alone(self):
        recorder = PidRecorder(10, False, pids_fn=lambda: {10}, read=reader(self.TABLE))
        recorder()
        recorder()
        self.assertEqual([(e.pid, e.start_time) for e in recorder.sampled()], [(10, None)])

    def test_nspid_is_recorded_only_for_flatpak(self):
        recorder = PidRecorder(10, False, pids_fn=lambda: {10}, read=reader(self.TABLE))
        recorder()
        self.assertEqual(recorder.sampled()[0].nspid, ())

    def test_a_tree_without_a_web_process_is_reported_as_such(self):
        recorder = PidRecorder(10, False, pids_fn=lambda: {10}, read=reader(self.TABLE))
        recorder()
        self.assertFalse(recorder.web_process_sampled())

    def test_the_web_process_is_found_by_cmdline_alone(self):
        table = {(12, "status"): "Name:\tsomething\n", (12, "cmdline"): "/x/WebKitWebProcess\0"}
        recorder = PidRecorder(12, False, pids_fn=lambda: {12}, read=reader(table))
        recorder()
        self.assertTrue(recorder.web_process_sampled())

    def test_a_pid_that_left_is_still_recorded(self):
        recorder = PidRecorder(99, False, pids_fn=lambda: {99}, read=reader({}))
        recorder()
        (entry,) = recorder.sampled()
        self.assertEqual((entry.pid, entry.comm, entry.web_process), (99, "", False))


def stat_line(pid: int, comm: str, start_time: int) -> str:
    return f"{pid} ({comm}) " + " ".join(["S", *["0"] * 18, str(start_time)]) + " 0 0\n"


class ReadProcFileTests(unittest.TestCase):
    def test_reads_are_bounded_to_4096_bytes(self):
        self.assertEqual(bench_g8session.MAX_PROC_READ, 4096)
        with tempfile.TemporaryDirectory() as directory:
            big = Path(directory) / "cmdline"
            big.write_bytes(b"a" * 20_000)
            real_open = os.open
            with patch.object(bench_g8session.os, "open",
                              lambda path, flags, *a: real_open(big, flags, *a)):
                self.assertEqual(len(bench_g8session.read_proc_file(1, "cmdline")), 4096)

    def test_reads_a_real_proc_file_and_closes_it(self):
        self.assertTrue(bench_g8session.read_proc_file(os.getpid(), "status").startswith("Name:"))
        self.assertTrue(bench_g8session.read_proc_file(os.getpid(), "stat").startswith(f"{os.getpid()} ("))

    def test_a_missing_pid_raises_oserror(self):
        with self.assertRaises(OSError):
            bench_g8session.read_proc_file(2**30, "stat")


def growth(pss: float, gpu: float | None) -> Growth:
    return Growth(pss, gpu, 100.0, 100.0 + pss, None if gpu is None else 500.0,
                  None if gpu is None else 500.0 + gpu)


def iteration(intervals, pss=10.0, gpu=5.0, peak=130.0, source="fdinfo", follow="on", number=1) -> G8Iteration:
    return G8Iteration(dataclasses.replace(SESSION, iteration=number), intervals, growth(pss, gpu), peak, source, follow)


class SummaryTests(unittest.TestCase):
    def test_intervals_are_pooled_and_memory_summarised_per_iteration(self):
        row = summarize("replay_scroll", [iteration([8.0, 9.0], 10, 4, 130, number=1),
                                          iteration([10.0, 20.0], 20, 6, 150, number=2)], 120.0)
        self.assertEqual((row["frames"], row["memory_growth_mb"], row["peak_pss_mb"], row["gpu_growth_mb"]),
                         (4, 15.0, 150, 5.0))
        self.assertEqual((row["scenario"], row["gpu_source"], row["follow"], row["status"]),
                         ("replay_scroll", "fdinfo", "on", "captured"))
        self.assertAlmostEqual(row["over_interval_percent"], 25.0)  # 20 ms is over 1.5 refresh periods

    def test_gpu_growth_is_blank_unless_every_iteration_has_it(self):
        row = summarize("replay_scroll", [iteration([8.0], gpu=4.0),
                                          iteration([8.0], gpu=None, source="unavailable:no_tool", number=2)], 120.0)
        self.assertIsNone(row["gpu_growth_mb"])
        self.assertEqual(row["gpu_source"], "fdinfo+unavailable:no_tool")

    def test_differing_follow_states_are_reported_as_mixed(self):
        row = summarize("replay_scroll", [iteration([8.0], follow="on"), iteration([8.0], follow="off", number=2)],
                        120.0)
        self.assertEqual(row["follow"], "mixed")

    def test_gpu_source_of_an_iteration(self):
        def mem(phase, mb, source):
            return MemoryRow(1, phase, 1.0, 1.0, mb, source)
        with_gpu = [mem("baseline", 5.0, "nvidia-smi"), mem("tail", 6.0, "nvidia-smi"), mem("replay", None, "x")]
        self.assertEqual(iteration_gpu_source(with_gpu, growth(1.0, 1.0)), "nvidia-smi")
        without = [mem("baseline", None, "unavailable:no_tool"), mem("tail", None, "unavailable:no_tool")]
        self.assertEqual(iteration_gpu_source(without, growth(1.0, None)), "unavailable:no_tool")
        self.assertEqual(iteration_gpu_source([mem("tail", None, "unavailable")], growth(1.0, None)),
                         "unavailable:no_sample")
        self.assertEqual(iteration_gpu_source([], growth(1.0, None)), "unavailable:no_sample")

    def test_gpu_source_without_growth_is_the_first_unavailable_reason_of_either_side(self):
        def mem(phase, mb, source):
            return MemoryRow(1, phase, 1.0, 1.0, mb, source)
        for rows, expected in (
                ([mem("baseline", None, "unavailable:no_tool"), mem("tail", 5.0, "fdinfo")], "unavailable:no_tool"),
                ([mem("baseline", 5.0, "fdinfo"), mem("tail", None, "unavailable:tool_failed")],
                 "unavailable:tool_failed"),
                ([mem("baseline", None, "unavailable:no_numeric_fb"), mem("tail", None, "unavailable:no_tool")],
                 "unavailable:no_numeric_fb"),
                ([mem("replay", None, "unavailable:no_tool"), mem("tail", None, "unavailable:probe_error")],
                 "unavailable:probe_error")):
            with self.subTest(expected=expected):
                self.assertEqual(iteration_gpu_source(rows, growth(1.0, None)), expected)

    def test_gpu_source_is_never_a_numeric_source_with_a_reason(self):
        def mem(phase, mb, source):
            return MemoryRow(1, phase, 1.0, 1.0, mb, source)
        # both sides have numeric sources, but growth is None (a side has no GPU sample of its own phase)
        rows = [mem("baseline", 5.0, "nvidia-smi"), mem("tail", 6.0, "nvidia-smi")]
        self.assertEqual(iteration_gpu_source(rows, growth(1.0, None)), "unavailable:no_sample")
        for rows in ([mem("baseline", 5.0, "fdinfo"), mem("tail", None, "unavailable")],
                     [mem("baseline", None, "nvidia-smi"), mem("tail", 6.0, "nvidia-smi")]):
            source = iteration_gpu_source(rows, growth(1.0, None))
            self.assertEqual(source, "unavailable:no_sample")
            self.assertNotIn("nvidia-smi:", source)
            self.assertNotIn("fdinfo:", source)


if __name__ == "__main__":
    unittest.main()
