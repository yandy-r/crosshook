"""G4 (scroll) and G9 (resize) input drivers: uinput wheel/keys and KWin window control.

Input is only sent while the wlpresent shim proves the app window holds the matching focus:
keys need the keyboard `K` enter on the toplevel, wheel notches need the pointer `M` enter.
The caller owns the InputDevice and the KWinScripts; nothing here closes them. Bounds are
CLOCK_MONOTONIC ns, the same clock as the shim and `InputDevice.wheel`.

Witnesses run before every input event: the app exit check, and keyboard or pointer focus read
from the shim log by `FocusTail`, which parses only what the shim appended since the last read.
`session_locked()` spawns processes, so it runs at most every LOCK_CHECK_S of wall time.
"""
from __future__ import annotations

import os
import stat
import sys
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

import bench_guard
from bench_capture import CaptureError, process_tree
from bench_input import KEY_ENTER, KEY_K, KEY_LEFTCTRL, InputDevice
from bench_kwin import KWinScripts, find_window, input_device_sysnames
from bench_present import (
    FIELD_COUNTS,
    FOCUS_DEVICES,
    FOCUS_EVENTS,
    LOG_GLOB,
    MAX_LOG_BYTES,
    Focus,
    PresentLog,
    distinct_sizes,
    parse_log,
    toplevel_frames,
)
from bench_provenance import write_private
from bench_session import locked_error, session_locked

NOTICE = "BENCH_NOTICE: injecting input into the bench window; do not touch the keyboard or mouse"
SCENARIOS = ("library", "console", "settings")
WINDOW_W, WINDOW_H = 1280, 900
CURSOR_OFFSET = {"library": (640, 450), "settings": (640, 450), "console": (640, 780)}
SCROLL_NOTCHES = 90          # per direction; 90 notches at 33 ms is about 3 s
IDLE_S = 1.0                 # console: no presented frame for this long means the replay is done
IDLE_TIMEOUT_S = 25.0        # 50k lines at the 5000 lines/s cap take about 10 s
SCROLL_LEG = 10              # alternate 10 down / 10 up so short pages never hit their end and go idle
NOTCH_S = 0.033
TAIL_NS = 300_000_000
FOCUS_TIMEOUT_S = 1.0
FOCUS_POLL_S = 0.05
LOCK_CHECK_S = 0.3
DEVICE_SETTLE_S = 1.5
DEVICE_POLL_S = 0.1
PALETTE_S, TYPED_S, ENTER_S = 0.3, 0.2, 1.0
RESIZE_S = 5.0
RESIZE_HZ = 30
# Both stay strictly inside the 1100 px and 1440 px width breakpoints and above the 720 px height one.
RESIZE_W = (1120, 1400)
RESIZE_H = (780, 880)
MIN_DISTINCT_SIZES = 10
CONSOLE_SETTINGS = "console_drawer_collapsed_default = false\n"
CONSOLE_LOG_RATE = "5000"
LOG_ENV = ("CROSSHOOK_BENCH_LOG", "CROSSHOOK_BENCH_LOG_RATE")

_noticed = False


@dataclass(frozen=True)
class DriveResult:
    start_ns: int
    end_ns: int
    injected: int  # wheel notches or resize steps sent inside the window
    driven_ns: int = 0  # first to last injected event, tail excluded


def _notice() -> None:
    global _noticed
    if not _noticed:
        _noticed = True
        print(NOTICE, file=sys.stderr, flush=True)


def _settings_path(env: dict[str, str], flatpak: bool) -> Path:
    try:
        config = (Path(env["HOME"]) / ".var/app" / bench_guard.FLATPAK_APP_ID / "config" if flatpak
                  else Path(env["XDG_CONFIG_HOME"])) / "crosshook"
    except KeyError as error:
        raise CaptureError(f"BENCH_ENV_INCOMPLETE: {error.args[0]} is not set") from error
    return config / "settings.toml"


def _remove_settings(path: Path) -> None:
    """Delete a previous scenario's settings file; refuse anything that is not a regular file."""
    try:
        mode = os.lstat(path).st_mode
    except FileNotFoundError:
        return
    if not stat.S_ISREG(mode):
        raise CaptureError(f"SETTINGS_UNSAFE: {path.name} is not a regular file; remove it before the run")
    path.unlink()


