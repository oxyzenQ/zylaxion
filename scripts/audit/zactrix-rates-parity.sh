#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-rates-parity.sh — night-depth-mariana-2, the cross-arch
# proof lane. Runs the rates mode ONCE on THIS machine and diffs the
# per-preset x per-rate rows against the committed baseline
# (scripts/audit/rates-parity-baseline.txt — stamped on the x86-64
# A/B machine under the denormal policy law, commit 52a7194). On the
# arm64 CI lane a clean diff is the cross-arch byte-identity proof:
# the same seed, the same rate, the same pinned floating-point policy,
# the same bytes. On any machine it doubles as the re-baseline drift
# detector. The wrapper modifies nothing: the rates mode and the
# harness are untouched, no new dependency, no audio device.
#
# Usage:
#   scripts/audit/zactrix-rates-parity.sh
#
# Exit code: 0 when every row matches the baseline, 1 otherwise (the
# diff is printed, so the failing preset x rate is named in the log).

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"

cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --rates \
    | tee "$OUT/rates-parity-run.txt"

grep '^| `' "$OUT/rates-parity-run.txt" > "$OUT/rates-parity-rows.txt"
grep '^| `' scripts/audit/rates-parity-baseline.txt > "$OUT/rates-parity-baseline-rows.txt"

if diff -u "$OUT/rates-parity-baseline-rows.txt" "$OUT/rates-parity-rows.txt"; then
    echo "PARITY PASS: every per-preset x per-rate row matches the committed baseline -"
    echo "the bytes this machine renders are the bytes the baseline machine rendered"
else
    echo "PARITY FAIL: rows differ from the committed baseline - the diverging"
    echo "preset x rate lines above name exactly where the byte-identity contract broke"
    exit 1
fi
