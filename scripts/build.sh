#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# Zylaxion build gatekeeper — local CI before commit/push.
# Usage: ./scripts/build.sh check-all [-q]
#
# Runs four checks in order:
#   1. cargo fmt --check   — formatting must be clean.
#   2. cargo clippy        — zero warnings allowed (-D warnings).
#   3. cargo test          — all unit + doc tests must pass.
#   4. cargo audit         — supply-chain CVE scan (v5.0.0+).
#
# -q / --quiet is the essentials-only shape (the zelynic interface):
# the per-step banners are suppressed, verdict-bearing lines still
# emit. A quiet run that ends silently is a green run.
#
# The audit step is OPTIONAL: if `cargo-audit` is not installed, the
# script prints a warning and continues instead of failing. This keeps
# the gatekeeper usable in fresh dev environments without forcing
# everyone to install cargo-audit. To enable the audit gate, install
# with:  cargo install cargo-audit

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="${SCRIPT_DIR}/.."
MANIFEST="${WORKSPACE_ROOT}/Cargo.toml"

MODE="${1:---check-all}"
QUIET=0

case "$MODE" in
    check-all | --check-all)
        # night-improve-1: accept the bare `check-all` spelling (the
        # zelynic gate shape, `./scripts/build.sh check-all -q`) next to
        # the historical `--check-all` so muscle memory and docs written
        # for either project work unchanged.
        shift || true
        ;;
    *)
        echo "Usage: ./scripts/build.sh check-all [-q]"
        exit 1
        ;;
esac

# The quiet flag rides the second position: `check-all -q`.
if [ "${1:-}" = "-q" ] || [ "${1:-}" = "--quiet" ]; then
    QUIET=1
fi

banner() {
    # Verdict-bearing lines always emit; step banners honor -q.
    if [ "$QUIET" -eq 0 ]; then
        echo "$1"
    fi
}

banner "==> [1/4] cargo fmt --check"
cargo fmt --manifest-path "$MANIFEST" --all -- --check

banner "==> [2/4] cargo clippy"
cargo clippy --manifest-path "$MANIFEST" --all-targets -- -D warnings

banner "==> [3/4] cargo test"
if [ "$QUIET" -eq 1 ]; then
    cargo test --manifest-path "$MANIFEST" --quiet
else
    cargo test --manifest-path "$MANIFEST"
fi

banner "==> [4/4] cargo audit (supply-chain CVE scan)"
if command -v cargo-audit >/dev/null 2>&1; then
    # Run cargo-audit against the workspace's Cargo.lock.
    # `cargo audit` does NOT accept --manifest-path — it scans
    # the Cargo.lock in the current directory. So we cd into
    # the workspace root first.
    #
    # `--ignore RUSTSEC-xxxx-xxxx` can be added here for
    # acknowledged-but-unfixable advisories (none currently).
    #
    # `--no-fetch` is NOT used — we want fresh advisory DB
    # updates every run so we catch newly-disclosed CVEs.
    #
    # Exit code 0 = no vulnerabilities. Non-zero = at least
    # one vulnerability found; the build fails.
    (cd "$WORKSPACE_ROOT" && cargo audit)
    echo "    OK  no vulnerabilities found."
else
    echo "    WARNING  cargo-audit not installed, skipping security scan."
    echo "    Install with:  cargo install cargo-audit"
    echo "    Then re-run:   ./scripts/build.sh check-all"
fi

banner "==> All checks passed."
