#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# OS: Linux only — zylaxion is a Linux sound keyboard; no other OS is supported.
# =============================================================================
# VERSION-TO-.SH SIBLING SCAN TEST BATTERY (night-cafe-mclaren-1)
# =============================================================================
# The discovery lane of scripts/dev/version-to.sh under test: the adaptive
# sibling scan (night-improve-9, generalized; generated-tree exclusion from
# night-cafe-mclaren-1) is extracted VERBATIM from the shipped script at
# runtime — the battery runs the scan a bump will actually run, never a
# copy that can drift. The promise under proof: any manifest the repo
# authors, anywhere in the tree, rides every bump — and nothing generated
# or third-party ever does. Ten fixtures:
#   T1  a future lane named cosmostrix-abc — discovered
#   T2  a deep lane three levels down — discovered (wherever it sits)
#   T3  a cargo-package extraction under a lane's target/ — NOT discovered
#   T4  a vendored upstream manifest under vendor/ — NOT discovered
#   T5  a manifest inside .git/ — NOT discovered
#   T6  a workspace-inherit member (version.workspace = true) — NOT
#       discovered (its version moves with the root automatically)
#   T7  the bump rewrite mirrors version-to.sh's sed on the scan output —
#       every discovered lane lands on the new version
#   T8  the planted generated manifest survives the rewrite untouched —
#       the corruption the exclusion exists to prevent
#   T9  a stale lane (old version) is healed to the new version — drift
#       cannot survive a bump
#   T10 the scan is deterministic — two runs, byte-identical output
# Usage: ./scripts/dev/test-version-to-sibling-scan.sh   (exit 0 = all green)
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${SCRIPT_DIR}"

SCRIPT="${SCRIPT_DIR}/scripts/dev/version-to.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

# Extract the scan and the root version reader verbatim from the shipped
# script — the battery's zero-drift discipline (the changelog battery's
# extraction law, applied to the discovery lane).
sed -n '/^sibling_scan() {/,/^}/p;/^current_version() {/,/^}/p' \
    "${SCRIPT}" > "${WORK}/funcs.sh"
if ! grep -q '^sibling_scan() {' "${WORK}/funcs.sh"; then
    echo "FATAL: the sibling_scan extraction from ${SCRIPT} produced nothing" >&2
    exit 2
fi

PASS=0
FAIL=0
ok() { PASS=$((PASS+1)); echo "  PASS: $1"; }
bad() { FAIL=$((FAIL+1)); echo "  FAIL: $1"; printf '    expected: %s\n    actual:   %s\n' "$2" "$3"; }
check() { if [ "$2" = "$3" ]; then ok "$1"; else bad "$1" "$2" "$3"; fi; }

# The fake root: a manifest the scan must always exclude ('./Cargo.toml'),
# and the lane manifests under proof. current_version() reads CWD's
# Cargo.toml, so the fixtures carry a root too — the version reader rides
# the same tree the scan sees.
mk_root() { # mk_root <version>
    printf '[package]\nname = "fixture-root"\nversion = "%s"\n' "$1" > "${WORK}/Cargo.toml"
}

scan() {
    (cd "${WORK}" && . "${WORK}/funcs.sh" && sibling_scan)
}

# ── T1: the owner's exact future-lane example ─────────────────────────────
mkdir -p "${WORK}/lanes/cosmostrix-abc"
printf '[package]\nname = "cosmostrix-abc"\nversion = "1.0.0"\n' \
    > "${WORK}/lanes/cosmostrix-abc/Cargo.toml"
mk_root "1.0.0"
echo "T1: a future cosmostrix-abc lane is discovered"
check "T1 cosmostrix-abc discovered" "1" \
    "$(scan | grep -c 'lanes/cosmostrix-abc/Cargo.toml')"

# ── T2: depth is irrelevant — wherever it sits ────────────────────────────
mkdir -p "${WORK}/a/b/c/deep-lane"
printf '[package]\nname = "deep-lane"\nversion = "1.0.0"\n' \
    > "${WORK}/a/b/c/deep-lane/Cargo.toml"
echo "T2: a lane three levels down is discovered"
check "T2 deep lane discovered" "1" \
    "$(scan | grep -c 'a/b/c/deep-lane/Cargo.toml')"

