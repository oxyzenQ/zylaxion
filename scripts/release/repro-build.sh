#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# repro-build.sh — night-depth-hardening-Z1, the D4 reproducible release
# chain. D3 (verify-reproducible-build.sh) proves ARTIFACT identity: the
# same host, twice, stamps + cmp. This script is the PROCESS half: ONE
# deterministic recipe that any independent host can follow, the exact
# recipe the CI two-host lane (reproducible.yml) runs on both sides
# before the verify job `cmp`s the two binaries byte-for-byte. The
# published hash must be the hash anyone can rebuild from source —
# the "modified build without a trace" class retires here.
#
# What is pinned on every host that runs this recipe:
#   - toolchain: rust-toolchain.toml (1.98.1, the repo's single version
#     source) + the x86_64-unknown-linux-musl target (self-contained
#     rust-lld linking, no host musl-gcc needed for the RUST side);
#   - epoch: SOURCE_DATE_EPOCH = the HEAD commit timestamp (git log -1
#     %ct), the reproducible-builds standard, the same pin D3 uses;
#   - flags: -C target-cpu=x86-64-v3 + +crt-static + redacted path
#     prefixes (${HOME}/.cargo and ${PWD} never enter the bytes);
#   - profile: pro-linux-amd64-v3-musl (static musl, release tier,
#     CGU 1, thin LTO), built through the parity-checked alias — the
#     from-source law, never a bare `cargo build --release`;
#   - deps: --locked (Cargo.lock), no shared build cache in CI;
#   - the one C dependency the musl shape carries: alsa-lib, compiled
#     from the sha256-pinned source tarball below with musl-gcc into a
#     private prefix. night-dragon-maintenance-T1 retired the musl
#     RELEASE lane because a cross-build from a glibc host welds the
#     HOST's glibc libasound onto a musl CRT — a binary that links but
#     cannot run. This lane avoids that trap BY CONSTRUCTION:
#     PKG_CONFIG_LIBDIR is pointed ONLY at the private prefix, so the
#     host's alsa.pc (glibc) is unreachable and only the musl-built
#     libasound.a can enter the link. The pinned pkg-config verdict is
#     asserted before the build.
#
# Honesty notes (recorded, not hidden): the static musl v3 shape is the
# process-identity INSTRUMENT — it is not a shipped release shape. The
# release contract stays glibc-only (night-dragon-maintenance-T1), and
# the static binary's ALSA runtime is partial by nature (libasound
# dlopens its PCM plugin tree at runtime); `-V` and the stamp surfaces
# run fine on any host, which is what the identity lane needs. Zero new
# release surface ships with this lane.
#
# The alsa prefix is cached OUTSIDE the work tree
# (ZYLAXION_REPRO_MUSL_DIR, default ~/.cache/zylaxion-repro-musl) keyed
# by the sha256 pin — a re-pin needs a documented cause (the tree's
# supply-chain law, the zactrix-kernel-matrix shape).
#
# Usage:
#   scripts/release/repro-build.sh                  # one build (the CI lane)
#   scripts/release/repro-build.sh --cold-two-pass  # same host, two cold builds + cmp
#
# Exit code: 0 when the requested proof holds (or the single build
# completed), 1 on any pinned-input mismatch, refusal, or build failure.

set -euo pipefail
cd "$(dirname "$0")/../.."

BIN_REL="target/x86_64-unknown-linux-musl/pro-linux-amd64-v3-musl/zylaxion"
PROFILE_DIR="target/x86_64-unknown-linux-musl/pro-linux-amd64-v3-musl"

# The tree must present ONE state to the build: a dirty work tree stamps
# -dirty (or any local edit) into the bytes, so the run measures noise
# instead of the process. The refusal names the fix, like D3's.
if [ -n "$(git status --porcelain)" ]; then
    echo "REFUSED: the work tree is dirty - commit or stash first (one tree, one stamp)"
    exit 1
fi

# ── tools, each failure named with its fix ────────────────────────────
for tool in cargo rustup musl-gcc pkg-config tar bzip2 curl sha256sum make cmp; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "REPRO FAIL: missing tool '$tool' (CI installs musl-tools + pkg-config; locally: the musl toolchain on PATH)"
        exit 1
    }
