#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# The supermassive wrapper (night-master-3): one canonical entry, the
# zelynic lineage's shape — bash is the front door, Python is the
# engine (subprocess control, /proc parsing, threaded uinput injection,
# and precise timing are things bash cannot do well).
#
# Usage:
#   ./scripts/supermassive/supermassive-test.sh --self-test          # rootless lane
#   ./scripts/supermassive/supermassive-test.sh --self-test --binary PATH
#   sudo ./scripts/supermassive/supermassive-test.sh --full          # the whole chaos (root lane)
#   sudo ./scripts/supermassive/supermassive-test.sh --full --binary /usr/bin/zylaxion
#
# The --full lane is what the micro-VM's PID 1
# (scripts/ci/supermassive-init.sh) drives inside the KVM guest on
# every supermassive CI leg: env probes, CLI/UX, the tap lifecycle,
# and the real-usage chain — a real user with no input group typing on
# a uinput keyboard, the kernel-side telemetry counting, the PCM
# witness running. Every probe lands on the ledger; the final
# MASS-VERDICT line is the relay the workflow's Verdict step greps.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# The root-owned pycache guard (night-dragon-maintenance-T1, the
# zelynic incident lesson): root-run supermassive legs must never
# leave root-owned __pycache__ caches beside the python modules —
# a later plain-user tree cleanup then dies on "Permission denied".
export PYTHONDONTWRITEBYTECODE=1
exec python3 "${SCRIPT_DIR}/supermassive-test.py" "$@"
