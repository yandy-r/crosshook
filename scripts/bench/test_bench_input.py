"""Focused uinput InputDevice checks; stdlib only."""
import errno
import os
import struct
import sys
import tempfile
import time
import unittest
import unittest.mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench_input as bi
from bench_capture import CaptureError

ALLOWED = {29, 37, 28, 31, 18, 20, 23, 49, 34}


class FakeKernel:
    """Records ioctl/write/close traffic in place of /dev/uinput."""

    def __init__(self, fail_on=None, sysname=b"input42"):
        self.ioctls, self.writes, self.closed, self.opened = [], [], [], []
        self.fail_on, self.sysname = fail_on, sysname

    def open(self, path, flags):
        self.opened.append((path, flags))
        return 7

    def ioctl(self, fd, request, arg):
        self.ioctls.append((fd, request, arg))
        if request == self.fail_on:
            raise OSError(errno.EIO, "boom")
        if request == bi.UI_GET_SYSNAME_80:
            return self.sysname.ljust(80, b"\0")
        return 0

    def write(self, fd, data):
        self.writes.append((fd, data))
        return len(data)

    def close(self, fd):
        self.closed.append(fd)

    def events(self):
        out = []
        for _fd, data in self.writes:
            out += [struct.unpack("llHHi", data[i:i + 24]) for i in range(0, len(data), 24)]
        return [(t, c, v) for _s, _u, t, c, v in out]

    def requests(self):
        return [r for _f, r, _a in self.ioctls]


class InputDeviceTests(unittest.TestCase):
    def setUp(self):
        self.kernel = FakeKernel()
        self.patches = [unittest.mock.patch.object(bi, name, getattr(self.kernel, name.lstrip("_")))
                        for name in ("_open", "_ioctl", "_write", "_close")]
        for patch in self.patches:
            patch.start()
            self.addCleanup(patch.stop)

    def device(self):
        dev = bi.InputDevice()
        dev.open()
        self.addCleanup(dev.close)
        return dev

    def test_open_flags_and_setup_sequence(self):
        self.device()
        path, flags = self.kernel.opened[0]
        self.assertEqual(path, "/dev/uinput")
        self.assertEqual(flags, os.O_WRONLY | os.O_NONBLOCK | os.O_CLOEXEC)
        expected = [bi.UI_SET_EVBIT, bi.UI_SET_EVBIT] + [bi.UI_SET_KEYBIT] * len(ALLOWED) \
            + [bi.UI_SET_RELBIT, bi.UI_DEV_SETUP, bi.UI_DEV_CREATE, bi.UI_GET_SYSNAME_80]
        self.assertEqual(self.kernel.requests(), expected)
        evbits = [a for _f, r, a in self.kernel.ioctls if r == bi.UI_SET_EVBIT]
        self.assertEqual(evbits, [bi.EV_KEY, bi.EV_REL])

    def test_constants(self):
        self.assertEqual((bi.UI_DEV_CREATE, bi.UI_DEV_DESTROY, bi.UI_DEV_SETUP),
                         (0x5501, 0x5502, 0x405C5503))
        self.assertEqual((bi.UI_SET_EVBIT, bi.UI_SET_KEYBIT, bi.UI_SET_RELBIT, bi.UI_GET_SYSNAME_80),
                         (0x40045564, 0x40045565, 0x40045566, 0x8050552C))

    def test_setup_bytes(self):
        self.device()
        setup = next(a for _f, r, a in self.kernel.ioctls if r == bi.UI_DEV_SETUP)
        self.assertEqual(len(setup), 92)
        self.assertEqual(setup, struct.pack("HHHH80sI", 6, 0, 0, 0, b"crosshook-bench", 0))
        self.assertEqual(setup[8:23], b"crosshook-bench")

    def test_keybit_set_equals_allowlist_and_no_hires(self):
        self.device()
        keybits = {a for _f, r, a in self.kernel.ioctls if r == bi.UI_SET_KEYBIT}
        self.assertEqual(keybits, ALLOWED)
        self.assertEqual(keybits, set(bi.ALLOWED_KEYS))
        relbits = [a for _f, r, a in self.kernel.ioctls if r == bi.UI_SET_RELBIT]
        self.assertEqual(relbits, [8])
        self.assertEqual(set(bi.LETTER_KEYS), set("settings"))

    def test_sysname_and_event_node(self):
        dev = self.device()
        self.assertEqual(dev.sysname(), "input42")
        with tempfile.TemporaryDirectory() as root:
            (Path(root) / "input42").mkdir()
            for name in ("event9", "mouse0", "power"):
                (Path(root) / "input42" / name).mkdir()
            with unittest.mock.patch.object(bi, "SYSFS_INPUT", Path(root)):
                self.assertEqual(dev.event_node(), "event9")
                (Path(root) / "input42" / "event9").rmdir()
                with self.assertRaises(CaptureError):
                    dev.event_node()

    def test_bad_sysname_rejected(self):
        self.kernel.sysname = b"../etc"
        with self.assertRaises(CaptureError):
            self.device().sysname()

    def test_wheel_bytes_and_stamp(self):
        dev = self.device()
        before = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
        stamp = dev.wheel(-1)
        after = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
        self.assertTrue(before <= stamp <= after)
        (fd, data), = self.kernel.writes
        self.assertEqual(fd, 7)
        self.assertEqual(len(data), 48)
        self.assertEqual(data[:24], struct.pack("llHHi", 0, 0, 2, 8, -1))
        self.assertEqual(data[24:], struct.pack("llHHi", 0, 0, 0, 0, 0))

    def test_wheel_is_bounded_to_one_notch_either_way(self):
        dev = self.device()
        for notches in (0, 2, -2, 3, 120):
            with self.subTest(notches=notches), self.assertRaises(ValueError):
                dev.wheel(notches)
        self.assertEqual(self.kernel.writes, [])
        dev.wheel(1)
        dev.wheel(-1)
        self.assertEqual(len(self.kernel.writes), 2)

    def test_chord_orders_and_pairs(self):
        dev = self.device()
        dev.chord(bi.KEY_LEFTCTRL, bi.KEY_K)
        self.assertEqual(self.kernel.events(), [
            (1, 29, 1), (0, 0, 0), (1, 37, 1), (0, 0, 0),
            (1, 37, 0), (0, 0, 0), (1, 29, 0), (0, 0, 0)])

    def test_key_and_type_text_pair_every_down(self):
        dev = self.device()
        dev.key(bi.KEY_ENTER)
        dev.type_text("set")
        events = self.kernel.events()
        keys = [(c, v) for t, c, v in events if t == 1]
        self.assertEqual(keys, [(28, 1), (28, 0), (31, 1), (31, 0), (18, 1), (18, 0), (20, 1), (20, 0)])
        for i, event in enumerate(events):
            if event[0] == 1:
                self.assertEqual(events[i + 1], (0, 0, 0))

    def test_type_text_runs_the_hook_before_every_key(self):
        dev = self.device()
        seen = []
        dev.type_text("set", before_each=lambda: seen.append(len(self.kernel.events())))
        self.assertEqual(seen, [0, 4, 8])  # before s, e, t: each earlier key is two events down, two up

    def test_a_raising_hook_stops_typing_before_the_next_key(self):
        dev = self.device()
        count = []

        def hook():
            count.append(1)
            if len(count) == 3:
                raise CaptureError("FOCUS_LOST: keyboard")
        with self.assertRaisesRegex(CaptureError, "^FOCUS_LOST"):
            dev.type_text("settings", before_each=hook)
        self.assertEqual([(c, v) for t, c, v in self.kernel.events() if t == 1],
                         [(31, 1), (31, 0), (18, 1), (18, 0)])

    def test_disallowed_codes_raise_without_writing(self):
        dev = self.device()
        with self.assertRaises(ValueError):
            dev.key(30)  # KEY_A
        with self.assertRaises(ValueError):
            dev.chord(bi.KEY_LEFTCTRL, 30)
        for text in ("Set", "x", "settings!", "a b"):
            with self.assertRaises(ValueError):
                dev.type_text(text)
        self.assertEqual(self.kernel.writes, [])

    def test_close_idempotent(self):
        dev = bi.InputDevice()
        dev.open()
        dev.close()
        dev.close()
        self.assertEqual(self.kernel.requests().count(bi.UI_DEV_DESTROY), 1)
        self.assertEqual(self.kernel.closed, [7])

    def test_close_releases_held_keys(self):
        dev = self.device()
        dev._press(bi.KEY_LEFTCTRL)
        dev._press(bi.KEY_K)
        self.kernel.writes.clear()
        dev.close()
        self.assertEqual(self.kernel.events(), [(1, 37, 0), (0, 0, 0), (1, 29, 0), (0, 0, 0)])
        self.assertEqual(self.kernel.requests()[-1], bi.UI_DEV_DESTROY)

    def test_close_after_partial_setup_failure(self):
        self.kernel.fail_on = bi.UI_DEV_SETUP
        dev = bi.InputDevice()
        with self.assertRaises(OSError):
            dev.open()
        self.assertEqual(self.kernel.closed, [7])
        self.assertNotIn(bi.UI_DEV_DESTROY, self.kernel.requests())
        dev.close()
        self.assertEqual(self.kernel.closed, [7])

    def test_failure_after_create_destroys_device(self):
        self.kernel.fail_on = bi.UI_GET_SYSNAME_80
        dev = bi.InputDevice()
        with self.assertRaises(OSError):
            dev.open()
        self.assertIn(bi.UI_DEV_DESTROY, self.kernel.requests())
        self.assertEqual(self.kernel.closed, [7])

    def test_emit_before_open_fails(self):
        with self.assertRaises(CaptureError):
            bi.InputDevice().wheel(1)

    def test_context_manager_closes(self):
        with bi.InputDevice() as dev:
            dev.wheel(1)
        self.assertEqual(self.kernel.closed, [7])


