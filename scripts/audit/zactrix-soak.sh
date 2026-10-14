#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-soak.sh — PROOF 1, the long-run soak
# (night-private-research-2xblade-3). Renders one million keystrokes,
# round-robin over all 24 presets at 48 kHz, on the harness's max-
# pressure schedule (pool saturated, oldest-first stealing as the
# voice terminator — the schedule compression is documented in
# docs/ZACTRIX_LTS.md). Runs the whole pass TWICE from fixed seeds
# (concurrently) and compares the final-1000-sample FNV-1a hashes:
# byte-identical is the determinism verdict. Streams RSS checkpoints,
# per-1000-keystroke wall-clock blocks, raw-path spot checks, and the
# pool's end state. No audio device is touched. Raw output lands in
# scripts/audit/out/ (gitignored).
#
# Usage:
#   scripts/audit/zactrix-soak.sh
#
# The harness is the same standalone crate the audit uses; the main
# package gains no dependency.

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"

cargo build --release -q --manifest-path scripts/audit/harness/Cargo.toml
BIN=scripts/audit/harness/target/release/zactrix-audit

"$BIN" --soak > "$OUT/soak-run1.txt" 2>&1 &
P1=$!
"$BIN" --soak > "$OUT/soak-run2.txt" 2>&1 &
P2=$!
wait "$P1" "$P2"

cat "$OUT/soak-run1.txt"
H1=$(grep -o 'hash (fnv1a64): [0-9a-f]*' "$OUT/soak-run1.txt" | awk '{print $3}')
H2=$(grep -o 'hash (fnv1a64): [0-9a-f]*' "$OUT/soak-run2.txt" | awk '{print $3}')
if [ -n "$H1" ] && [ "$H1" = "$H2" ]; then
    echo "PROOF 1 hash equality: MATCH ($H1) - two independent one-million-keystroke runs end byte-identical"
else
    echo "PROOF 1 hash equality: MISMATCH (run1 $H1 vs run2 $H2)"
    exit 1
fi
