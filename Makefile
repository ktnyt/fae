# fae Development Makefile
# Uses mise-managed tools for consistent environment

.PHONY: test test-coverage test-coverage-open clean lint format check build help watch-dev watch-format watch-coverage setup

# Default target
help:
	@echo "fae Development Commands (mise-powered)"
	@echo ""
	@echo "Setup:"
	@echo "  setup            - Complete development environment setup"
	@echo "  mise install     - Install tools (Rust + cargo tools)"
	@echo ""
	@echo "Development:"
	@echo "  test             - Run all tests"
	@echo "  test-coverage    - Run tests with coverage analysis"
	@echo "  watch-dev        - Watch files and run format + coverage on changes"
	@echo "  dev              - Alias for watch-dev"
	@echo ""
	@echo "Code Quality:"
	@echo "  format           - Format code with rustfmt"
	@echo "  lint             - Run clippy linter"
	@echo "  check            - Check compilation without building"
	@echo ""
	@echo "Build & Clean:"
	@echo "  build            - Build the project"
	@echo "  clean            - Clean build artifacts"
	@echo ""
	@echo "Individual Watch Modes:"
	@echo "  watch-format     - Watch files and auto-format on changes"
	@echo "  watch-coverage   - Watch files and update coverage on changes"

# Run all tests
test:
	timeout 30s cargo test --lib -- --test-threads=1

# Run tests with coverage (using mise-managed cargo-llvm-cov)
test-coverage:
	mise exec -- cargo llvm-cov --lib --package fae --lcov --output-path coverage/lcov.info -- --test-threads=1
	@echo "Coverage report generated in coverage/lcov.info"


# Show test coverage summary
test-coverage-summary:
	mise exec -- cargo llvm-cov --lib --package fae --lcov --summary-only
	@echo "Coverage report generated in coverage/lcov.info"

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/llvm-cov coverage/
	@echo "🧹 Clean complete - ready for fresh build"

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
	mise exec -- cargo llvm-cov --lib --package fae -- --test-threads=1

# Development workflow: format, lint, test, coverage
dev: format lint test test-coverage
	@echo "✅ Development workflow completed successfully"

# Watch for file changes and auto-format (using mise-managed cargo-watch)
watch-format:
	@echo "🔍 Watching Rust files for auto-formatting..."
	mise exec -- cargo watch -w src -x fmt

# Watch for file changes and update coverage (using mise-managed cargo-watch)
watch-coverage:
	@echo "🔍 Watching Rust files for coverage updates..."
	mise exec -- cargo watch -w src -s "make test-coverage"

# Watch for file changes and run format + coverage (development mode)
watch-dev:
	@echo "🔍 Watching Rust files for development workflow..."
	mise exec -- cargo watch -w src -s "make format && make test-coverage && echo '✅ Auto-workflow completed'"

# Alias for watch-dev (matches mise task)
dev: watch-dev

# Complete development environment setup (delegates to mise)
setup:
	@echo "🔧 Setting up development environment with mise..."
	mise install
	mise run setup