def prepare_scenario(scenario: str, env: dict[str, str], flatpak: bool) -> dict[str, str]:
    """Return `env` adjusted for `scenario`.

    Every scenario shares one isolated root, so each starts from no `settings.toml`; `console`
    then writes the one setting it needs.
    """
    if scenario not in SCENARIOS:
        raise CaptureError(f"UNKNOWN_SCENARIO: {scenario[:40]!r}")
    updated = {key: value for key, value in env.items() if key not in LOG_ENV}
    path = _settings_path(env, flatpak)
    _remove_settings(path)  # a console iteration's file must not leak into the next scenario
    if scenario != "console":
        return updated
    root = env.get("CROSSHOOK_BENCH_ROOT")
    if root is None:
        raise CaptureError("BENCH_ENV_INCOMPLETE: CROSSHOOK_BENCH_ROOT is not set")
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    write_private(path, CONSOLE_SETTINGS)
    updated["CROSSHOOK_BENCH_LOG"] = str(Path(root) / "logs/launch-50k.log")
    updated["CROSSHOOK_BENCH_LOG_RATE"] = CONSOLE_LOG_RATE
    return updated


def settle_device(dev: InputDevice, clock: Callable[[], int] = time.monotonic_ns,
                  sleep: Callable[[float], None] = time.sleep) -> None:
    """Wait until KWin lists the new device; KWin drops events sent before it has opened it."""
    node = f"'{dev.event_node()}'"
    deadline = clock() + int(DEVICE_SETTLE_S * 1e9)
    while node not in input_device_sysnames():
        if clock() >= deadline:
            raise CaptureError("INPUT_NOT_DELIVERED: KWin did not register the bench input device")
        sleep(DEVICE_POLL_S)


def _live_logs(log_dir: Path) -> list[PresentLog]:
    return [parse_log(path) for path in sorted(Path(log_dir).glob(LOG_GLOB))]


@dataclass
class _Cursor:
    offset: int = 0
    lines: int = 0
    roles: dict[int, str] = field(default_factory=dict)


class FocusTail:
    """Keyboard and pointer focus from the shim logs, reading only what was appended since the last call.

    Parses just the `S` role and `K`/`M` focus records, with the same strictness and the same
    "toplevel at event time" rule as `parse_log`; the full parse happens once the app has stopped.
    """

    def __init__(self, log_dir: Path):
        self._dir = Path(log_dir)
        self._cursors: dict[str, _Cursor] = {}
        self._latest: dict[str, Focus] = {}

    def focused(self, device: str) -> bool:
        """True when the latest `device` focus event, across every log, is an enter on a toplevel."""
        for path in sorted(self._dir.glob(LOG_GLOB)):
            self._read_new(path, self._cursors.setdefault(path.name, _Cursor()))
        event = self._latest.get(device)
        return event is not None and event.entered and event.toplevel

    def _read_new(self, path: Path, cursor: _Cursor) -> None:
        try:
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
        except OSError as error:
            raise CaptureError(f"PRESENT_LOG_UNREADABLE: {path.name}: {error.strerror}") from error
        try:
            size = os.fstat(fd).st_size
            if size > MAX_LOG_BYTES:
                raise CaptureError(f"PRESENT_LOG_TOO_LARGE: {path.name}")
            data = os.pread(fd, size - cursor.offset, cursor.offset) if size > cursor.offset else b""
        except OSError as error:
            raise CaptureError(f"PRESENT_LOG_UNREADABLE: {path.name}: {error.strerror}") from error
        finally:
            os.close(fd)
        end = data.rfind(b"\n") + 1  # a trailing partial record waits for the next read
        cursor.offset += end
        for line in data[:end].decode("utf-8", errors="replace").splitlines():
            cursor.lines += 1
            self._apply(path.name, cursor, line)

    def _apply(self, name: str, cursor: _Cursor, line: str) -> None:
        fields = line.split("\t")
        kind = fields[0]
        if kind not in ("S", *FOCUS_DEVICES):
            return
        try:
            if len(fields) != FIELD_COUNTS[kind]:
                raise ValueError(kind)
            if kind == "S":
                cursor.roles[int(fields[2])] = fields[3]
                return
            surface_id = int(fields[3])
            event = Focus(int(fields[1]), FOCUS_DEVICES[kind], FOCUS_EVENTS[fields[2]], surface_id,
                          cursor.roles.get(surface_id) == "toplevel")
        except (ValueError, KeyError) as error:
            raise CaptureError(f"PRESENT_LOG_MALFORMED: {name}:{cursor.lines}") from error
        previous = self._latest.get(event.device)
        if previous is None or event.ns >= previous.ns:
            self._latest[event.device] = event


