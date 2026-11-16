"""Build the bench-only wl_surface presentation shim at run time; stdlib only."""
from __future__ import annotations

import hashlib
import shlex
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

from bench_capture import CaptureError
from bench_provenance import sha256_file

SDK = "org.gnome.Sdk//50"
SOURCE_DIR = Path(__file__).resolve().parent / "presentation"
SOURCES = (SOURCE_DIR / "wlpresent.c", SOURCE_DIR / "wlfocus.c")
HEADER = SOURCE_DIR / "wlpresent.h"
PROTOCOL_XML = SOURCE_DIR / "presentation-time.xml"
TIMEOUT = 120
# wl_proxy_get_interface arrived in libwayland 1.24; Ubuntu 24.04 ships 1.22.
WAYLAND_MIN = "1.24"
MISSING_WAYLAND = f"MISSING_TOOL: libwayland-client >= {WAYLAND_MIN}"
# Warning flags shared by every compile. Real builds omit -Werror so a newer compiler's new
# warnings never block a measurement; check() (CI) adds it.
CFLAGS = ("-O2", "-fPIC", "-std=gnu11", "-pthread", "-Wall", "-Wextra")
STRICT = "-Werror"
# Paths arrive only as positional args ($1 xml, $2 and $3 sources, $4 out dir, $5... compiler
# flags), never inside this text.
SCRIPT = (
    "set -e\n"
    'xml="$1"; src1="$2"; src2="$3"; out="$4"; shift 4\n'
    f'pkg-config --atleast-version={WAYLAND_MIN} wayland-client || {{ echo "{MISSING_WAYLAND}" >&2; exit 1; }}\n'
    'wayland-scanner client-header "$xml" "$out/presentation-time-client-protocol.h"\n'
    'wayland-scanner private-code "$xml" "$out/presentation-time-protocol.c"\n'
    'cc "$@" -shared -I"$out" -o "$out/libwlpresent.so" '
    '"$src1" "$src2" "$out/presentation-time-protocol.c" $(pkg-config --cflags --libs wayland-client) -ldl\n'
)


@dataclass(frozen=True)
class ShimBuild:
    path: Path
    so_sha256: str
    source_sha256: str
    built_in: str  # "host" or SDK


def source_sha256() -> str:
    """One digest over every shim source file and the shared header, in a fixed order."""
    digest = hashlib.sha256()
    for path in (*SOURCES, HEADER):
        digest.update(f"{path.name}\0{sha256_file(path)}\n".encode())
    return digest.hexdigest()


def _run(command: list[str]) -> subprocess.CompletedProcess:
    try:
        done = subprocess.run(command, capture_output=True, text=True, timeout=TIMEOUT)
    except subprocess.TimeoutExpired as error:
        raise CaptureError("SHIM_BUILD_FAILED: timeout") from error
    except OSError as error:
        raise CaptureError(f"MISSING_TOOL: {command[0]}") from error
    if done.returncode != 0:
        lines = [line for line in (done.stderr or "").splitlines() if line.strip()]
        if lines and lines[-1].strip() == MISSING_WAYLAND:  # reported by the SDK script
            raise CaptureError(MISSING_WAYLAND)
        raise CaptureError(f"SHIM_BUILD_FAILED: {lines[-1].strip()[:200] if lines else command[0]}")
    return done


def _require(*tools: str) -> None:
    for tool in tools:
        if shutil.which(tool) is None:
            raise CaptureError(f"MISSING_TOOL: {tool}")


def _require_wayland() -> None:
    try:
        found = subprocess.run(["pkg-config", f"--atleast-version={WAYLAND_MIN}", "wayland-client"],
                               capture_output=True, text=True, timeout=TIMEOUT).returncode == 0
    except (subprocess.SubprocessError, OSError):
        found = False
    if not found:
        raise CaptureError(MISSING_WAYLAND)


def _scan(out: Path) -> None:
    _run(["wayland-scanner", "client-header", str(PROTOCOL_XML), str(out / "presentation-time-client-protocol.h")])
    _run(["wayland-scanner", "private-code", str(PROTOCOL_XML), str(out / "presentation-time-protocol.c")])


def _pkg_flags() -> list[str]:
    return shlex.split(_run(["pkg-config", "--cflags", "--libs", "wayland-client"]).stdout)


def build(out_dir: Path, target: str) -> ShimBuild:
    out_dir = Path(out_dir)
    if ":" in str(out_dir) or "," in str(out_dir):  # would corrupt the --filesystem= option
        raise CaptureError("SHIM_BUILD_FAILED: unsupported path")
    so = out_dir / "libwlpresent.so"
    if target == "native":
        _require("wayland-scanner", "cc", "pkg-config")
        _require_wayland()
        _scan(out_dir)
        _run(["cc", *CFLAGS, "-shared", f"-I{out_dir}", "-o", str(so), *map(str, SOURCES),
              str(out_dir / "presentation-time-protocol.c"), *_pkg_flags(), "-ldl"])
        built_in = "host"
    elif target == "flatpak":
        _require("flatpak")
        try:
            known = subprocess.run(["flatpak", "info", SDK], capture_output=True, text=True,
                                   timeout=TIMEOUT).returncode == 0
        except (subprocess.SubprocessError, OSError):
            known = False
        if not known:
            raise CaptureError(f"MISSING_TOOL: {SDK}")
        _run(["flatpak", "run", "--command=sh", f"--filesystem={out_dir}", f"--filesystem={SOURCE_DIR}:ro",
              SDK, "-c", SCRIPT, "sh", str(PROTOCOL_XML), *map(str, SOURCES),
              str(out_dir), *CFLAGS])
        built_in = SDK
    else:
        raise CaptureError(f"SHIM_BUILD_FAILED: unknown target {target}")
    if not so.is_file():
        raise CaptureError("SHIM_BUILD_FAILED: libwlpresent.so missing")
    return ShimBuild(so, sha256_file(so), source_sha256(), built_in)


def check() -> int:
    """Syntax-only compile of the shim for CI."""
    try:
        _require("wayland-scanner", "cc", "pkg-config")
        _require_wayland()
        with tempfile.TemporaryDirectory() as tmp:
            _scan(Path(tmp))
            _run(["cc", "-fsyntax-only", *CFLAGS, STRICT, f"-I{tmp}", *map(str, SOURCES), *_pkg_flags()])
    except CaptureError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    if sys.argv[1:] != ["--check"]:
        print("usage: bench_shim.py --check", file=sys.stderr)
        sys.exit(2)
    sys.exit(check())
