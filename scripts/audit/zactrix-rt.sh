#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-rt.sh — PROOF 3, the measured half of the RT-safety audit
# (night-private-research-2xblade-3). Renders one million samples
# through the real VoicePool in 64-frame chunks — the orchestrator's
# MAX_RENDER_CHUNK shape — and records the wall-clock of every chunk:
# one natural typing pass (5 keystrokes/s with holds) and one
# saturated pass (MAX_POLYPHONY held for the whole run). Targets:
# max/p50 < 10x, no block over 5 ms wall-clock. The written
# no-alloc / no-lock / no-syscall audit of the callback and the
# per-sample render path lives in docs/ZACTRIX_LTS.md. No audio
# device is touched. Raw output lands in scripts/audit/out/
# (gitignored).
#
# Usage:
#   scripts/audit/zactrix-rt.sh
#
# The harness is the same standalone crate the audit uses; the main
# package gains no dependency.

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"

exec cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --rt \
    | tee "$OUT/rt.txt"
