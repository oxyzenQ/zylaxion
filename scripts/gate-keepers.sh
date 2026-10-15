#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# ZYLAXION GATE-KEEPERS — the pre-commit / pre-push gate orchestrator
# (night-improve-12, the zelynic gate-keepers lineage).
#
# Every section is a hard gate: a non-zero exit anywhere fails the run.
# All warnings are failures; none are ignored. Run it before every
# commit; CI (Dragon Guard - CI) enforces its own lanes on every push:
# fmt, clippy, tests, actionlint/yamllint/codespell, the supply-chain
# scan, and the supermassive build matrix.
#
# Sections:
#   1.  File permissions (scripts/gates/check-permissions.sh — the
#       exact 644/755 contract, shebang parity included; --fix-all
#       repairs in place)
#   2.  SPDX headers (scripts/gates/check-headers.sh — copyright +
#       GPL-3.0-only identifier on every code/config/doc file, the
#       OS line on every .sh)
#   3.  Rust LOC cap (scripts/gates/check-loc.sh — 600 lines, every
#       tracked .rs anywhere in the tree; night-improve-18's no-mercy
#       era honors no marker — over the cap means SPLIT — plus the
#       src-root single-file policy and the Pattern C test tree)
#   4.  Scripts LOC cap (scripts/gates/check-scripts-loc.sh — 1000
#       lines for every .sh/.py under scripts/; # LOC_EXEMPT: marker)
#   5.  Language discipline (scripts/gates/check-language.sh —
#       English-only tree, non-Latin sweep with the
#       NON_LATIN_FIXTURE marker, Indonesian vocabulary blocklist)
#   6.  Emoji sweep (inline — the codepoint classes that matter:
#       astral emoji, dingbats, misc symbols/arrows, variation
#       selectors, the zero-width joiner. Frozen historical records —
#       CHANGELOG.md and the v6.0.0-era audit docs — are excluded as
#       the never-rewritten class, the same policy as every other
#       gate)
#   7.  Actions pins health (scripts/gates/check-actions-pins.sh —
#       the self-healing SHA-pin estate; honest-skips offline, warn
#       mode by default, never blocks a push on upstream flakiness)
#   8.  Release parity (night-improve-12c —
#       scripts/gates/check-release-parity.sh: the live platforms'
#       -C codegen tokens in .cargo/config.toml's pro-linux-amd64-*
#       aliases must equal the release workflow matrix's rustflags, so
#       a local alias build reproduces the release optimization tier
#       the README documents bit-for-bit; a one-file edit can no
#       longer silently break that contract)
#   9.  Name-case rule (night-improve-20 —
#       scripts/gates/check-name-case.py: the project name is lowercase
#       zylaxion in every context; the gate scans EVERY tracked file
#       and every tracked path (git ls-files — the .cargo/ and
#       .github/ hidden trees included, nothing excluded) and fails on
#       any casing outside the two legal families: lowercase, and the
#       identifier family (ZYLAXION_* env vars, the scripts/ banner
#       titles). A wrong-case name anywhere fails the push)
#   10. Pre-commit hook self-install (night-improve-31 — the zelynic
#       NIGHT-dinner-1 shape: the first wholesale run on a clone
#       points core.hooksPath at .githooks, repo-local, never
#       --global, so every future commit on that clone executes the
#       commit-time prebuilt gate (scripts/gates/check-commit-gate.sh:
#       parity + the ebpf/ <-> ebpf-prebuilt/ pairing rule) even when
#       this wholesale script is forgotten. The hook enforces the same
#       lane contract at commit time that sections above enforce at
#       push time)
#
# Usage:
#   bash scripts/gate-keepers.sh             # full gate run
#   bash scripts/gate-keepers.sh --fix-all   # permission fixes in
#                                            # place, then full run
#
# Platform: UNIX-only (bash, git, python3). Not for Windows cmd.exe.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# night-total-lts-4: a failed cd must never leave the wholesale gates
# policing the wrong tree — fail fast instead.
cd "$REPO_ROOT" || exit 1

FIX_ALL=false
if [[ "${1:-}" == "--fix-all" ]]; then
        FIX_ALL=true
