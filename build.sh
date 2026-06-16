#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Zylaxion build gatekeeper — local CI before commit/push.
# Usage: ./build.sh --check-all

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="${SCRIPT_DIR}/Cargo.toml"

MODE="${1:---check-all}"

case "$MODE" in
    --check-all)
        echo "==> [1/3] cargo fmt --check"
        cargo fmt --manifest-path "$MANIFEST" --all -- --check

        echo "==> [2/3] cargo clippy"
        cargo clippy --manifest-path "$MANIFEST" --all-targets -- -D warnings

        echo "==> [3/3] cargo test"
        cargo test --manifest-path "$MANIFEST"

        echo "==> All checks passed."
        ;;
    *)
        echo "Usage: ./build.sh --check-all"
        exit 1
        ;;
esac