"""KWin control for the G4/G9 input drivers: find, place and resize the app window over D-Bus.

Windows are matched by host pid, never by caption. Script effects are verified only by
polling getWindowInfo, because KWin script print() output does not reach the journal.
"""
from __future__ import annotations

import os
import re
import shutil
import stat
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

from bench_capture import CaptureError
from bench_provenance import write_private
from bench_session import name_has_owner

DEST = "org.kde.KWin"
WINDOWS_MATCH = ["gdbus", "call", "--session", "--dest", DEST, "--object-path", "/WindowsRunner",
                 "--method", "org.kde.krunner1.Match", ""]
GET_WINDOW_INFO = ["gdbus", "call", "--session", "--dest", DEST, "--object-path", "/KWin",
                   "--method", "org.kde.KWin.getWindowInfo"]
SCRIPTING = ["gdbus", "call", "--session", "--dest", DEST, "--object-path", "/Scripting", "--method"]
SCRIPT_METHOD = "org.kde.kwin.Script.run"
DEVICES_PROPERTY = ["gdbus", "call", "--session", "--dest", DEST,
                    "--object-path", "/org/kde/KWin/InputDevice",
                    "--method", "org.freedesktop.DBus.Properties.Get",
                    "org.kde.KWin.InputDeviceManager", "devicesSysNames"]

CALL_TIMEOUT_S = 5
WINDOW_TIMEOUT_S = 10
GEOMETRY_TIMEOUT_S = 3
POLL_S = 0.1
GEOMETRY_TOLERANCE_PX = 2
UNSUPPORTED = "UNSUPPORTED_COMPOSITOR: KWin is required for G4/G9 input drivers"

_UUID = re.compile(r"0_(\{[0-9a-f-]{36}\})")
_ENTRY = re.compile(r"'([A-Za-z0-9_.-]+)': <")
_RUN_TAG = re.compile(r"[a-z0-9]{1,16}")
_PLUGIN = re.compile(r"crosshook-bench-[a-z0-9-]{1,48}")
_UNSAFE_PATH = re.compile(r"['\\\x00-\x1f\x7f]")
_INT = re.compile(r"(?:u?int(?:16|32|64) )?(-?\d+)")
_NUMBER = re.compile(r"-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?")
_REQUIRED = ("pid", "x", "y", "width", "height", "minimized")


@dataclass(frozen=True)
class WindowInfo:
    uuid: str
    pid: int
    x: float
    y: float
    width: float
    height: float
    minimized: bool


def _run(argv: list[str], timeout: float) -> subprocess.CompletedProcess:
    return subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, text=True,
                          timeout=timeout, check=False)


_clock, _sleep = time.monotonic, time.sleep  # test seams


def _gdbus(argv: list[str]) -> str:
    method = argv[argv.index("--method") + 1]
    try:
        result = _run(argv, CALL_TIMEOUT_S)
    except (OSError, subprocess.SubprocessError) as error:
        raise CaptureError(f"KWIN_CALL_FAILED: {method}: {error}") from error
    if result.returncode != 0:
        raise CaptureError(f"KWIN_CALL_FAILED: {method}: {result.stderr.strip()[:200]}")
    return result.stdout.strip()


def require_kwin() -> None:
    if shutil.which("gdbus") is None:
        raise CaptureError("MISSING_TOOL: gdbus")
    if name_has_owner(DEST) is not True:
        raise CaptureError(UNSUPPORTED)


def input_device_sysnames() -> str:
    """KWin's `devicesSysNames` reply: the raw text listing every event node KWin has opened."""
    return _gdbus(DEVICES_PROPERTY)


def _skip_value(text: str, start: int) -> int:
    """Index of the '>' closing the variant that begins at `start`; strings and nesting are skipped."""
    depth, i = 0, start
    while i < len(text):
        char = text[i]
        if char in "'\"":
            i = _skip_string(text, i)
            continue
        if char in "[({<":
            depth += 1
        elif char in "])}":
            depth -= 1
        elif char == ">":
            if depth == 0:
                return i
            depth -= 1
        i += 1
    raise CaptureError("KWIN_PROTOCOL: unterminated value in getWindowInfo reply")


def _skip_string(text: str, start: int) -> int:
    quote, i = text[start], start + 1
    while i < len(text):
        if text[i] == "\\":
            i += 2
        elif text[i] == quote:
            return i + 1
        else:
            i += 1
    raise CaptureError("KWIN_PROTOCOL: unterminated string in getWindowInfo reply")