elif [[ $# -gt 0 ]]; then
        echo "ERROR: unknown argument: $1 (expected --fix-all or none)"
        exit 1
fi

FAILED_SECTIONS=()

run_section() {
        local num="$1"
        local name="$2"
        shift 2
        echo ""
        echo "── [$num] $name ──────────────────────────────────────────"
        if "$@"; then
                return 0
        else
                FAILED_SECTIONS+=("$num. $name")
                return 0 # keep running the remaining sections; verdict at the end
        fi
}

echo "zylaxion gate-keepers — the full gate run"
echo "repository: $REPO_ROOT"

# ── 0. pre-commit hook self-install (night-improve-31) ────────────────────
# The wiring, not a check: point core.hooksPath at .githooks so every
# commit on this clone runs the commit gate. Repo-local config, never
# --global; harmless in CI where the checkout is disposable.
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        hook_path="$(git config --local core.hooksPath || true)"
        if [ "${hook_path}" != ".githooks" ]; then
                git config --local core.hooksPath .githooks
                echo ""
                echo "── [0] pre-commit hook (self-install) ──────────────────"
                echo "core.hooksPath=.githooks — every commit now runs the prebuilt commit gate"
        else
                echo "core.hooksPath already .githooks — the commit gate is active"
        fi
else
        echo "not inside a git work tree — hook self-install skipped"
fi

if $FIX_ALL; then
        run_section 1 "File permissions (--fix-all)" \
                bash scripts/gates/check-permissions.sh --fix
else
        run_section 1 "File permissions" \
                bash scripts/gates/check-permissions.sh
fi

run_section 2 "SPDX headers" \
        bash scripts/gates/check-headers.sh

run_section 3 "Rust LOC cap (600)" \
        bash scripts/gates/check-loc.sh

run_section 4 "Scripts LOC cap (1000)" \
        bash scripts/gates/check-scripts-loc.sh

run_section 5 "Language discipline" \
        bash scripts/gates/check-language.sh

# ── 6. Emoji sweep (inline) ────────────────────────────────────────────────
run_section 6 "Emoji sweep" python3 - <<'PYEOF'
import os
import sys

# Emoji-class codepoint ranges (enough coverage for the sweep; the
# goal is to block emoji, not to enumerate every Unicode symbol).
RANGES = (
    (0x1F000, 0x1FFFF),  # astral emoji (incl. regional indicator flags)
    (0x2600, 0x27BF),   # Miscellaneous Symbols .. Dingbats
    (0x2300, 0x23FF),   # Misc Technical (clocks, media controls, key caps)
    (0x2B00, 0x2BFF),   # Misc Symbols and Arrows (stars, thick arrows)
    (0xFE0F, 0xFE0F),   # Variation Selector-16 (emoji presentation)
    (0xFE0E, 0xFE0E),   # Variation Selector-15 (text presentation)
    (0x200D, 0x200D),   # Zero-Width Joiner (emoji composition)
)

# Allowed non-ASCII (typographic house style, never flagged): prose
# arrows U+2190..21FF, box drawing + geometric U+2500..25FF, em/en dash,
# ellipsis, bullet, math operators. None of those ranges intersect the
# fail blocks, so the allowlist needs no carve-outs.

SKIP_DIRS = {".git", "target", "assets", "dist", "node_modules", "__pycache__"}
# Frozen historical records — never rewritten, the same exclusion
# policy as every other gate.
SKIP_FILES = {
    "CHANGELOG.md",
    "CHANGELOG-v6.0.0.md",  # the frozen v6.0.0 era record (archive home)
    "audit-v6.0.0.md",
    "audit-v6.0.0-evidence.md",
    "release-readiness-v6.0.0.md",
}
SKIP_SUFFIXES = (".png", ".lock")


def is_emoji(cp):
    return any(lo <= cp <= hi for lo, hi in RANGES)


findings = []
for root, dirnames, filenames in os.walk("."):
    dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
    for name in sorted(filenames):
        if name in SKIP_FILES or name.endswith(SKIP_SUFFIXES):
            continue
        path = os.path.join(root, name)
        try:
            with open(path, encoding="utf-8") as fh:
                text = fh.read()
        except (UnicodeDecodeError, OSError):
            continue  # binary — not text content
        for lineno, line in enumerate(text.splitlines(), 1):
            if any(is_emoji(ord(ch)) for ch in line):
                findings.append(
                    "  %s:%d: %s" % (path, lineno, line.strip()[:80])
                )

if findings:
    print("EMOJI SWEEP FAIL (%d finding(s)):" % len(findings))
    for f in findings[:20]:
        print(f)
    if len(findings) > 20:
        print("  ... and %d more" % (len(findings) - 20))
    print("The tree carries no emoji. Remove the characters; word")
    print("verdicts and ASCII art carry the meaning instead.")
    sys.exit(1)

print("OK: emoji sweep clean (word verdicts only, no garbage glyphs)")
PYEOF

# ── 7. Actions pins health (honest skips offline; warn by default) ─────────
run_section 7 "Actions pins health" \
        bash scripts/gates/check-actions-pins.sh

# ── 8. Release parity (the alias/matrix optimization-tier contract) ────────
run_section 8 "Release parity" \
        bash scripts/gates/check-release-parity.sh

# ── 9. Name-case rule (the lowercase law, nothing excluded) ─────────────────
run_section 9 "Name-case rule" \
        python3 scripts/gates/check-name-case.py

# ── Verdict ─────────────────────────────────────────────────────────────
echo ""
if [ "${#FAILED_SECTIONS[@]}" -gt 0 ]; then
        echo "GATE-KEEPERS FAIL (${#FAILED_SECTIONS[@]} section(s)):"
        for s in "${FAILED_SECTIONS[@]}"; do
                echo "  - $s"
        done
        exit 1
fi

echo "GATE-KEEPERS OK: all sections green."
exit 0
