#!/usr/bin/env python3
"""
Build platform-specific Python wheels for tryx-store-postgres.

This script:
  1. Discovers all compiled .so libraries from the CI artifacts directory
  2. Maps each library to its correct Python platform wheel tag
  3. Generates one wheel per platform containing only that platform's library
  4. Produces a source distribution (sdist) with no bundled libraries

Usage (CI):
    python scripts/build_wheels.py

The script expects libraries in:
    artifacts/lib-<label>/libtryx_postgres-<os>-<arch>[-<variant>].so
"""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import NamedTuple

# ── Configuration ──────────────────────────────────────────────────
PROJECT_ROOT = Path(__file__).resolve().parent.parent
ARTIFACTS_DIR = PROJECT_ROOT / "artifacts"
DIST_DIR = PROJECT_ROOT / "dist"
PACKAGE_NAME = "tryx_store_postgres"
SRC_DIR = PROJECT_ROOT / "src"

# ── Platform mapping ──────────────────────────────────────────────
# Maps (os_family, arch, variant) → Python platform tag
PLATFORM_MAP: dict[tuple[str, str, str], str] = {
    # Linux musl
    ("linux", "x86_64",  "musl"):        "musllinux_1_2_x86_64",
    ("linux", "aarch64", "musl"):        "musllinux_1_2_aarch64",
    ("linux", "armv7l",  "musleabihf"):  "musllinux_1_2_armv7l",
    ("linux", "i686",    "musl"):        "musllinux_1_2_i686",
    ("linux", "s390x",   "musl"):        "musllinux_1_2_s390x",
    ("linux", "ppc64le", "musl"):        "musllinux_1_2_ppc64le",
    # Android
    ("android", "aarch64", ""):  "linux_aarch64",   # Android wheels use linux tags
    ("android", "armv7l",  ""):  "linux_armv7l",
    ("android", "x86_64",  ""):  "linux_x86_64",
    ("android", "i686",    ""):  "linux_i686",
}


class LibraryInfo(NamedTuple):
    """Parsed info from a library filename."""
    path: Path
    os_family: str
    arch: str
    variant: str
    label: str          # e.g. "linux-x86_64-musl" or "android-aarch64"
    platform_tag: str   # e.g. "musllinux_1_2_x86_64"


# Library filename pattern:
# libtryx_postgres-<os>-<arch>[-<variant>].so
LIB_RE = re.compile(
    r"^libtryx_postgres-"
    r"(?P<os>[a-z]+)-"
    r"(?P<arch>[a-z0-9_]+)"
    r"(?:-(?P<variant>[a-z0-9]+))?"
    r"\.so$"
)


def discover_libraries() -> list[LibraryInfo]:
    """Find all .so files in the artifacts directory and parse their metadata."""
    libs: list[LibraryInfo] = []

    if not ARTIFACTS_DIR.exists():
        print(f"⚠  Artifacts directory not found: {ARTIFACTS_DIR}")
        return libs

    for so_file in sorted(ARTIFACTS_DIR.rglob("*.so")):
        m = LIB_RE.match(so_file.name)
        if not m:
            print(f"⚠  Skipping unrecognized library: {so_file.name}")
            continue

        os_family = m.group("os")
        arch = m.group("arch")
        variant = m.group("variant") or ""
        label = f"{os_family}-{arch}"
        if variant:
            label += f"-{variant}"

        key = (os_family, arch, variant)
        platform_tag = PLATFORM_MAP.get(key)
        if not platform_tag:
            print(f"⚠  No platform mapping for {key}, skipping {so_file.name}")
            continue

        libs.append(LibraryInfo(
            path=so_file,
            os_family=os_family,
            arch=arch,
            variant=variant,
            label=label,
            platform_tag=platform_tag,
        ))
        size_mb = so_file.stat().st_size / (1024 * 1024)
        print(f"✓  Found: {so_file.name} → {platform_tag} ({size_mb:.1f} MB)")

    return libs


def read_version() -> str:
    """Read version from pyproject.toml."""
    pyproject = PROJECT_ROOT / "pyproject.toml"
    content = pyproject.read_text()
    m = re.search(r'^version\s*=\s*"([^"]+)"', content, re.MULTILINE)
    if not m:
        raise ValueError("Could not parse version from pyproject.toml")
    return m.group(1)


