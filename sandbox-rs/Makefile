.PHONY: build run test check clean dev fmt help

# Default target
.DEFAULT_GOAL := help

# Build debug binary
build:
	cargo build

# Build release binary
build-release:
	cargo build --release

# Run the server
run:
	cargo run

# Run all tests
test:
	cargo test

# Run clippy and check formatting
check:
	cargo clippy -- -D warnings
	cargo fmt -- --check

# Format code
fmt:
	cargo fmt

# Remove build artifacts
clean:
	cargo clean

# Run with auto-reload (requires cargo-watch)
dev:
	@which cargo-watch > /dev/null || (echo "cargo-watch not installed. Install with: cargo install cargo-watch" && exit 1)
	cargo watch -x run

# Show help
help:
	@echo "Available targets:"
	@echo "  build         - Build debug binary"
	@echo "  build-release - Build release binary"
	@echo "  run           - Run the server"
	@echo "  test          - Run all tests"
	@echo "  check         - Run clippy and check formatting"
	@echo "  fmt           - Format code"
	@echo "  clean         - Remove build artifacts"
	@echo "  dev           - Run with auto-reload (requires cargo-watch)"
	@echo "  help          - Show this help message"
