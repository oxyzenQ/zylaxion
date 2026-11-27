#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# OS: Linux only — zylaxion is a Linux sound keyboard; no other OS is supported.
# =============================================================================
# VERSION-TO-.SH CHANGELOG CARRY TEST BATTERY (night-ledger-1)
# =============================================================================
# The changelog lane of scripts/dev/version-to.sh under test: the ledger
# line and the detail carry (night-ledger-1) are extracted VERBATIM from
# the shipped script at runtime — the battery runs the functions a bump
# will actually run, never a copy that can drift. Ten fixtures:
#   F1  the real ledger shape, a seeded running body — line, carry,
#       emptied running section
#   F2  no tagline — the minimal heading
#   F3  an empty running section — nothing to carry, line alone
#   F4  idempotence — a re-run at a recorded version writes nothing
#   F5  no anchors at all — warn, write nothing
#   F6  a foreign ## [Released] shape — warn, skip line AND carry
#   F7  the running section last in file — carried at end of file
#   F8  the carried body is byte-verbatim against its source
#   F9  an ambiguous ledger — line writes, the carry refuses
#   F10 a fresh ledger (no ## [Released] yet) — created AND carry fires
# Usage: ./scripts/dev/test-version-to-changelog.sh   (exit 0 = all green)
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${SCRIPT_DIR}"

# The shell brand palette (night-cafe-1), the same lane version-to.sh runs on.
# shellcheck disable=SC1091 source=scripts/lib/ux_palette.sh
. "${SCRIPT_DIR}/scripts/lib/ux_palette.sh"

SCRIPT="${SCRIPT_DIR}/scripts/dev/version-to.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

# Extract the two functions verbatim from the shipped script.
sed -n '/^changelog_release() {/,/^}/p;/^changelog_carry_detail() {/,/^}/p' \
    "${SCRIPT}" > "${WORK}/funcs.sh"
if [ ! -s "${WORK}/funcs.sh" ]; then
    echo "FATAL: the function extraction from ${SCRIPT} produced nothing" >&2
    exit 2
fi

PASS=0
FAIL=0
ok() { PASS=$((PASS+1)); echo "  PASS: $1"; }
bad() { FAIL=$((FAIL+1)); echo "  FAIL: $1"; printf '    expected: %s\n    actual:   %s\n' "$2" "$3"; }
check() { if [ "$2" = "$3" ]; then ok "$1"; else bad "$1" "$2" "$3"; fi; }

run_bump() { # run_bump <fixture-file> <version> [tagline]
    local f="$1" ver="$2" tag="${3:-}"
    cp "$f" "${WORK}/CHANGELOG.md"
    (cd "${WORK}" && . "${WORK}/funcs.sh" && changelog_release "${ver}" "${tag}")
}

# F1 rides the real ledger as it stands — the shape a bump actually meets.
# The live running body rides the release cycle instead: empty right after
# a bump, full mid-cycle. A body-less carry left this battery red on every
# post-release commit (the v80.2.2 lesson: green mid-cycle, five failures
# the day the next release shipped). The fixture keeps the real ledger and
# versioned sections above it and SEEDS a known body under the live
# ## [Unreleased] heading, so the carry fires deterministically on every
# cycle state — the script under test still meets the real file's shape.
# The bump rides 99.0.0 - a version the live ledger will not record in
# this file's era: a recorded version no-ops (the F4 law) and the carry
# would never fire (the v80.3.0-era constants went stale the day the
# ledger outgrew them - night-depth-cleanup-2 retires the drift).
if ! grep -q '^## \[Unreleased\]$' "${SCRIPT_DIR}/CHANGELOG.md"; then
    echo "FATAL: the live CHANGELOG.md carries no ## [Unreleased] anchor — the F1 fixture cannot seed it" >&2
    exit 2
