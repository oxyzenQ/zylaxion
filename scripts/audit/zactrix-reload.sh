#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-reload.sh — PROOF 5, hot-reload safety
# (night-private-research-2xblade-3). One headless player session of
# 60 minutes of audio time (172.8M frames at 48 kHz, the orchestrator's
# 64-frame chunks): 120 preset switches (the watcher's exact ArcSwap
# mutation), 360 volume switches (the watcher's exact volume_bits
# store), 18,000 keystrokes at ~5/s with 300 ms holds. Measures the
# 10x-baseline artifact threshold, the pool's balance after a full
# drain, and the NaN-to-0 net's own signature (an exact-zero step from
# a live mix). The switch cadence rides the audio-time axis; the
# compression is documented in docs/ZACTRIX_LTS.md. No audio device,
# no eBPF attachment, no control map. Raw output lands in
# scripts/audit/out/ (gitignored).
#
# Usage:
#   scripts/audit/zactrix-reload.sh
#
# The harness is the same standalone crate the audit uses; the main
# package gains no dependency (the driver's ArcSwap is the crate the
# main package already pins).

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"

exec cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --reload \
    | tee "$OUT/reload.txt"
