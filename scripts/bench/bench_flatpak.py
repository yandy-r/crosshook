"""Cleanup for Flatpak sandboxes and portal subsandboxes started by bench runs; stdlib only."""
from __future__ import annotations

import os
import stat
import sys
import time
from collections.abc import Callable, Iterable
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
MAX_ANCESTOR_HOPS = 64
# `flatpak kill` returns as soon as it signals the sandbox (~50 ms), while `flatpak ps`
# still lists the instance for ~1 s until processes exit and instance locks close.
CLEANUP_POLL_S = 5.0
CLEANUP_POLL_STEP_S = 0.1
REKILL_INTERVAL_S = 1.0


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


def is_valid_instance_id(value: str) -> bool:
    """Numeric instance ID only — `flatpak kill` also accepts an app ID, which must never happen here."""
    return bool(value) and value.isascii() and value.isdigit()


_RUN_VALUE_FLAGS = frozenset({
    "--arch",
    "--branch",
    "--command",
    "--commit",
    "--cwd",
    "--device",
    "--env",
    "--env-fd",
    "--filesystem",
    "--installation",
    "--nofilesystem",
    "--nosocket",
    "--own-name",
    "--parent-pid",
    "--persist",
    "--runtime",
    "--runtime-version",
    "--share",
    "--socket",
    "--system-own-name",
    "--system-talk-name",
    "--talk-name",
    "--unshare",
    "--user",
    "-a",
    "-p",
})


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
    ppid = int(fields[1])
    start_time = int(fields[19])
    if ppid < 0 or start_time < 0:
        raise ValueError("negative /proc stat field")
    return ppid, start_time


def _read_small_file(path: Path, limit: int, *, nofollow: bool = False, expected_uid: int | None = None) -> str | None:
    flags = os.O_RDONLY | os.O_CLOEXEC
    if nofollow:
        flags |= os.O_NOFOLLOW | os.O_NONBLOCK
    try:
        fd = os.open(path, flags)
    except OSError:
        return None
    try:
        st = os.fstat(fd)
        if not stat.S_ISREG(st.st_mode):
            return None
        if expected_uid is not None and st.st_uid != expected_uid:
            return None
        chunks = bytearray()
        while len(chunks) <= limit:
            chunk = os.read(fd, limit + 1 - len(chunks))
            if not chunk:
                break
            chunks += chunk
        if len(chunks) > limit:
            return None
        return chunks.decode("utf-8", errors="replace")
    except OSError:
        return None
    finally:
        os.close(fd)


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
    if uid_after != uid:
        return None
    return ProcInfo(pid, ppid, start_time, uid)


def read_instance_info(runtime_dir: Path | None, instance: str, uid: int) -> InstanceInfo | None:
    """Read `[Instance]` metadata from `$XDG_RUNTIME_DIR/.flatpak/<instance>/info` safely."""
    if runtime_dir is None or not is_valid_instance_id(instance):
        return None
    text = _read_small_file(
        runtime_dir / ".flatpak" / instance / "info",
        INFO_MAX_BYTES,
        nofollow=True,
        expected_uid=uid,
    )
    if text is None:
        return None
    in_instance = False
    instance_path: str | None = None
    is_subsandbox = False
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


def _host_flatpak_env(launch_env: dict[str, str] | None) -> dict[str, str]:
    env = dict(os.environ)
    for key in bench_guard.SCRUBBED:
        env.pop(key, None)
    if launch_env:
        runtime_dir = launch_env.get("XDG_RUNTIME_DIR")
        if runtime_dir:
            env["XDG_RUNTIME_DIR"] = runtime_dir
    return env


def _run_flatpak(
    command: list[str],
    env: dict[str, str],
    timeout: float,
    *,
    stdout_limit: int | None = None,
) -> tuple[int, str] | None:
    try:
        return run_cancellable(command, timeout, None, stdout_limit=stdout_limit, step=0.1, env=env)
    except (FileNotFoundError, OSError, CommandFailed):
        return None


def list_instances(launch_env: dict[str, str] | None = None, *, timeout: float = PS_TIMEOUT_S) -> list[FlatpakInstance] | None:
    """Return active Flatpak instances, or None when `flatpak ps` is unavailable or fails."""
    env = _host_flatpak_env(launch_env)
    res = _run_flatpak(["flatpak", "ps", f"--columns={PS_COLUMNS}"], env, timeout, stdout_limit=PS_STDOUT_LIMIT)
    if res is None:
        return None
    returncode, stdout = res
    return parse_ps(stdout) if returncode == 0 else None


