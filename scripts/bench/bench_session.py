"""Desktop-session preflight for launched captures.

READY fires after the frontend's double requestAnimationFrame. WebKitGTK only
runs animation frames and paints while the compositor presents the window, so
a locked session, a display that is off or a minimized window leaves the
webview blank and READY never arrives (YAN-1985). These probes are read-only.
"""
from __future__ import annotations

import subprocess

LOCKED_HINT = ["loginctl", "show-session", "auto", "-p", "LockedHint", "--value"]
SCREENSAVER_OWNED = ["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus",
                     "--object-path", "/org/freedesktop/DBus",
                     "--method", "org.freedesktop.DBus.NameHasOwner", "org.freedesktop.ScreenSaver"]
SCREENSAVER_ACTIVE = ["gdbus", "call", "--session", "--dest", "org.freedesktop.ScreenSaver",
                      "--object-path", "/org/freedesktop/ScreenSaver",
                      "--method", "org.freedesktop.ScreenSaver.GetActive"]

PRESENTATION_HINT = ("READY waits for two painted frames, so the app window must be presented: "
                     "unlock the session, keep the display on and do not minimize the window")


def _probe(command: list[str]) -> str | None:
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=2, check=False)
    except (OSError, subprocess.SubprocessError):
        return None
    return result.stdout.strip() if result.returncode == 0 else None


def _logind_locked() -> bool | None:
    value = _probe(LOCKED_HINT)
    return {"yes": True, "no": False}.get(value or "")


def _screensaver_active() -> bool | None:
    # Only ask a running service: calling an unowned name could bus-activate a screensaver.
    if _probe(SCREENSAVER_OWNED) != "(true,)":
        return None
    value = _probe(SCREENSAVER_ACTIVE)
    return {"(true,)": True, "(false,)": False}.get(value or "")


def session_locked() -> bool | None:
    """True if any probe reports a locked session, False if one reports it unlocked,
    None when no probe answered (no logind session or no screensaver service)."""
    answers = [answer for answer in (_logind_locked(), _screensaver_active()) if answer is not None]
    if not answers:
        return None
    return any(answers)
