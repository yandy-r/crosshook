"""Desktop-session preflight for launched captures.

READY fires after the frontend's double requestAnimationFrame. WebKitGTK only
runs animation frames and paints while the compositor presents the window, so
a locked session, a display that is off or a minimized window leaves the
webview blank and READY never arrives (YAN-1985). These probes are read-only.
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

LOCKED_HINT = ["loginctl", "show-session", "auto", "-p", "Type", "-p", "LockedHint"]
GRAPHICAL_TYPES = ("wayland", "x11")
NAME_HAS_OWNER = ["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
                  "--object-path", "/org/freedesktop/DBus",
                  "--method", "org.freedesktop.DBus.NameHasOwner"]
SCREENSAVER_OWNED = [*NAME_HAS_OWNER, "org.freedesktop.ScreenSaver"]
SCREENSAVER_ACTIVE = ["gdbus", "call", "--session", "--dest", "org.freedesktop.ScreenSaver",
                      "--object-path", "/org/freedesktop/ScreenSaver",
                      "--method", "org.freedesktop.ScreenSaver.GetActive"]

PRESENTATION_HINT = ("READY waits for two painted frames, so the app window must be presented: "
                     "unlock the session, keep the display on and do not minimize the window")


def _probe(command: list[str]) -> str | None:
    try:
        result = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, text=True,
                                timeout=2, check=False)
    except (OSError, subprocess.SubprocessError):
        return None
    return result.stdout.strip() if result.returncode == 0 else None


def _logind_locked() -> bool | None:
    """LockedHint of the caller's session; unknown unless that session is graphical
    (from ssh or a TTY, `auto` resolves to a session that never locks)."""
    fields = dict(line.split("=", 1) for line in (_probe(LOCKED_HINT) or "").splitlines() if "=" in line)
    if fields.get("Type") not in GRAPHICAL_TYPES:
        return None
    return {"yes": True, "no": False}.get(fields.get("LockedHint", ""))


def _session_bus_available() -> bool:
    # Without a bus address gdbus would autolaunch a stray dbus-daemon.
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    return bool(os.environ.get("DBUS_SESSION_BUS_ADDRESS")) or bool(runtime and Path(runtime, "bus").exists())


def name_has_owner(name: str) -> bool | None:
    """Whether `name` has an owner on the session bus; None when there is no bus or no answer."""
    if not _session_bus_available():
        return None
    return {"(true,)": True, "(false,)": False}.get(_probe([*NAME_HAS_OWNER, name]) or "")


def _screensaver_active() -> bool | None:
    # Only ask a running service: calling an unowned name could bus-activate a screensaver.
    if not _session_bus_available() or _probe(SCREENSAVER_OWNED) != "(true,)":
        return None
    value = _probe(SCREENSAVER_ACTIVE)
    return {"(true,)": True, "(false,)": False}.get(value or "")


def session_locked() -> bool | None:
    """True if any probe reports a locked session, False if one reports it unlocked,
    None when no probe answered (no graphical logind session, no screensaver service)."""
    answers = []
    for probe in (_logind_locked, _screensaver_active):
        answer = probe()
        if answer:
            return True
        answers.append(answer)
    return False if False in answers else None


def locked_error(context: str) -> str:
    """The SESSION_LOCKED failure text; `bench.py` matches error codes by prefix."""
    return f"SESSION_LOCKED: the session is locked {context}; {PRESENTATION_HINT}"
