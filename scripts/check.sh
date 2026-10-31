#!/usr/bin/env bash
# Single quality gate: run locally and by CI. Fails on the first failing step.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

msrv="1.85.0"

step() { printf '\n==> %s\n' "$*"; }

if ! cargo deny --version 2>/dev/null | grep -q '0\.19'; then
  echo "error: cargo-deny 0.19.x is required (missing or wrong version)." >&2
  echo "install it with: cargo install cargo-deny --version 0.19.0 --locked" >&2
  exit 1
fi

if ! rustup toolchain list | grep -q "^${msrv}"; then
  echo "error: Rust ${msrv} is required for the MSRV gate." >&2
  echo "install it with: rustup toolchain install ${msrv} --profile minimal" >&2
  exit 1
fi

step "cargo fmt --all --check"
cargo fmt --all --check

step "cargo clippy --all-targets -- -D warnings"
cargo clippy --locked --all-targets -- -D warnings

step "cargo test --workspace"
cargo test --locked --workspace

step "cargo +${msrv} test --workspace (MSRV)"
cargo "+${msrv}" test --locked --workspace

step "cargo doc --no-deps --workspace (warnings denied)"
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --workspace

step "cargo deny check"
cargo deny check

printf '\nAll checks passed.\n'
