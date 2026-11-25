#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# TAMPER-SIG PROBE (night-depth-hardening-Z1 D2)
#
# The end-to-end tamper test the D2 task demands: a build leg that
# swaps ONE byte of the embedded tap object must fail `sig` BY NAME.
# The D4 tap-object row recomputes the blake3 over the bytes the
# binary actually carries and compares them to the build-time stamp
# (build/tapstamp.rs); a post-build patch cannot keep both digests
# agreeing.
#
# The probe simulates the exact attack story against a real artifact:
# locate the object's bytes inside the built binary (the staged
# OUT_DIR object is the byte-exact needle - include_bytes! embedded
# them verbatim), flip one byte in a patched COPY (the original
# artifact is never modified), and run `sig` from the copy:
#
#   control arm - the pristine binary reports the build's OWN stamped
#                 truth (night-netrunner-Z1): AUTHENTIC (6/6) on a
#                 HEAD signed by the pinned maintainer key, honestly
#                 UNSIGNED (5/6) with the D5 commit-signature row
#                 FAILing on an unsigned HEAD (a CI build from an
#                 unsigned commit)
#   tamper arm  - the patched binary exits 2, the verdict line reads
#                 TAMPERED, and the D4 tap-object row FAILs naming
#                 both digests ("the tap was patched after the build")
#
# The unit tree pins the same three arms through the pure row function
# (test/info/signature_tests.rs); this probe proves the mechanism on a
# real patched binary, which no unit test can.
#
# Usage:
#   scripts/audit/tamper-sig-probe.sh target/debug/zylaxion
#
# Exit: 0 every arm holds; 1 the arm that failed names itself.

set -euo pipefail

BIN="${1:?usage: tamper-sig-probe.sh <built-zylaxion-binary>}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

PASS=0

pass() { PASS=$((PASS + 1)); echo "PASS: $*"; }
die() { echo "FAIL: $*"; exit 1; }

[[ -f "$BIN" && -x "$BIN" ]] || die "the binary $BIN is absent or not executable - build first (cargo build)"

# The staged object candidates: every out/zylaxion-tap.o the target
# tree holds (debug and release lanes each stage one). The needle is
# whichever candidate the binary actually carries - the host-built
# object is the embedded one on a git checkout (the ebpf-prebuilt/
# mirror is the commit-time pin, not this host's fresh bytes).
mapfile -t STAGED < <(find "$ROOT/target" -path '*/out/zylaxion-tap.o' -print 2>/dev/null | sort -u)
((${#STAGED[@]} > 0)) || die "no staged object under $ROOT/target (*/out/zylaxion-tap.o) - the cargo build did not run its build script"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
PATCHED="$TMP/zylaxion-patched"

# The byte surgery (python3, std only): find one staged candidate's
# byte-exact run inside the image, flip ONE byte at its center, write
# the patched copy. Prints the offset it chose - the evidence line.
python3 - "$BIN" "$PATCHED" "${STAGED[@]}" <<'PY'
import pathlib
import sys

image = pathlib.Path(sys.argv[1]).read_bytes()
patched_path = pathlib.Path(sys.argv[2])
for cand in sys.argv[3:]:
    data = pathlib.Path(cand).read_bytes()
    at = image.find(data)
    if at >= 0:
        flip = at + len(data) // 2
        patched = bytearray(image)
        patched[flip] ^= 0x01
        patched_path.write_bytes(patched)
        print(f"probe: embedded object found at offset {at} ({len(data)} bytes, "
              f"staged {cand}); byte {flip} flipped")
        sys.exit(0)
print("probe: no staged object's bytes appear in the binary - the image "
      "does not carry the object the build staged", file=sys.stderr)
sys.exit(3)
PY
chmod 755 "$PATCHED"

# Control arm: the pristine binary reports the build's OWN stamped
# truth (night-netrunner-Z1, the six-depth honesty law): AUTHENTIC
# (6/6) when HEAD's commit is signed by the pinned maintainer key,
# TAMPERED (5/6) with the D5 commit-signature row FAILing on an
# unsigned HEAD (a contributor/CI build from an unsigned commit is
# the pristine truth for its build, not a tamper). The probe derives
# the expectation from the repo's own HEAD the same way build/gpg.rs
# does - a gpgsig header on the commit object - so the control arm
# can never demand a verdict the build could not honestly produce.
# The rc capture rides `|| RC=$?` (the tamper arm's pattern): under
# set -e a bare assignment whose substitution exits non-zero (the
# unsigned-HEAD exit 1) would kill the probe before the expectation
# arms can read it.
CTRL_RC=0
CTRL="$("$BIN" sig 2>&1)" || CTRL_RC=$?
if git -C "$ROOT" cat-file commit HEAD 2>/dev/null | grep -q '^gpgsig '; then
    [[ "$CTRL_RC" -eq 0 ]] \
        || die "control arm: a signed-HEAD build must report AUTHENTIC, got exit $CTRL_RC:\n$CTRL"
    grep -F "SIGNATURE: AUTHENTIC (6/6 depths verified)" <<<"$CTRL" >/dev/null \
        || die "control arm: no AUTHENTIC (6/6) verdict in the signed-HEAD report:\n$CTRL"
    grep -E '^D5 commit-signature +pass' <<<"$CTRL" >/dev/null \
        || die "control arm: the signed-HEAD D5 commit-signature row is not green:\n$CTRL"
    pass "control arm: signed-HEAD sig AUTHENTIC (6/6), D4 tap-object and D5 commit-signature pass"
else
    [[ "$CTRL_RC" -eq 1 ]] \
        || die "control arm: an unsigned-HEAD build must fail D5 and exit 1, got exit $CTRL_RC:\n$CTRL"
    grep -F "SIGNATURE: UNSIGNED (5/6 depths verified)" <<<"$CTRL" >/dev/null \
        || die "control arm: no UNSIGNED (5/6) verdict in the unsigned-HEAD report:\n$CTRL"
    grep -E '^D5 commit-signature +FAIL' <<<"$CTRL" >/dev/null \
        || die "control arm: the unsigned-HEAD D5 row did not fail:\n$CTRL"
    pass "control arm: unsigned-HEAD sig honestly UNSIGNED (5/6), D5 commit-signature FAIL by name"
fi
grep -E '^D4 tap-object +pass' <<<"$CTRL" >/dev/null \
    || die "control arm: the pristine D4 tap-object row is not green:\n$CTRL"

# Tamper arm: the patched copy refuses BY NAME, exit 2. The copy is
# addressed by its absolute path - the "./" prefix would turn an
# absolute value into a wrong relative one.
RC=0
TAMP="$("$PATCHED" sig 2>&1)" || RC=$?
grep -F "SIGNATURE: TAMPERED" <<<"$TAMP" >/dev/null \
    || die "tamper arm: the patched binary did not read TAMPERED (exit=$RC):\n$TAMP"
grep -E '^D4 tap-object +FAIL' <<<"$TAMP" >/dev/null \
    || die "tamper arm: the D4 tap-object row did not fail in the patched report:\n$TAMP"
grep -F "the tap was patched after the build" <<<"$TAMP" >/dev/null \
    || die "tamper arm: the D4 row detail does not name the patch:\n$TAMP"
[[ "$RC" -eq 2 ]] || die "tamper arm: expected the tamper exit code 2, got $RC:\n$TAMP"
pass "tamper arm: patched sig TAMPERED (exit 2), D4 tap-object FAIL names both digests"

echo "tamper-sig probe: $PASS/2 arms hold - the ring-0 asset is inside the self-verification circle"
