# tryx-store-postgres

PostgreSQL storage backend for the [Tryx](https://github.com/krypton-byte/tryx) WhatsApp automation framework.

This package provides a high-performance PostgreSQL implementation of Tryx's store interface via **C FFI** — the Rust-compiled shared library (`.so`) is loaded by Tryx at runtime through `libloading`, bypassing Python entirely for all database operations.

## Architecture

```
┌─────────────────────┐     duck-typing      ┌───────────────────┐
│  Python User Code   │ ──── lib_path ──────▶ │  Tryx (Rust/PyO3) │
│  PostgresStore()    │      config_json      │  FfiBridgeStore   │
└─────────────────────┘                       └────────┬──────────┘
                                                       │ libloading
                                              ┌────────▼──────────┐
                                              │  libtryx_postgres  │
                                              │  (Rust cdylib .so) │
                                              └────────┬──────────┘
                                                       │ deadpool-postgres
                                              ┌────────▼──────────┐
                                              │    PostgreSQL      │
                                              └───────────────────┘
```

**Key insight:** Python **never** loads the `.so` directly. Python only resolves the path and stores connection config — Tryx's Rust runtime does the actual FFI loading.

## Quick Start

```python
from tryx_store_postgres import PostgresStore
from tryx.client import Tryx

store = PostgresStore(
    host="localhost",
    port=5432,
    database="tryx",
    user="postgres",
    password="secret",
)

client = Tryx(store)
```

## Development

### Prerequisites

- **Rust** (stable) — `rustup install stable`
- **PostgreSQL** — running locally with a `tryx` database
- **Python 3.10+** — with `pip`

### Build Commands

```bash
# Build .so (release) and auto-stage to correct location
make build

# Build .so (debug, much faster compile)
make build-debug

# Validate all 29 FFI symbols are exported
make verify

# Run CRUD smoke test against PostgreSQL
make test-db

# Run Rust integration tests
make test-rust

# Build + verify + install in development mode
make install-dev

# Full cycle: build + verify
make all

# Clean everything
make clean

# Show all available commands
make help
```

### Manual Build Steps

If you prefer not to use `make`:

```bash
# 1. Build the Rust library
cd tryx-postgres-rs
cargo build --release

# 2. Stage with correct platform name
#    Format: libtryx_postgres-{os}-{arch}-{variant}.so
cp target/release/libtryx_postgres.so \
   ../src/tryx_store_postgres/libs/libtryx_postgres-linux-x86_64-glibc.so

# 3. Verify symbols
python3 ../scripts/verify_abi.py \
   ../src/tryx_store_postgres/libs/libtryx_postgres-linux-x86_64-glibc.so

# 4. Install in dev mode
cd ..
pip install -e .
```

### Testing Without Tryx

The smoke test validates the `.so` works standalone:

```bash
# Ensure PostgreSQL has a test database
createdb tryx

# Run the full CRUD test
PG_DSN="host=localhost dbname=tryx user=postgres" \
    python3 scripts/smoke_test.py \
    src/tryx_store_postgres/libs/libtryx_postgres-linux-x86_64-glibc.so
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PG_DSN` | `host=localhost port=5432 dbname=tryx user=postgres password= sslmode=disable` | PostgreSQL connection string |
| `CARGO_PROFILE` | `release` | Cargo build profile (`release` or `debug`) |

## Project Structure

```
tryx-store-postgres/
├── Makefile                    # Developer workflow commands
├── pyproject.toml              # Python package config
├── src/
│   └── tryx_store_postgres/
│       ├── __init__.py         # PostgresStore class (Python)
│       ├── _loader.py          # Platform detection + .so path resolver
│       └── libs/               # Pre-compiled .so files (per platform)
├── tryx-postgres-rs/           # Rust library (cdylib)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs              # C ABI exports (29 symbols)
│   │   ├── pg.rs               # PostgreSQL implementation
│   │   └── schema.sql          # Database schema (auto-migrated)
│   └── tests/
│       └── integration.rs      # Standalone Rust tests
├── scripts/
│   ├── verify_abi.py           # FFI symbol validator
│   ├── smoke_test.py           # End-to-end PostgreSQL test
│   └── build_wheels.py         # CI wheel builder
└── .github/workflows/
    └── build.yml               # CI/CD pipeline
```

## FFI Contract

The `.so` must export exactly these 29 C ABI symbols:

| Category | Symbols |
|----------|---------|
| **Lifecycle** | `tryx_store_connect`, `tryx_store_destroy`, `tryx_store_free_buffer` |
| **Identity** | `tryx_store_put_identity`, `tryx_store_load_identity`, `tryx_store_delete_identity` |
| **Session** | `tryx_store_get_session`, `tryx_store_put_session`, `tryx_store_delete_session` |
| **PreKey** | `tryx_store_store_prekey`, `tryx_store_load_prekey`, `tryx_store_remove_prekey`, `tryx_store_get_max_prekey_id` |
| **Signed PreKey** | `tryx_store_store_signed_prekey`, `tryx_store_load_signed_prekey`, `tryx_store_remove_signed_prekey` |
| **Sender Key** | `tryx_store_put_sender_key`, `tryx_store_get_sender_key`, `tryx_store_delete_sender_key` |
| **AppSync** | `tryx_store_get_sync_key`, `tryx_store_set_sync_key`, `tryx_store_get_version`, `tryx_store_set_version`, `tryx_store_get_latest_sync_key_id` |
| **Device** | `tryx_store_save_device`, `tryx_store_load_device`, `tryx_store_device_exists`, `tryx_store_create_device` |
| **Generic** | `tryx_store_call` |

Run `make verify` to validate all symbols are present.

## License

MIT
