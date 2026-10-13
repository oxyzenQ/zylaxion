#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-audit.sh — the one-command Phase 0 acoustic audit
# (night-private-research-2xblade). Renders every shipped preset
# headlessly at 96 kHz through the real VoicePool render path and
# prints the 24x4 physical-movement matrix, the A/B/C tier counts,
# and the Phase 0 gate verdict. No audio device is touched.
#
# Usage:
#   scripts/audit/zactrix-audit.sh                 # the full matrix
#   scripts/audit/zactrix-audit.sh --preset NAME   # one-preset verifier row
#
# The harness is a standalone stdlib-plus-log crate under
# scripts/audit/harness/; the main package gains no dependency.

set -euo pipefail
cd "$(dirname "$0")/../.."
exec cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml "$@"
