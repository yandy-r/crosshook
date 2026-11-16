"""G8 observation CSVs: columns, blanks for missing readings, and private files."""
import csv
import stat
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bench_g8io import (
    FRAME_COLUMNS,
    MEMORY_COLUMNS,
    frame_records,
    frames_text,
    memory_text,
    write_observations,
)
from bench_memory import MemoryRow


def read(text: str) -> list[dict]:
    return list(csv.DictReader(text.splitlines()))


class TextTests(unittest.TestCase):
    def test_memory_columns_and_blank_readings(self):
        text = memory_text([MemoryRow(5, "baseline", 100.25, 40.5, 512.0, "fdinfo", 1),
                            MemoryRow(6, "tail", None, None, None, "unavailable:no_tool", 0, 3)])
        self.assertEqual(text.splitlines()[0], ",".join(MEMORY_COLUMNS))
        first, second = read(text)
        self.assertEqual(first, {"t_ns": "5", "phase": "baseline", "pss_mb": "100.250", "uss_mb": "40.500",
                                 "gpu_mb": "512.000", "gpu_source": "fdinfo", "failed_pids": "0"})
        self.assertEqual((second["pss_mb"], second["gpu_mb"], second["gpu_source"]),
                         ("", "", "unavailable:no_tool"))

    def test_failed_pids_is_the_last_memory_column_and_keeps_the_count(self):
        self.assertEqual(MEMORY_COLUMNS[-1], "failed_pids")
        rows = read(memory_text([MemoryRow(1, "tail", 1.0, 1.0, None, "unavailable", 0, 2)]))
        self.assertEqual(rows[0]["failed_pids"], "2")

    def test_a_zero_gpu_reading_is_written_as_zero_not_blank(self):
        (row,) = read(memory_text([MemoryRow(1, "tail", 1.0, 1.0, 0.0, "nvidia-smi")]))
        self.assertEqual(row["gpu_mb"], "0.000")

    def test_frame_records_flag_the_window_and_blank_the_first_interval(self):
        records = frame_records([30_000_000, 10_000_000, 20_000_000, 40_000_000], 20_000_000, 30_000_000)
        self.assertEqual([r.presented_ns for r in records], [10_000_000, 20_000_000, 30_000_000, 40_000_000])
        self.assertEqual([r.frame_ms for r in records], [None, 10.0, 10.0, 10.0])
        self.assertEqual([r.in_window for r in records], [False, True, True, False])  # edges are inclusive

    def test_frames_text_columns(self):
        text = frames_text(frame_records([1_000_000, 9_333_333], 0, 10_000_000))
        self.assertEqual(text.splitlines()[0], ",".join(FRAME_COLUMNS))
        self.assertEqual(read(text), [{"presented_ns": "1000000", "frame_ms": "", "in_window": "1"},
                                      {"presented_ns": "9333333", "frame_ms": "8.333", "in_window": "1"}])


class WriteTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        (Path(self.temp.name) / "home").mkdir()
        self.env = {"HOME": str(Path(self.temp.name) / "home")}

    def test_both_files_are_written_privately_beside_the_home_and_replaced_on_rewrite(self):
        rows = [MemoryRow(1, "baseline", 1.0, 1.0, None, "unavailable:no_tool")]
        memory, frames = write_observations(self.env, "replay_scroll", rows, frame_records([1, 2], 0, 5))
        observations = Path(self.temp.name) / "observations"
        self.assertEqual((memory, frames), (observations / "G8-replay_scroll-memory.csv",
                                            observations / "G8-replay_scroll-frames.csv"))
        for path in (memory, frames):
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        write_observations(self.env, "replay_scroll", rows * 2, frame_records([1, 2, 3], 0, 5))
        self.assertEqual(len(read(memory.read_text())), 2)
        self.assertEqual(len(read(frames.read_text())), 3)


if __name__ == "__main__":
    unittest.main()
