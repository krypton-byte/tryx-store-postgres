# ============================================================================
# tryx-store-postgres — Developer Makefile
# ============================================================================
#
# Quick reference:
#   make build        → Build .so in release mode + auto-stage
#   make build-debug  → Build .so in debug mode (faster compile)
#   make verify       → Validate all FFI symbols are present
#   make test-db      → Run full CRUD smoke test against PostgreSQL
#   make test-rust    → Run Rust-level integration tests
#   make install-dev  → Install package in editable mode with .so
#   make clean        → Remove build artifacts
#   make all          → build + verify + test-db (full cycle)
#
# Environment variables:
#   PG_DSN            → PostgreSQL connection string (default: see below)
#   CARGO_PROFILE     → Cargo profile: release|debug (default: release)
#
# ============================================================================

.PHONY: all build build-debug verify test-db test-rust install-dev clean help stage-lib

# ── Configuration ──────────────────────────────────────────────────────────────
CARGO_PROFILE   ?= release
RUST_DIR        := tryx-postgres-rs
RUST_TARGET_DIR := $(RUST_DIR)/target/$(CARGO_PROFILE)
LIB_NAME        := libtryx_postgres.so
LIBS_DIR        := src/tryx_store_postgres/libs

# Detect platform for staging
OS_FAMILY       := $(shell python3 -c "import sys; print('linux' if sys.platform.startswith('linux') else 'unknown')")
ARCH            := $(shell python3 -c "import platform; print(platform.machine().lower())")
VARIANT         := $(shell python3 -c "\
import subprocess, pathlib; \
r = subprocess.run(['ldd','--version'], capture_output=True, text=True); \
o = r.stdout + r.stderr; \
print('musl' if 'musl' in o.lower() else 'glibc')" 2>/dev/null || echo "glibc")
STAGED_LIB_NAME := libtryx_postgres-$(OS_FAMILY)-$(ARCH)-$(VARIANT).so

PG_DSN          ?= host=localhost port=5432 dbname=tryx user=postgres password= sslmode=disable

# ── Colors ─────────────────────────────────────────────────────────────────────
GREEN  := \033[0;32m
YELLOW := \033[0;33m
RED    := \033[0;31m
CYAN   := \033[0;36m
BOLD   := \033[1m
RESET  := \033[0m

# ── Default target ─────────────────────────────────────────────────────────────
all: build verify
	@echo ""
	@echo "$(GREEN)$(BOLD)✅ All checks passed!$(RESET)"
	@echo "   Run $(CYAN)make test-db$(RESET) to test against PostgreSQL"
	@echo "   Run $(CYAN)make install-dev$(RESET) to install in development mode"

# ── Build ──────────────────────────────────────────────────────────────────────
build:
	@echo "$(CYAN)$(BOLD)🔨 Building $(LIB_NAME) ($(CARGO_PROFILE))...$(RESET)"
	cd $(RUST_DIR) && cargo build --$(CARGO_PROFILE)
	@$(MAKE) --no-print-directory stage-lib
	@echo "$(GREEN)✓ Build complete$(RESET)"

build-debug:
	@echo "$(CYAN)$(BOLD)🔨 Building $(LIB_NAME) (debug)...$(RESET)"
	cd $(RUST_DIR) && cargo build
	@$(MAKE) --no-print-directory stage-lib CARGO_PROFILE=debug
	@echo "$(GREEN)✓ Debug build complete$(RESET)"

stage-lib:
	@echo "$(CYAN)📦 Staging library...$(RESET)"
	@mkdir -p $(LIBS_DIR)
	@SRC="$(RUST_DIR)/target/$(CARGO_PROFILE)/$(LIB_NAME)"; \
	if [ ! -f "$$SRC" ]; then \
		echo "$(RED)❌ Build output not found: $$SRC$(RESET)"; \
		exit 1; \
	fi; \
	cp "$$SRC" "$(LIBS_DIR)/$(STAGED_LIB_NAME)"; \
	SIZE=$$(du -h "$(LIBS_DIR)/$(STAGED_LIB_NAME)" | cut -f1); \
	echo "$(GREEN)   ✓ Staged: $(LIBS_DIR)/$(STAGED_LIB_NAME) ($$SIZE)$(RESET)"

# ── Verify ABI ─────────────────────────────────────────────────────────────────
verify:
	@echo "$(CYAN)$(BOLD)🔍 Verifying FFI ABI conformance...$(RESET)"
	@python3 scripts/verify_abi.py "$(LIBS_DIR)/$(STAGED_LIB_NAME)"

# ── Test (Python smoke test) ───────────────────────────────────────────────────
test-db:
	@echo "$(CYAN)$(BOLD)🧪 Running PostgreSQL smoke test...$(RESET)"
	@PG_DSN="$(PG_DSN)" python3 scripts/smoke_test.py "$(LIBS_DIR)/$(STAGED_LIB_NAME)"

# ── Test (Rust integration) ───────────────────────────────────────────────────
test-rust:
	@echo "$(CYAN)$(BOLD)🦀 Running Rust integration tests...$(RESET)"
	cd $(RUST_DIR) && PG_DSN="$(PG_DSN)" cargo test -- --nocapture

# ── Install (development mode) ─────────────────────────────────────────────────
install-dev:
	@echo "$(CYAN)$(BOLD)📥 Installing in development mode...$(RESET)"
	@$(MAKE) --no-print-directory build
	@$(MAKE) --no-print-directory verify
	pip install -e .
	@echo "$(GREEN)✓ Installed tryx-store-postgres (dev mode)$(RESET)"
	@echo "$(YELLOW)  Library: $(LIBS_DIR)/$(STAGED_LIB_NAME)$(RESET)"

# ── Clean ──────────────────────────────────────────────────────────────────────
clean:
	@echo "$(CYAN)🧹 Cleaning...$(RESET)"
	rm -f $(LIBS_DIR)/libtryx_postgres-*.so
	cd $(RUST_DIR) && cargo clean
	rm -rf dist/ *.egg-info src/*.egg-info
	@echo "$(GREEN)✓ Clean$(RESET)"

# ── Help ───────────────────────────────────────────────────────────────────────
help:
	@echo ""
	@echo "$(BOLD)tryx-store-postgres Development Commands$(RESET)"
	@echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
	@echo ""
	@echo "  $(CYAN)make build$(RESET)        Build .so (release) and stage to libs/"
	@echo "  $(CYAN)make build-debug$(RESET)  Build .so (debug, faster compile)"
	@echo "  $(CYAN)make verify$(RESET)       Check all 23 FFI symbols are exported"
	@echo "  $(CYAN)make test-db$(RESET)      Run CRUD smoke test against PostgreSQL"
	@echo "  $(CYAN)make test-rust$(RESET)    Run Rust integration tests"
	@echo "  $(CYAN)make install-dev$(RESET)  Build + verify + pip install -e ."
	@echo "  $(CYAN)make clean$(RESET)        Remove all build artifacts"
	@echo "  $(CYAN)make all$(RESET)          Build + verify (default)"
	@echo ""
	@echo "$(BOLD)Environment:$(RESET)"
	@echo "  PG_DSN=$(PG_DSN)"
	@echo "  Platform: $(OS_FAMILY)-$(ARCH)-$(VARIANT)"
	@echo ""
