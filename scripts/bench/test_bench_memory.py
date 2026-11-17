"""Memory sampler cadence, phase tagging, growth, caps and PID-reuse handling; no real /proc reads."""
from __future__ import annotations

import sys
import threading
import unittest
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_memory
from bench_memory import MemoryBenchError, MemoryRow, MemorySampler, ProcMemory

UID = 1000


@dataclass
class Gpu:
    mb: float | None
    source: str


class FakeClock:
    def __init__(self) -> None:
        self.now = 0
        self.slept: list[float] = []

    def __call__(self) -> int:
        return self.now

    def sleep(self, seconds: float) -> None:
        self.slept.append(seconds)
        self.now += int(seconds * 1e9)


def sampler(pids=(1,), mem=None, gpu=None, **kwargs):
    clock = kwargs.pop("clock", FakeClock())
    mem = mem if mem is not None else {1: ProcMemory(10, UID, 2048, 1024)}
    gpu = gpu or (lambda pids: Gpu(None, "unavailable"))
    return MemorySampler(lambda: set(pids), gpu, clock, clock.sleep, read_fn=lambda pid: _read(mem, pid),
                         uid=UID, **kwargs), clock, mem


def _read(mem, pid):
    value = mem[pid]
    if isinstance(value, Exception):
        raise value
    return value


def row(phase, pss, gpu=None, seq=0, t_ns=0, gpu_t_ns=0):
    return MemoryRow(t_ns, phase, pss, pss, gpu, "fdinfo" if gpu is not None else "unavailable", seq, 0, gpu_t_ns)


class TickTests(unittest.TestCase):
    def test_row_sums_pss_and_uss_across_pids_in_mb(self):
        s, clock, _ = sampler(pids=(1, 2), mem={1: ProcMemory(1, UID, 1024, 512), 2: ProcMemory(2, UID, 3072, 1536)})
        clock.now = 42
        s.tick_pss()
        [r] = s.rows()
        self.assertEqual((r.t_ns, r.pss_mb, r.uss_mb, r.failed_pids), (42, 4.0, 2.0, 0))
        self.assertFalse(r.flagged)

    def test_phase_tagging_follows_set_phase(self):
        s, _, _ = sampler(phase="boot")
        s.tick_pss()
        s.set_phase("baseline")
        s.tick_pss()
        s.tick_pss()
        s.set_phase("tail")
        s.tick_pss()
        self.assertEqual([r.phase for r in s.rows()], ["boot", "baseline", "baseline", "tail"])

    def test_unreadable_pid_is_counted_and_flags_the_row(self):
        s, _, _ = sampler(pids=(1, 2), mem={1: ProcMemory(1, UID, 1024, 512), 2: PermissionError("denied")})
        s.tick_pss()
        [r] = s.rows()
        self.assertEqual((r.pss_mb, r.failed_pids), (1.0, 1))
        self.assertTrue(r.flagged)

    def test_foreign_uid_is_dropped_and_counted(self):
        s, _, _ = sampler(pids=(1, 2), mem={1: ProcMemory(1, UID, 1024, 512), 2: ProcMemory(2, 0, 9999, 9999)})
        s.tick_pss()
        [r] = s.rows()
        self.assertEqual((r.pss_mb, r.failed_pids), (1.0, 1))

    def test_pid_reuse_drops_that_sample_and_the_newcomer_is_never_adopted(self):
        s, _, mem = sampler(mem={1: ProcMemory(10, UID, 1024, 512)})
        s.tick_pss()
        mem[1] = ProcMemory(99, UID, 8192, 8192)  # same pid, different start time
        s.tick_pss()
        s.tick_pss()  # still the newcomer: the first start time stays the identity
        mem[1] = ProcMemory(10, UID, 2048, 1024)  # the original process identity reads again
        s.tick_pss()
        first, reused, still_reused, original = s.rows()
        self.assertEqual(first.pss_mb, 1.0)
        self.assertEqual((reused.pss_mb, reused.failed_pids), (None, 1))
        self.assertEqual((still_reused.pss_mb, still_reused.failed_pids), (None, 1))
        self.assertEqual((original.pss_mb, original.failed_pids), (2.0, 0))

    def test_a_pid_first_seen_foreign_keeps_its_first_start_time(self):
        s, _, mem = sampler(mem={1: ProcMemory(10, 0, 1024, 512)})  # another user's process
        s.tick_pss()
        mem[1] = ProcMemory(10, UID, 1024, 512)
        s.tick_pss()
        self.assertEqual([r.failed_pids for r in s.rows()], [1, 0])

    def test_the_phase_is_the_one_in_force_when_the_row_is_stamped(self):
        mem = {1: ProcMemory(10, UID, 1024, 512)}
        clock = FakeClock()
        holder = []

        def read(pid):
            if holder:  # the phase changes while the (slow) read is in progress
                holder[0].set_phase("tail")
            return mem[pid]
        s = MemorySampler(lambda: {1}, lambda pids: Gpu(None, "unavailable"), clock, clock.sleep, read_fn=read,
                          uid=UID, phase="baseline")
        holder.append(s)
        clock.now = 7
        s.tick_pss()
        holder.clear()
        clock.now = 8
        s.tick_pss()
        self.assertEqual([(r.t_ns, r.phase) for r in s.rows()], [(7, "baseline"), (8, "tail")])

    def test_stamp_and_phase_are_read_under_the_same_lock(self):
        seen = []
        mem = {1: ProcMemory(10, UID, 1024, 512)}

        def clock():
            seen.append(sampler_under_test._lock.locked())
            return 5
        sampler_under_test = MemorySampler(lambda: {1}, lambda pids: Gpu(None, "unavailable"), clock,
                                           read_fn=lambda pid: mem[pid], uid=UID)
        sampler_under_test.tick_pss()
        self.assertEqual(seen, [True])

    def test_no_readable_pid_gives_none_not_zero(self):
        s, _, _ = sampler(mem={1: OSError("gone")})
        s.tick_pss()
        [r] = s.rows()
        self.assertIsNone(r.pss_mb)
        self.assertIsNone(r.uss_mb)


