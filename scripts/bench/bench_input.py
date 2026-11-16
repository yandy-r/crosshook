"""Allowlisted virtual keyboard/wheel over /dev/uinput; stdlib only (x86_64 ioctl numbers)."""
import errno
import fcntl
import os
import re
import struct
import time
from collections.abc import Callable
from pathlib import Path

from bench_capture import CaptureError

UINPUT_PATH = "/dev/uinput"
SYSFS_INPUT = Path("/sys/devices/virtual/input")
DEVICE_NAME = b"crosshook-bench"
DENIED_HINT = "add a user ACL (see docs/internal/bench/README.md)"

UI_DEV_CREATE = 0x5501
UI_DEV_DESTROY = 0x5502
UI_DEV_SETUP = 0x405C5503
UI_SET_EVBIT = 0x40045564
UI_SET_KEYBIT = 0x40045565
UI_SET_RELBIT = 0x40045566
UI_GET_SYSNAME_80 = 0x8050552C
BUS_VIRTUAL = 0x06

EV_SYN, EV_KEY, EV_REL = 0, 1, 2
SYN_REPORT = 0
REL_WHEEL = 8
KEY_ENTER, KEY_LEFTCTRL, KEY_K = 28, 29, 37
LETTER_KEYS = {"s": 31, "e": 18, "t": 20, "i": 23, "n": 49, "g": 34}  # the letters of "settings"
ALLOWED_KEYS = frozenset({KEY_LEFTCTRL, KEY_K, KEY_ENTER, *LETTER_KEYS.values()})

_SETUP = struct.Struct("HHHH80sI")  # bustype, vendor, product, version, name, ff_effects_max
_EVENT = struct.Struct("llHHi")     # sec, usec (kernel ignores both), type, code, value
_SYSNAME = re.compile(r"input[0-9]+")
_EVENT_NODE = re.compile(r"event[0-9]+")


# Test seams: tests replace these module attributes.
def _open(path: str, flags: int) -> int:
    return os.open(path, flags)


def _ioctl(fd: int, request: int, arg):
    return fcntl.ioctl(fd, request, arg)


def _write(fd: int, data: bytes) -> int:
    written = os.write(fd, data)
    if written != len(data):  # a cut-off key down or its SYN would leave the key stuck or the event unseen
        raise CaptureError(f"INPUT_WRITE_SHORT: wrote {written} of {len(data)} bytes to the input device")
    return written


def _close(fd: int) -> None:
    os.close(fd)


def _event(kind: int, code: int, value: int) -> bytes:
    return _EVENT.pack(0, 0, kind, code, value)


def _syn() -> bytes:
    return _event(EV_SYN, SYN_REPORT, 0)


def _require_allowed(codes) -> None:
    for code in codes:
        if code not in ALLOWED_KEYS:
            raise ValueError(f"key code {code} is not in the bench input allowlist")


