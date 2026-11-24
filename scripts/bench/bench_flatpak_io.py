"""Flatpak listing, kill, and bounded `/proc` & `.flatpak/*/info` metadata I/O; stdlib only."""
from __future__ import annotations

import os
import stat
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import bench_guard
from bench_proc import CommandFailed, run_cancellable

PS_COLUMNS = "instance:f,pid:f,child-pid:f,application:f"
PS_TIMEOUT_S = 5.0
KILL_TIMEOUT_S = 5.0
PS_STDOUT_LIMIT = 1024 * 1024
INFO_MAX_BYTES = 64 * 1024
PROC_STAT_MAX_BYTES = 4096
ENVIRON_MAX_BYTES = 256 * 1024

_RUN_VALUE_FLAGS = frozenset({
    "--arch", "--branch", "--command", "--commit", "--cwd", "--device", "--env", "--env-fd",
    "--filesystem", "--installation", "--nofilesystem", "--nosocket", "--own-name", "--parent-pid",
    "--persist", "--runtime", "--runtime-version", "--share", "--socket", "--system-own-name",
    "--system-talk-name", "--talk-name", "--unshare",
})


@dataclass(frozen=True)
class FlatpakInstance:
    """One row from `flatpak ps --columns=instance:f,pid:f,child-pid:f,application:f`."""

    instance: str
    pid: int
    child_pid: int | None
    application: str


@dataclass(frozen=True)
class ProcInfo:
    """Kernel identity of one `/proc/<pid>` entry (`start_time` in CLOCK_BOOTTIME ticks)."""

    pid: int
    ppid: int
    start_time: int
    uid: int


@dataclass(frozen=True)
class InstanceInfo:
    """Subset of `/run/user/$UID/.flatpak/<instance>/info` `[Instance]` needed for matching."""

    instance_path: str | None
    is_subsandbox: bool


ProcInfoFn = Callable[[int], ProcInfo | None]
InstanceInfoFn = Callable[[str], InstanceInfo | None]


def is_valid_instance_id(value: str) -> bool:
    """Numeric instance ID only — `flatpak kill` also accepts an app ID, which must never happen here."""
    return bool(value) and value.isascii() and value.isdigit()


def extract_app_id(command: list[str], default: str = bench_guard.FLATPAK_APP_ID) -> str:
    """Return the application ID from a `flatpak run ... <app-id> [...]` argv."""
    seen_run = False
    skip_next = False
    after_double_dash = False
    for arg in command[1:]:
        if not seen_run:
            if arg == "run":
                seen_run = True
            continue
        if skip_next:
            skip_next = False
            continue
        if not after_double_dash:
            if arg == "--":
                after_double_dash = True
                continue
            if arg in _RUN_VALUE_FLAGS:
                skip_next = True
                continue
            if arg.startswith("-"):
                continue
        app_id = arg.partition("//")[0]
        return app_id or default
    return default


def parse_ps(text: str) -> list[FlatpakInstance]:
    """Parse tab-separated `flatpak ps` output; malformed or non-numeric rows are ignored."""
    instances: list[FlatpakInstance] = []
    for raw_line in text.splitlines():
        if not raw_line.strip():
            continue
        parts = raw_line.rstrip("\r\n").split("\t")
        if len(parts) != 4:
            continue
        instance, raw_pid, raw_child_pid, application = (part.strip() for part in parts)
        if not is_valid_instance_id(instance) or not application:
            continue
        if not (raw_pid.isascii() and raw_pid.isdigit()):
            continue
        pid = int(raw_pid)
        if pid <= 0:
            continue
        child_pid = None
        if raw_child_pid.isascii() and raw_child_pid.isdigit():
            parsed_child = int(raw_child_pid)
            if parsed_child > 0:
                child_pid = parsed_child
        instances.append(FlatpakInstance(instance, pid, child_pid, application))
    return instances


def parse_proc_stat(stat_text: str) -> tuple[int, int]:
    """Return `(ppid, start_time)` from `/proc/<pid>/stat`; raises ValueError/IndexError if invalid."""
    _, sep, tail = stat_text.rpartition(")")
    if not sep:
        raise ValueError("malformed /proc stat line")
    fields = tail.split()
    ppid, start_time = int(fields[1]), int(fields[19])
    if ppid < 0 or start_time < 0:
        raise ValueError("negative /proc stat field")
    return ppid, start_time


def _read_small_bytes(
    path: Path, limit: int, *, nofollow: bool = False, expected_uid: int | None = None
) -> bytes | None:
    flags = os.O_RDONLY | os.O_CLOEXEC | (os.O_NOFOLLOW | os.O_NONBLOCK if nofollow else 0)
    try:
        fd = os.open(path, flags)
    except OSError:
        return None
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode) or (expected_uid is not None and st.st_uid != expected_uid):
            return None
        chunks = bytearray()
        while len(chunks) <= limit:
            chunk = os.read(fd, limit + 1 - len(chunks))
            if not chunk:
                break
            chunks += chunk
        return None if len(chunks) > limit else bytes(chunks)
    except OSError:
        return None
    finally:
        os.close(fd)


