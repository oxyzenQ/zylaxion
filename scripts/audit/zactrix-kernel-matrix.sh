#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-kernel-matrix.sh — night-frontier-2B, the kernel matrix
# proof. The README's "Linux 5.15+" stops being a claim: the tap's
# kernel contract is verified against REAL BTF snapshots of the three
# LTS lines, extracted from pinned distro kernel images:
#
#   5.15 — Ubuntu jammy, linux-image-unsigned-5.15.0-97-generic
#   6.1  — Debian bookworm, linux-image-6.1.0-50-amd64-unsigned
#   6.6  — Arch Linux LTS, linux-lts 6.6.72-1
#
# Every artifact rides a sha256 pin (the tree's supply-chain law);
# the pin table lives below and is the ONLY legal way the artifacts
# change. The chain per kernel: download -> sha256 verify -> extract
# the compressed kernel image (dpkg-deb / tar --zstd) -> decompress
# to the vmlinux ELF (the extract-vmlinux magic scan) -> carve the
# embedded .BTF section (objcopy) -> run the harness's --btf-matrix
# mode (the aya-obj loader path: parse, the CO-RE posture, relocations,
# the fentry target lookup). Map-fd relocation stays a live-kernel
# fact — the harness records the map inventory instead, and the attach
# chain owns the fd verdict.
#
# Artifacts cache under ZYLAXION_KERNEL_MATRIX_DIR (default:
# ~/.cache/zylaxion-kernel-matrix — outside the work tree, so the
# extracted distro trees never enter any tree-wide gate's scan) keyed
# by the sha256 pins — CI caches that directory, so a warm run skips
# the downloads.
#
# Exit code: 0 when every snapshot passes, 1 otherwise.

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT="${ZYLAXION_KERNEL_MATRIX_DIR:-$HOME/.cache/zylaxion-kernel-matrix}"
mkdir -p "$OUT"

# url | sha256 | artifact name | vmlinuz member path | label
PINS=(
    "https://archive.ubuntu.com/ubuntu/pool/main/l/linux/linux-image-unsigned-5.15.0-97-generic_5.15.0-97.107_amd64.deb|0f9a0d59436e6227c4b604708d6928ccad25020b89aaca1bc289dc558f12795b|jammy-5.15.0-97.deb|boot/vmlinuz-5.15.0-97-generic|5.15.0-97-generic"
    "https://deb.debian.org/debian/pool/main/l/linux/linux-image-6.1.0-50-amd64-unsigned_6.1.176-1_amd64.deb|439422e41d2dbb840b60b81e3bf5e5955bfa56b7a8c268aec83c9ff882d421e6|bookworm-6.1.0-50-amd64.deb|boot/vmlinuz-6.1.0-50-amd64|6.1.0-50-amd64"
    "https://archive.archlinux.org/packages/l/linux-lts/linux-lts-6.6.72-1-x86_64.pkg.tar.zst|a527cb8907781c2110d6b1bebb3340689c46a73ff4ad7de1a387946c44907a14|arch-6.6.72-1-lts.pkg.tar.zst|usr/lib/modules/6.6.72-1-lts/vmlinuz|6.6.72-1-lts"
)

# One-time tools check, named (the run must fail with the fix, not a
# mystery code from a missing binary).
for tool in curl sha256sum dpkg-deb tar zstd objcopy; do
    command -v "$tool" >/dev/null || { echo "KERNEL-MATRIX FAIL: missing tool '$tool'"; exit 1; }
done

