"""Ownership selection and cleanup orchestration for Flatpak sandboxes/subsandboxes; stdlib only."""
from __future__ import annotations

import os
import sys
import time
from collections.abc import Callable, Iterable
from pathlib import Path

from bench_flatpak_io import (
    ENVIRON_MAX_BYTES,
    INFO_MAX_BYTES,
    KILL_TIMEOUT_S,
    PROC_STAT_MAX_BYTES,
    PS_COLUMNS,
    PS_STDOUT_LIMIT,
    PS_TIMEOUT_S,
    FlatpakInstance,
    InstanceInfo,
    InstanceInfoFn,
    ProcInfo,
    ProcInfoFn,
    _read_direct_children,
    _read_small_bytes,
    extract_app_id,
    is_valid_instance_id,
    kill_instance,
    list_instances,
    now_boot_ticks,
    parse_proc_stat,
    parse_ps,
    read_instance_info,
    read_proc_info,
)

MAX_ANCESTOR_HOPS = 64
# `flatpak kill` returns as soon as it signals the sandbox (~50 ms), while `flatpak ps`
# still lists the instance for ~1 s until processes exit and instance locks close.
CLEANUP_POLL_S = 5.0
CLEANUP_POLL_STEP_S = 0.1
REKILL_INTERVAL_S = 1.0
_ISOLATED_ENV_KEYS = (b"CROSSHOOK_BENCH_ROOT", b"FLATPAK_SANDBOX_DIR", b"HOME")
SubsandboxEnvFn = Callable[[FlatpakInstance, Path, int, int, ProcInfoFn], bool]

__all__ = [
    "CLEANUP_POLL_S",
    "CLEANUP_POLL_STEP_S",
    "ENVIRON_MAX_BYTES",
    "FlatpakInstance",
    "FlatpakSandboxScope",
    "INFO_MAX_BYTES",
    "InstanceInfo",
    "InstanceInfoFn",
    "KILL_TIMEOUT_S",
    "MAX_ANCESTOR_HOPS",
    "PROC_STAT_MAX_BYTES",
    "PS_COLUMNS",
    "PS_STDOUT_LIMIT",
    "PS_TIMEOUT_S",
    "ProcInfo",
    "ProcInfoFn",
    "REKILL_INTERVAL_S",
    "SubsandboxEnvFn",
    "extract_app_id",
    "is_valid_instance_id",
    "kill_instance",
    "list_instances",
    "now_boot_ticks",
    "parse_proc_stat",
    "parse_ps",
    "read_instance_info",
    "read_proc_info",
    "select_instances_to_kill",
]


def _instance_proc_info(
    inst: FlatpakInstance, uid: int, proc_info_fn: ProcInfoFn
) -> ProcInfo | None:
    for candidate in (inst.pid, inst.child_pid):
        if candidate is None:
            continue
        info = proc_info_fn(candidate)
        if info is not None:
            return info if info.uid == uid else None
    return None