def _read_small_file(
    path: Path, limit: int, *, nofollow: bool = False, expected_uid: int | None = None
) -> str | None:
    raw = _read_small_bytes(path, limit, nofollow=nofollow, expected_uid=expected_uid)
    return None if raw is None else raw.decode("utf-8", errors="replace")


def read_proc_info(pid: int) -> ProcInfo | None:
    """Read `(pid, ppid, start_time, uid)` for `pid`, or None when unreadable / reused mid-read."""
    if pid <= 0:
        return None
    try:
        uid = os.stat(f"/proc/{pid}").st_uid
    except OSError:
        return None
    stat_text = _read_small_file(Path(f"/proc/{pid}/stat"), PROC_STAT_MAX_BYTES)
    if stat_text is None:
        return None
    try:
        ppid, start_time = parse_proc_stat(stat_text)
        uid_after = os.stat(f"/proc/{pid}").st_uid
    except (OSError, ValueError, IndexError):
        return None
    return ProcInfo(pid, ppid, start_time, uid) if uid_after == uid else None


def read_instance_info(runtime_dir: Path | None, instance: str, uid: int) -> InstanceInfo | None:
    """Read `[Instance]` metadata from `$XDG_RUNTIME_DIR/.flatpak/<instance>/info` safely."""
    if runtime_dir is None or not is_valid_instance_id(instance):
        return None
    text = _read_small_file(
        runtime_dir / ".flatpak" / instance / "info", INFO_MAX_BYTES, nofollow=True, expected_uid=uid
    )
    if text is None:
        return None
    in_instance, instance_path, is_subsandbox = False, None, False
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("[") and line.endswith("]"):
            in_instance = line[1:-1].strip() == "Instance"
            continue
        if not in_instance or "=" not in line:
            continue
        key, value = (part.strip() for part in line.split("=", 1))
        if key == "instance-path" and value:
            instance_path = value
        elif key == "sandbox":
            is_subsandbox = value.lower() == "true"
    return InstanceInfo(instance_path, is_subsandbox)


def now_boot_ticks() -> int:
    """Current kernel clock in `SC_CLK_TCK` units matching `/proc/<pid>/stat` field 22."""
    return time.clock_gettime_ns(time.CLOCK_BOOTTIME) * os.sysconf("SC_CLK_TCK") // 1_000_000_000


def _read_direct_children(pid: int) -> list[int]:
    children: list[int] = []
    seen: set[int] = set()
    try:
        tids = os.listdir(f"/proc/{pid}/task")
    except OSError:
        return children
    for tid in tids[:32]:
        text = _read_small_file(Path(f"/proc/{pid}/task/{tid}/children"), INFO_MAX_BYTES)
        if not text:
            continue
        for tok in text.split():
            if tok.isascii() and tok.isdigit():
                cpid = int(tok)
                if cpid > 1 and cpid not in seen:
                    seen.add(cpid)
                    children.append(cpid)
    return children


def _host_flatpak_env(launch_env: dict[str, str] | None) -> dict[str, str]:
    env = dict(os.environ)
    for key in bench_guard.SCRUBBED:
        env.pop(key, None)
    if launch_env and (runtime_dir := launch_env.get("XDG_RUNTIME_DIR")):
        env["XDG_RUNTIME_DIR"] = runtime_dir
    return env


def _run_flatpak(
    command: list[str], env: dict[str, str], timeout: float, *, stdout_limit: int | None = None
) -> tuple[int, str] | None:
    try:
        return run_cancellable(command, timeout, None, stdout_limit=stdout_limit, step=0.1, env=env)
    except (FileNotFoundError, OSError, CommandFailed):
        return None


def list_instances(
    launch_env: dict[str, str] | None = None, *, timeout: float = PS_TIMEOUT_S
) -> list[FlatpakInstance] | None:
    """Return active Flatpak instances, or None when `flatpak ps` is unavailable or fails."""
    res = _run_flatpak(
        ["flatpak", "ps", f"--columns={PS_COLUMNS}"],
        _host_flatpak_env(launch_env),
        timeout,
        stdout_limit=PS_STDOUT_LIMIT,
    )
    return parse_ps(res[1]) if res is not None and res[0] == 0 else None


def kill_instance(
    instance: str, launch_env: dict[str, str] | None = None, *, timeout: float = KILL_TIMEOUT_S
) -> bool:
    """Run `flatpak kill <numeric-instance-id>`; non-numeric inputs are refused without spawning."""
    if not is_valid_instance_id(instance):
        return False
    res = _run_flatpak(["flatpak", "kill", instance], _host_flatpak_env(launch_env), timeout)
    return res is not None and res[0] == 0
