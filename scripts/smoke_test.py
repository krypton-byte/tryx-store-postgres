#!/usr/bin/env python3
"""
Smoke Test — End-to-end PostgreSQL FFI integration test.

Tests the full lifecycle: load .so → connect → CRUD → cleanup.
This runs WITHOUT Tryx, validating the .so works standalone.

Usage:
    python scripts/smoke_test.py path/to/libtryx_postgres-*.so
    PG_DSN="host=localhost dbname=tryx user=postgres" python scripts/smoke_test.py ...

Environment:
    PG_DSN  — PostgreSQL DSN (default: host=localhost port=5432 ...)
"""

from __future__ import annotations

import ctypes
import json
import os
import sys
import time
from pathlib import Path

G = "\033[0;32m"
R = "\033[0;31m"
Y = "\033[0;33m"
C = "\033[0;36m"
B = "\033[1m"
N = "\033[0m"

DEFAULT_DSN = "host=localhost port=5432 dbname=tryx user=postgres password= sslmode=disable"


class TryxBuffer(ctypes.Structure):
    _fields_ = [("data", ctypes.POINTER(ctypes.c_uint8)), ("len", ctypes.c_size_t)]

    def to_bytes(self) -> bytes | None:
        if not self.data or self.len == 0:
            return None
        return bytes(ctypes.cast(self.data, ctypes.POINTER(ctypes.c_uint8 * self.len)).contents)


class FFIStore:
    """Thin ctypes wrapper around the tryx_store C ABI."""

    def __init__(self, lib_path: str, dsn: str):
        self.lib = ctypes.CDLL(lib_path)
        self._setup_prototypes()

        config = json.dumps({"dsn": dsn, "pool_min": 1, "pool_max": 3})
        self.handle = ctypes.c_void_p()
        rc = self.lib.tryx_store_connect(
            config.encode("utf-8"), ctypes.byref(self.handle)
        )
        if rc != 0:
            raise RuntimeError(f"tryx_store_connect failed (rc={rc})")

    def _setup_prototypes(self):
        L = self.lib
        # connect
        L.tryx_store_connect.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_void_p)]
        L.tryx_store_connect.restype = ctypes.c_int
        # destroy
        L.tryx_store_destroy.argtypes = [ctypes.c_void_p]
        L.tryx_store_destroy.restype = None
        # free_buffer
        L.tryx_store_free_buffer.argtypes = [TryxBuffer]
        L.tryx_store_free_buffer.restype = None
        # identity
        L.tryx_store_put_identity.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t]
        L.tryx_store_put_identity.restype = ctypes.c_int
        L.tryx_store_load_identity.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.POINTER(TryxBuffer)]
        L.tryx_store_load_identity.restype = ctypes.c_int
        L.tryx_store_delete_identity.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        L.tryx_store_delete_identity.restype = ctypes.c_int
        # session
        L.tryx_store_put_session.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t]
        L.tryx_store_put_session.restype = ctypes.c_int
        L.tryx_store_get_session.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.POINTER(TryxBuffer)]
        L.tryx_store_get_session.restype = ctypes.c_int
        L.tryx_store_delete_session.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        L.tryx_store_delete_session.restype = ctypes.c_int
        # prekey
        L.tryx_store_store_prekey.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.c_int]
        L.tryx_store_store_prekey.restype = ctypes.c_int
        L.tryx_store_load_prekey.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(TryxBuffer)]
        L.tryx_store_load_prekey.restype = ctypes.c_int
        L.tryx_store_remove_prekey.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        L.tryx_store_remove_prekey.restype = ctypes.c_int
        L.tryx_store_get_max_prekey_id.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32)]
        L.tryx_store_get_max_prekey_id.restype = ctypes.c_int
        # device
        L.tryx_store_device_exists.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int)]
        L.tryx_store_device_exists.restype = ctypes.c_int
        L.tryx_store_create_device.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_int)]
        L.tryx_store_create_device.restype = ctypes.c_int
        # call
        L.tryx_store_call.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.POINTER(TryxBuffer)]
        L.tryx_store_call.restype = ctypes.c_int

    def close(self):
        if self.handle:
            self.lib.tryx_store_destroy(self.handle)
            self.handle = None

    def __del__(self):
        self.close()


