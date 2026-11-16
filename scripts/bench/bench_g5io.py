"""G5 side effects: toggle-state snapshot and restore, and the observation CSV.

The toggle scenarios change the isolated profile (a favorite, an optimization), so the profile
TOML directory and `metadata.db` (with its WAL files) are copied before the scenario and put back
after every iteration, while the app is stopped. Every path is resolved first and refused unless
it lies inside the `crosshook-bench-*` root; symlinks are never followed, copied or restored.
"""
from __future__ import annotations

import csv
import io
import os
import shutil
import stat
from pathlib import Path

from bench_capture import CaptureError
from bench_drivers import _bench_root, app_dir
from bench_drivers_g5 import TrialRecord
from bench_provenance import write_private

TOGGLE_SCENARIOS = ("toggle_favorite", "toggle_optimization")
OBSERVATION_COLUMNS = ("seq", "inject_ns", "render_ns", "commit_ns", "presented_ns", "latency_ms",
                       "uncertainty_ms", "status")
OBSERVATIONS_DIR = "observations"
STATE_DIR = "g5-state"
DB_FILES = ("metadata.db", "metadata.db-wal", "metadata.db-shm")


def _unsafe(path: Path, why: str) -> CaptureError:
    return CaptureError(f"STATE_UNSAFE: {path.name} {why}; refusing to touch it")


def _state_paths(env: dict[str, str], flatpak: bool) -> list[Path]:
    config, data = app_dir(env, flatpak, "config"), app_dir(env, flatpak, "data")
    return [config / "profiles", *(data / name for name in DB_FILES)]


def _remove(path: Path) -> None:
    """Delete a file or tree without following symlinks (a link is unlinked, never entered)."""
    try:
        mode = os.lstat(path).st_mode
    except FileNotFoundError:
        return
    if stat.S_ISDIR(mode):
        for child in os.listdir(path):
            _remove(path / child)
        path.rmdir()
    else:
        path.unlink()


def _copy(source: Path, target: Path) -> None:
    """Copy a regular file or a tree of them; anything else (symlink, socket, ...) is refused."""
    mode = os.lstat(source).st_mode
    if stat.S_ISDIR(mode):
        target.mkdir(mode=0o700)
        for child in sorted(os.listdir(source)):
            _copy(source / child, target / child)
    elif stat.S_ISREG(mode):
        shutil.copyfile(source, target)
        target.chmod(stat.S_IMODE(mode))
    else:
        raise _unsafe(source, "is not a regular file or directory")


class ToggleState:
    """Snapshot of the isolated profile state; `restore()` puts it back and may be called repeatedly."""

    def __init__(self, env: dict[str, str], flatpak: bool):
        root = _bench_root(env)
        self._live = []
        for path in _state_paths(env, flatpak):
            resolved = path.parent.resolve() / path.name
            if not resolved.is_relative_to(root):
                raise _unsafe(path, "resolves outside the bench root")
            self._live.append(resolved)
        self._store = root / STATE_DIR
        self._saved: list[bool] = []
        self._created = False  # this instance made the store directory, so only it may delete it

    def capture(self) -> None:
        """Copy what exists now (the app must be stopped); what does not exist is restored as absent."""
        if os.path.lexists(self._store):
            raise CaptureError(f"STATE_UNSAFE: {self._store.name} already exists in the bench root")
        self._store.mkdir(mode=0o700)
        self._created = True
        saved = []
        for index, path in enumerate(self._live):
            present = os.path.lexists(path)
            if present:
                _copy(path, self._store / str(index))
            saved.append(present)
        self._saved = saved  # set only once complete: a failed capture restores nothing

    def restore(self) -> None:
        """Put the captured state back, replacing whatever the app left (the app must be stopped)."""
        if not self._saved:
            return
        for index, (path, present) in enumerate(zip(self._live, self._saved, strict=True)):
            _remove(path)
            if present:
                path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                _copy(self._store / str(index), path)

    def discard(self) -> None:
        """Delete the snapshot this instance created; a store it refused to touch is left alone."""
        if self._created:
            _remove(self._store)
            self._created = False


def observation_text(records: list[TrialRecord]) -> str:
    """The observation CSV: one row per trial; `seq` counts the trials of the scenario across iterations."""
    out = io.StringIO()
    writer = csv.DictWriter(out, fieldnames=OBSERVATION_COLUMNS, lineterminator="\n")
    writer.writeheader()
    for index, record in enumerate(records):
        trial = record.trial
        writer.writerow({"seq": index, "inject_ns": record.inject_ns, "render_ns": record.render_ns,
                         "commit_ns": "" if trial.commit_ns is None else trial.commit_ns,
                         "presented_ns": "" if trial.presented_ns is None else trial.presented_ns,
                         "latency_ms": "" if trial.latency_ms is None else f"{trial.latency_ms:.3f}",
                         "uncertainty_ms": f"{record.uncertainty_ns / 1e6:.3f}", "status": trial.status})
    return out.getvalue()


def write_observations(env: dict[str, str], scenario: str, records: list[TrialRecord]) -> Path:
    """Write `observations/G5-<scenario>.csv` beside the shim logs; `finalize_raw` seals it."""
    directory = Path(env["HOME"]).parent / OBSERVATIONS_DIR
    directory.mkdir(mode=0o700, exist_ok=True)
    path = directory / f"G5-{scenario}.csv"
    if os.path.lexists(path):  # rewritten after every iteration so a later failure keeps the evidence
        path.unlink()
    write_private(path, observation_text(records))
    return path
