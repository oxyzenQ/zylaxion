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
# Two gates, one honest contract:
#
#   default   — the STRUCTURAL gate always: a missing or added row
#               fails (the measurand's shape is machine-independent),
#               and every byte delta is named with its was/now pair
#               while the run exits 0. The measured boundary lives in
#               the init-time transcendental math (per-target libm and
#               CPU dispatch), so the hash rows are machine
#               fingerprints and a heterogeneous x86 fleet cannot hold
#               a byte gate honestly — the claims lane runs this mode
#               and the freeze stays VISIBLE, named, every run.
#   --strict  — byte identity is ALSO gated (any changed row fails).
#               For fleet-homogeneous lanes only: the arm64 native
#               lane (one CPU family; its committed baseline holds
#               byte-identically, as the parity lane in the same
#               workflow proves) and the owner's A/B machine. A
#               strict failure is a DOCUMENTED-CAUSE event: a
#               re-baseline walks the rates-parity law (a logged run,
#               the law moved first, the why in the ledger). The gate
#               never stamps the baseline itself — stamping stays
#               zactrix-rates-parity.sh's exclusive, logged-run
#               contract.
#
# The engine is untouched by this script: it only renders and
# compares. No audio device, no network, no eBPF attachment.
#
# Usage:
#   scripts/audit/zactrix-golden.sh [--strict] [baseline-file]
#
# Exit code: 0 when the structural gate holds and (under --strict)
# every row re-renders byte-identical; 1 otherwise (every delta named
# in the output).

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"

STRICT=0
if [ "${1:-}" = "--strict" ]; then
        STRICT=1
        shift
fi
BASELINE="${1:-scripts/audit/rates-parity-baseline.txt}"

cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- --rates \
    | tee "$OUT/golden-run.txt"

grep '^| `' "$OUT/golden-run.txt" > "$OUT/golden-rows.txt"
grep '^| `' "$BASELINE" > "$OUT/golden-baseline-rows.txt"

echo "GOLDEN gate: the re-rendered corpus vs $BASELINE (one corpus, no duplicates)"
python3 - "$OUT/golden-baseline-rows.txt" "$OUT/golden-rows.txt" "$STRICT" <<'PYEOF'
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
strict = sys.argv[3] == "1"
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

structural = bool(missing or added)
if structural:
    print("GOLDEN FAIL: the corpus shape moved — a measurand row appeared or")
    print("vanished; the render audit's scope itself changed and must be")
    print("named in a re-baseline.")
    sys.exit(1)
if strict and changed:
    print("GOLDEN FAIL (strict): byte identity broke on a fleet-homogeneous")
    print("lane — every delta is named above; a re-baseline needs the")
    print("documented-cause law (a logged run, the why in the ledger).")
    sys.exit(1)
if changed:
    print(
        f"GOLDEN RECORD (not gated on this lane): {len(changed)} of "
        f"{len(base)} rows carry the init-time transcendental boundary "
        "(per-target libm/CPU dispatch) — named above, the same boundary "
        "the rates-parity lane documents; the freeze stays visible."
    )
else:
    print(
        "GOLDEN PASS: every committed render row is byte-identical — "
        "the sound IS the bytes"
    )
PYEOF