class SmokeTest:
    def __init__(self, store: FFIStore):
        self.store = store
        self.passed = 0
        self.failed = 0

    def _ok(self, name: str, detail: str = ""):
        self.passed += 1
        extra = f" ({detail})" if detail else ""
        print(f"   {G}✓{N} {name}{extra}")

    def _fail(self, name: str, detail: str = ""):
        self.failed += 1
        extra = f" ({detail})" if detail else ""
        print(f"   {R}✗{N} {name}{extra}")

    def test_identity(self):
        h = self.store.handle
        L = self.store.lib
        addr = b"test@s.whatsapp.net"
        key = bytes(range(32))
        key_arr = (ctypes.c_uint8 * 32)(*key)

        rc = L.tryx_store_put_identity(h, addr, key_arr, 32)
        if rc == 0:
            self._ok("put_identity")
        else:
            self._fail("put_identity", f"rc={rc}")
            return

        buf = TryxBuffer()
        rc = L.tryx_store_load_identity(h, addr, ctypes.byref(buf))
        if rc == 0 and buf.to_bytes() == key:
            self._ok("load_identity", "data matches")
        else:
            self._fail("load_identity", f"rc={rc}")

        rc = L.tryx_store_delete_identity(h, addr)
        if rc == 0:
            self._ok("delete_identity")
        else:
            self._fail("delete_identity", f"rc={rc}")

    def test_session(self):
        h = self.store.handle
        L = self.store.lib
        addr = b"session-test@s.whatsapp.net"
        data = b"session-record-data-12345"
        data_arr = (ctypes.c_uint8 * len(data))(*data)

        rc = L.tryx_store_put_session(h, addr, data_arr, len(data))
        if rc == 0:
            self._ok("put_session")
        else:
            self._fail("put_session", f"rc={rc}")
            return

        buf = TryxBuffer()
        rc = L.tryx_store_get_session(h, addr, ctypes.byref(buf))
        if rc == 0 and buf.to_bytes() == data:
            self._ok("get_session", "data matches")
        else:
            self._fail("get_session", f"rc={rc}")

        rc = L.tryx_store_delete_session(h, addr)
        if rc == 0:
            self._ok("delete_session")
        else:
            self._fail("delete_session", f"rc={rc}")

    def test_prekey(self):
        h = self.store.handle
        L = self.store.lib
        pk_data = b"prekey-record-bytes"
        pk_arr = (ctypes.c_uint8 * len(pk_data))(*pk_data)

        rc = L.tryx_store_store_prekey(h, 42, pk_arr, len(pk_data), 0)
        if rc == 0:
            self._ok("store_prekey")
        else:
            self._fail("store_prekey", f"rc={rc}")
            return

        buf = TryxBuffer()
        rc = L.tryx_store_load_prekey(h, 42, ctypes.byref(buf))
        if rc == 0 and buf.to_bytes() == pk_data:
            self._ok("load_prekey", "data matches")
        else:
            self._fail("load_prekey", f"rc={rc}")

        out_id = ctypes.c_uint32()
        rc = L.tryx_store_get_max_prekey_id(h, ctypes.byref(out_id))
        if rc == 0 and out_id.value >= 42:
            self._ok("get_max_prekey_id", f"id={out_id.value}")
        else:
            self._fail("get_max_prekey_id", f"rc={rc}, id={out_id.value}")

        rc = L.tryx_store_remove_prekey(h, 42)
        if rc == 0:
            self._ok("remove_prekey")
        else:
            self._fail("remove_prekey", f"rc={rc}")

    def test_device(self):
        h = self.store.handle
        L = self.store.lib

        out = ctypes.c_int()
        rc = L.tryx_store_create_device(h, ctypes.byref(out))
        if rc == 0:
            self._ok("create_device", f"id={out.value}")
        else:
            self._fail("create_device", f"rc={rc}")

        exists = ctypes.c_int()
        rc = L.tryx_store_device_exists(h, ctypes.byref(exists))
        if rc == 0 and exists.value != 0:
            self._ok("device_exists", "exists=true")
        else:
            self._fail("device_exists", f"rc={rc}, val={exists.value}")

    def run_all(self) -> int:
        print(f"\n{C}{B}═══ Identity Store ═══{N}")
        self.test_identity()
        print(f"\n{C}{B}═══ Session Store ═══{N}")
        self.test_session()
        print(f"\n{C}{B}═══ PreKey Store ═══{N}")
        self.test_prekey()
        print(f"\n{C}{B}═══ Device Store ═══{N}")
        self.test_device()

        total = self.passed + self.failed
        print(f"\n{'━' * 45}")
        if self.failed == 0:
            print(f"{G}{B}✅ ALL {total} TESTS PASSED{N}")
            return 0
        else:
            print(f"{R}{B}❌ {self.failed}/{total} TESTS FAILED{N}")
            return 1


def main() -> int:
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <path-to-libtryx_postgres.so>")
        print(f"\nSet PG_DSN env var for custom PostgreSQL connection.")
        return 2

    lib_path = sys.argv[1]
    dsn = os.environ.get("PG_DSN", DEFAULT_DSN)

    print(f"{C}{B}🧪 tryx-store-postgres Smoke Test{N}")
    print(f"{'━' * 45}")
    print(f"   Library: {Path(lib_path).name}")
    print(f"   DSN:     {dsn[:60]}...")

    try:
        store = FFIStore(lib_path, dsn)
        print(f"   {G}✓ Connected to PostgreSQL{N}")
    except Exception as e:
        print(f"   {R}✗ Connection failed: {e}{N}")
        print(f"\n{Y}Hint: Make sure PostgreSQL is running and the database exists.{N}")
        print(f"{Y}  createdb tryx  # create the database{N}")
        return 1

    tests = SmokeTest(store)
    rc = tests.run_all()
    store.close()
    return rc


if __name__ == "__main__":
    sys.exit(main())
