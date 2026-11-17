"""The one cancellable subprocess runner the bench tools share (G7 `perf`, the G8 GPU adapter); stdlib only."""
from __future__ import annotations

import os
import selectors
import signal
import subprocess
import threading
import time

CANCELLED = "cancelled"
TIMEOUT = "timeout"
OVERFLOW = "overflow"
MAX_READ_CHUNK = 64 * 1024


class CommandFailed(RuntimeError):
    """The command was killed: `kind` is `cancelled`, `timeout` or `overflow` (more stdout than allowed)."""

    def __init__(self, kind: str, command: list[str]) -> None:
        super().__init__(f"{kind}: {command[0]}")
        self.kind = kind


def _kill_group(proc: subprocess.Popen) -> None:
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    proc.wait()


def _drain_stdout(proc: subprocess.Popen, selector: selectors.BaseSelector, buffer: bytearray, limit: int,
                  step: float) -> bool:
    """Read what the child wrote for up to `step` seconds; True once it closed stdout and exited.

    Reads at most `limit + 1` bytes in total: the extra byte is how an overflow shows, and the caller
    kills the group instead of reading on.
    """
    assert proc.stdout is not None
    if selector.get_map():
        if selector.select(step):
            chunk = os.read(proc.stdout.fileno(), min(MAX_READ_CHUNK, limit + 1 - len(buffer)))
            if chunk:
                buffer += chunk
            else:
                selector.unregister(proc.stdout)
        return False
    try:
        proc.wait(timeout=step)
    except subprocess.TimeoutExpired:
        return False
    return True


def run_cancellable(command: list[str], timeout: float, cancel: threading.Event | None, *,
                    stdout_limit: int | None = None, step: float = .5) -> tuple[int, str]:
    """Run `command` without a shell in its own session and return `(returncode, text)`.

    The process group is killed on timeout or when `cancel` is set (`CommandFailed`). `text` is the child's
    stderr (stdout discarded), or, with `stdout_limit`, its stdout decoded with `errors="replace"` and
    stderr discarded. At most `stdout_limit + 1` bytes of stdout are ever read: one byte more than the limit
    kills the group (`CommandFailed("overflow")`) instead of truncating.
    """
    capture = stdout_limit is not None
    proc = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL if capture else subprocess.PIPE,
                            text=not capture, start_new_session=True)
    deadline = time.monotonic() + timeout
    buffer = bytearray()
    selector = selectors.DefaultSelector()
    try:
        if capture:
            assert proc.stdout is not None
            os.set_blocking(proc.stdout.fileno(), False)
            selector.register(proc.stdout, selectors.EVENT_READ)
        while True:
            try:
                if capture:
                    assert stdout_limit is not None
                    finished = _drain_stdout(proc, selector, buffer, stdout_limit, step)
                    if len(buffer) > stdout_limit:
                        _kill_group(proc)
                        raise CommandFailed(OVERFLOW, command)
                    if finished:
                        return proc.returncode, buffer.decode("utf-8", errors="replace")
                else:
                    _, stderr = proc.communicate(timeout=step)
                    return proc.returncode, stderr
            except subprocess.TimeoutExpired:
                pass
            cancelled = cancel is not None and cancel.is_set()
            if cancelled or time.monotonic() > deadline:
                _kill_group(proc)
                raise CommandFailed(CANCELLED if cancelled else TIMEOUT, command) from None
    finally:
        selector.close()
        for stream in (proc.stdout, proc.stderr):
            if stream is not None:
                stream.close()