def build_wheel(lib: LibraryInfo, version: str) -> Path:
    """
    Build a platform-specific wheel containing the given library.

    The wheel is a ZIP file with the structure:
        {package_name}/
            __init__.py
            _loader.py
            libs/
                libtryx_postgres-{label}.so
        {dist_info}/
            METADATA
            WHEEL
            RECORD
    """
    dist_name = "tryx_store_postgres"
    wheel_tag = f"py3-none-{lib.platform_tag}"
    wheel_name = f"{dist_name}-{version}-{wheel_tag}.whl"
    wheel_path = DIST_DIR / wheel_name

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)

        # Package directory
        pkg_dir = tmp / PACKAGE_NAME
        libs_dir = pkg_dir / "libs"
        libs_dir.mkdir(parents=True)

        # Copy Python source files
        for py_file in (SRC_DIR / PACKAGE_NAME).rglob("*.py"):
            rel = py_file.relative_to(SRC_DIR / PACKAGE_NAME)
            dest = pkg_dir / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(py_file, dest)

        # If __init__.py is in src/ directly (flat layout), copy that
        src_init = SRC_DIR / "__init__.py"
        if src_init.exists() and not (pkg_dir / "__init__.py").exists():
            shutil.copy2(src_init, pkg_dir / "__init__.py")

        # Copy _loader.py
        loader_src = SRC_DIR / PACKAGE_NAME / "_loader.py"
        if not loader_src.exists():
            # Fallback: generate from the root src/_loader.py
            loader_src = SRC_DIR / "_loader.py"
        if loader_src.exists():
            shutil.copy2(loader_src, pkg_dir / "_loader.py")

        # Copy the library
        shutil.copy2(lib.path, libs_dir / lib.path.name)

        # Dist-info
        dist_info = tmp / f"{dist_name}-{version}.dist-info"
        dist_info.mkdir()

        # METADATA
        (dist_info / "METADATA").write_text(
            f"Metadata-Version: 2.4\n"
            f"Name: tryx-store-postgres\n"
            f"Version: {version}\n"
            f"Summary: PostgreSQL backend for the Tryx framework (FFI)\n"
            f"Requires-Python: >=3.10\n"
            f"License: MIT\n"
        )

        # WHEEL
        (dist_info / "WHEEL").write_text(
            f"Wheel-Version: 1.0\n"
            f"Generator: tryx-build-wheels\n"
            f"Root-Is-Purelib: false\n"
            f"Tag: {wheel_tag}\n"
        )

        # RECORD (list all files with sha256 hash)
        records: list[str] = []
        for f in sorted(tmp.rglob("*")):
            if f.is_file() and f.name != "RECORD":
                rel_path = f.relative_to(tmp)
                sha = hashlib.sha256(f.read_bytes()).hexdigest()
                size = f.stat().st_size
                records.append(f"{rel_path},sha256={sha},{size}")

        # RECORD itself has no hash
        record_path = dist_info / "RECORD"
        records.append(f"{record_path.relative_to(tmp)},,")
        record_path.write_text("\n".join(records) + "\n")

        # Build ZIP (wheel)
        DIST_DIR.mkdir(parents=True, exist_ok=True)
        import zipfile
        with zipfile.ZipFile(wheel_path, "w", zipfile.ZIP_DEFLATED) as zf:
            for f in sorted(tmp.rglob("*")):
                if f.is_file():
                    zf.write(f, f.relative_to(tmp))

    size_mb = wheel_path.stat().st_size / (1024 * 1024)
    print(f"📦 Built: {wheel_name} ({size_mb:.1f} MB)")
    return wheel_path


def main() -> int:
    print("=" * 60)
    print("  tryx-store-postgres wheel builder")
    print("=" * 60)

    version = read_version()
    print(f"\n📋 Version: {version}")

    print(f"\n🔍 Discovering libraries in {ARTIFACTS_DIR}...")
    libs = discover_libraries()

    if not libs:
        print("\n❌ No libraries found! Aborting.")
        return 1

    print(f"\n🏗  Building {len(libs)} platform wheels...\n")
    wheels: list[Path] = []
    for lib in libs:
        try:
            wheel = build_wheel(lib, version)
            wheels.append(wheel)
        except Exception as e:
            print(f"❌ Failed to build wheel for {lib.label}: {e}")
            return 1

    print(f"\n✅ Successfully built {len(wheels)} wheels:")
    for w in wheels:
        print(f"   • {w.name}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
