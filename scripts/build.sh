#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Zylaxion build gatekeeper — local CI before commit/push.
# Usage: ./scripts/build.sh --check-all
#
# Runs four checks in order:
#   1. cargo fmt --check   — formatting must be clean.
#   2. cargo clippy        — zero warnings allowed (-D warnings).
#   3. cargo test          — all unit + doc tests must pass.
#   4. cargo audit         — supply-chain CVE scan (v5.0.0+).
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

case "$MODE" in
    --check-all)
        echo "==> [1/4] cargo fmt --check"
        cargo fmt --manifest-path "$MANIFEST" --all -- --check

        echo "==> [2/4] cargo clippy"
        cargo clippy --manifest-path "$MANIFEST" --all-targets -- -D warnings

        echo "==> [3/4] cargo test"
        cargo test --manifest-path "$MANIFEST"

        echo "==> [4/4] cargo audit (supply-chain CVE scan)"
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
            echo "    Then re-run:   ./scripts/build.sh --check-all"
        fi

        echo "==> All checks passed."
        ;;
    *)
        echo "Usage: ./scripts/build.sh --check-all"
        exit 1
        ;;
esac
