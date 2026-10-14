#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-rates.sh — PROOF 2, the sample-rate span
# (night-private-research-2xblade-3). One keystroke per preset through
# the real VoicePool path at 44.1 / 48 / 88.2 / 96 / 192 kHz: the voice
# lifetime (the c^(R/S) law's exact measurand, gated < 0.5% across
# rates), the -60 dB tail endpoint (recorded as excitation statistics,
# per the 2xblade-2 control experiment), the composite ring fit where
# its window yields maxima, and per-rate FNV-1a output hashes with an
# in-run re-render equality check. The wrapper runs the mode TWICE and
# diffs the outputs: byte-identical is the cross-run hash proof. No
# audio device is touched. Raw output lands in scripts/audit/out/
# (gitignored).
#
# Usage:
#   scripts/audit/zactrix-rates.sh
#
# The harness is the same standalone crate the audit uses; the main
# package gains no dependency.

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"

cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --rates \
    | tee "$OUT/rates-run1.txt"
cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --rates \
    | tee "$OUT/rates-run2.txt"

if diff -q "$OUT/rates-run1.txt" "$OUT/rates-run2.txt" > /dev/null; then
    echo "PROOF 2 cross-run proof: outputs diff-clean - every per-rate hash is byte-identical across runs"
else
    echo "PROOF 2 cross-run proof: OUTPUTS DIFFER - determinism broken"
    exit 1
fi
