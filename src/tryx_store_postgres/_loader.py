"""
Platform detection and native library loader for tryx-store-postgres.

Automatically detects the current OS, architecture, and C library variant
to load the correct pre-compiled ``libtryx_postgres`` shared library.

Supported platforms:
    - Linux musl: x86_64, aarch64, armv7l, i686, s390x, ppc64le
    - Android:    aarch64, armv7l, x86_64, i686
"""

from __future__ import annotations

import ctypes
import os
import platform
import struct
import sys
from pathlib import Path
from typing import Optional

__all__ = ["load_library", "get_library_path", "detect_platform"]

# ── Constants ──────────────────────────────────────────────────────
_LIBS_DIR = Path(__file__).parent / "libs"

# Machine → normalized arch name
_ARCH_MAP: dict[str, str] = {
    "x86_64":   "x86_64",
    "amd64":    "x86_64",
    "aarch64":  "aarch64",
    "arm64":    "aarch64",
    "armv7l":   "armv7l",
    "armv8l":   "armv7l",  # 32-bit compat on aarch64
    "i686":     "i686",
    "i386":     "i686",
    "s390x":    "s390x",
    "ppc64le":  "ppc64le",
}


def _is_android() -> bool:
    """Detect if running on Android (e.g. via Termux, Chaquopy, etc.)."""
    # Android sets specific env vars and has /system/build.prop
    if "ANDROID_ROOT" in os.environ or "ANDROID_DATA" in os.environ:
        return True
    if Path("/system/build.prop").exists():
        return True
    # Termux
    if "com.termux" in os.environ.get("PREFIX", ""):
        return True
    return False


def _is_musl() -> bool:
    """Detect if the system uses musl libc (vs glibc)."""
    try:
        import subprocess
        result = subprocess.run(
            ["ldd", "--version"],
            capture_output=True, text=True, timeout=5
        )
        output = result.stdout + result.stderr
        return "musl" in output.lower()
    except Exception:
        pass

    # Fallback: check if libc.musl-*.so exists
    libc_paths = list(Path("/lib").glob("libc.musl-*")) + \
                 list(Path("/lib64").glob("libc.musl-*"))
    if libc_paths:
        return True

    # Alpine detection
    if Path("/etc/alpine-release").exists():
        return True

    return False


def detect_platform() -> tuple[str, str, str]:
    """
    Detect the current platform.

    Returns:
        Tuple of (os_family, arch, variant).
        Example: ("linux", "x86_64", "musl") or ("android", "aarch64", "")
    """
    machine = platform.machine().lower()
    arch = _ARCH_MAP.get(machine)
    if not arch:
        raise RuntimeError(
            f"Unsupported architecture: {machine}. "
            f"Supported: {', '.join(sorted(_ARCH_MAP.keys()))}"
        )

    if _is_android():
        return ("android", arch, "")

    if sys.platform.startswith("linux"):
        if _is_musl():
            # armv7l with hard float
            variant = "musleabihf" if arch == "armv7l" else "musl"
            return ("linux", arch, variant)
        else:
            # glibc — try musl library anyway (statically linked musl .so
            # should work on glibc systems too)
            variant = "musleabihf" if arch == "armv7l" else "musl"
            return ("linux", arch, variant)

    raise RuntimeError(
        f"Unsupported platform: {sys.platform} ({machine}). "
        f"Only Linux and Android are supported."
    )


def _build_library_name(os_family: str, arch: str, variant: str) -> str:
    """Build the library filename from platform info."""
    if variant:
        return f"libtryx_postgres-{os_family}-{arch}-{variant}.so"
    return f"libtryx_postgres-{os_family}-{arch}.so"


def get_library_path(
    os_family: Optional[str] = None,
    arch: Optional[str] = None,
    variant: Optional[str] = None,
) -> Path:
    """
    Get the path to the native library for the given or detected platform.

    Parameters
    ----------
    os_family : str, optional
        Override OS detection ("linux" or "android").
    arch : str, optional
        Override architecture detection.
    variant : str, optional
        Override variant detection ("musl", "musleabihf", or "").

    Returns
    -------
    Path
        Absolute path to the .so library.

    Raises
    ------
    RuntimeError
        If no library is found for the platform.
    """
    if os_family is None or arch is None or variant is None:
        detected = detect_platform()
        os_family = os_family or detected[0]
        arch = arch or detected[1]
        variant = variant if variant is not None else detected[2]

    lib_name = _build_library_name(os_family, arch, variant)
    lib_path = _LIBS_DIR / lib_name

    if not lib_path.exists():
        # List available libraries for helpful error message
        available = sorted(f.name for f in _LIBS_DIR.glob("*.so")) if _LIBS_DIR.exists() else []
        available_str = "\n  ".join(available) if available else "(none)"
        raise RuntimeError(
            f"Native library not found: {lib_name}\n"
            f"Expected at: {lib_path}\n"
            f"Detected platform: os={os_family}, arch={arch}, variant={variant}\n"
            f"Available libraries:\n  {available_str}"
        )

    return lib_path


def load_library(
    lib_path: Optional[str | Path] = None,
) -> ctypes.CDLL:
    """
    Load the native tryx_postgres shared library.

    Parameters
    ----------
    lib_path : str or Path, optional
        Explicit path to the .so file. If not provided, auto-detects
        the correct library for the current platform.

    Returns
    -------
    ctypes.CDLL
        Loaded shared library handle.
    """
    if lib_path is None:
        lib_path = get_library_path()
    else:
        lib_path = Path(lib_path)
        if not lib_path.exists():
            raise FileNotFoundError(f"Library not found: {lib_path}")

    return ctypes.CDLL(str(lib_path))