class _LockGate:
    """`session_locked()` spawns processes: ask at most once per LOCK_CHECK_S of the injected clock."""

    def __init__(self, clock: Callable[[], int]):
        self._clock = clock
        self._last: int | None = None

    def check(self, context: str) -> None:
        now = self._clock()
        if self._last is not None and now - self._last < int(LOCK_CHECK_S * 1e9):
            return
        self._last = now
        _require_unlocked(context)


def _require_running(proc, context: str) -> None:
    code = proc.poll()
    if code is not None:
        raise CaptureError(f"APP_EXIT: the app exited with status {code} {context}")


def _require_unlocked(context: str) -> None:
    if session_locked():
        raise CaptureError(locked_error(context))


def _lost(device: str, context: str) -> CaptureError:
    return CaptureError(f"FOCUS_LOST: {device} focus is not on the app window {context}")


def _wait_focus(tail: FocusTail, device: str, clock, sleep, error: Callable[[], CaptureError]) -> None:
    deadline = clock() + int(FOCUS_TIMEOUT_S * 1e9)
    while True:
        if tail.focused(device):
            return
        if clock() >= deadline:
            raise error()
        sleep(FOCUS_POLL_S)


def _window(proc, kwin: KWinScripts, dx: int, dy: int):
    win = find_window(process_tree(proc.pid) | {proc.pid})
    return kwin.place(win, WINDOW_W, WINDOW_H, dx, dy)


def _key_check(proc, tail: FocusTail, gate: _LockGate, context: str) -> Callable[[], None]:
    """The witness run before every key: app alive, session unlocked, keyboard enter on the toplevel."""
    def check() -> None:
        _require_running(proc, context)
        gate.check(context)
        if not tail.focused("keyboard"):
            raise _lost("keyboard", context)
    return check


def _open_settings(proc, tail: FocusTail, gate: _LockGate, dev: InputDevice, sleep) -> None:
    check = _key_check(proc, tail, gate, "while opening settings")
    check()
    dev.chord(KEY_LEFTCTRL, KEY_K)
    sleep(PALETTE_S)
    dev.type_text("settings", before_each=check)
    sleep(TYPED_S)
    check()
    dev.key(KEY_ENTER)
    sleep(ENTER_S)


def _pace(clock, sleep, due_ns: int, period_ns: int) -> int:
    """Sleep until `due_ns`; return the next due time without catching up after a late step."""
    now = clock()
    if due_ns > now:
        sleep((due_ns - now) / 1e9)
    return max(due_ns + period_ns, clock())


def _last_frame_ns(log_dir: Path) -> int:
    try:
        frames = toplevel_frames(_live_logs(log_dir))
    except CaptureError:  # no toplevel commit yet
        return 0
    return frames[-1].presented_ns if frames else 0


def _wait_idle(proc, log_dir: Path, clock, sleep) -> None:
    """Wait until the app stops presenting (console replay finished) so G4 times scrolling only."""
    deadline = clock() + int(IDLE_TIMEOUT_S * 1e9)
    while clock() < deadline:
        _require_running(proc, "while waiting for the console replay")
        last = _last_frame_ns(log_dir)
        if last and clock() - last >= int(IDLE_S * 1e9):
            return
        sleep(0.2)
    raise CaptureError(f"REPLAY_NOT_SETTLED: the console kept presenting for {IDLE_TIMEOUT_S:.0f} s")


