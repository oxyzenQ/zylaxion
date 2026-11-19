#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-tap-golden.sh — night-frontier-Z5, PROOF 9: the tap-behavior
# golden corpus — the kernel side pinned as bytes. The HOST-side
# wrapper.
#
# The lane's engine lives in the audit harness (`--tap-golden`): the
# production tap object rides the loader's OWN aya-obj parser (the
# shared lane head with PROOF 7), creates FRESH maps from the parsed
# definitions through raw bpf() syscalls, walks the loader's map-fd
# relocation, loads the kprobe body unattached as the syscall type,
# and drives a scripted event sequence through BPF_PROG_TEST_RUN —
# the gate-off silence, the keymap death, the autorepeat filter, the
# exact record order, the ring saturation capacity (derived from the
# kernel's own ringbuf math), the drop-on-full posture, and the
# count-the-drops counter law — every record tuple and every stats
# delta compared against the compiled golden contract (timestamps
# masked: the kernel clock is not behavior). Unlike PROOF 7, this
# lane needs NO attach and NO pins: fresh maps only, the process
# exits, the kernel reaps them. The e2e micro-VM guest runs the
# payload's harness binary directly (scripts/ci/supermassive-init.sh)
# — this wrapper is the host convenience with the same preconditions.
#
# Usage (root):
#   scripts/audit/zactrix-tap-golden.sh
#
# Environment:
#   TAP_OBJECT      the production tap object
#                   (default: ebpf-prebuilt/zylaxion-tap.o)
#
# Exit code: 0 when every tuple and delta matches the golden contract
# (or the kernel honestly says unsupported — recorded skip), 1 when a
# tuple, a delta, or the capacity moved (the delta named).

set -uo pipefail
cd "$(dirname "$0")/../.."

TAP_OBJECT="${TAP_OBJECT:-ebpf-prebuilt/zylaxion-tap.o}"

if [ "$(id -u)" -ne 0 ]; then
        echo "TAP GOLDEN: SKIP — the lane needs root (CAP_BPF/CAP_PERFMON for the load); run it inside the e2e guest"
        exit 0
fi
if [ ! -r "$TAP_OBJECT" ]; then
        echo "TAP GOLDEN: FAIL — the production tap object is missing at $TAP_OBJECT"
        exit 1
fi

exec cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- \
        --tap-golden "$TAP_OBJECT"