def kill_instance(instance: str, launch_env: dict[str, str] | None = None, *, timeout: float = KILL_TIMEOUT_S) -> bool:
    """Run `flatpak kill <numeric-instance-id>`; non-numeric inputs are refused without spawning."""
    if not is_valid_instance_id(instance):
        return False
    env = _host_flatpak_env(launch_env)
    res = _run_flatpak(["flatpak", "kill", instance], env, timeout)
    return res is not None and res[0] == 0


def _instance_proc_info(
    inst: FlatpakInstance,
    uid: int,
    proc_info_fn: Callable[[int], ProcInfo | None],
) -> ProcInfo | None:
    for candidate in (inst.pid, inst.child_pid):
        if candidate is None:
            continue
        info = proc_info_fn(candidate)
        if info is not None:
            return info if info.uid == uid else None
    return None


def _has_ancestor(
    pid: int,
    target_pid: int,
    target_start_ticks: int,
    uid: int,
    proc_info_fn: Callable[[int], ProcInfo | None],
) -> bool:
    current: int | None = pid
    seen: set[int] = set()
    for _ in range(MAX_ANCESTOR_HOPS):
        if current is None or current <= 1 or current in seen:
            return False
        seen.add(current)
        info = proc_info_fn(current)
        if info is None or info.uid != uid:
            return False
        if current == target_pid:
            return info.start_time == target_start_ticks
        if info.ppid == target_pid:
            parent_info = proc_info_fn(target_pid)
            if parent_info is not None:
                return parent_info.uid == uid and parent_info.start_time == target_start_ticks
            return True
        current = info.ppid
    return False


def _path_within(child: str, root: Path) -> bool:
    if not os.path.isabs(child):
        return False
    try:
        child_real = Path(os.path.realpath(child))
        root_real = Path(os.path.realpath(root))
    except OSError:
        return False
    return child_real == root_real or root_real in child_real.parents


def _partition_run_instances(
    instances: Iterable[FlatpakInstance],
    *,
    app_id: str,
    baseline_instances: frozenset[str],
    baseline_known: bool,
    launch_pid: int | None,
    launch_start_ticks: int,
    tracked_tree_pids: frozenset[int] = frozenset(),
    isolated_root: Path | None = None,
    already_targeted: frozenset[str] = frozenset(),
    uid: int | None = None,
    proc_info_fn: Callable[[int], ProcInfo | None] = read_proc_info,
    instance_info_fn: Callable[[str], InstanceInfo | None] | None = None,
) -> tuple[list[FlatpakInstance], list[FlatpakInstance]]:
    """Split `instances` into `(to_kill, draining)` belonging to this bench launch/run.

    `to_kill` are live instances proved to belong to this run. `draining` are run-owned
    instances whose wrapper and child PIDs have already exited (`proc_info` is None) while
    `flatpak ps` is still tearing down their runtime lock (~1-2 s after exit/kill).
    """
    owner_uid = os.getuid() if uid is None else uid
    to_kill: list[FlatpakInstance] = []
    draining: list[FlatpakInstance] = []
    for inst in instances:
        if inst.application != app_id or not is_valid_instance_id(inst.instance):
            continue
        if inst.instance in baseline_instances:
            continue

        inst_info = instance_info_fn(inst.instance) if instance_info_fn is not None else None
        path_inside_root = False
        if isolated_root is not None and inst_info is not None and inst_info.instance_path:
            if _path_within(inst_info.instance_path, isolated_root):
                path_inside_root = True
            elif not inst_info.is_subsandbox:
                continue

        in_tracked_pids = bool(
            (launch_pid is not None and (inst.pid == launch_pid or inst.child_pid == launch_pid))
            or (
                tracked_tree_pids
                and (
                    inst.pid in tracked_tree_pids
                    or (inst.child_pid is not None and inst.child_pid in tracked_tree_pids)
                )
            )
        )

        proc_info = _instance_proc_info(inst, owner_uid, proc_info_fn)
        if proc_info is None:
            if inst.instance in already_targeted or path_inside_root or in_tracked_pids:
                draining.append(inst)
            continue

        started_since_launch = proc_info.start_time >= launch_start_ticks
        in_tree = in_tracked_pids and started_since_launch
        via_parent = bool(
            launch_pid is not None
            and started_since_launch
            and any(
                _has_ancestor(candidate, launch_pid, launch_start_ticks, owner_uid, proc_info_fn)
                for candidate in (inst.pid, inst.child_pid)
                if candidate is not None
            )
        )
        ancestry_match = in_tree or via_parent
        if (path_inside_root and started_since_launch) or ancestry_match or (
            baseline_known and started_since_launch
        ):
            to_kill.append(inst)
    return to_kill, draining