class OpenErrorTests(unittest.TestCase):
    def open_with(self, error):
        with unittest.mock.patch.object(bi, "_open", side_effect=error):
            with self.assertRaises(CaptureError) as ctx:
                bi.InputDevice().open()
        return str(ctx.exception)

    def test_eacces_maps_to_denied(self):
        for error in (PermissionError(errno.EACCES, "denied"), OSError(errno.EACCES, "denied")):
            self.assertEqual(self.open_with(error),
                             "INPUT_DEVICE_DENIED: /dev/uinput is not writable; add a user ACL "
                             "(see docs/internal/bench/README.md)")

    def test_missing_device(self):
        message = self.open_with(FileNotFoundError(errno.ENOENT, "missing"))
        self.assertTrue(message.startswith("INPUT_DEVICE_DENIED: /dev/uinput does not exist"))


@unittest.skipUnless(os.access("/dev/uinput", os.W_OK) and os.environ.get("CROSSHOOK_BENCH_HW_TESTS") == "1",
                     "needs writable /dev/uinput and CROSSHOOK_BENCH_HW_TESTS=1")
class HardwareTests(unittest.TestCase):
    def test_create_and_destroy_real_device(self):
        with bi.InputDevice() as dev:
            node = dev.event_node()
            self.assertRegex(node, r"^event[0-9]+$")
            self.assertTrue(os.path.exists(f"/sys/devices/virtual/input/{dev.sysname()}/{node}"))


if __name__ == "__main__":
    unittest.main()