def _scroll_pattern() -> list[int]:
    leg = [-1] * SCROLL_LEG + [1] * SCROLL_LEG
    return leg * (SCROLL_NOTCHES // SCROLL_LEG)


def drive_g4(proc, scenario: str, log_dir: Path, kwin: KWinScripts, dev: InputDevice,
             clock: Callable[[], int] = time.monotonic_ns,
             sleep: Callable[[float], None] = time.sleep) -> DriveResult:
    """Scroll the app window down then up and return the window of the timed notches."""
    if scenario not in SCENARIOS:
        raise CaptureError(f"UNKNOWN_SCENARIO: {scenario[:40]!r}")
    _notice()
    _window(proc, kwin, *CURSOR_OFFSET[scenario])
    tail, gate = FocusTail(log_dir), _LockGate(clock)
    if scenario == "settings":
        _wait_focus(tail, "keyboard", clock, sleep, lambda: _lost("keyboard", "after placing it"))
        _open_settings(proc, tail, gate, dev, sleep)
    if scenario == "console":
        _wait_idle(proc, log_dir, clock, sleep)
    # KWin sends no pointer enter when it moves a window under a still cursor, so one untimed
    # notch (up: a no-op at the top of the page) makes the compositor deliver it.
    _require_running(proc, "before the priming notch")
    _require_unlocked("before the priming notch")
    dev.wheel(1)
    _wait_focus(tail, "pointer", clock, sleep, lambda: CaptureError(
        "POINTER_NOT_OVER_WINDOW: no pointer enter on the app window after placing it"))
    period_ns = int(NOTCH_S * 1e9)
    due_ns = clock()
    start_ns = last_ns = 0
    for direction in _scroll_pattern():
        due_ns = _pace(clock, sleep, due_ns, period_ns)
        _require_running(proc, "during the scroll")
        gate.check("during the scroll")
        if not tail.focused("pointer"):
            raise _lost("pointer", "during the scroll")
        last_ns = dev.wheel(direction)
        start_ns = start_ns or last_ns
    end_ns = last_ns + TAIL_NS
    remaining = end_ns - clock()
    if remaining > 0:
        sleep(remaining / 1e9)  # let the tail's frames reach the log before the app is stopped
    return DriveResult(start_ns, end_ns, 2 * SCROLL_NOTCHES, last_ns - start_ns)


def _triangle(step: int, period: int, low: int, high: int) -> int:
    phase = (step % period) / period
    return low + round((high - low) * (1 - abs(2 * phase - 1)))


def drive_g9(proc, log_dir: Path, kwin: KWinScripts,
             clock: Callable[[], int] = time.monotonic_ns,
             sleep: Callable[[float], None] = time.sleep) -> DriveResult:
    """Resize the window continuously for RESIZE_S and require the shim to see the sizes."""
    _notice()
    win = _window(proc, kwin, WINDOW_W // 2, WINDOW_H // 2)
    period_ns = int(1e9 / RESIZE_HZ)
    gate = _LockGate(clock)
    due_ns = start_ns = last_ns = clock()
    deadline_ns = start_ns + int(RESIZE_S * 1e9)  # wall time bounds the run; the step rate is only a target
    steps = 0
    while True:
        _require_running(proc, "during the resize")
        gate.check("during the resize")
        due_ns = _pace(clock, sleep, due_ns, period_ns)
        if clock() >= deadline_ns:
            break
        # Unverified per step (a getWindowInfo round trip would not fit in 33 ms); the shim's
        # configure records are the witness, checked below.
        kwin.resize_unverified(win, _triangle(steps, 30, *RESIZE_W), _triangle(steps, 20, *RESIZE_H))
        steps += 1
        last_ns = clock()
    end_ns = last_ns + TAIL_NS
    remaining = end_ns - clock()
    if remaining > 0:
        sleep(remaining / 1e9)
    seen = distinct_sizes(_live_logs(log_dir), start_ns, end_ns)
    if seen < MIN_DISTINCT_SIZES:
        raise CaptureError(f"RESIZE_NOT_OBSERVED: the shim saw {seen} distinct toplevel sizes, "
                           f"need {MIN_DISTINCT_SIZES}")
    return DriveResult(start_ns, end_ns, steps, last_ns - start_ns)
