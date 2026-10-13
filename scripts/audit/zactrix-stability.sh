#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-stability.sh — the numerical stability gate of the Range
# Question pass (night-private-research-2xblade-2, Path A step 2).
# Renders each ring-promise preset with EVERY stage's resonance forced
# to the contract ceiling (ranges::RESONANCE_MAX), one million samples
# per rate, through the RAW AcousticModel path — no VoicePool safety
# net, so an internal blow-up shows as data, not silence. Checks NaN,
# Inf, f32 overflow, denormal drift, the six TPT integrator states at
# rest, and the c^(R/S) invariance law (tail length within 0.5%
# across 44.1 / 48 / 96 kHz). No audio device is touched.
#
# Usage:
#   scripts/audit/zactrix-stability.sh
#
# The harness is the same standalone crate the audit uses; the main
# package gains no dependency.

set -euo pipefail
cd "$(dirname "$0")/../.."
exec cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --stability