def _raw_entries(text: str) -> dict[str, str]:
    """Map each key of a gdbus `({'k': <v>, ...},)` reply to its raw value text (any type)."""
    if not (text.startswith("({") and text.endswith("},)")):
        raise CaptureError("KWIN_PROTOCOL: getWindowInfo reply is not an a{sv} dictionary")
    body, i, entries = text[:-3], 2, {}
    while i < len(body):
        match = _ENTRY.match(body, i)
        if match is None:
            raise CaptureError("KWIN_PROTOCOL: malformed dictionary entry in getWindowInfo reply")
        end = _skip_value(body, match.end())
        entries[match.group(1)] = body[match.end():end]
        i = end + 1
        if body.startswith(", ", i):
            i += 2
        elif i != len(body):
            raise CaptureError("KWIN_PROTOCOL: malformed entry separator in getWindowInfo reply")
    return entries


def _number(key: str, raw: str, integer: bool) -> int | float:
    match = _INT.fullmatch(raw) if integer else None
    if match:
        return int(match.group(1))
    if not integer and _NUMBER.fullmatch(raw.removeprefix("double ")):
        return float(raw.removeprefix("double "))
    raise CaptureError(f"KWIN_PROTOCOL: getWindowInfo key {key} is not a number: {raw[:40]}")


def parse_window_info(uuid: str, text: str) -> WindowInfo:
    """Strict parser for the a{sv} subset: only the six required keys are read; every
    other value (captions and so on) is skipped without interpretation."""
    entries = _raw_entries(text.strip())
    missing = [key for key in _REQUIRED if key not in entries]
    if missing:
        raise CaptureError(f"KWIN_PROTOCOL: getWindowInfo reply lacks {', '.join(missing)}")
    if entries["minimized"] not in ("true", "false"):
        raise CaptureError("KWIN_PROTOCOL: getWindowInfo key minimized is not a bool")
    x, y, width, height = (_number(key, entries[key], False) for key in ("x", "y", "width", "height"))
    return WindowInfo(uuid, _number("pid", entries["pid"], True), x, y, width, height,  # type: ignore[arg-type]
                      entries["minimized"] == "true")


def window_info(uuid: str) -> WindowInfo:
    return parse_window_info(uuid, _gdbus([*GET_WINDOW_INFO, f"'{uuid}'"]))


def _snapshot(pids: set[int]) -> list[WindowInfo]:
    matches = []
    for uuid in dict.fromkeys(_UUID.findall(_gdbus(WINDOWS_MATCH))):  # captions can repeat an id
        reply = _gdbus([*GET_WINDOW_INFO, f"'{uuid}'"])
        if reply.strip() in ("({},)", "(@a{sv} {},)"):
            continue  # closed between Match and getWindowInfo
        info = parse_window_info(uuid, reply)
        if info.pid in pids:
            matches.append(info)
    return matches


def find_window(pids: set[int], timeout_s: float = WINDOW_TIMEOUT_S) -> WindowInfo:
    deadline = _clock() + timeout_s
    while True:
        matches = _snapshot(pids)
        if len(matches) > 1:
            raise CaptureError(f"WINDOW_AMBIGUOUS: {len(matches)} KWin windows belong to pids {sorted(pids)}")
        if matches:
            return matches[0]
        if _clock() >= deadline:
            raise CaptureError(f"WINDOW_NOT_FOUND: no KWin window for pids {sorted(pids)} within {timeout_s}s")
        _sleep(POLL_S)


def _quoted_path(path: Path) -> str:
    text = str(path)
    if not text.startswith("/") or _UNSAFE_PATH.search(text):
        raise CaptureError("KWIN_SCRIPT_PATH_INVALID: script path must be absolute without quotes, "
                           "backslashes or control characters")
    return f"'{text}'"


def _quoted_plugin(plugin: str) -> str:
    if not _PLUGIN.fullmatch(plugin):
        raise CaptureError(f"KWIN_PLUGIN_INVALID: {plugin[:80]!r}")
    return f"'{plugin}'"


def _ints(**values: int) -> None:
    for name, value in values.items():
        if type(value) is not int:
            raise TypeError(f"{name} must be int")


def _verified_tools_dir(tools_dir: Path) -> Path:
    try:
        info = os.lstat(tools_dir)
    except OSError as error:
        raise CaptureError(f"KWIN_TOOLS_DIR_UNSAFE: {tools_dir}: {error.strerror}") from error
    if not (stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700):
        raise CaptureError(f"KWIN_TOOLS_DIR_UNSAFE: {tools_dir} must be an owned 0700 directory")
    return tools_dir


