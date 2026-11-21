#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-prog-run.sh — night-frontier-Z1 A1, PROOF 7: the tap's
# in-kernel cost, measured by the kernel itself. The HOST-side wrapper.
#
# The lane's engine lives in the audit harness (`--prog-run`): the
# production tap object rides the loader's OWN aya-obj parser (the
# first CI attempt named the wall bpftool cannot cross — the aya-ebpf
# object carries LEGACY map definitions, and libbpf v1.0 / every
# modern bpftool refuses to open it before any measurement), creates
# real maps from the parsed definitions through raw bpf() syscalls,
# walks the loader's map-fd relocation, loads the kprobe body
# unattached, and drives it with BPF_PROG_TEST_RUN and synthetic
# input_event args in a real x86_64 pt_regs. The verdict is the
# RELATIVE law (gate-off, unmapped, autorepeat all below the full
# press path); the absolute ns/call rows are recorded measurements.
# The e2e micro-VM guest runs the payload's harness binary directly
# (scripts/ci/supermassive-init.sh) — this wrapper is the host
# convenience with the same preconditions.
#
# Usage (root):
#   scripts/audit/zactrix-prog-run.sh
#
# Environment:
#   TAP_OBJECT      the production tap object
#                   (default: ebpf-prebuilt/zylaxion-tap.o)
#
# Exit code: 0 when the law holds (or the kernel honestly says
# unsupported — recorded skip), 1 when the mechanism is broken or the
# relative law fails.

set -uo pipefail
cd "$(dirname "$0")/../.."

TAP_OBJECT="${TAP_OBJECT:-ebpf-prebuilt/zylaxion-tap.o}"

if [ "$(id -u)" -ne 0 ]; then
        echo "PROOF 7: SKIP — the lane needs root (CAP_BPF/CAP_PERFMON for the load); run it inside the e2e guest"
        exit 0
fi
if [ ! -r "$TAP_OBJECT" ]; then
        echo "PROOF 7: FAIL — the production tap object is missing at $TAP_OBJECT"
        exit 1
fi
if [ ! -e /sys/fs/bpf/zylaxion/control ]; then
        echo "PROOF 7: SKIP — no pinned control map at /sys/fs/bpf/zylaxion/control (attach first)"
        exit 0
fi

exec cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- \
        --prog-run "$TAP_OBJECT"