done
if ! rustup target list --installed 2>/dev/null | grep -qx 'x86_64-unknown-linux-musl'; then
    echo "REPRO FAIL: the pinned lane builds x86_64-unknown-linux-musl, which is not installed"
    echo "  Fix: rustup target add x86_64-unknown-linux-musl"
    exit 1
fi
if ! command -v bpf-linker >/dev/null 2>&1 && [ ! -x "${HOME}/.local/bin/bpf-linker" ]; then
    echo "REPRO FAIL: bpf-linker not on PATH - the source-built eBPF lane (a git checkout) needs it"
    echo "  Fix: ./scripts/dev/bootstrap-ebpf.sh"
    exit 1
fi

# ── the epoch pin (the commit's own timestamp, stable on every host) ──
EPOCH="$(git log -1 --format=%ct HEAD)"
export SOURCE_DATE_EPOCH="$EPOCH"
export CARGO_INCREMENTAL=0
echo "repro epoch: SOURCE_DATE_EPOCH=$EPOCH (HEAD commit time, the reproducible-builds pin)"

# ── the sha256-pinned alsa source, compiled with musl-gcc ─────────────
# url | sha256 | archive name | version
# The pin table is the ONLY legal way the source changes (the tree's
# supply-chain law). Ubuntu's pool serves the orig tarball; the source
# is the upstream alsa-lib 1.2.13 release.
ALSAMIX_URL="https://archive.ubuntu.com/ubuntu/pool/main/a/alsa-lib/alsa-lib_1.2.13.orig.tar.bz2"
ALSAMIX_SHA="8c4ff37553cbe89618e187e4c779f71a9bb2a8b27b91f87ed40987cc9233d8f6"
ALSAMIX_NAME="alsa-lib-1.2.13"
MUSL_ROOT="${ZYLAXION_REPRO_MUSL_DIR:-$HOME/.cache/zylaxion-repro-musl}"
mkdir -p "$MUSL_ROOT"

if [ -f "$MUSL_ROOT/prefix/lib/libasound.a" ] \
        && [ -f "$MUSL_ROOT/pin-sha256.txt" ] \
        && [ "$(cat "$MUSL_ROOT/pin-sha256.txt")" = "$ALSAMIX_SHA" ]; then
    echo "alsa prefix: cached (pin $ALSAMIX_SHA verified at build time)"
else
    echo "alsa prefix: building the musl-native libasound from the pinned source"
    ARCHIVE="$MUSL_ROOT/$ALSAMIX_NAME.tar.bz2"
    curl -fsSL --retry 5 --retry-all-errors -o "$ARCHIVE" "$ALSAMIX_URL"
    echo "$ALSAMIX_SHA  $ARCHIVE" | sha256sum -c -
    WORK="$MUSL_ROOT/work"
    rm -rf "$WORK"
    mkdir -p "$WORK"
    tar -xjf "$ARCHIVE" -C "$WORK"
    # Deterministic CFLAGS: -g0 drops debug paths, -fno-ident drops the
    # compiler's .comment identity, and SOURCE_DATE_EPOCH (exported
    # above) pins __DATE__/__TIME__ if any TU used them.
    # The kernel UAPI include window (the header edition of the
    # weld-trap law): musl-gcc's -nostdinc excludes /usr/include, and
    # alsa's sources need <linux/types.h> and <asm/types.h>. Putting
    # /usr/include itself on the search path is FORBIDDEN here — its
    # glibc headers shadow musl's (the CI run named it:
    # /usr/include/stdio.h winning over musl's, __gnuc_va_list chaos).
    # The window carries ONLY the libc-agnostic kernel UAPI trees, so
    # musl's headers win every name and no glibc object can enter.
    UAPI="$MUSL_ROOT/uapi"
    mkdir -p "$UAPI"
    ln -sfn /usr/include/linux "$UAPI/linux"
    ASM_SRC="/usr/include/x86_64-linux-gnu/asm"
    [ -d "$ASM_SRC" ] || ASM_SRC="/usr/include/asm"
    ln -sfn "$ASM_SRC" "$UAPI/asm"
    ln -sfn /usr/include/asm-generic "$UAPI/asm-generic"
    (
        cd "$WORK/$ALSAMIX_NAME"
        # --host is the cross mode the musl lane NEEDS (musl-gcc cannot
        # run configure's test programs on a glibc build host).
        CC=musl-gcc \
            CFLAGS="-O2 -g0 -fno-ident -I$UAPI" \
            ./configure --host=x86_64-unknown-linux-musl \
            --prefix="$MUSL_ROOT/prefix" \
            --disable-shared --enable-static --disable-python >/dev/null
        make -j"$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)" >/dev/null
        make install >/dev/null
    )
    printf '%s\n' "$ALSAMIX_SHA" > "$MUSL_ROOT/pin-sha256.txt"
    echo "alsa prefix: built and pinned ($ALSAMIX_SHA)"