extract_vmlinux() {
    # $1 = vmlinuz, $2 = out. The kernel's own extract-vmlinux shape:
    # scan for each compression magic, decompress everything after the
    # first hit, keep the result only when it is a vmlinux ELF.
    local image="$1" out="$2" magic pos tmp
    tmp="$(mktemp)"
    for magic in $'\x28\xb5\x2f\xfd' $'\xfd\x37\x7a\x58\x5a\x00' $'\x1f\x8b\x08'; do
        pos="$(grep -aboF -- "$magic" "$image" 2>/dev/null | head -1 | cut -d: -f1 || true)"
        [ -n "$pos" ] || continue
        # The decompressor's exit code is deliberately ignored: the
        # kernel's own extract-vmlinux feeds the WHOLE tail, so zstd -d
        # legitimately exits non-zero on the trailing garbage after the
        # frame while having already written the full vmlinux — the
        # output is the verdict (the ELF magic), not the exit code.
        if [ "${magic:0:3}" = $'\x28\xb5\x2f' ]; then
            tail -c +"$((pos + 1))" "$image" | zstd -d > "$tmp" 2>/dev/null || true
        elif [ "${magic:0:3}" = $'\xfd\x37\x7a' ]; then
            tail -c +"$((pos + 1))" "$image" | xz -d > "$tmp" 2>/dev/null || true
        else
            tail -c +"$((pos + 1))" "$image" | gzip -d > "$tmp" 2>/dev/null || true
        fi
        if [ "$(head -c 4 "$tmp" | od -An -tx1 | tr -d ' \n')" = "7f454c46" ]; then
            mv "$tmp" "$out"
            return 0
        fi
    done
    rm -f "$tmp"
    return 1
}

PASS=0
FAIL=0
for pin in "${PINS[@]}"; do
    IFS='|' read -r url sha256 artifact member label <<< "$pin"
    image="$OUT/$artifact"
    btf="$OUT/btf-$label.bin"
    if [ ! -f "$image" ]; then
        echo "KERNEL-MATRIX [$label] downloading $(basename "$url")"
        curl -fsSL --retry 5 --retry-all-errors -o "$image" "$url"
    fi
    echo "$sha256  $image" | sha256sum -c - || {
        echo "KERNEL-MATRIX FAIL [$label]: sha256 pin mismatch - the artifact moved; a re-pin needs a documented cause"
        FAIL=$((FAIL + 1))
        continue
    }
    if [ ! -f "$btf" ]; then
        root="$OUT/root-$label"
        rm -rf "$root"
        mkdir -p "$root"
        case "$artifact" in
            *.deb) dpkg-deb -x "$image" "$root" ;;
            *.pkg.tar.zst) tar --zstd -xf "$image" -C "$root" "$member" ;;
            *) echo "KERNEL-MATRIX FAIL [$label]: unknown artifact shape"; FAIL=$((FAIL + 1)); continue ;;
        esac
        vmlinux="$OUT/vmlinux-$label"
        if ! extract_vmlinux "$root/$member" "$vmlinux"; then
            echo "KERNEL-MATRIX FAIL [$label]: no compression magic produced a vmlinux ELF"
            FAIL=$((FAIL + 1))
            continue
        fi
        objcopy -O binary --only-section=.BTF "$vmlinux" "$btf"
        [ -s "$btf" ] || {
            echo "KERNEL-MATRIX FAIL [$label]: the kernel image carries no embedded .BTF (CONFIG_DEBUG_INFO_BTF unset?)"
            FAIL=$((FAIL + 1))
            continue
        }
    fi
    # No grep -q in the pipeline: an early-exiting consumer SIGPIPEs the
    # tee and swallows the per-snapshot verdict lines before the battery
    # leg can read them — the full stream passes through, the verdict
    # comes from the harness's own rc plus its log file.
    set +e
    cargo run --release -q --manifest-path scripts/audit/harness/Cargo.toml -- \
        --btf-matrix "$btf" --btf-label "$label" 2>&1 | tee "$OUT/matrix-$label.log"
    harness_rc=$?
    set -e
    if [ "$harness_rc" -eq 0 ] && grep -q "KERNEL-MATRIX PASS \[" "$OUT/matrix-$label.log"; then
        PASS=$((PASS + 1))
    else
        echo "KERNEL-MATRIX FAIL [$label]: harness rc=$harness_rc - see $OUT/matrix-$label.log"
        FAIL=$((FAIL + 1))
    fi
done

echo "KERNEL-MATRIX verdict: $PASS passed, $FAIL failed (the LTS floor lines 5.15 / 6.1 / 6.6, real BTF snapshots)"
[ "$FAIL" -eq 0 ] || exit 1
echo "KERNEL-MATRIX PASS: the tap's kernel contract resolves on every pinned LTS BTF snapshot"