def select_instances_to_kill(
    instances: Iterable[FlatpakInstance],
    *,
    app_id: str,
    baseline_instances: frozenset[str],
    baseline_known: bool,
    launch_pid: int | None,
    launch_start_ticks: int,
    tracked_tree_pids: frozenset[int] = frozenset(),
    isolated_root: Path | None = None,
    uid: int | None = None,
    proc_info_fn: Callable[[int], ProcInfo | None] = read_proc_info,
    instance_info_fn: Callable[[str], InstanceInfo | None] | None = None,
) -> list[FlatpakInstance]:
    """Select only live Flatpak instances proved to belong to this bench launch/run.

    Matching layers:
    1. Exact `app_id`, numeric instance ID, same UID, and never in `baseline_instances`.
    2. When `instance-path` is present (main sandboxes; portal `sandbox=true` subsandboxes
       omit `instance-path`): paths outside `isolated_root` are hard-excluded so a concurrent
       user CrossHook instance is never killed, while paths inside `isolated_root` match
       directly once start time >= launch_start_ticks (or ancestry matches).
    3. Direct PID ancestry (pre-termination `tracked_tree_pids` or live `ppid` chain to
       `launch_pid`) with `start_time >= launch_start_ticks`.
    4. Reparented portal subsandboxes (e.g. `glycin-svg`): when `baseline_known` is True,
       instances absent from the pre-launch baseline whose process started at or after
       `launch_start_ticks`.
    """
    to_kill, _ = _partition_run_instances(
        instances,
        app_id=app_id,
        baseline_instances=baseline_instances,
        baseline_known=baseline_known,
        launch_pid=launch_pid,
        launch_start_ticks=launch_start_ticks,
        tracked_tree_pids=tracked_tree_pids,
        isolated_root=isolated_root,
        uid=uid,
        proc_info_fn=proc_info_fn,
        instance_info_fn=instance_info_fn,
    )
    return to_kill