fi
{
    # Drop whatever body the live running section holds this cycle; the
    # seeded body replaces it. The seed splices DIRECTLY under the
    # anchored "## [Unreleased]" heading — night-cafe-mclaren-1: the
    # live ledger carries historical sections below the running one
    # (the Carried record and Frozen archives), so the old "append the
    # seed at EOF" builder planted the body in the archive zone, the
    # carry found the running section empty, and F1/F2 went red on the
    # shipped tree while the carry itself was healthy (the failures
    # were the fixture's, not the tool's — F7/F10 pass on the same
    # extracted functions). A "## " heading at column zero still ends
    # the drop (the running body ends where the next section starts).
    awk '
        /^## \[Unreleased\]$/ && !done {
            drop = 1
            print
            print ""
            print "### Changed"
            print ""
            print "- seed-alpha the carry rides a body the battery planted"
            print "- seed-beta the second line rides along"
            done = 1
            next
        }
        drop && /^## / { drop = 0 }
        drop { next }
        { print }
    ' "${SCRIPT_DIR}/CHANGELOG.md"
} > "${WORK}/f1"

echo "F1: real shape - ledger line, carry into own section, Unreleased emptied"
run_bump "${WORK}/f1" "99.0.0" "growth naturally"
check "F1 ledger line on top" "v99.0.0" \
    "$(awk '/^## \[Released\]$/{getline; getline; print; exit}' "${WORK}/CHANGELOG.md")"
check "F1 unreleased emptied" "1" \
    "$(awk '/^## \[Unreleased\]$/{f=1;next} f&&/^## /{exit} f{c++} END{print c+0}' "${WORK}/CHANGELOG.md")"
check "F1 section heading shape" "1" \
    "$(grep -c '^## \[v99.0.0\] — growth naturally — [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$' "${WORK}/CHANGELOG.md")"
check "F1 section order" "## [Released]|## [v99.0.0] — growth naturally" \
    "$(grep -m2 '^## \[' "${WORK}/CHANGELOG.md" | sed 's/ — [0-9-]*$//' | paste -sd'|')"
F1_UNREL_LINE="$(grep -n '^## \[Unreleased\]$' "${WORK}/CHANGELOG.md" | cut -d: -f1)"
F1_SEC_LINE="$(grep -n '^## \[v99.0.0\]' "${WORK}/CHANGELOG.md" | cut -d: -f1)"
if [ "${F1_SEC_LINE:-99999}" -lt "${F1_UNREL_LINE:-0}" ]; then ok "F1 carried above unreleased"; else bad "F1 carried above unreleased" "section line < unreleased line" "${F1_SEC_LINE:-?} vs ${F1_UNREL_LINE:-?}"; fi
F1_MARKER="$(awk '/^## \[Unreleased\]$/{f=1;next} f&&/^## /{exit} f&&NF&&!/^### /{print; exit}' "${WORK}/f1")"
check "F1 first bullet carried" "1" \
    "$(awk '/^## \[v99.0.0\]/{f=1;next} f&&/^## /{exit} f{print}' "${WORK}/CHANGELOG.md" | grep -cF -- "${F1_MARKER}")"
check "F1 last released intact" "1" \
    "$(grep -c '^## \[v80.0.2\] — be honest — 2026-10-31$' "${WORK}/CHANGELOG.md")"
cp "${WORK}/CHANGELOG.md" "${WORK}/f1.result"

echo "F2: no tagline - minimal heading"
run_bump "${WORK}/f1" "99.0.1"
check "F2 heading without tagline" "1" \
    "$(grep -c '^## \[v99.0.1\] — [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$' "${WORK}/CHANGELOG.md")"

echo "F3: empty unreleased body - nothing to carry, ledger line only"
printf '# Changelog\n\n## [Released]\n\nv80.2.1\n\n## [Unreleased]\n\n' > "${WORK}/f3"
run_bump "${WORK}/f3" "80.2.2"
check "F3 ledger written" "v80.2.2" \
    "$(awk '/^## \[Released\]$/{getline; getline; print; exit}' "${WORK}/CHANGELOG.md")"
check "F3 no carried section" "0" "$(grep -c '^## \[v80.2.2\] —' "${WORK}/CHANGELOG.md")"

echo "F4: idempotence - re-run at a recorded version writes nothing"
cp "${WORK}/CHANGELOG.md" "${WORK}/f4.before"
run_bump "${WORK}/f4.before" "80.2.2"
if diff -q "${WORK}/f4.before" "${WORK}/CHANGELOG.md" > /dev/null; then ok "F4 no change"; else bad "F4 no change" "identical" "changed"; fi

echo "F5: no anchors at all - warn, write nothing"
printf '# Changelog\n\nJust prose.\n' > "${WORK}/f5"
cp "${WORK}/f5" "${WORK}/f5.before"
run_bump "${WORK}/f5" "99.0.0"
if diff -q "${WORK}/f5.before" "${WORK}/CHANGELOG.md" > /dev/null; then ok "F5 untouched"; else bad "F5 untouched" "identical" "changed"; fi

