"""
tryx-store-postgres — PostgreSQL backend for the Tryx framework.

This package provides a high-performance PostgreSQL storage implementation
that communicates with the Rust-compiled ``libtryx_postgres`` shared library
via C FFI (ctypes).

Platform detection and library loading are handled automatically by the
``_loader`` module.

Usage::

    from tryx_store_postgres import PostgresStore

    store = PostgresStore(
        host="localhost",
        port=5432,
        database="tryx",
        user="postgres",
        password="secret",
    )
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Optional

from ._loader import detect_platform, get_library_path, load_library

__all__ = ["PostgresStore", "detect_platform", "get_library_path"]


class PostgresStore:
    """
    A robust, high-performance PostgreSQL backend for the Tryx framework.
    This class conforms to the Tryx FFI Store Protocol (Duck Typing).

    The native library is resolved automatically based on the current
    platform (OS + architecture + C library variant). You may override
    the library path via the ``lib_path`` parameter.

    Parameters
    ----------
    host : str, optional
        PostgreSQL server hostname, by default "localhost".
    port : int, optional
        PostgreSQL server port, by default 5432.
    database : str, optional
        Database name, by default "tryx".
    user : str, optional
        Database user, by default "postgres".
    password : str, optional
        Database password, by default "".
    pool_min : int, optional
        Minimum number of connections in the pool, by default 2.
    pool_max : int, optional
        Maximum number of connections in the pool, by default 10.
    ssl_mode : str, optional
        SSL mode for the connection ("disable", "prefer", "require"),
        by default "prefer".
    lib_path : str or Path, optional
        Explicit path to the ``libtryx_postgres`` shared library.
        If not provided, the correct library for the current platform
        is auto-detected from the bundled ``libs/`` directory.
    """

    def __init__(
        self,
        host: str = "localhost",
        port: int = 5432,
        database: str = "tryx",
        user: str = "postgres",
        password: str = "",
        pool_min: int = 2,
        pool_max: int = 10,
        ssl_mode: str = "prefer",
        lib_path: Optional[str | Path] = None,
    ) -> None:
        # Resolve native library path
        if lib_path is not None:
            self.lib_path = str(Path(lib_path).resolve())
        else:
            self.lib_path = str(get_library_path())

        config = {
            "dsn": (
                f"host={host} port={port} dbname={database} "
                f"user={user} password={password} sslmode={ssl_mode}"
            ),
            "pool_min": pool_min,
            "pool_max": pool_max,
        }
        self.config_json = json.dumps(config)

    @property
    def platform_info(self) -> tuple[str, str, str]:
        """Return the detected (os_family, arch, variant) tuple."""
        return detect_platform()
