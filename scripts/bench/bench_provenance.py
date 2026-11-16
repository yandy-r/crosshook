"""Capture provenance: private raw run directories, manifests and checksums.

Every raw capture lives in its own run directory next to a provenance manifest
and a SHA256SUMS file, so a derived result can prove which commit, variant,
environment and boot produced it and that the bytes are unchanged (YAN-1007).
"""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
import shutil
import subprocess
import uuid
from pathlib import Path

from bench_capture import CaptureError

SCHEMA = "crosshook-bench-provenance/1"
MANIFEST = "provenance.json"
SUMS = "SHA256SUMS"
LOCK_NAME = ".bench.lock"
BOOT_ID_PATH = Path("/proc/sys/kernel/random/boot_id")
MAX_WALK_LEVELS = 4


def lock_output(output: Path) -> int:
    """Exclusive, non-blocking lock on <output>/.bench.lock; the caller closes the fd to release it.

    Two runs sharing <output> would otherwise replace each other's CSVs and meta.json
    between capture and sealing."""
    try:
        fd = os.open(output / LOCK_NAME, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    except OSError as error:
        raise CaptureError(f"OUTPUT_BUSY: cannot open the lock in {output}: {error.strerror}") from error
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        os.close(fd)
        raise CaptureError(f"OUTPUT_BUSY: another bench run is writing {output}") from None
    return fd


def new_run_id() -> str:
    return uuid.uuid4().hex


def boot_id() -> str:
    try:
        return BOOT_ID_PATH.read_text().strip()
    except OSError:
        return "unavailable"


def _git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True,
                          timeout=30, check=True).stdout


def git_state(repo: Path) -> tuple[str, bool]:
    commit = _git(repo, "rev-parse", "HEAD").strip()
    return commit, bool(_git(repo, "status", "--porcelain").strip())


def create_raw_dir(output: Path, run_id: str) -> Path:
    raw = output / "raw"
    if raw.is_symlink():
        raise CaptureError(f"RAW_DIR_SYMLINK: {raw} is a symlink")
    raw.mkdir(mode=0o700, parents=True, exist_ok=True)
    run_dir = raw / run_id
    try:
        run_dir.mkdir(mode=0o700)
    except FileExistsError:
        raise CaptureError(f"RAW_DIR_EXISTS: {run_dir} already exists") from None
    return run_dir


def write_private(path: Path, data: str) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
    fd = os.open(path, flags, 0o600)
    with os.fdopen(fd, "w") as handle:
        handle.write(data)


def write_manifest(raw_dir: Path, data: dict) -> None:
    text = json.dumps({"schema": SCHEMA, **data}, indent=2, sort_keys=True) + "\n"
    write_private(raw_dir / MANIFEST, text)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_sums(raw_dir: Path) -> None:
    entries = []
    for root, _dirs, files in os.walk(raw_dir, followlinks=False):
        for name in files:
            path = Path(root) / name
            if path.is_symlink() or not path.is_file():
                continue
            relpath = path.relative_to(raw_dir).as_posix()
            if relpath != SUMS:
                entries.append((relpath, sha256_file(path)))
    write_private(raw_dir / SUMS, "".join(f"{digest}  {rel}\n" for rel, digest in sorted(entries)))


def _copy_private(source: Path, target: Path) -> None:
    if source.is_symlink():
        return
    target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    target.chmod(0o600)


def finalize_raw(raw_dir: Path, root: Path | None, output: Path, written: list[Path],
                 manifest: dict, binary_sha256: str | None = None) -> None:
    """Move shim logs and result files into the raw run directory, then seal it.

    Runs even after a failed capture so partial evidence stays checksummed. Only the
    result files this run wrote (`written`) are sealed; symlinks are never followed.
    """
    present = root / "present" if root else None
    for log in sorted(present.rglob("wlpresent-*.log")) if present and present.is_dir() else []:
        if log.is_file():
            _copy_private(log, raw_dir / "present" / log.relative_to(present))
    for path in [*written, output / "meta.json"]:
        if path.is_file():
            _copy_private(path, raw_dir / path.name)
    if binary_sha256 is not None:
        manifest = {**manifest, "binary_sha256": binary_sha256}
    write_manifest(raw_dir, manifest)
    write_sums(raw_dir)


def _find_raw_dir(csv_path: Path) -> Path:
    directory = csv_path.parent
    for _ in range(MAX_WALK_LEVELS):
        if (directory / MANIFEST).is_file():
            return directory
        if directory.parent.name == "raw" or directory.parent == directory:
            break
        directory = directory.parent
    raise CaptureError("STALE_CAPTURE: no provenance.json beside the capture; "
                       "captures must come from a raw run directory")


def _verify_sums(raw_dir: Path, csv_path: Path) -> None:
    try:
        lines = (raw_dir / SUMS).read_text().splitlines()
    except OSError:
        raise CaptureError(f"STALE_CAPTURE: {SUMS} missing") from None
    listed = set()
    for line in lines:
        digest, _, relpath = line.partition("  ")
        path = raw_dir / relpath
        if (not relpath or Path(relpath).is_absolute() or ".." in Path(relpath).parts
                or not path.resolve().is_relative_to(raw_dir.resolve())
                or path.is_symlink() or not path.is_file() or sha256_file(path) != digest):
            raise CaptureError(f"STALE_CAPTURE: checksum mismatch {relpath}")
        listed.add(path.resolve())
    if (raw_dir / MANIFEST).resolve() not in listed:
        raise CaptureError(f"STALE_CAPTURE: {MANIFEST} not listed in {SUMS}")
    if csv_path.resolve() not in listed:
        raise CaptureError("STALE_CAPTURE: capture not listed in SHA256SUMS")


def verify_capture(csv_path: Path, expected: dict) -> dict:
    raw_dir = _find_raw_dir(csv_path)
    _verify_sums(raw_dir, csv_path)
    try:
        manifest = json.loads((raw_dir / MANIFEST).read_text())
    except (OSError, ValueError):
        raise CaptureError(f"STALE_CAPTURE: {MANIFEST} unreadable") from None
    if not isinstance(manifest, dict):
        raise CaptureError(f"STALE_CAPTURE: {MANIFEST} unreadable")
    for key, value in expected.items():
        if manifest.get(key) != value:
            raise CaptureError(f"STALE_CAPTURE: {key} mismatch")
    if manifest.get("status") != "captured":
        raise CaptureError("STALE_CAPTURE: run status is not captured")
    if manifest.get("dirty") is not False:
        raise CaptureError("STALE_CAPTURE: capture was taken from a dirty working tree")
    if manifest.get("boot_id") in (None, "unavailable"):
        raise CaptureError("STALE_CAPTURE: boot_id unavailable")
    return manifest
