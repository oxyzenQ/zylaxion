#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# ZYLAXION RUST SOURCE FILE LOC CHECK
#
# Ensures every Rust source file in the tree stays under the hard LOC
# cap. Owner rule (night-improve-18, the no-mercy era — the zelynic
# NIGHT-improve-44 wording): the cap is 600 lines, it applies to EVERY
# tracked *.rs file ANYWHERE in the tree — the checker walks the whole
# git index (cached plus untracked-but-present, respecting .gitignore),
# so a new over-cap file fails BEFORE the commit, not after CI picks it
# up. NO file is exempt: the // LOC_EXEMPT: marker the migration era
# honored as tracked debt is RETIRED — over the cap means SPLIT, the
# same hour, no marker, no mercy, with precision kept (the split is a
# pure move, gates green in between).
#
# Layout law (night-improve-18, enforced here beside the cap):
#
#   1. src-root single-file policy — every crate's src/ root holds
#      exactly its entry file (main.rs for the binary, lib.rs for a
#      library). Every other module lives in its subsystem directory
#      (dir/mod.rs style). A stray root module is a layout failure,
#      not a size one — it fails before any line is counted.
#   2. the test tree — every *.rs test file lives under its crate's
#      test/ directory (the zelynic Pattern C: #[path]-wired modules,
#      one theme per file), never inline in src/. The tree-wide walk
#      counts test files under the same 600 cap.
#
# Usage: scripts/gates/check-loc.sh [MAX_LINES]
#   MAX_LINES: override the default limit (default: 600)
#
# Platform: UNIX-only (uses `git`, `wc -l`). Not for Windows cmd.exe.

set -euo pipefail

MAX_LINES="${1:-600}"
FAILED=0
FOUND=0

# ── src-root single-file policy (night-improve-18) ─────────────────────
# Checked FIRST: a stray root module is a layout failure, not a size
# one. Every crate directory under crates/ (plus a repo-root src/ if
# one ever appears) may hold exactly one .rs file at its src/ root —
# the entry point (main.rs or lib.rs).
ROOT_VIOLATIONS=0
check_src_root() {
        local src_dir="$1"
        local entry="$2"
        local strays
        strays=$(find "$src_dir" -maxdepth 1 -name '*.rs' ! -name "$entry" 2>/dev/null || true)
        if [ -n "$strays" ]; then
                ROOT_VIOLATIONS=$((ROOT_VIOLATIONS + 1))
                echo "FAIL: $src_dir/ root must contain only $entry (single-file policy)."
                echo "Stray root module(s) found:"
                while IFS= read -r stray; do
                        echo "    ${stray}"
                done <<<"$strays"
                echo ""
                echo "Move each into its subsystem directory (dir/mod.rs style):"
                echo "    git mv ${src_dir}/<module>.rs ${src_dir}/<module>/mod.rs"
                echo "(a moved module's relative #[path] wirings gain one ../ level)"
                echo ""
        fi
}

if [ -d src ]; then
        # repo-root src (the zelynic shape; zylaxion is a workspace but the
        # policy holds if a root crate ever appears)
        if [ -f src/main.rs ]; then
                check_src_root src main.rs
        else
                check_src_root src lib.rs
        fi
fi
for crate_src in crates/*/src; do
        [ -d "$crate_src" ] || continue
        if [ -f "$crate_src/main.rs" ]; then
                check_src_root "$crate_src" main.rs
        else
                check_src_root "$crate_src" lib.rs
        fi
done

if [ "$ROOT_VIOLATIONS" -gt 0 ]; then
        exit 1
fi

echo "Rust source file line counts, tree-wide (max ${MAX_LINES}):"
echo ""

FILES=$(git ls-files --cached --others --exclude-standard -- '*.rs' 2>/dev/null |
        grep -v -E '^target/' | sort)

if [ -z "$FILES" ]; then
        echo "No .rs files found in the tree"
        exit 0
fi

while IFS= read -r f; do
        # a mid-rename working tree can carry stale index entries — the
        # gate scores files that exist
        [ -f "$f" ] || continue
        LINES=$(wc -l <"$f")
        printf "  %5d  %s\n" "$LINES" "$f"
        if [ "$LINES" -gt "$MAX_LINES" ]; then
                FAILED=$((FAILED + 1))
                echo "    ^^^ VIOLATES ${MAX_LINES} limit (over the cap means SPLIT —"
                echo "           a pure move into modules, gates green in between;"
                echo "           tests move to the crate's test/ tree, the no-mercy"
                echo "           era honors no // LOC_EXEMPT: marker)"
        fi
        FOUND=$((FOUND + 1))
done <<<"$FILES"

echo ""
echo "Total files: ${FOUND}"
echo "Files over ${MAX_LINES} (NOT exempt — BUILD FAIL): ${FAILED}"

if [ "$FAILED" -gt 0 ]; then
        echo ""
        echo "FAIL: ${FAILED} file(s) exceed ${MAX_LINES} lines. The no-mercy era"
        echo "(night-improve-18) honors no marker: split the file into modules —"
        echo "a pure move, gates green in between; tests live under the crate's"
        echo "test/ tree (Pattern C #[path] wiring), one theme per file."
        exit 1
fi

echo ""
echo "OK: all ${FOUND} .rs files are within the ${MAX_LINES}-line cap, every"
echo "src/ root carries only its entry file, and the test tree is Pattern C."
exit 0