class InputDevice:
    """Context manager owning one virtual device that can only emit allowlisted input."""

    def __init__(self) -> None:
        self._fd: int | None = None
        self._created = False
        self._sysname: str | None = None
        self._held: list[int] = []

    def __enter__(self) -> "InputDevice":
        self.open()
        return self

    def __exit__(self, *_exc) -> None:
        self.close()

    def open(self) -> None:
        flags = os.O_WRONLY | os.O_NONBLOCK | os.O_CLOEXEC
        try:
            self._fd = _open(UINPUT_PATH, flags)
        except FileNotFoundError as error:
            raise CaptureError(f"INPUT_DEVICE_DENIED: {UINPUT_PATH} does not exist "
                               "(load the uinput kernel module)") from error
        except OSError as error:
            if isinstance(error, PermissionError) or error.errno == errno.EACCES:
                raise CaptureError(f"INPUT_DEVICE_DENIED: {UINPUT_PATH} is not writable; "
                                   f"{DENIED_HINT}") from error
            raise CaptureError(f"INPUT_DEVICE_DENIED: {UINPUT_PATH} cannot be opened: "
                               f"{error.strerror}") from error
        try:
            self._setup()
        except BaseException:
            self.close()
            raise

    def _setup(self) -> None:
        fd = self._fd
        _ioctl(fd, UI_SET_EVBIT, EV_KEY)
        _ioctl(fd, UI_SET_EVBIT, EV_REL)
        for code in sorted(ALLOWED_KEYS):
            _ioctl(fd, UI_SET_KEYBIT, code)
        _ioctl(fd, UI_SET_RELBIT, REL_WHEEL)  # never REL_WHEEL_HI_RES
        _ioctl(fd, UI_DEV_SETUP, _SETUP.pack(BUS_VIRTUAL, 0, 0, 0, DEVICE_NAME, 0))
        _ioctl(fd, UI_DEV_CREATE, 0)
        self._created = True
        raw = _ioctl(fd, UI_GET_SYSNAME_80, bytes(80))
        self._sysname = bytes(raw).split(b"\0", 1)[0].decode("ascii", "replace")

    def sysname(self) -> str:
        if self._sysname is None or not _SYSNAME.fullmatch(self._sysname):
            raise CaptureError(f"INPUT_DEVICE_SETUP: unexpected sysname {self._sysname!r}")
        return self._sysname

    def event_node(self) -> str:
        """Return `eventN` for the device, resolved from sysfs."""
        directory = SYSFS_INPUT / self.sysname()
        try:
            names = sorted(n for n in os.listdir(directory) if _EVENT_NODE.fullmatch(n))
        except OSError as error:
            raise CaptureError(f"INPUT_DEVICE_SETUP: {directory.name}: {error.strerror}") from error
        if not names:
            raise CaptureError(f"INPUT_DEVICE_SETUP: no event node under {directory.name}")
        return names[0]

    def _emit(self, data: bytes) -> None:
        if self._fd is None or not self._created:
            raise CaptureError("INPUT_DEVICE_SETUP: device is not open")
        _write(self._fd, data)

    def wheel(self, notches: int) -> int:
        """Scroll by one detent (-1 scrolls down, per evdev REL_WHEEL); a larger burst is never needed.

        Returns the CLOCK_MONOTONIC ns read immediately before the write.
        """
        if notches not in (-1, 1):
            raise ValueError("wheel sends exactly one notch per call: -1 or 1")
        data = _event(EV_REL, REL_WHEEL, int(notches)) + _syn()
        stamp_ns = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
        self._emit(data)
        return stamp_ns

    def _press(self, code: int) -> None:
        self._held.append(code)  # before the write: a down that fails midway must still be released
        self._emit(_event(EV_KEY, code, 1) + _syn())

    def _release(self, code: int) -> None:
        self._emit(_event(EV_KEY, code, 0) + _syn())
        self._held.remove(code)

    def key(self, code: int) -> None:
        self.chord(code)

    def chord(self, *codes: int) -> None:
        """Press in order, then release in reverse order."""
        _require_allowed(codes)
        try:
            for code in codes:
                self._press(code)
            for code in reversed(codes):
                self._release(code)
        except BaseException:
            self._release_held()
            raise

    def type_text(self, text: str, before_each: Callable[[], None] | None = None) -> None:
        """Type lowercase text made only of the LETTER_KEYS letters.

        `before_each` runs before every key, so a caller can refuse the next key when its
        precondition (window focus) no longer holds; whatever it raises stops the typing.
        """
        if any(char not in LETTER_KEYS for char in text):
            raise ValueError("type_text accepts only lowercase letters from the allowlist")
        for char in text:
            if before_each is not None:
                before_each()
            self.key(LETTER_KEYS[char])

    def _release_held(self) -> None:
        while self._held:
            code = self._held[-1]
            try:
                self._release(code)
            except (OSError, CaptureError):
                self._held.clear()

    def close(self) -> None:
        """Release held keys, destroy the device and close the fd; idempotent."""
        fd, self._fd = self._fd, None
        if fd is None:
            return
        created, self._created = self._created, False
        try:
            if created:
                for code in reversed(self._held):
                    try:
                        _write(fd, _event(EV_KEY, code, 0) + _syn())
                    except (OSError, CaptureError):
                        break
                self._held.clear()
                try:
                    _ioctl(fd, UI_DEV_DESTROY, 0)
                except OSError:
                    pass
        finally:
            _close(fd)
