#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux eBPF tool; no other OS is supported.
#
# ZYLAXION COMMIT-TIME PREBUILT GATE (night-improve-31, the zelynic
# NIGHT-dinner-1 shape; AUTO-HEALED since night-dragon-maintenance-0,
# the zejtron night-weekly-maintenance-1 shape).
#
# The commit-time arm of the prebuilt eBPF freshness contract. The
# contract has three arms, one per place a stale lane could slip
# through:
#   - commit time: THIS gate, wired into git via .githooks/pre-commit
#     (core.hooksPath=.githooks, self-installed by gate-keepers.sh)
#   - push time:   gate-keepers.sh and the wholesale CI gate run
#   - ship time:   the parity steps in crates-io.yml and release.yml,
#     the last line of defense before the registry upload and the
#     release binaries
#
# What it enforces at the moment a commit is created:
#   1. Prebuilt parity (delegated to the one writer's read-only --check
#      verdict): the ebpf/ tree blake3 must still match
#      ebpf-prebuilt/manifest.toml's pin, the staged object must still
#      be the pinned bytes, the toolchain pin must still match
#      ebpf/rust-toolchain.toml, and the version mirror (root
#      Cargo.toml == ebpf/Cargo.toml == the manifest's [contract]
#      version) must hold. A stale verdict now HEALS instead of
#      failing: the one writer rebuilds the tap object, rewrites the
#      manifest, and the refreshed lane is staged into THIS commit -
#      "edit ebpf/, commit" carries a fresh lane with no separate
#      manual step (the build-time arm, build/ebpf.rs's
#      refresh_prebuilt_lane, does the same for every cargo build).
#      The fail-closed arm remains for the heal's own failure mode -
#      missing prerequisites - because a gate that would ship a stale
#      lane silently is the dishonest gate.
#   2. Pairing: a commit that stages ebpf/ must also stage
#      ebpf-prebuilt/ - the "refreshed but forgot git add" case.
#      Auto-healed the same way: when the on-disk lane already
#      verifies (a regenerator ran earlier, the staging was forgotten)
#      the lane is staged into the commit; when it does not, section
#      1's heal has already run and the fail-closed arm names the fix.
#   3. CI actions-pin health (delegated to check-actions-pins.sh):
#      the freshness of every `uses:` pin against upstream - the
#      actions-pin health contract's one automatic seat.
#
# Two contract classes share this gate, and the split is the point:
# sections 1-2 fail closed because prebuilt parity is a LOCAL fact
# this machine can always prove; section 3 is advisory by default
# because pin freshness is a REMOTE fact that needs the network and
# the API's quota right now — a gate that blocks a commit on facts
# it cannot reach would be a dishonest gate. The full contract (cache,
# budget, honesty rules) lives in check-actions-pins.sh's header.
#
# The escape hatches are git's own --no-verify (skips any local hook —
# a git contract, not a hole here: the wholesale push gate and the
# ship-time parity steps still block the push and the publish) and
# the shared ZYLAXION_SKIP_PREBUILT_REGEN=1 (build/ebpf.rs honors the
# same variable), which turns the auto-heal back into the fail-closed
# shape.
#
# Usage:
#   bash scripts/gates/check-commit-gate.sh    # same checks, preview
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

GATE_FAILED=0

# ── 1. prebuilt parity (auto-heal - one implementation, one truth) ────────
# The stale verdict heals through the one writer, the refreshed lane
# rides the commit, and the re-check must pass. The build-time arm
# (build/ebpf.rs's refresh_prebuilt_lane) does the same for every
# cargo build. ZYLAXION_SKIP_PREBUILT_REGEN=1 is the shared escape
# hatch: under it the stale verdict fails closed again, the pre-heal
# shape.
if [ "${ZYLAXION_SKIP_PREBUILT_REGEN:-}" = "1" ]; then
        if ! bash scripts/dev/regenerate-ebpf-prebuilt.sh --check; then
                echo "      ZYLAXION_SKIP_PREBUILT_REGEN=1 skips the auto-heal; refresh by hand:"
                echo "        ./scripts/dev/regenerate-ebpf-prebuilt.sh, then git add ebpf-prebuilt"
                GATE_FAILED=1
        fi
elif ! bash scripts/dev/regenerate-ebpf-prebuilt.sh --check; then
        echo "      prebuilt lane stale - regenerating through the one writer..."
        if bash scripts/dev/regenerate-ebpf-prebuilt.sh; then
                git add ebpf-prebuilt
                bash scripts/dev/regenerate-ebpf-prebuilt.sh --check || {
                        echo "      the rewrite landed but the re-check still says stale."
                        GATE_FAILED=1
                }
                if [ "${GATE_FAILED}" -eq 0 ]; then
                        echo "      lane regenerated and staged into this commit."
                fi
        else
                echo "      the regeneration failed (missing prerequisites?) - install the trio:"
                echo "        ./scripts/dev/bootstrap-ebpf.sh, then commit again"
                GATE_FAILED=1
        fi
fi

# ── 2. ebpf/ staged without ebpf-prebuilt/ ───────────────────────────────
# The "refreshed but forgot git add" case. A fresh on-disk lane (the
# --check verdict above) is staged into the commit; a stale one was
# already healed or failed in section 1, so this arm only ever names
# the fix when the lane itself could not be made fresh.
if [ -n "$(git diff --cached --name-only -- ebpf/)" ]; then
        if ! git diff --cached --name-only -- ebpf-prebuilt/ | grep -q .; then
                if bash scripts/dev/regenerate-ebpf-prebuilt.sh --check; then
                        git add ebpf-prebuilt
                        echo "      ebpf-prebuilt was fresh but unstaged - staged it into this commit."
                else
                        echo "FAIL: ebpf/ changes are staged but ebpf-prebuilt/ is neither staged nor fresh."
                        echo "      The commit would ship sources the prebuilt lane does"
                        echo "      not describe. Refresh and stage the lane:"
                        echo "        ./scripts/dev/regenerate-ebpf-prebuilt.sh, then"
                        echo "          git add ebpf-prebuilt"
                        GATE_FAILED=1
                fi
        fi
fi

# ── 3. CI actions-pin health (advisory by default) ────────────────
# Runs LAST: the fail-closed lane checks above never wait behind a
# network probe. Every skip reason stays a skip; the check owns its
# own exit-zero and its own output.
if ! bash scripts/gates/check-actions-pins.sh; then
        GATE_FAILED=1
fi

if [ "${GATE_FAILED}" -ne 0 ]; then
        echo "FAIL: commit gate — the tree is not committable in this state"
        echo "      (sections 1-2 fail-closed on the prebuilt lane; section 3"
        echo "      blocks on a stale verdict — see the check's own output)"
        exit 1
fi

echo "OK: commit gate — prebuilt lane in parity, ebpf/ staged with its lane, actions-pin health reported"
