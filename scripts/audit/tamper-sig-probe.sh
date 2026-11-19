#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
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
#   control arm - the pristine binary reports AUTHENTIC (5/5 depths)
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

# Control arm: the pristine binary is AUTHENTIC with the D4 row green.
CTRL="$("$BIN" sig 2>&1)" || die "control arm: the pristine binary's sig exited non-zero - the build itself is not authentic"
grep -F "SIGNATURE: AUTHENTIC (5/5 depths verified)" <<<"$CTRL" >/dev/null \
    || die "control arm: no AUTHENTIC (5/5) verdict in the pristine report:\n$CTRL"
grep -E '^D4 tap-object +pass' <<<"$CTRL" >/dev/null \
    || die "control arm: the pristine D4 tap-object row is not green:\n$CTRL"
pass "control arm: pristine sig AUTHENTIC (5/5), D4 tap-object pass"

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
