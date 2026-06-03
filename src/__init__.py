import json

class PostgresStore:
    """
    A robust, high-performance PostgreSQL backend for the Tryx framework.
    This class conforms to the Tryx FFI Store Protocol (Duck Typing).
    
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
        SSL mode for the connection ("disable", "prefer", "require"), by default "prefer".
    lib_path : str, optional
        Path to the `libtryx_postgres.so` compiled library.
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
        lib_path: str = "libtryx_postgres.so"
    ) -> None:
        self.lib_path = lib_path
        config = {
            "dsn": f"host={host} port={port} dbname={database} user={user} password={password} sslmode={ssl_mode}",
            "pool_min": pool_min,
            "pool_max": pool_max
        }
        self.config_json = json.dumps(config)
