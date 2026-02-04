#!/usr/bin/env bash
set -euo pipefail

echo "==> Formatting"
cargo fmt --check

echo "==> Linting"
cargo clippy -- -D warnings

echo "==> Tests"
cargo test

echo "==> Build"
cargo build

echo "==> All checks passed"
