"""YAN-782 bench guard. Import-only: no mkdir/write/copy happens here.

Everything the harness may create or read-as-input is checked against the
user's real CrossHook stores (default + explicit XDG + Flatpak + configured
profile dirs) BEFORE any filesystem mutation.
"""

from __future__ import annotations

import hashlib
import os
import pwd
import stat
from dataclasses import dataclass
from pathlib import Path

import tomllib

FLATPAK_APP_ID = "dev.crosshook.CrossHook"
SYSTEM_TOPS = {"usr", "etc", "opt", "bin", "sbin", "lib", "lib64", "boot", "dev", "proc", "sys", "run"}
# (XDG var, default relative to home)
XDG_DEFAULTS = {
    "XDG_CONFIG_HOME": ".config",
    "XDG_DATA_HOME": ".local/share",
    "XDG_CACHE_HOME": ".cache",
    "XDG_STATE_HOME": ".local/state",
}
_SETTINGS_MAX = 256 * 1024


class GuardError(Exception):
    """Refusal. Raised before any write; CLI exits 10."""


def real(p: str | os.PathLike[str]) -> Path:
    """realpath: resolves symlinks in every existing ancestor, appends missing tail."""
    return Path(os.path.realpath(os.path.expanduser(os.fspath(p))))


@dataclass(frozen=True)
class Originals:
    home: str | None
    pw_home: str | None
    xdg: dict[str, str]  # explicitly set (non-empty) inherited XDG_* only
    runtime_dir: str | None

    @classmethod
    def capture(cls, env: dict[str, str] | None = None) -> Originals:
        """Prefer BENCH_ORIG_* (exported by run.sh before anything else) over live env."""
        e = os.environ if env is None else env

        def pick(name: str) -> str | None:
            v = e.get(f"BENCH_ORIG_{name}")
            if v is None:
                v = e.get(name)
            return v or None

        try:
            pw = pwd.getpwuid(os.getuid()).pw_dir or None
        except KeyError:
            pw = None
        xdg = {k: v for k in XDG_DEFAULTS if (v := pick(k))}
        return cls(pick("HOME"), pw, xdg, pick("XDG_RUNTIME_DIR"))

    def homes(self) -> list[Path]:
        return sorted({real(h) for h in (self.home, self.pw_home) if h})


def _config_homes(o: Originals) -> list[Path]:
    out = [real(o.xdg["XDG_CONFIG_HOME"])] if "XDG_CONFIG_HOME" in o.xdg else []
    out += [h / ".config" for h in o.homes()]
    return out


def configured_profile_dirs(o: Originals) -> list[Path]:
    """Read-only tomllib parse of the real settings.toml: custom profiles_directory."""
    found: list[Path] = []
    for cfg in _config_homes(o):
        f = cfg / "crosshook" / "settings.toml"
        try:
            if not f.is_file() or f.stat().st_size > _SETTINGS_MAX:
                continue
            data = tomllib.loads(f.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError):
            continue
        v = str(data.get("profiles_directory", "") or "").strip()
        if v:
            p = Path(v).expanduser()
            found.append(real(p if p.is_absolute() else cfg / "crosshook" / p))
    return found


def forbidden(o: Originals) -> list[tuple[str, Path, bool]]:
    """(label, path, block_descendants). Homes block self/ancestors only; stores block both."""
    out: list[tuple[str, Path, bool]] = []
    for h in o.homes():
        out.append(("HOME", h, False))
        for sub in (".config/crosshook", ".local/share/crosshook", ".cache/crosshook",
                    f".var/app/{FLATPAK_APP_ID}"):
            out.append((f"store:{sub}", h / sub, True))
    for var, v in o.xdg.items():
        out.append((f"{var}/crosshook", real(v) / "crosshook", True))
    for pd in configured_profile_dirs(o):
        out.append(("configured profiles_directory", pd, True))
    return out


def check_path(path: str | os.PathLike[str], what: str, o: Originals) -> Path:
    """Refuse path if empty, `/`, system dir, home/protected ancestor, or inside/equal a real store."""
    if not os.fspath(path).strip():
        raise GuardError(f"{what}: empty path")
    r = real(path)
    if r == Path(r.anchor):
        raise GuardError(f"{what}: refuses filesystem root")
    if len(r.parts) > 1 and r.parts[1] in SYSTEM_TOPS:
        raise GuardError(f"{what}: refuses system path {r}")
    for label, f, block_desc in forbidden(o):
        if r == f or r in f.parents:
            raise GuardError(f"{what}: {r} is/contains protected {label} {f}")
        if block_desc and f in r.parents:
            raise GuardError(f"{what}: {r} is inside protected {label} {f}")
    return r


def check_inherited_xdg(o: Originals) -> None:
    """Explicitly user-set XDG_* pointing at a real default location or an existing crosshook store."""
    for var, v in o.xdg.items():
        r = real(v)
        for h in o.homes():
            if r == h / XDG_DEFAULTS[var]:
                raise GuardError(f"inherited {var}={v} is the real user directory; unset it or point it elsewhere")
        if (r / "crosshook").exists():
            raise GuardError(f"inherited {var}={v} contains a real crosshook store")


