#!/usr/bin/env bash

# Use before committing.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

MSRV="1.94"

# Check that a toolchain is present.
require_toolchain() {
  local tc="$1"; shift
  if ! rustup toolchain list | grep -q "^$tc"; then
    echo "Toolchain '$tc' is missing. Install it with:" >&2
    echo "  rustup toolchain install $tc $*" >&2
    exit 1
  fi
}

require_toolchain stable --component rustfmt --component clippy
require_toolchain "$MSRV"

run() {
  echo "==> $*"
  "$@"
}

run cargo +stable fmt --check
run cargo +stable clippy --workspace --all-features --all-targets -- -D warnings

# The pool test needs a live Postgres; skip it when DATABASE_URL is unset.
if [[ -n "${DATABASE_URL:-}" ]]; then
  run cargo +stable test --workspace --all-features
else
  echo "==> DATABASE_URL not set: skipping the live-database pool test."
  run cargo +stable test --workspace --all-features -- --skip session_timeouts_are_applied
fi

run env RUSTDOCFLAGS="-D warnings" cargo +stable doc --workspace --all-features --no-deps

# Each feature must compile on its own; the ones left out are implied
# by the ones listed (pagination, retry pull their dependencies).
for features in "" "repository-error" "retry" "cursor-pagination" \
                "offset-pagination" "partial-row" "db-init" \
                "repository-error,cursor-pagination"; do
  run cargo +stable check -p pgkit --no-default-features ${features:+--features "$features"}
done

echo "==> Checking MSRV ($MSRV)..."
run cargo +"$MSRV" check --workspace --all-features

echo "All checks passed."