class FlatpakSandboxScope:
    """Tracks pre-launch Flatpak baseline and cleans up run-started sandboxes/subsandboxes."""

    def __init__(
        self,
        *,
        app_id: str,
        launch_env: dict[str, str],
        isolated_root: Path | None,
        baseline_instances: frozenset[str],
        baseline_known: bool,
        launch_start_ticks: int,
        uid: int | None = None,
        list_fn: Callable[[], list[FlatpakInstance] | None] | None = None,
        kill_fn: Callable[[str], bool] | None = None,
        proc_info_fn: Callable[[int], ProcInfo | None] = read_proc_info,
        instance_info_fn: Callable[[str], InstanceInfo | None] | None = None,
        sleep_fn: Callable[[float], None] = time.sleep,
        monotonic_fn: Callable[[], float] = time.monotonic,
    ) -> None:
        self.app_id = app_id
        self.launch_env = dict(launch_env)
        self.isolated_root = isolated_root
        self.baseline_instances = baseline_instances
        self.baseline_known = baseline_known
        self.launch_start_ticks = launch_start_ticks
        self.launch_pid: int | None = None
        self.uid = os.getuid() if uid is None else uid
        runtime_str = self.launch_env.get("XDG_RUNTIME_DIR") or os.environ.get("XDG_RUNTIME_DIR")
        self.runtime_dir = Path(runtime_str) if runtime_str else None
        self._list_fn = list_fn or (lambda: list_instances(self.launch_env))
        self._kill_fn = kill_fn or (lambda inst_id: kill_instance(inst_id, self.launch_env))
        self._proc_info_fn = proc_info_fn
        self._instance_info_fn = instance_info_fn or (
            lambda inst_id: read_instance_info(self.runtime_dir, inst_id, self.uid)
        )
        self._sleep_fn = sleep_fn
        self._monotonic_fn = monotonic_fn

    @classmethod
    def capture_before_launch(
        cls,
        app_id: str,
        launch_env: dict[str, str],
        *,
        isolated_root: Path | None = None,
        uid: int | None = None,
        list_fn: Callable[[], list[FlatpakInstance] | None] | None = None,
        kill_fn: Callable[[str], bool] | None = None,
        proc_info_fn: Callable[[int], ProcInfo | None] = read_proc_info,
        instance_info_fn: Callable[[str], InstanceInfo | None] | None = None,
        clock_ticks_fn: Callable[[], int] = now_boot_ticks,
        sleep_fn: Callable[[float], None] = time.sleep,
        monotonic_fn: Callable[[], float] = time.monotonic,
    ) -> FlatpakSandboxScope:
        """Snapshot running `app_id` instances and boot ticks before launching Flatpak."""
        owner_uid = os.getuid() if uid is None else uid
        effective_list = list_fn or (lambda: list_instances(launch_env))
        listed = effective_list()
        baseline_known = listed is not None
        baseline = frozenset(
            inst.instance
            for inst in (listed or ())
            if inst.application == app_id and is_valid_instance_id(inst.instance)
        )
        return cls(
            app_id=app_id,
            launch_env=launch_env,
            isolated_root=isolated_root,
            baseline_instances=baseline,
            baseline_known=baseline_known,
            launch_start_ticks=clock_ticks_fn(),
            uid=owner_uid,
            list_fn=effective_list,
            kill_fn=kill_fn,
            proc_info_fn=proc_info_fn,
            instance_info_fn=instance_info_fn,
            sleep_fn=sleep_fn,
            monotonic_fn=monotonic_fn,
        )

    def bind_launch(self, pid: int) -> None:
        """Attach the spawned `flatpak run` PID and refine `launch_start_ticks` from `/proc`."""
        self.launch_pid = pid
        info = self._proc_info_fn(pid)
        if info is not None and info.uid == self.uid:
            self.launch_start_ticks = info.start_time

    def cleanup(
        self,
        *,
        tree_pids: Iterable[int] = (),
        wait_timeout: float = CLEANUP_POLL_S,
        poll_step: float = CLEANUP_POLL_STEP_S,
        rekill_interval: float = REKILL_INTERVAL_S,
    ) -> list[str]:
        """Kill any run-started Flatpak sandboxes/subsandboxes and wait for `flatpak ps` to clear."""
        tracked = frozenset(tree_pids)
        targeted: list[str] = []
        targeted_set: set[str] = set()
        last_kill_at: dict[str, float] = {}
        deadline = self._monotonic_fn() + max(0.0, wait_timeout)
        active: list[FlatpakInstance] = []
        ps_failed = False

        while True:
            listed = self._list_fn()
            now = self._monotonic_fn()
            if listed is None:
                if not active and not self.baseline_known:
                    return targeted
                ps_failed = True
                if now >= deadline:
                    break
                self._sleep_fn(min(poll_step, max(0.0, deadline - now)))
                continue
            ps_failed = False
            to_kill, draining = _partition_run_instances(
                listed,
                app_id=self.app_id,
                baseline_instances=self.baseline_instances,
                baseline_known=self.baseline_known,
                launch_pid=self.launch_pid,
                launch_start_ticks=self.launch_start_ticks,
                tracked_tree_pids=tracked,
                isolated_root=self.isolated_root,
                already_targeted=frozenset(targeted_set),
                uid=self.uid,
                proc_info_fn=self._proc_info_fn,
                instance_info_fn=self._instance_info_fn,
            )
            active = [*to_kill, *draining]
            if not active:
                return targeted
            # If the deadline already passed after at least one kill pass, avoid an un-waited extra kill.
            if now < deadline or not last_kill_at:
                for inst in to_kill:
                    previous = last_kill_at.get(inst.instance)
                    if previous is None or (now - previous) >= rekill_interval:
                        if inst.instance not in targeted_set:
                            targeted.append(inst.instance)
                            targeted_set.add(inst.instance)
                        last_kill_at[inst.instance] = now
                        self._kill_fn(inst.instance)
            if now >= deadline:
                break
            self._sleep_fn(min(poll_step, max(0.0, deadline - now)))

        if active:
            ids = ", ".join(inst.instance for inst in active)
            print(f"FLATPAK_CLEANUP_INCOMPLETE: remaining instances: {ids}", file=sys.stderr)
        elif ps_failed:
            print("FLATPAK_CLEANUP_INCOMPLETE: flatpak ps failed during cleanup", file=sys.stderr)
        return targeted