fi

# The weld-trap proof, asserted (not assumed): the pkg-config verdict
# must resolve inside the private prefix ONLY - the host's glibc
# alsa.pc is unreachable with PKG_CONFIG_LIBDIR scoped like this.
export PKG_CONFIG_PATH="$MUSL_ROOT/prefix/lib/pkgconfig"
export PKG_CONFIG_LIBDIR="$MUSL_ROOT/prefix/lib/pkgconfig"
ALSALIBS="$(pkg-config --static --libs alsa)"
case "$ALSALIBS" in
    *-lasound*) echo "alsa link verdict: $ALSALIBS (private musl prefix only)" ;;
    *) echo "REPRO FAIL: pkg-config does not resolve alsa inside the private prefix: '$ALSALIBS'"; exit 1 ;;
esac

# ── the pinned flag composition (redacted paths, static musl, v3) ─────
export RUSTFLAGS="-C target-cpu=x86-64-v3 -C target-feature=+crt-static --remap-path-prefix=${HOME}/.cargo=redacted --remap-path-prefix=${PWD}=redacted"
echo "repro flags: $RUSTFLAGS"

build_once() {
    local label="$1" dest="$2"
    echo "== repro build: $label =="
    cargo pro-linux-amd64-v3-musl --locked
    mkdir -p "$(dirname "$dest")"
    cp "$BIN_REL" "$dest"
}

if [ "${1:-}" = "--cold-two-pass" ]; then
    # The local process proof, same host, two COLD builds: the profile's
    # build directory is wiped between passes so the whole graph
    # recompiles from the lockfile (the cache-free CI shape). The
    # stronger proof - two truly independent hosts - lives in CI
    # (reproducible.yml); this verdict is the local half.
    STAGE="$(mktemp -d)"
    trap 'rm -rf "$STAGE"' EXIT
    build_once "pass 1 of 2 (cold)" "$STAGE/pass-a/zylaxion"
    rm -rf "$PROFILE_DIR"
    build_once "pass 2 of 2 (cold)" "$STAGE/pass-b/zylaxion"
    sha256sum "$STAGE/pass-a/zylaxion" "$STAGE/pass-b/zylaxion"
    if cmp -s "$STAGE/pass-a/zylaxion" "$STAGE/pass-b/zylaxion"; then
        echo "REPRODUCIBLE RELEASE CHAIN (local, same host, musl v3): PASS - two cold builds are byte-identical"
    else
        echo "REPRODUCIBLE RELEASE CHAIN (local, same host, musl v3): FAIL - the two builds differ"
        cmp -l "$STAGE/pass-a/zylaxion" "$STAGE/pass-b/zylaxion" | head -20 || true
        exit 1
    fi
    exit 0
fi

# The single-build shape the CI two-host lane runs (one pass per host);
# the artifact stages FLAT in the workspace root so the verify job's
# paths stay trivial. The sha verdict is printed, never swallowed.
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
build_once "single pass (the CI host shape)" "$STAGE/zylaxion-throwaway"
cp "$BIN_REL" reproducible-zylaxion
chmod 755 reproducible-zylaxion
sha256sum reproducible-zylaxion
echo "REPRODUCIBLE RELEASE CHAIN: this host's half is done - the CI verify job owns the cmp"