class GpuTests(unittest.TestCase):
    def test_unavailable_gpu_propagates_as_none(self):
        s, _, _ = sampler(gpu=lambda pids: Gpu(None, "unavailable"))
        s.tick_gpu()
        s.tick_pss()
        [r] = s.rows()
        self.assertIsNone(r.gpu_mb)
        self.assertEqual(r.gpu_source, "unavailable")

    def test_rows_carry_the_latest_gpu_sample_and_receive_the_pid_set(self):
        seen = []
        values = iter([Gpu(100.0, "fdinfo"), Gpu(150.0, "fdinfo")])
        s, _, _ = sampler(pids=(1, 2), mem={1: ProcMemory(1, UID, 1, 1), 2: ProcMemory(2, UID, 1, 1)},
                          gpu=lambda pids: seen.append(pids) or next(values))
        s.tick_gpu()
        s.tick_pss()
        s.tick_pss()
        s.tick_gpu()
        s.tick_pss()
        rows = s.rows()
        self.assertEqual([r.gpu_mb for r in rows], [100.0, 100.0, 150.0])
        self.assertEqual([r.gpu_seq for r in rows], [1, 1, 2])
        self.assertEqual(seen, [{1, 2}, {1, 2}])

    def test_gpu_probe_exception_is_unavailable(self):
        def boom(pids):
            raise RuntimeError("probe died")
        s, _, _ = sampler(gpu=boom)
        s.tick_gpu()
        s.tick_pss()
        [r] = s.rows()
        self.assertEqual((r.gpu_mb, r.gpu_source), (None, "unavailable:probe_error"))
        self.assertEqual(bench_memory.GPU_PROBE_ERROR, "unavailable:probe_error")

    def test_a_pids_function_that_raises_is_a_probe_error_too(self):
        def boom():
            raise RuntimeError("tree walk died")
        s = MemorySampler(boom, lambda pids: Gpu(1.0, "fdinfo"), read_fn=lambda pid: None, uid=UID)
        s.tick_gpu()
        self.assertEqual(s._gpu[2], "unavailable:probe_error")

    def test_rows_carry_the_time_the_gpu_sample_was_taken(self):
        s, clock, _ = sampler(gpu=lambda pids: Gpu(5.0, "fdinfo"))
        clock.now = 1_000
        s.tick_gpu()
        clock.now = 1_500
        s.tick_pss()
        clock.now = 2_000
        s.tick_gpu()
        clock.now = 2_600
        s.tick_pss()
        self.assertEqual([(r.t_ns, r.gpu_seq, r.gpu_t_ns) for r in s.rows()], [(1_500, 1, 1_000), (2_600, 2, 2_000)])

    def test_row_before_any_gpu_sample_is_unavailable(self):
        s, _, _ = sampler()
        s.tick_pss()
        self.assertEqual(s.rows()[0].gpu_source, "unavailable")