echo "F6: unexpected Released shape - warn, skip line AND carry"
printf '# Changelog\n\n## [Released]\nv80.2.1\n\n## [Unreleased]\n\n### Added\n\n- one\n' > "${WORK}/f6"
cp "${WORK}/f6" "${WORK}/f6.before"
run_bump "${WORK}/f6" "99.0.0"
if diff -q "${WORK}/f6.before" "${WORK}/CHANGELOG.md" > /dev/null; then ok "F6 untouched"; else bad "F6 untouched" "identical" "changed"; fi

echo "F7: unreleased is the last section - carried under the Released ledger"
printf '# Changelog\n\n## [Released]\n\nv80.1.0\n\n## [Unreleased]\n\n### Fixed\n\n- late note\n' > "${WORK}/f7"
run_bump "${WORK}/f7" "80.1.1"
check "F7 carried section exists" "1" \
    "$(grep -c '^## \[v80.1.1\] — [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$' "${WORK}/CHANGELOG.md")"
check "F7 unreleased emptied" "0" \
    "$(awk '/^## \[Unreleased\]$/{f=1;next} f&&/^## /{exit} f{print}' "${WORK}/CHANGELOG.md" | grep -c 'late note')"
check "F7 note preserved" "1" "$(grep -c 'late note' "${WORK}/CHANGELOG.md")"

echo "F8: carried body is byte-verbatim"
awk '/^## \[Unreleased\]$/{f=1;next} f&&/^## /{exit} f{print}' "${WORK}/f1" |
    awk 'NF && !s { s = NR } NF { e = NR } { l[NR] = $0 } END { for (i = s; i <= e; i++) print l[i] }' \
    > "${WORK}/f8.expected"
awk '/^## \[v99.0.0\]/{f=1;next} f&&/^## /{exit} f{print}' "${WORK}/f1.result" |
    awk 'NF && !s { s = NR } NF { e = NR } { l[NR] = $0 } END { for (i = s; i <= e; i++) print l[i] }' \
    > "${WORK}/f8.actual"
if diff -q "${WORK}/f8.expected" "${WORK}/f8.actual" > /dev/null; then ok "F8 body verbatim"; else bad "F8 body verbatim" "byte-identical" "drifted"; fi

echo "F9: ambiguous ledger (two Unreleased headings) - line writes, carry refuses"
printf '# Changelog\n\n## [Released]\n\nv80.0.1\n\n## [Unreleased]\n\n### Added\n\n- one\n\n## [Unreleased]\n\n### Fixed\n\n- two\n' > "${WORK}/f9"
run_bump "${WORK}/f9" "80.0.2"
check "F9 ledger written" "v80.0.2" \
    "$(awk '/^## \[Released\]$/{getline; getline; print; exit}' "${WORK}/CHANGELOG.md")"
check "F9 no section" "0" "$(grep -c '^## \[v80.0.2\] —' "${WORK}/CHANGELOG.md")"
check "F9 bodies stay" "2" "$(grep -c '^- one\|^- two' "${WORK}/CHANGELOG.md")"

echo "F10: fresh ledger (no Released yet) - section created AND carry fires"
printf '# Changelog\n\n## [Unreleased]\n\n### Added\n\n- first cycle\n' > "${WORK}/f10"
run_bump "${WORK}/f10" "81.0.0"
check "F10 ledger created" "v81.0.0" \
    "$(awk '/^## \[Released\]$/{getline; getline; print; exit}' "${WORK}/CHANGELOG.md")"
check "F10 carry fired" "1" \
    "$(grep -c '^## \[v81.0.0\] — [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$' "${WORK}/CHANGELOG.md")"
check "F10 note preserved once" "1" "$(grep -c 'first cycle' "${WORK}/CHANGELOG.md")"
check "F10 unreleased clean" "0" \
    "$(awk '/^## \[Unreleased\]$/{f=1;next} f&&/^## /{exit} f&&NF{c++} END{print c+0}' "${WORK}/CHANGELOG.md")"

echo ""
echo "RESULT: ${PASS} passed, ${FAIL} failed"
[ "${FAIL}" -eq 0 ]
