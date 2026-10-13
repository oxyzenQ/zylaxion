#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-verify.sh — the per-preset verifier of the Zactrix forge pass
# (night-private-research-2xblade, Phase 1). Renders ONE named preset
# headlessly at 96 kHz through the real render path and reports the
# same four physical movement metrics the Phase 0 audit measured, each
# with its in-band verdict. Tuning without this verifier is tuning by
# taste; that is not this pass.
#
# Usage:
#   scripts/audit/zactrix-verify.sh topre
#
# The harness is the same standalone crate the audit uses; the main
# package gains no dependency.

set -euo pipefail
cd "$(dirname "$0")/../.."
if [ "$#" -ne 1 ]; then
    echo "usage: $0 <preset-name>" >&2
    exit 2
fi
exec cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --preset "$1"