class CapAndCadenceTests(unittest.TestCase):
    def test_exceeding_max_samples_raises_on_rows(self):
        s, _, _ = sampler(max_samples=2)
        for _ in range(3):
            s.tick_pss()
        with self.assertRaisesRegex(MemoryBenchError, bench_memory.MEMORY_SAMPLES_EXCEEDED):
            s.rows()

    def test_loop_uses_the_pss_period_with_a_fake_clock_and_stops_on_cap(self):
        s, clock, _ = sampler(max_samples=3, pss_period=0.1)
        s._loop(s.tick_pss, 0.1)
        self.assertEqual(clock.slept, [0.1, 0.1, 0.1])
        self.assertEqual(clock.now, 300_000_000)
        with self.assertRaises(MemoryBenchError):
            s.stop()

    def test_a_tick_that_raises_stops_the_sampler_and_rows_reraises_it(self):
        s, _, _ = sampler(mem={1: KeyError("smaps")})  # not an OSError/ValueError: the read does not absorb it
        s._loop(s.tick_pss, 0.1)  # returns instead of raising, and sleeps never
        self.assertTrue(s._stop.is_set())
        for call in (s.rows, s.stop):
            with self.assertRaisesRegex(MemoryBenchError,
                                        "^MEMORY_SAMPLER_FAILED: KeyError: 'smaps'"):
                call()

    def test_the_first_error_is_kept(self):
        s, _, _ = sampler(max_samples=1)
        s.tick_pss()
        s.tick_pss()  # MEMORY_SAMPLES_EXCEEDED
        s._loop(lambda: (_ for _ in ()).throw(RuntimeError("later")), 0.1)
        with self.assertRaisesRegex(MemoryBenchError, bench_memory.MEMORY_SAMPLES_EXCEEDED):
            s.rows()

    def test_a_thread_that_dies_fails_stop_and_stops_the_other_thread(self):
        def read(pid):
            raise RuntimeError("boom")

        s = MemorySampler(lambda: {1}, lambda pids: Gpu(1.0, "fdinfo"), pss_period=0.001, gpu_period=0.001,
                          read_fn=read, uid=UID)
        s.start()
        for thread in s._threads:
            thread.join(5)
        self.assertFalse(any(t.is_alive() for t in s._threads))
        with self.assertRaisesRegex(MemoryBenchError, "^MEMORY_SAMPLER_FAILED: RuntimeError: boom"):
            s.stop()

    def test_start_stop_runs_both_threads_and_returns_rows(self):
        got_row = threading.Event()
        mem = {1: ProcMemory(10, UID, 2048, 1024)}

        def read(pid):
            got_row.set()
            return mem[pid]

        s = MemorySampler(lambda: {1}, lambda pids: Gpu(5.0, "fdinfo"), pss_period=0.001, gpu_period=0.001,
                          read_fn=read, uid=UID)
        s.start()
        self.assertTrue(got_row.wait(5))
        rows = s.stop()
        self.assertTrue(rows)
        self.assertTrue(all(r.pss_mb == 2.0 for r in rows))
        self.assertEqual(rows[0].gpu_mb, 5.0)  # the first row already carries a GPU sample
        self.assertFalse(any(t.is_alive() for t in s._threads))

    def test_double_start_is_rejected(self):
        s, _, _ = sampler()
        s.start()
        try:
            with self.assertRaises(MemoryBenchError):
                s.start()
        finally:
            s.stop()


