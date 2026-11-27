#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# OS: Linux only — zylaxion is a Linux sound keyboard; no other OS is supported.
#
# ZYLAXION VERSION PARITY GATE (night-cafe-mclaren-1)
#
# The bump's own-version siblings are discovered, never named: any
# manifest the repo authors, anywhere in the tree, rides every
# scripts/dev/version-to.sh bump through sibling_scan(). Until this
# gate nothing audited that contract AFTER the fact. A lane added
# tomorrow (a new test crate, a cosmostrix-* lane, wherever it sits)
# that carries its own version line and drifts from the root would
# build, test, and ship with a stale version pinned into its
# manifest — exactly the zero-stale state the bump works to refuse.
#
# This gate is the read-only after-the-fact arm of the bump:
#   1. every own-version sibling manifest's package version equals
#      the root package version (the bump's end state, enforced
#      between bumps);
#   2. every sibling that carries its own Cargo.lock carries the
#      manifest's version in that lock (the stale-lane-lock death
#      the v80.0.0-alpha.1 rehearsal died of — a --locked build of
#      the lane refuses to run until the lock is resynced);
#   3. the root Cargo.lock carries the root package version.
#
# The discovery law is imported VERBATIM from version-to.sh at run
# time (the test battery's extraction pattern): sibling_scan() and
# current_version() are pulled from the shipped script, never copied.
# A scan change in the bump changes this gate in the same commit —
# two implementations of one law is how a gate learns to lie.
#
# Pure read: no cargo invocation, no tree mutation, no network. The
# heal for every failure is the bump's own recovery door — re-run
# ./scripts/dev/version-to.sh v<root version> and commit the result.
#
# Usage: bash scripts/gates/check-version-parity.sh
#
# Platform: UNIX-only (uses `find`, `grep`, `awk`). Not for Windows cmd.exe.

set -uo pipefail

# The root-owned __pycache__ wall, shell layer (night-cafe-1): any
# interpreter this script spawns inherits dont-write-bytecode, so no
# repo-local bytecode lands beside the scripts at any privilege level.
export PYTHONDONTWRITEBYTECODE=1

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

BUMP_SCRIPT="scripts/dev/version-to.sh"
if [ ! -f "$BUMP_SCRIPT" ]; then
        echo "FATAL: $BUMP_SCRIPT is missing — the gate imports its discovery law from it"
        exit 2
fi

# Extract the two functions verbatim from the shipped script.
sed -n '/^sibling_scan() {/,/^}/p;/^current_version() {/,/^}/p' \
        "$BUMP_SCRIPT" > /tmp/zylaxion-version-parity-funcs.$$
trap 'rm -f /tmp/zylaxion-version-parity-funcs.$$' EXIT
if ! grep -q '^sibling_scan() {' /tmp/zylaxion-version-parity-funcs.$$; then
        echo "FATAL: the sibling_scan extraction from $BUMP_SCRIPT produced nothing — the scan's shape moved, fix the extraction"
        exit 2
fi

# shellcheck disable=SC1090 source=/tmp/zylaxion-version-parity-funcs
. /tmp/zylaxion-version-parity-funcs.$$

FAILED=0
ROOT_VERSION="$(current_version)"
if [ -z "${ROOT_VERSION}" ]; then
        echo "FAIL: the root Cargo.toml carries no package version — nothing to parity against"
        exit 1
fi
echo "Root package version: ${ROOT_VERSION}"

# ── 1. Every own-version sibling manifest equals the root version ────────
SIBLINGS="$(sibling_scan)"
if [ -z "${SIBLINGS}" ]; then
        echo "OK: no own-version sibling manifests (members inherit the root version)"
else
        while IFS=$'\t' read -r f n v; do
                [ -z "$f" ] && continue
                if [ "$v" = "${ROOT_VERSION}" ]; then
                        echo "OK: $f ($n) = ${ROOT_VERSION}"
                else
                        echo "FAIL: $f ($n) carries version $v, root carries ${ROOT_VERSION}"
                        echo "      heal: ./scripts/dev/version-to.sh v${ROOT_VERSION} — the recovery door rewrites and resyncs the lane"
                        FAILED=$((FAILED + 1))
                fi
        done <<< "${SIBLINGS}"
fi

# ── 2. Every sibling lock carries its manifest's version ──────────────────
if [ -n "${SIBLINGS}" ]; then
        while IFS=$'\t' read -r f n v; do
                [ -z "$f" ] && continue
                dir="$(dirname "$f")"
                lock="${dir}/Cargo.lock"
                [ -f "$lock" ] || continue
                entry_ver="$(grep -A1 "^name = \"${n}\"$" "$lock" | grep '^version = "' | head -1 | sed 's/^version = "\(.*\)"/\1/')"
                if [ "${entry_ver}" = "$v" ]; then
                        echo "OK: $lock ($n) = ${entry_ver}"
                else
                        echo "FAIL: $lock carries $n at ${entry_ver:-absent}, the manifest carries $v"
                        echo "      heal: ./scripts/dev/version-to.sh v${ROOT_VERSION} — the recovery door resyncs every lane lock"
                        FAILED=$((FAILED + 1))
                fi
        done <<< "${SIBLINGS}"
fi

# ── 3. The root lock carries the root package version ─────────────────────
if [ -f Cargo.lock ]; then
        root_name="$(grep -m1 '^name = ' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
        entry_ver="$(grep -A1 "^name = \"${root_name}\"$" Cargo.lock | grep '^version = "' | head -1 | sed 's/^version = "\(.*\)"/\1/')"
        if [ "${entry_ver}" = "${ROOT_VERSION}" ]; then
                echo "OK: Cargo.lock (${root_name}) = ${ROOT_VERSION}"
        else
                echo "FAIL: Cargo.lock carries ${root_name} at ${entry_ver:-absent}, the root manifest carries ${ROOT_VERSION}"
                echo "      heal: cargo update --workspace (offline first), or re-run ./scripts/dev/version-to.sh v${ROOT_VERSION}"
                FAILED=$((FAILED + 1))
        fi
fi

echo ""
if [ "$FAILED" -gt 0 ]; then
        echo "FAIL: $FAILED version parity violation(s) — the tree carries a stale version surface"
        exit 1
fi
echo "OK: version parity holds — root, every own-version sibling manifest, and every sibling lock agree"
exit 0
