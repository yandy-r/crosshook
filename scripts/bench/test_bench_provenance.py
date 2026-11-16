"""Provenance regression checks for raw capture directories; stdlib only."""
import json
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_provenance
from bench_capture import CaptureError
from bench_provenance import (boot_id, create_raw_dir, new_run_id, verify_capture,
                              write_manifest, write_private, write_sums)

EXPECTED = {"commit": "abc123", "variant": "baseline", "environment": "native", "boot_id": "boot-1"}
MANIFEST = {**EXPECTED, "status": "captured", "dirty": False}


class ProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name) / "out"
        self.run_id = new_run_id()
        self.raw = create_raw_dir(self.output, self.run_id)
        self.csv = self.raw / "capture.csv"
        write_private(self.csv, "a,b\n1,2\n")
        self.seal(MANIFEST)

    def seal(self, manifest: dict) -> None:
        for name in ("provenance.json", "SHA256SUMS"):
            (self.raw / name).unlink(missing_ok=True)
        write_manifest(self.raw, manifest)
        write_sums(self.raw)

    def test_run_id_and_boot_id(self):
        self.assertRegex(new_run_id(), "^[0-9a-f]{32}$")
        self.assertNotEqual(new_run_id(), new_run_id())
        self.assertTrue(boot_id())
        with patch.object(bench_provenance, "BOOT_ID_PATH", Path(self.temp.name) / "missing"):
            self.assertEqual(boot_id(), "unavailable")

    def test_round_trip(self):
        manifest = verify_capture(self.csv, EXPECTED)
        self.assertEqual(manifest["schema"], "crosshook-bench-provenance/1")
        self.assertEqual(manifest["commit"], "abc123")
        self.assertEqual(verify_capture(self.csv, {}), manifest)

    def test_nested_capture_finds_manifest(self):
        nested = self.raw / "sub"
        nested.mkdir()
        write_private(nested / "deep.csv", "x\n")
        (self.raw / "SHA256SUMS").unlink()
        write_sums(self.raw)
        self.assertEqual(verify_capture(nested / "deep.csv", EXPECTED)["variant"], "baseline")

    def test_tampered_file(self):
        self.csv.write_text("a,b\n9,9\n")
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: checksum mismatch capture.csv"):
            verify_capture(self.csv, EXPECTED)

    def test_missing_listed_file(self):
        self.csv.unlink()
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: checksum mismatch"):
            verify_capture(self.csv, EXPECTED)

    def test_foreign_commit(self):
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: commit mismatch"):
            verify_capture(self.csv, {**EXPECTED, "commit": "def456"})

    def test_foreign_boot_id(self):
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: boot_id mismatch"):
            verify_capture(self.csv, {**EXPECTED, "boot_id": "boot-2"})

    def test_missing_manifest(self):
        (self.raw / "provenance.json").unlink()
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: no provenance.json"):
            verify_capture(self.csv, EXPECTED)

    def test_unlisted_csv(self):
        other = self.raw / "other.csv"
        write_private(other, "late\n")
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: capture not listed in SHA256SUMS"):
            verify_capture(other, EXPECTED)

    def test_manifest_must_be_listed_in_sums(self):
        sums = self.raw / "SHA256SUMS"
        sums.write_text("".join(line for line in sums.read_text().splitlines(True)
                                if not line.endswith("  provenance.json\n")))
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: provenance.json not listed in SHA256SUMS$"):
            verify_capture(self.csv, EXPECTED)

    def test_sealed_sums_list_the_manifest(self):
        listed = [line.split("  ", 1)[1] for line in (self.raw / "SHA256SUMS").read_text().splitlines()]
        self.assertIn("provenance.json", listed)

    def test_output_lock_is_exclusive_and_released(self):
        out = Path(self.temp.name) / "locked"
        out.mkdir()
        first = bench_provenance.lock_output(out)
        try:
            self.assertEqual(stat.S_IMODE((out / ".bench.lock").stat().st_mode), 0o600)
            with self.assertRaisesRegex(CaptureError, "^OUTPUT_BUSY: another bench run is writing "):
                bench_provenance.lock_output(out)
        finally:
            os.close(first)
        os.close(bench_provenance.lock_output(out))

    def test_output_lock_refuses_symlink(self):
        out = Path(self.temp.name) / "linked"
        out.mkdir()
        (out / ".bench.lock").symlink_to(Path(self.temp.name) / "elsewhere")
        with self.assertRaisesRegex(CaptureError, "^OUTPUT_BUSY: "):
            bench_provenance.lock_output(out)

    def test_finalize_records_given_binary_hash(self):
        output = Path(self.temp.name) / "results"
        output.mkdir()
        (output / "meta.json").write_text("{}\n")
        run = create_raw_dir(Path(self.temp.name) / "out3", new_run_id())
        bench_provenance.finalize_raw(run, None, output, [], MANIFEST, "f" * 64)
        self.assertEqual(json.loads((run / "provenance.json").read_text())["binary_sha256"], "f" * 64)

    def test_existing_run_dir(self):
        with self.assertRaisesRegex(CaptureError, "^RAW_DIR_EXISTS: "):
            create_raw_dir(self.output, self.run_id)

    def test_symlinked_raw_refused(self):
        other = Path(self.temp.name) / "other"
        (other / "target").mkdir(parents=True)
        (other / "raw").symlink_to(other / "target", target_is_directory=True)
        with self.assertRaisesRegex(CaptureError, "^RAW_DIR_SYMLINK: "):
            create_raw_dir(other, new_run_id())

    def test_symlink_skipped_in_sums(self):
        outside = Path(self.temp.name) / "outside.txt"
        outside.write_text("secret")
        (self.raw / "link.txt").symlink_to(outside)
        (self.raw / "SHA256SUMS").unlink()
        write_sums(self.raw)
        lines = (self.raw / "SHA256SUMS").read_text().splitlines()
        self.assertEqual([line.split("  ", 1)[1] for line in lines],
                         ["capture.csv", "provenance.json"])

    def test_modes(self):
        mode = lambda path: stat.S_IMODE(path.stat().st_mode)
        self.assertEqual(mode(self.output / "raw"), 0o700)
        self.assertEqual(mode(self.raw), 0o700)
        for name in ("capture.csv", "provenance.json", "SHA256SUMS"):
            self.assertEqual(mode(self.raw / name), 0o600, name)

    def test_write_private_refuses_existing(self):
        with self.assertRaises(FileExistsError):
            write_private(self.csv, "again")

    def test_manifest_format(self):
        text = (self.raw / "provenance.json").read_text()
        self.assertTrue(text.endswith("\n"))
        self.assertEqual(json.loads(text), {"schema": "crosshook-bench-provenance/1", **MANIFEST})

    def test_refuses_unsound_manifests(self):
        cases = {"dirty working tree": {**MANIFEST, "dirty": True},
                 "dirty working tree (missing)": {k: v for k, v in MANIFEST.items() if k != "dirty"},
                 "run status is not captured": {**MANIFEST, "status": "failed"},
                 "run status is not captured (missing)": {k: v for k, v in MANIFEST.items() if k != "status"},
                 "boot_id unavailable": {**MANIFEST, "boot_id": "unavailable"}}
        for label, manifest in cases.items():
            with self.subTest(label):
                self.seal(manifest)
                expected = {**EXPECTED, "boot_id": manifest.get("boot_id")}
                with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: "):
                    verify_capture(self.csv, expected)

    def test_corrupt_manifest_is_stale(self):
        (self.raw / "provenance.json").write_text("{not json")
        (self.raw / "SHA256SUMS").unlink()
        write_sums(self.raw)
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: provenance.json unreadable$"):
            verify_capture(self.csv, EXPECTED)

    def test_absolute_path_in_sums_refused(self):
        outside = Path(self.temp.name) / "outside.txt"
        outside.write_text("x")
        digest = bench_provenance.sha256_file(outside)
        sums = self.raw / "SHA256SUMS"
        sums.write_text(sums.read_text() + f"{digest}  {outside}\n")
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: checksum mismatch /"):
            verify_capture(self.csv, EXPECTED)

    def test_symlinked_directory_in_sums_refused(self):
        outside = Path(self.temp.name) / "outside"
        outside.mkdir()
        (outside / "x.txt").write_text("x")
        (self.raw / "linked").symlink_to(outside, target_is_directory=True)
        sums = self.raw / "SHA256SUMS"
        sums.write_text(sums.read_text() + f"{bench_provenance.sha256_file(outside / 'x.txt')}  linked/x.txt\n")
        with self.assertRaisesRegex(CaptureError, "^STALE_CAPTURE: checksum mismatch linked/x.txt"):
            verify_capture(self.csv, EXPECTED)

    def test_finalize_seals_only_written_and_skips_symlinks(self):
        output = Path(self.temp.name) / "results"
        output.mkdir()
        (output / "G1.csv").write_text("stale\n")
        (output / "G2.csv").write_text("fresh\n")
        (output / "meta.json").write_text("{}\n")
        secret = Path(self.temp.name) / "secret"
        secret.write_text("s")
        (output / "G4.csv").symlink_to(secret)
        run = create_raw_dir(Path(self.temp.name) / "out2", new_run_id())
        bench_provenance.finalize_raw(run, None, output, [output / "G2.csv", output / "G4.csv"], MANIFEST)
        self.assertEqual(sorted(p.name for p in run.iterdir()),
                         ["G2.csv", "SHA256SUMS", "meta.json", "provenance.json"])
        self.assertEqual(verify_capture(run / "G2.csv", EXPECTED)["status"], "captured")

    def test_sha256_file_is_chunked_digest(self):
        import hashlib
        big = Path(self.temp.name) / "big"
        big.write_bytes(b"a" * (3 << 20))
        self.assertEqual(bench_provenance.sha256_file(big), hashlib.sha256(b"a" * (3 << 20)).hexdigest())


if __name__ == "__main__":
    unittest.main()
