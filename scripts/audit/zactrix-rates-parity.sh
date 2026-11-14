#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-rates-parity.sh — night-depth-mariana-2, the parity lane.
# Runs the rates mode ONCE on THIS machine and diffs the per-preset x
# per-rate rows against a committed baseline. Two baselines exist:
#
#   scripts/audit/rates-parity-baseline.txt        (x86-64, default)
#   scripts/audit/rates-parity-baseline-arm64.txt  (the arm64 lane)
#
# The FATAL diff is always same-arch: this machine's rows against its
# own architecture's committed baseline — a clean diff proves the
# bytes are stable across machines of the same architecture (fresh
# CI runners included). The cross-arch row diff (arm64 against the
# x86-64 baseline) is RECORDED, never gated: the attack-region
# fingerprints measured on both sides show the divergence lives in
# the init-time transcendental math (per-target libm), before any
# render dust can exist — the boundary docs/ZACTRIX_AUDIT.md names,
# and the correctly-rounded pure-Rust transcendental lane is the
# future work that closes it.
#
# Stamp mode: when the given baseline file does not exist yet, the
# script prints the full row table and fails with the instruction —
# a baseline can only be born from a logged run, never from thin air.
#
# Usage:
#   scripts/audit/zactrix-rates-parity.sh [baseline-file]
#
# Exit code: 0 when the same-arch rows match the baseline, 1
# otherwise (the diff is printed, so the failing preset x rate is
# named in the log).

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"
BASELINE="${1:-scripts/audit/rates-parity-baseline.txt}"

cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --rates \
    | tee "$OUT/rates-parity-run.txt"

grep '^| `' "$OUT/rates-parity-run.txt" > "$OUT/rates-parity-rows.txt"

if [ ! -f "$BASELINE" ]; then
    echo "STAMP MODE: $BASELINE does not exist - the full row table above is"
    echo "the baseline to commit; the lane gates on it from the next run"
    exit 1
fi

grep '^| `' "$BASELINE" > "$OUT/rates-parity-baseline-rows.txt"

echo "PARITY gate: this arch's rows vs $BASELINE"
if diff -u "$OUT/rates-parity-baseline-rows.txt" "$OUT/rates-parity-rows.txt"; then
    echo "PARITY PASS: every per-preset x per-rate row matches the committed"
    echo "same-arch baseline - the bytes this machine renders are the bytes"
    echo "the baseline machine rendered"
else
    echo "PARITY FAIL: rows differ from the committed same-arch baseline - the"
    echo "diverging preset x rate lines above name exactly where the machine"
    echo "surface moved (CPU/libm dispatch); a re-baseline needs a documented cause"
    exit 1
fi

# The cross-arch report: RECORDED, never gated. When the two baselines
# differ (they do, per the measured boundary), print the divergence
# scope so the boundary stays visible in every lane log.
if [ "$BASELINE" != "scripts/audit/rates-parity-baseline.txt" ] && [ -f scripts/audit/rates-parity-baseline.txt ]; then
    DIVERGED=$(diff "$OUT/rates-parity-baseline-rows.txt" \
        <(grep '^| `' scripts/audit/rates-parity-baseline.txt) | grep -c '^<' || true)
    TOTAL=$(grep -c '^| `' "$BASELINE")
    echo "RECORDED (not gated): cross-arch row diff vs the x86-64 baseline: $DIVERGED of $TOTAL rows -"
    echo "the init-time transcendental boundary (docs/ZACTRIX_AUDIT.md); the tail's"
    echo "pure mul/add arithmetic is exact cross-arch, the curve coefficients are not"
fi
