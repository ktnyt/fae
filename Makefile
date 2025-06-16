# fae Development Makefile

.PHONY: test test-coverage test-coverage-open clean lint format check build help watch-dev watch-format watch-coverage

# Default target
help:
	@echo "Available targets:"
	@echo "  test             - Run all tests"
	@echo "  test-coverage    - Run tests with coverage analysis"
	@echo "  clean            - Clean build artifacts"
	@echo "  lint             - Run clippy linter"
	@echo "  format           - Format code with rustfmt"
	@echo "  check            - Check compilation without building"
	@echo "  build            - Build the project"
	@echo "  watch-dev        - Watch files and run format + coverage on changes"
	@echo "  watch-format     - Watch files and auto-format on changes"
	@echo "  watch-coverage   - Watch files and update coverage on changes"

# Run all tests
test:
	timeout 30s cargo test --lib -- --test-threads=1

# Run tests with coverage
test-coverage:
	cargo llvm-cov --lib --package fae --lcov --output-path coverage/lcov.info -- --test-threads=1
	@echo "Coverage report generated in coverage/lcov.info"


# Clean build artifacts
clean:
	cargo clean
	rm -rf target/llvm-cov

# Run linter
lint:
	cargo clippy -- -D warnings

# Format code
format:
	cargo fmt

# Check compilation
check:
	cargo check

# Build project
build:
	cargo build --release

# Run specific test module
test-command:
	timeout 30s cargo test -p fae command --lib -- --test-threads=1

# CI-friendly coverage (no HTML)
test-coverage-ci:
	cargo llvm-cov --lib --package fae -- --test-threads=1

# Development workflow: format, lint, test, coverage
dev: format lint test test-coverage
	@echo "✅ Development workflow completed successfully"

# Watch for file changes and auto-format
watch-format:
	@echo "🔍 Watching Rust files for auto-formatting..."
	cargo watch -w src -x fmt

# Watch for file changes and update coverage
watch-coverage:
	@echo "🔍 Watching Rust files for coverage updates..."
	cargo watch -w src -s "make test-coverage"

# Watch for file changes and run format + coverage (development mode)
watch-dev:
	@echo "🔍 Watching Rust files for development workflow..."
	cargo watch -w src -s "make format && make test-coverage && echo '✅ Auto-workflow completed'"