def _has_ancestor(
    pid: int, target_pid: int, target_start_ticks: int, uid: int, proc_info_fn: ProcInfoFn
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


def _subsandbox_env_in_root(
    inst: FlatpakInstance, isolated_root: Path, uid: int, launch_start_ticks: int, proc_info_fn: ProcInfoFn
) -> bool:
    """Return True if a readable child process of `inst` carries isolated-root env vars."""
    try:
        root_real = Path(os.path.realpath(isolated_root))
    except OSError:
        return False
    candidates, seen = [], set()
    for root_pid in (inst.child_pid, inst.pid):
        if root_pid is None:
            continue
        root_info = proc_info_fn(root_pid)
        if root_info is None or root_info.uid != uid or root_info.start_time < launch_start_ticks:
            continue
        for pid in (root_pid, *_read_direct_children(root_pid)):
            if pid not in seen and len(candidates) < 16:
                seen.add(pid)
                candidates.append(pid)
    for cpid in candidates:
        before = proc_info_fn(cpid)
        if before is None or before.uid != uid or before.start_time < launch_start_ticks:
            continue
        raw = _read_small_bytes(Path(f"/proc/{cpid}/environ"), ENVIRON_MAX_BYTES, nofollow=True, expected_uid=uid)
        if not raw:
            continue
        after = proc_info_fn(cpid)
        if after is None or after.uid != uid or after.start_time != before.start_time:
            continue
        for entry in raw.split(b"\x00"):
            key, sep, val = entry.partition(b"=")
            if sep and key in _ISOLATED_ENV_KEYS and _path_within(os.fsdecode(val), root_real):
                return True
    return False


def _partition_run_instances(
    instances: Iterable[FlatpakInstance], *, app_id: str, baseline_instances: frozenset[str],
    baseline_known: bool, launch_pid: int | None, launch_start_ticks: int,
    tracked_tree_pids: frozenset[int] = frozenset(), isolated_root: Path | None = None,
    already_targeted: frozenset[str] = frozenset(), known_owned: frozenset[str] = frozenset(),
    saw_foreign: bool = False, uid: int | None = None, proc_info_fn: ProcInfoFn = read_proc_info,
    instance_info_fn: InstanceInfoFn | None = None,
    subsandbox_in_root_fn: SubsandboxEnvFn = _subsandbox_env_in_root,
) -> tuple[list[FlatpakInstance], list[FlatpakInstance], bool]:
    """Split `instances` into `(to_kill, draining, has_foreign)` for this bench launch/run.

    `to_kill` are live instances proved to belong to this run. `draining` are run-owned
    instances whose wrapper and child PIDs have already exited (`proc_info` is None) while
    `flatpak ps` is still tearing down their runtime lock (~1-2 s after exit/kill).
    """
    owner_uid = os.getuid() if uid is None else uid
    app_rows = [i for i in instances if i.application == app_id and is_valid_instance_id(i.instance)]
    infos = {i.instance: (instance_info_fn(i.instance) if instance_info_fn else None) for i in app_rows}
    has_foreign = saw_foreign or bool(baseline_instances) or any(
        i.instance not in baseline_instances and isolated_root is not None
        and (info := infos.get(i.instance)) is not None and bool(info.instance_path)
        and not _path_within(info.instance_path, isolated_root) and not info.is_subsandbox
        for i in app_rows
    )
    to_kill, draining = [], []
    for inst in app_rows:
        if inst.instance in baseline_instances:
            continue
        inst_info = infos.get(inst.instance)
        path_inside_root = False
        if isolated_root is not None and inst_info is not None and inst_info.instance_path:
            if _path_within(inst_info.instance_path, isolated_root):
                path_inside_root = True
            elif not inst_info.is_subsandbox:
                continue
        in_tracked_pids = bool(
            (launch_pid is not None and (inst.pid == launch_pid or inst.child_pid == launch_pid))
            or (tracked_tree_pids and (
                inst.pid in tracked_tree_pids
                or (inst.child_pid is not None and inst.child_pid in tracked_tree_pids)
            ))
        )
        proc_info = _instance_proc_info(inst, owner_uid, proc_info_fn)
        if proc_info is None:
            if (
                inst.instance in already_targeted
                or inst.instance in known_owned
                or path_inside_root
                or in_tracked_pids
                or (baseline_known and not has_foreign)
            ):
                draining.append(inst)
            continue
        started_since_launch = proc_info.start_time >= launch_start_ticks
        via_parent = bool(
            launch_pid is not None and started_since_launch and any(
                _has_ancestor(cand, launch_pid, launch_start_ticks, owner_uid, proc_info_fn)
                for cand in (inst.pid, inst.child_pid) if cand is not None
            )
        )
        ancestry_match = (in_tracked_pids and started_since_launch) or via_parent
        subsandbox_env_match = bool(
            isolated_root is not None and started_since_launch
            and (inst_info is None or inst_info.is_subsandbox or not inst_info.instance_path)
            and subsandbox_in_root_fn(inst, isolated_root, owner_uid, launch_start_ticks, proc_info_fn)
        )
        isolated_fallback = baseline_known and not has_foreign and started_since_launch
        if (path_inside_root and started_since_launch) or ancestry_match or subsandbox_env_match or isolated_fallback:
            to_kill.append(inst)
    return to_kill, draining, has_foreign


def select_instances_to_kill(
    instances: Iterable[FlatpakInstance], *, app_id: str, baseline_instances: frozenset[str],
    baseline_known: bool, launch_pid: int | None, launch_start_ticks: int,
    tracked_tree_pids: frozenset[int] = frozenset(), isolated_root: Path | None = None,
    uid: int | None = None, proc_info_fn: ProcInfoFn = read_proc_info,
    instance_info_fn: InstanceInfoFn | None = None,
    subsandbox_in_root_fn: SubsandboxEnvFn = _subsandbox_env_in_root,
) -> list[FlatpakInstance]:
    """Select only live Flatpak instances proved to belong to this bench launch/run.

    Fails closed on unattributed subsandboxes whenever a baseline or concurrent non-isolated
    `app_id` instance exists, unless positive proof matches (`isolated_root` `instance-path`,
    PID ancestry, or isolated child environment).
    """
    to_kill, _, _ = _partition_run_instances(
        instances, app_id=app_id, baseline_instances=baseline_instances,
        baseline_known=baseline_known, launch_pid=launch_pid,
        launch_start_ticks=launch_start_ticks, tracked_tree_pids=tracked_tree_pids,
        isolated_root=isolated_root, uid=uid, proc_info_fn=proc_info_fn,
        instance_info_fn=instance_info_fn, subsandbox_in_root_fn=subsandbox_in_root_fn,
    )
    return to_kill


class FlatpakSandboxScope:
    """Tracks pre-launch Flatpak baseline and cleans up run-started sandboxes/subsandboxes."""

    def __init__(
        self, *, app_id: str, launch_env: dict[str, str], isolated_root: Path | None,
        baseline_instances: frozenset[str], baseline_known: bool, launch_start_ticks: int,
        uid: int | None = None, list_fn: Callable[[], list[FlatpakInstance] | None] | None = None,
        kill_fn: Callable[[str], bool] | None = None, proc_info_fn: ProcInfoFn = read_proc_info,
        instance_info_fn: InstanceInfoFn | None = None,
        subsandbox_in_root_fn: SubsandboxEnvFn = _subsandbox_env_in_root,
        sleep_fn: Callable[[float], None] = time.sleep,
        monotonic_fn: Callable[[], float] = time.monotonic,
    ) -> None:
        self.app_id, self.launch_env = app_id, dict(launch_env)
        self.isolated_root = isolated_root
        self.baseline_instances, self.baseline_known = baseline_instances, baseline_known
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
        self._subsandbox_in_root_fn = subsandbox_in_root_fn
        self._sleep_fn, self._monotonic_fn = sleep_fn, monotonic_fn
        self._saw_foreign = bool(baseline_instances)
        self._owned_instances: set[str] = set()

    @classmethod
    def capture_before_launch(
        cls, app_id: str, launch_env: dict[str, str], *, isolated_root: Path | None = None,
        uid: int | None = None, list_fn: Callable[[], list[FlatpakInstance] | None] | None = None,
        kill_fn: Callable[[str], bool] | None = None, proc_info_fn: ProcInfoFn = read_proc_info,
        instance_info_fn: InstanceInfoFn | None = None,
        subsandbox_in_root_fn: SubsandboxEnvFn = _subsandbox_env_in_root,
        clock_ticks_fn: Callable[[], int] = now_boot_ticks,
        sleep_fn: Callable[[float], None] = time.sleep,
        monotonic_fn: Callable[[], float] = time.monotonic,
    ) -> FlatpakSandboxScope:
        """Snapshot running `app_id` instances and boot ticks before launching Flatpak."""
        owner_uid = os.getuid() if uid is None else uid
        effective_list = list_fn or (lambda: list_instances(launch_env))
        listed = effective_list()
        baseline = frozenset(
            inst.instance for inst in (listed or ())
            if inst.application == app_id and is_valid_instance_id(inst.instance)
        )
        return cls(
            app_id=app_id, launch_env=launch_env, isolated_root=isolated_root,
            baseline_instances=baseline, baseline_known=listed is not None,
            launch_start_ticks=clock_ticks_fn(), uid=owner_uid, list_fn=effective_list,
            kill_fn=kill_fn, proc_info_fn=proc_info_fn, instance_info_fn=instance_info_fn,
            subsandbox_in_root_fn=subsandbox_in_root_fn, sleep_fn=sleep_fn, monotonic_fn=monotonic_fn,
        )

    def bind_launch(self, pid: int) -> None:
        """Attach the spawned `flatpak run` PID and refine `launch_start_ticks` from `/proc`."""
        self.launch_pid = pid
        info = self._proc_info_fn(pid)
        if info is not None and info.uid == self.uid:
            self.launch_start_ticks = info.start_time

    def observe_live(self, *, tree_pids: Iterable[int] = ()) -> frozenset[str]:
        """Record positively attributed live instance IDs before teardown kills their processes."""
        listed = self._list_fn()
        if listed is None:
            return frozenset(self._owned_instances)
        to_kill, _, self._saw_foreign = _partition_run_instances(
            listed, app_id=self.app_id, baseline_instances=self.baseline_instances,
            baseline_known=self.baseline_known, launch_pid=self.launch_pid,
            launch_start_ticks=self.launch_start_ticks, tracked_tree_pids=frozenset(tree_pids),
            isolated_root=self.isolated_root, known_owned=frozenset(self._owned_instances),
            saw_foreign=self._saw_foreign, uid=self.uid, proc_info_fn=self._proc_info_fn,
            instance_info_fn=self._instance_info_fn, subsandbox_in_root_fn=self._subsandbox_in_root_fn,
        )
        self._owned_instances.update(inst.instance for inst in to_kill)
        return frozenset(self._owned_instances)

    def cleanup(
        self, *, tree_pids: Iterable[int] = (), wait_timeout: float = CLEANUP_POLL_S,
        poll_step: float = CLEANUP_POLL_STEP_S, rekill_interval: float = REKILL_INTERVAL_S,
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
            to_kill, draining, self._saw_foreign = _partition_run_instances(
                listed, app_id=self.app_id, baseline_instances=self.baseline_instances,
                baseline_known=self.baseline_known, launch_pid=self.launch_pid,
                launch_start_ticks=self.launch_start_ticks, tracked_tree_pids=tracked,
                isolated_root=self.isolated_root, already_targeted=frozenset(targeted_set),
                known_owned=frozenset(self._owned_instances), saw_foreign=self._saw_foreign,
                uid=self.uid, proc_info_fn=self._proc_info_fn,
                instance_info_fn=self._instance_info_fn, subsandbox_in_root_fn=self._subsandbox_in_root_fn,
            )
            self._owned_instances.update(inst.instance for inst in to_kill)
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
