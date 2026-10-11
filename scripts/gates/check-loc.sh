#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# ZYLAXION RUST SOURCE FILE LOC CHECK
#
# Ensures every Rust source file in the tree stays under the hard LOC
# cap. Owner rule (night-improve-12, the zelynic 600-line-law lineage):
# the cap is 600 lines, it applies to EVERY tracked *.rs file ANYWHERE
# in the tree — the checker walks the whole git index (cached plus
# untracked-but-present, respecting .gitignore), so a new over-cap file
# fails BEFORE the commit, not after CI picks it up.
#
# Migration ledger (night-improve-12): the zelynic endgame is zero
# markers (over the cap means SPLIT, same hour). Six zylaxion files
# predate the gate and carry the honest `// LOC_EXEMPT:` marker with a
# per-file justification — tracked migration debt whose split schedule
# is the Owner's call (the same way version bumps are). A NEW file may
# not claim the marker without an explicit owner decision recorded in
# its justification; the endgame stays zero markers.
#
# Usage: scripts/gates/check-loc.sh [MAX_LINES]
#   MAX_LINES: override the default limit (default: 600)
#
# Platform: UNIX-only (uses `git`, `wc -l`). Not for Windows cmd.exe.

set -euo pipefail

MAX_LINES="${1:-600}"
FAILED=0
FOUND=0
EXEMPT_VIOLATIONS=0

# Marker a file uses to self-declare an LOC exemption (tracked debt).
# Must be followed by a justification (one line, free-form text).
EXEMPT_MARKER='// LOC_EXEMPT:'

echo "Rust source file line counts, tree-wide (max ${MAX_LINES}):"
echo ""

FILES=$(git ls-files --cached --others --exclude-standard -- '*.rs' 2>/dev/null |
	grep -v -E '^target/' | sort)

if [ -z "$FILES" ]; then
	echo "No .rs files found in the tree"
	exit 0
fi

while IFS= read -r f; do
	LINES=$(wc -l <"$f")
	printf "  %5d  %s\n" "$LINES" "$f"
	if [ "$LINES" -gt "$MAX_LINES" ]; then
		if grep -qF "$EXEMPT_MARKER" "$f"; then
			EXEMPT_VIOLATIONS=$((EXEMPT_VIOLATIONS + 1))
			echo "    ^^^ over ${MAX_LINES} (exempt via // LOC_EXEMPT: marker — tracked debt, Owner-sequenced split)"
		else
			FAILED=$((FAILED + 1))
			echo "    ^^^ VIOLATES ${MAX_LINES} limit (over the cap means SPLIT —"
			echo "           a pure move into modules, gates green in between)"
		fi
	fi
	FOUND=$((FOUND + 1))
done <<<"$FILES"

echo ""
echo "Total files: ${FOUND}"
echo "Files over ${MAX_LINES} (exempt via // LOC_EXEMPT: marker): ${EXEMPT_VIOLATIONS}"
echo "Files over ${MAX_LINES} (NOT exempt — BUILD FAIL): ${FAILED}"

if [ "$FAILED" -gt 0 ]; then
	echo ""
	echo "FAIL: ${FAILED} file(s) exceed ${MAX_LINES} lines without a"
	echo "// LOC_EXEMPT: marker. Split the file into modules — a pure move,"
	echo "gates green in between. The marker is Owner-granted migration debt,"
	echo "never a self-service escape hatch."
	exit 1
fi

if [ "$EXEMPT_VIOLATIONS" -gt 0 ]; then
	echo ""
	echo "OK (with migration debt): ${EXEMPT_VIOLATIONS} file(s) exceed ${MAX_LINES}"
	echo "but self-declare the // LOC_EXEMPT: marker. The endgame is zero"
	echo "markers — the Owner sequences the splits."
	exit 0
fi

echo ""
echo "OK: all ${FOUND} .rs files are within the ${MAX_LINES}-line cap."
exit 0