def preflight(o: Originals, **paths: str | os.PathLike[str] | None) -> dict[str, Path]:
    """Run every guard; returns resolved paths. Nothing is written."""
    check_inherited_xdg(o)
    return {k: check_path(v, k, o) for k, v in paths.items() if v is not None}


# Loader/debug variables that could inject code or noise into the app under test.
# The runner sets its own LD_PRELOAD later, only for `--collect presentation`.
SCRUBBED = ("LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT", "WAYLAND_DEBUG",
            "LD_DEBUG", "LD_DEBUG_OUTPUT", "LD_PROFILE", "LD_PROFILE_OUTPUT", "GLIBC_TUNABLES",
            "GTK_MODULES", "GTK3_MODULES", "GIO_EXTRA_MODULES", "GST_PLUGIN_PATH")
# Flatpak override files that change the sandbox the app runs in, relative to the user installation.
FLATPAK_OVERRIDE_FILES = ("overrides/global", f"overrides/{FLATPAK_APP_ID}")


def _owned_real_dir(path: str | os.PathLike[str]) -> bool:
    """A real directory (not a symlink) owned by the current user."""
    try:
        st = os.lstat(path)
    except OSError:
        return False
    return stat.S_ISDIR(st.st_mode) and st.st_uid == os.getuid()


def _user_flatpak_dir(env: dict[str, str]) -> str | None:
    """The real per-user Flatpak installation, so an isolated launcher HOME can still find
    apps and runtimes installed with `--user`. Only the installation is shared, never app data.
    It must be a real directory owned by the current user, otherwise nothing is shared."""
    if env.get("FLATPAK_USER_DIR"):
        candidate = env["FLATPAK_USER_DIR"]
    else:
        data = env.get("XDG_DATA_HOME") or (str(Path(env["HOME"]) / ".local/share") if env.get("HOME") else "")
        candidate = str(Path(data) / "flatpak") if data else ""
    return candidate if candidate and _owned_real_dir(candidate) else None


def flatpak_overrides(user_dir: str | None) -> dict[str, str]:
    """sha256 of the Flatpak override files that apply to CrossHook, so provenance records them.
    Only regular files are read (never through a symlink); missing ones are left out."""
    found: dict[str, str] = {}
    if not user_dir:
        return found
    for name in FLATPAK_OVERRIDE_FILES:
        path = Path(user_dir) / name
        try:  # O_NONBLOCK so a FIFO planted here cannot hang the open
            fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
        except OSError:
            continue
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            os.close(fd)
            continue
        with os.fdopen(fd, "rb") as handle:
            found[name] = hashlib.sha256(handle.read()).hexdigest()
    return found


def build_env(root: Path, base: dict[str, str] | None = None, *, flatpak: bool = False) -> dict[str, str]:
    """Fresh isolated env. Caller creates the dirs (and runtime dir) after guards pass."""
    env = dict(os.environ if base is None else base)
    user_flatpak = _user_flatpak_dir(env) if flatpak else None
    for k in list(env):
        if k in ("HOME", "FLATPAK_USER_DIR", *XDG_DEFAULTS, *SCRUBBED) or k.startswith(("CROSSHOOK_", "BENCH_ORIG_", "HOST_XDG_")):
            del env[k]
    env["HOME"] = str(root / "home")
    env["CROSSHOOK_FLATPAK_HOST_XDG"] = "0"
    env["CROSSHOOK_BENCH"] = "1"
    if flatpak:  # flatpak overwrites XDG_* to <HOME>/.var/app/<id>/...; keep launcher XDG under HOME
        for k, rel in XDG_DEFAULTS.items():
            env[k] = str(root / "home" / rel)
        if user_flatpak:  # validated path only; an inherited value was scrubbed above
            # locate a --user install; per-app data still derives from the isolated HOME
            env["FLATPAK_USER_DIR"] = user_flatpak
    else:
        for k, sub in (("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                       ("XDG_CACHE_HOME", "cache"), ("XDG_STATE_HOME", "state")):
            env[k] = str(root / sub)
    rt = env.get("XDG_RUNTIME_DIR", "")
    if not rt or not Path(rt).is_dir():
        env["XDG_RUNTIME_DIR"] = str(root / "runtime")
    return env


def make_dirs(root: Path, env: dict[str, str]) -> None:
    """Create isolated dirs (call only after preflight). Runtime dir 0700 if we own it."""
    for v in ("HOME", *XDG_DEFAULTS):
        Path(env[v]).mkdir(parents=True, exist_ok=True)
    rt = Path(env["XDG_RUNTIME_DIR"])
    if root in rt.parents:
        rt.mkdir(mode=0o700, parents=True, exist_ok=True)
