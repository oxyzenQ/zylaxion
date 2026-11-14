#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# verify-reproducible-build.sh — night-frontier-2B, the reproducible
# build attestation. Builds the parity-checked pro alias TWICE from
# the same commit and the same lockfile, both with SOURCE_DATE_EPOCH
# pinned (the reproducible-builds standard) to the HEAD commit's own
# timestamp, in two separate target directories — then compares:
#
#   1. the -V provenance stamps, line by line — the attestation the
#      build.rs chain produces must be byte-identical (Build, Build-
#      time with its clock source, Lock fingerprint, Signature,
#      Copyright, License, Source);
#   2. the binaries themselves, byte for byte (cmp) — the full
#      reproducible-builds property, reported as its own verdict:
#      proven when it holds, named as unproven when it does not (the
#      anchor's self-hash makes any binary divergence visible in the
#      Depth-matrix line).
#
# Usage:
#   scripts/dev/verify-reproducible-build.sh
#
# Exit code: 0 when the stamps match (the attestation), regardless of
# the binary verdict; 1 on any stamp mismatch or build failure. The
# binary verdict is printed, never swallowed.

set -euo pipefail
cd "$(dirname "$0")/../.."

# The tree must present ONE state to both builds: a dirty work tree
# stamps -dirty on both sides identically, but the honest run wants
# the clean lane — refuse the ambiguity instead of measuring noise.
if [ -n "$(git status --porcelain)" ]; then
    echo "REFUSED: the work tree is dirty - commit or stash first (one tree, one stamp)"
    exit 1
fi

EPOCH="$(git log -1 --format=%ct HEAD)"
export SOURCE_DATE_EPOCH="$EPOCH"
echo "repro epoch: SOURCE_DATE_EPOCH=$EPOCH (HEAD commit time, the reproducible-builds pin)"

A="target/.repro-a"
B="target/.repro-b"
rm -rf "$A" "$B"

echo "== build 1 of 2 (target $A) =="
CARGO_TARGET_DIR="$A" cargo pro-linux-amd64-gnu --locked -q
BIN_A="$A/x86_64-unknown-linux-gnu/release/zylaxion"

echo "== build 2 of 2 (target $B) =="
CARGO_TARGET_DIR="$B" cargo pro-linux-amd64-gnu --locked -q
BIN_B="$B/x86_64-unknown-linux-gnu/release/zylaxion"

"$BIN_A" -V > /tmp/repro-a.txt
"$BIN_B" -V > /tmp/repro-b.txt

STAMPS_MATCH=1
if diff -u /tmp/repro-a.txt /tmp/repro-b.txt; then
    echo "STAMPS: byte-identical - the provenance chain (commit, clock source, lock fingerprint, self-hash anchor) renders the same bytes on both builds"
else
    STAMPS_MATCH=0
    echo "STAMPS: DIFFER - the attestation is broken; the diff above names the drifted field"
fi

if cmp -s "$BIN_A" "$BIN_B"; then
    echo "BINARY: byte-identical - the full reproducible-builds property holds on this toolchain (same commit, same lock, same epoch, same rustc)"
else
    echo "BINARY: differs - the stamps are reproducible but the binaries are not; the honest verdict is the stamp-level attestation (cmp + sha256 follow)"
    sha256sum "$BIN_A" "$BIN_B"
fi

[ "$STAMPS_MATCH" -eq 1 ] || exit 1
echo "REPRODUCIBLE-BUILD ATTESTATION: PASS"
