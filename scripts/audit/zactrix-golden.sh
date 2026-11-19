#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-golden.sh — night-frontier-Z1 A3, the golden render corpus
# gate. The freeze (entry 31) says the engine's changes need a why
# that passes the audit harness; this gate makes the SAME claim
# mechanical at byte level: the committed per-preset x per-rate corpus
# (the rates-parity baseline — 24 presets x 5 rates plus the
# per-preset summary rows, one corpus, no duplicate baseline) is
# re-rendered through the REAL VoicePool path on this machine and
# compared row by row, keyed on (preset, rate). The verdict is
# machine-readable and names every delta:
#
#   GOLDEN VERDICT: baseline=N live=N unchanged=N changed=N missing=N added=N
#     CHANGED  <preset> @ <rate>k   (with the was/now row pair)
#     MISSING  <preset> @ <rate>k
#     ADDED    <preset> @ <rate>k
#
# night-frontier-Z5 upgraded the gate's class. Before the certified
# transcendental lane the hash rows were machine fingerprints (the
# init-time libm boundary) and only fleet-homogeneous lanes could run
# --strict; the heterogeneous x86 claims lane recorded byte deltas
# without gating them. The lane closed that boundary: the rendered
# bytes no longer depend on the host's libm or CPU dispatch, so the
# byte gate is the DEFAULT on every lane, every architecture — a
# changed row FAILS, and its only legal cause is the documented-
# cause re-baseline law (a logged run, the why in the ledger). The
# gate never stamps the baseline itself — stamping stays
# zactrix-rates-parity.sh's exclusive, logged-run contract.
#
# The engine is untouched by this script: it only renders and
# compares. No audio device, no network, no eBPF attachment.
#
# Usage:
#   scripts/audit/zactrix-golden.sh [baseline-file]
#
# Exit code: 0 when the corpus shape holds and every row re-renders
# byte-identical; 1 otherwise (every delta named in the output).

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"

BASELINE="${1:-scripts/audit/rates-parity-baseline.txt}"

cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --rates \
    | tee "$OUT/golden-run.txt"

grep '^| `' "$OUT/golden-run.txt" > "$OUT/golden-rows.txt"
grep '^| `' "$BASELINE" > "$OUT/golden-baseline-rows.txt"

echo "GOLDEN gate: the re-rendered corpus vs $BASELINE (one corpus, no duplicates)"
python3 - "$OUT/golden-baseline-rows.txt" "$OUT/golden-rows.txt" <<'PYEOF'
import sys


def rows(path):
    table = {}
    for line in open(path):
        cols = [c.strip() for c in line.split("|")]
        key = (cols[1], cols[2])
        table[key] = line.rstrip("\n")
    return table


base = rows(sys.argv[1])
live = rows(sys.argv[2])
changed = [k for k in base if k in live and base[k] != live[k]]
missing = [k for k in base if k not in live]
added = [k for k in live if k not in base]
unchanged = len(base) - len(changed) - len(missing)
print(
    f"GOLDEN VERDICT: baseline={len(base)} live={len(live)} "
    f"unchanged={unchanged} changed={len(changed)} "
    f"missing={len(missing)} added={len(added)}"
)
for key in changed:
    print(f"  CHANGED  {key[0]} @ {key[1]}k")
    print(f"    was: {base[key]}")
    print(f"    now: {live[key]}")
for key in missing:
    print(f"  MISSING  {key[0]} @ {key[1]}k")
for key in added:
    print(f"  ADDED    {key[0]} @ {key[1]}k")

if missing or added:
    print("GOLDEN FAIL: the corpus shape moved — a measurand row appeared or")
    print("vanished; the render audit's scope itself changed and must be")
    print("named in a re-baseline.")
    sys.exit(1)
if changed:
    print("GOLDEN FAIL (night-frontier-Z5): the rendered bytes moved against the")
    print("committed corpus on this machine — under the certified transcendental")
    print("lane there is no machine surface left, so a hash delta is either a")
    print("non-deterministic float op back in the path or a re-baseline that")
    print("must ride its documented cause (a logged run, the why in the ledger).")
    sys.exit(1)
print(
    "GOLDEN PASS: every committed render row is byte-identical on this"
    " machine — the sound IS the bytes, on every arch (night-frontier-Z5)"
)
PYEOF
