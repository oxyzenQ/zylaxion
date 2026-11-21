#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-rates-parity.sh — night-frontier-Z5, the machine-independence
# parity lane. Runs the rates mode ONCE on THIS machine and diffs the
# per-preset x per-rate rows against the ONE committed corpus:
#
#   scripts/audit/rates-parity-baseline.txt
#
# night-depth-mariana-2 built this lane with TWO baselines and a
# recorded-only cross-arch diff, because the init-time transcendentals
# resolved through per-target libm and the hash rows were machine
# fingerprints. night-frontier-Z5 closed that boundary (every libm
# transcendental left the audio path; src/engine/detmath/ carries the
# certified lane), so the machine-independence property now holds as a
# GATE on every architecture: this machine's rows against the shared
# corpus must be byte-identical — the arm64 runner proving itself
# against the x86-committed bytes is exactly the cross-arch proof the
# old lane could only record.
#
# Stamp mode: when the corpus file does not exist yet, the script
# prints the full row table and fails with the instruction — a corpus
# can only be born from a logged run, never from thin air.
#
# Usage:
#   scripts/audit/zactrix-rates-parity.sh [baseline-file]
#
# Exit code: 0 when every row matches the corpus byte-identically, 1
# otherwise (the diff is printed, so the failing preset x rate is
# named in the log).

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"
BASELINE="${1:-scripts/audit/rates-parity-baseline.txt}"

cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --rates \
    | tee "$OUT/rates-parity-run.txt"

grep '^| `' "$OUT/rates-parity-run.txt" > "$OUT/rates-parity-rows.txt"

if [ ! -f "$BASELINE" ]; then
    echo "STAMP MODE: $BASELINE does not exist - the full row table above is"
    echo "the corpus to commit; the lane gates on it from the next run"
    exit 1
fi

grep '^| `' "$BASELINE" > "$OUT/rates-parity-baseline-rows.txt"

echo "PARITY gate: this machine's rows vs $BASELINE (one corpus, every arch)"
if diff -u "$OUT/rates-parity-baseline-rows.txt" "$OUT/rates-parity-rows.txt"; then
    echo "PARITY PASS: every per-preset x per-rate row matches the committed"
    echo "corpus byte-identically — the bytes this machine renders are the"
    echo "bytes every machine renders (night-frontier-Z5: the sound is the"
    echo "bytes on every arch, no libm in the path)"
else
    echo "PARITY FAIL: rows differ from the committed corpus — the diverging"
    echo "preset x rate lines above name exactly where the bytes moved. Under"
    echo "the certified lane there is no machine surface left: a diff here is"
    echo "either a non-deterministic float op reintroduced into the path or a"
    echo "corpus re-baseline riding its documented cause"
    exit 1
fi