# ── T3: the generated-tree exclusion (night-cafe-mclaren-1) ───────────────
mkdir -p "${WORK}/lanes/cosmostrix-abc/target/package/cosmostrix-abc-1.0.0"
printf '[package]\nname = "cosmostrix-abc"\nversion = "0.0.1"\n' \
    > "${WORK}/lanes/cosmostrix-abc/target/package/cosmostrix-abc-1.0.0/Cargo.toml"
echo "T3: a cargo-package extraction under a lane target/ is invisible"
check "T3 generated manifest excluded" "0" \
    "$(scan | grep -c 'target/package')"

# ── T4: vendored upstream manifests are invisible ─────────────────────────
mkdir -p "${WORK}/vendor/upstream"
printf '[package]\nname = "upstream"\nversion = "9.9.9"\n' \
    > "${WORK}/vendor/upstream/Cargo.toml"
echo "T4: a vendored manifest is invisible"
check "T4 vendor manifest excluded" "0" \
    "$(scan | grep -c 'vendor/')"

# ── T5: nothing inside .git/ is a lane ────────────────────────────────────
mkdir -p "${WORK}/.git/fake"
printf '[package]\nname = "git-manifest"\nversion = "8.8.8"\n' \
    > "${WORK}/.git/fake/Cargo.toml"
echo "T5: a manifest inside .git/ is invisible"
check "T5 .git manifest excluded" "0" "$(scan | grep -c '.git/')"

# ── T6: a workspace-inherit member has no own version line ────────────────
mkdir -p "${WORK}/members/inherited"
printf '[package]\nname = "inherited"\nversion.workspace = true\n' \
    > "${WORK}/members/inherited/Cargo.toml"
echo "T6: a version.workspace member is absent (moves with the root)"
check "T6 inherit member absent" "0" "$(scan | grep -c 'members/inherited')"

# ── T7: the bump rewrite lands every discovered lane on the new version ──
# The rewrite mirrors version-to.sh's own sibling sed (the bump loop's
# one-liner); the scan under it is the extracted, zero-drift one.
echo "T7: the bump rewrite heals every discovered lane to the new version"
NEW_VERSION="2.0.0"
scan | while IFS=$'\t' read -r f n v; do
    [ -z "$f" ] && continue
    if [ "${v}" != "${NEW_VERSION}" ]; then
        sed -i "0,/^version = \".*\"/s//version = \"${NEW_VERSION}\"/" "${WORK}/${f#./}"
    fi
done
check "T7 cosmostrix-abc rewritten" "2.0.0" \
    "$(grep -m1 '^version = ' "${WORK}/lanes/cosmostrix-abc/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
check "T7 deep lane rewritten" "2.0.0" \
    "$(grep -m1 '^version = ' "${WORK}/a/b/c/deep-lane/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"

# ── T8: the planted generated manifest survives the rewrite untouched ────
echo "T8: the generated manifest was never rewritten (the corruption proof)"
check "T8 generated manifest untouched" "0.0.1" \
    "$(grep -m1 '^version = ' "${WORK}/lanes/cosmostrix-abc/target/package/cosmostrix-abc-1.0.0/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"

# ── T9: a stale lane is healed — drift cannot survive a bump ──────────────
mkdir -p "${WORK}/lanes/stale-lane"
printf '[package]\nname = "stale-lane"\nversion = "0.5.0"\n' \
    > "${WORK}/lanes/stale-lane/Cargo.toml"
scan | while IFS=$'\t' read -r f n v; do
    [ -z "$f" ] && continue
    if [ "${v}" != "${NEW_VERSION}" ]; then
        sed -i "0,/^version = \".*\"/s//version = \"${NEW_VERSION}\"/" "${WORK}/${f#./}"
    fi
done
echo "T9: a stale lane at 0.5.0 is healed to ${NEW_VERSION}"
check "T9 stale lane healed" "2.0.0" \
    "$(grep -m1 '^version = ' "${WORK}/lanes/stale-lane/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"

# ── T10: the scan is deterministic ────────────────────────────────────────
echo "T10: two runs, byte-identical output"
check "T10 deterministic" "1" \
    "$(scan > "${WORK}/scan1"; scan > "${WORK}/scan2"; cmp -s "${WORK}/scan1" "${WORK}/scan2" && echo 1 || echo 0)"

echo ""
echo "RESULT: ${PASS} passed, ${FAIL} failed"
[ "${FAIL}" -eq 0 ]