class KWinScripts:
    """Runs short KWin scripts one at a time under unique plugin names."""

    def __init__(self, tools_dir: Path, run_tag: str):
        if not _RUN_TAG.fullmatch(run_tag):
            raise CaptureError(f"KWIN_PLUGIN_INVALID: run tag {run_tag[:40]!r}")
        self._tools_dir = _verified_tools_dir(tools_dir)
        self._run_tag = run_tag
        self._count = 0

    def run(self, js: str) -> None:
        self._count += 1
        plugin = f"crosshook-bench-{self._run_tag}-{self._count}"
        quoted_plugin = _quoted_plugin(plugin)
        path = self._tools_dir / f"{plugin}.js"
        quoted_path = _quoted_path(path)
        try:
            write_private(path, js)
        except OSError as error:
            raise CaptureError(f"KWIN_SCRIPT_WRITE_FAILED: {path.name}: {error.strerror}") from error
        try:
            self._load_run_unload(js_path=quoted_path, plugin=quoted_plugin, name=plugin)
        finally:
            path.unlink(missing_ok=True)  # after the unload: nothing keeps reading the file

    def _load_run_unload(self, js_path: str, plugin: str, name: str) -> None:
        if _gdbus([*SCRIPTING, "org.kde.kwin.Scripting.isScriptLoaded", plugin]) == "(true,)":
            raise CaptureError(f"KWIN_SCRIPT_BUSY: plugin {name} is already loaded")
        owned = True  # the plugin is ours to unload unless loadScript reported it taken
        try:
            loaded = re.fullmatch(r"\((-?\d+),\)", _gdbus(
                [*SCRIPTING, "org.kde.kwin.Scripting.loadScript", js_path, plugin]))
            if loaded is None:
                raise CaptureError("KWIN_PROTOCOL: loadScript reply is not an int32")
            if int(loaded.group(1)) < 0:
                owned = False
                raise CaptureError(f"KWIN_SCRIPT_BUSY: loadScript refused plugin {name}")
            _gdbus(["gdbus", "call", "--session", "--dest", DEST,
                    "--object-path", f"/Scripting/Script{int(loaded.group(1))}", "--method", SCRIPT_METHOD])
        except BaseException:
            if owned:
                self._unload(plugin, quiet=True)
            raise
        self._unload(plugin, quiet=False)

    @staticmethod
    def _unload(quoted_plugin: str, quiet: bool) -> None:
        try:
            _gdbus([*SCRIPTING, "org.kde.kwin.Scripting.unloadScript", quoted_plugin])
        except CaptureError:
            if not quiet:
                raise

    def place(self, win: WindowInfo, width: int, height: int, cursor_dx: int, cursor_dy: int) -> WindowInfo:
        """Un-maximize, size the window and put the cursor at (cursor_dx, cursor_dy) inside it."""
        _ints(pid=win.pid, width=width, height=height, cursor_dx=cursor_dx, cursor_dy=cursor_dy)
        self.run(f"""\
const t = workspace.windowList().filter(w => w.pid === {win.pid});
if (t.length === 1) {{
  const w = t[0];
  w.setMaximize(false, false);
  const c = workspace.cursorPos;
  w.frameGeometry = {{x: c.x - {cursor_dx}, y: c.y - {cursor_dy}, width: {width}, height: {height}}};
  workspace.activeWindow = w;
}}
""")
        return self._await_geometry(win, width, height)

    def resize_to(self, win: WindowInfo, width: int, height: int) -> WindowInfo:
        """Resize in place (the origin is kept, the window is not activated) and verify the geometry."""
        self.resize_unverified(win, width, height)
        return self._await_geometry(win, width, height)

    def resize_unverified(self, win: WindowInfo, width: int, height: int) -> None:
        """`resize_to` without the geometry poll, for callers that must keep a tight cadence."""
        _ints(pid=win.pid, width=width, height=height)
        self.run(f"""\
const t = workspace.windowList().filter(w => w.pid === {win.pid});
if (t.length === 1) {{
  const w = t[0];
  w.setMaximize(false, false);
  const g = w.frameGeometry;
  w.frameGeometry = {{x: g.x, y: g.y, width: {width}, height: {height}}};
}}
""")

    def _await_geometry(self, win: WindowInfo, width: int, height: int) -> WindowInfo:
        deadline = _clock() + GEOMETRY_TIMEOUT_S
        while True:
            info = window_info(win.uuid)
            if (abs(round(info.width) - width) <= GEOMETRY_TOLERANCE_PX
                    and abs(round(info.height) - height) <= GEOMETRY_TOLERANCE_PX):
                return info
            if _clock() >= deadline:
                raise CaptureError(f"KWIN_GEOMETRY_NOT_APPLIED: wanted {width}x{height}, "
                                   f"window is {round(info.width)}x{round(info.height)}")
            _sleep(POLL_S)