class GrowthTests(unittest.TestCase):
    def test_growth_is_tail_median_minus_baseline_median(self):
        rows = [row("base", v) for v in (10.0, 12.0, 100.0)] + [row("tail", v) for v in (20.0, 22.0, 30.0, 500.0)]
        g = bench_memory.growth(rows, "base", "tail")
        self.assertEqual((g.base_pss_mb, g.tail_pss_mb, g.pss_mb), (12.0, 26.0, 14.0))
        self.assertIsNone(g.gpu_mb)

    def test_gpu_growth_uses_one_value_per_gpu_sample(self):
        base = [row("base", 1.0, 100.0, 1)] * 4 + [row("base", 1.0, 200.0, 2), row("base", 1.0, 300.0, 3)]
        tail = [row("tail", 1.0, 400.0, 4)] * 3 + [row("tail", 1.0, 600.0, 5)] * 3
        g = bench_memory.growth(base + tail, "base", "tail")
        self.assertEqual((g.base_gpu_mb, g.tail_gpu_mb, g.gpu_mb), (200.0, 500.0, 300.0))

    def test_gpu_growth_is_none_when_one_side_lacks_gpu(self):
        rows = [row("base", 1.0, 10.0, 1)] * 3 + [row("tail", 1.0)] * 3
        self.assertIsNone(bench_memory.growth(rows, "base", "tail").gpu_mb)

    def test_gpu_samples_taken_before_the_phase_started_are_left_out(self):
        base = [row("base", 1.0, 100.0, 1, t_ns=1_000, gpu_t_ns=900),   # taken before the phase: dropped
                row("base", 1.0, 100.0, 1, t_ns=1_100, gpu_t_ns=900),
                row("base", 1.0, 120.0, 2, t_ns=2_000, gpu_t_ns=1_900),
                row("base", 1.0, 140.0, 3, t_ns=3_000, gpu_t_ns=2_900)]
        tail = [row("tail", 1.0, 999.0, 4, t_ns=10_000, gpu_t_ns=9_500),  # still the settle phase's sample
                row("tail", 1.0, 999.0, 4, t_ns=10_100, gpu_t_ns=9_500),
                row("tail", 1.0, 300.0, 5, t_ns=11_000, gpu_t_ns=10_900),
                row("tail", 1.0, 320.0, 6, t_ns=12_000, gpu_t_ns=11_900)]
        g = bench_memory.growth(base + tail, "base", "tail")
        self.assertEqual((g.base_gpu_mb, g.tail_gpu_mb, g.gpu_mb), (130.0, 310.0, 180.0))

    def test_a_sample_taken_at_the_first_row_counts(self):
        rows = [row("base", 1.0, 10.0, 1, t_ns=100, gpu_t_ns=100)] * 3 + [row("tail", 1.0, 30.0, 2, t_ns=200, gpu_t_ns=200)] * 3
        self.assertEqual(bench_memory.growth(rows, "base", "tail").gpu_mb, 20.0)

    def test_a_side_whose_gpu_samples_all_predate_it_has_no_gpu_growth(self):
        rows = ([row("base", 1.0, 10.0, 1, t_ns=100, gpu_t_ns=100)] * 3
                + [row("tail", 1.0, 30.0, 2, t_ns=200, gpu_t_ns=150)] * 3)
        g = bench_memory.growth(rows, "base", "tail")
        self.assertIsNone(g.gpu_mb)
        self.assertEqual((g.base_gpu_mb, g.tail_gpu_mb), (10.0, None))

    def test_empty_phase_is_unavailable(self):
        rows = [row("base", 1.0)] * 3 + [row("tail", None)] * 3
        with self.assertRaisesRegex(MemoryBenchError, bench_memory.MEMORY_UNAVAILABLE):
            bench_memory.growth(rows, "base", "tail")
        with self.assertRaisesRegex(MemoryBenchError, bench_memory.MEMORY_UNAVAILABLE):
            bench_memory.growth(rows, "base", "missing")

    def test_fewer_than_three_samples_raises(self):
        for base, tail in ((2, 3), (3, 2)):
            rows = [row("base", 1.0)] * base + [row("tail", 2.0)] * tail
            with self.assertRaisesRegex(MemoryBenchError, bench_memory.MEMORY_SAMPLES_INSUFFICIENT):
                bench_memory.growth(rows, "base", "tail")

    def test_unreadable_rows_do_not_count_toward_the_minimum(self):
        rows = [row("base", 1.0)] * 2 + [row("base", None)] + [row("tail", 2.0)] * 3
        with self.assertRaisesRegex(MemoryBenchError, bench_memory.MEMORY_SAMPLES_INSUFFICIENT):
            bench_memory.growth(rows, "base", "tail")


class PeakTests(unittest.TestCase):
    def test_peak_over_all_rows_or_one_phase(self):
        rows = [row("a", 5.0), row("a", None), row("b", 9.0), row("b", 7.0)]
        self.assertEqual(bench_memory.peak_pss(rows), 9.0)
        self.assertEqual(bench_memory.peak_pss(rows, "a"), 5.0)

    def test_peak_without_readable_samples_is_unavailable(self):
        with self.assertRaisesRegex(MemoryBenchError, bench_memory.MEMORY_UNAVAILABLE):
            bench_memory.peak_pss([row("a", None)])
        with self.assertRaisesRegex(MemoryBenchError, bench_memory.MEMORY_UNAVAILABLE):
            bench_memory.peak_pss([])


class ProcReaderTests(unittest.TestCase):
    def test_reads_own_process(self):
        import os
        reading = bench_memory.read_proc_memory(os.getpid())
        self.assertEqual(reading.uid, os.getuid())
        self.assertGreater(reading.pss_kb, 0)

    def test_tree_pids_fn_includes_root(self):
        import os
        self.assertIn(os.getpid(), bench_memory.tree_pids_fn(os.getpid())())


if __name__ == "__main__":
    unittest.main()
