#!/usr/bin/env python3
"""
ABI Conformance Verifier for tryx-store-postgres.

Validates that a compiled .so exports ALL required FFI symbols
that Tryx (karat) expects via FfiBridgeStore::connect().

Usage:
    python scripts/verify_abi.py path/to/libtryx_postgres-*.so
"""

from __future__ import annotations

import ctypes
import sys
from pathlib import Path

REQUIRED_SYMBOLS: list[str] = [
    "tryx_store_connect",
    "tryx_store_destroy",
    "tryx_store_free_buffer",
    "tryx_store_put_identity",
    "tryx_store_load_identity",
    "tryx_store_delete_identity",
    "tryx_store_get_session",
    "tryx_store_put_session",
    "tryx_store_delete_session",
    "tryx_store_store_prekey",
    "tryx_store_load_prekey",
    "tryx_store_remove_prekey",
    "tryx_store_get_max_prekey_id",
    "tryx_store_store_signed_prekey",
    "tryx_store_load_signed_prekey",
    "tryx_store_remove_signed_prekey",
    "tryx_store_put_sender_key",
    "tryx_store_get_sender_key",
    "tryx_store_delete_sender_key",
    "tryx_store_get_sync_key",
    "tryx_store_set_sync_key",
    "tryx_store_get_version",
    "tryx_store_set_version",
    "tryx_store_get_latest_sync_key_id",
    "tryx_store_save_device",
    "tryx_store_load_device",
    "tryx_store_device_exists",
    "tryx_store_create_device",
    "tryx_store_call",
]

G = "\033[0;32m"
R = "\033[0;31m"
Y = "\033[0;33m"
C = "\033[0;36m"
B = "\033[1m"
N = "\033[0m"


def verify_library(lib_path: str) -> int:
    path = Path(lib_path)
    if not path.exists():
        print(f"{R}❌ Library not found: {lib_path}{N}")
        return 2

    size_mb = path.stat().st_size / (1024 * 1024)
    print(f"{C}   Library: {path.name} ({size_mb:.1f} MB){N}")

    try:
        lib = ctypes.CDLL(str(path))
    except OSError as e:
        print(f"{R}❌ Failed to load library: {e}{N}")
        return 2

    found = 0
    missing: list[str] = []

    for sym in REQUIRED_SYMBOLS:
        try:
            getattr(lib, sym)
            found += 1
            print(f"   {G}✓{N} {sym}")
        except AttributeError:
            missing.append(sym)
            print(f"   {R}✗{N} {sym} {R}(MISSING){N}")

    print()
    total = len(REQUIRED_SYMBOLS)
    if missing:
        print(f"{R}{B}❌ ABI FAILED: {found}/{total} found, {len(missing)} missing{N}")
        for sym in missing:
            print(f"   - {sym}")
        return 1
    else:
        print(f"{G}{B}✅ ABI PASSED: All {total} symbols verified{N}")
        return 0


def main() -> int:
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <path-to-libtryx_postgres.so>")
        return 2
    return verify_library(sys.argv[1])


if __name__ == "__main__":
    sys.exit(main())
