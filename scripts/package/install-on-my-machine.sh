#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only - zylaxion is a Linux keyboard-sound tool; no other OS is supported.
# Copyright (C) 2026 rezky_nightky
#
# Fast one-command machine setup (night-master-2): detect the CPU's
# x86-64 microarchitecture level, build the matching release shape
# through the parity-checked cargo aliases, install the binary to
# /usr/bin/ and the boot oneshot to /etc/systemd/system/.
#
# There are no mode flags on purpose. One machine, one posture: the
# system-wide install. This is the installer (the family keeps one —
# the zejtron shape: install-on-my-machine + uninstall, no install.sh
# sibling); the portable lanes are the GitHub release tarballs and
# `cargo install zylaxion`.
#
# Build shapes, by detection result (the build-from-source law — no
# bare `cargo build --release` anywhere in the script lanes):
#   x86-64-v4 (full AVX-512 set) -> cargo pro-linux-amd64-v4-gnu
#   x86-64-v3 (AVX2/FMA/BMI set) -> cargo pro-linux-amd64-v3-gnu
#   x86-64 below v3              -> cargo pro-linux-amd64-gnu (the
#                                   portable baseline shape)
#   anything else (non-amd64)    -> refused: the shipping shapes are
#                                   the amd64 gnu aliases only
#
# The v3/v4 lanes reuse the exact aliases .cargo/config.toml defines
# (night-improve-12c) so a locally installed binary is the same
# optimization tier release.yml ships - no ad-hoc RUSTFLAGS here, the
# aliases are the contract-checked shapes.
#
# Run WITHOUT sudo: the build always runs as the current user; sudo is
# invoked only for the install steps. Running the whole script as root
# works too (container posture) - the build then runs as root, which
# leaves root-owned files in target/.

set -euo pipefail

PROJECT_NAME="zylaxion"
# Two levels up: this script lives at scripts/package/, the checkout
# root it operates on is the repo root (cargo must find Cargo.toml
# there, and the systemd unit comes from assets/).
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ATTACH_UNIT_SRC="${REPO_ROOT}/assets/${PROJECT_NAME}-attach.service"
HANDOVER_PATH_SRC="${REPO_ROOT}/assets/${PROJECT_NAME}-handover.path"
HANDOVER_SERVICE_SRC="${REPO_ROOT}/assets/${PROJECT_NAME}-handover.service"

usage() {
    cat <<EOF
Usage: $0

One command, no flags: detect the CPU level (amd64 v3/v4), build the
matching release shape, and install:
  binary    -> /usr/bin/${PROJECT_NAME}
  oneshot   -> /etc/systemd/system/${PROJECT_NAME}-attach.service
               (enabled: plants the kernel-side tap at every boot)
  hand-over -> /etc/systemd/system/${PROJECT_NAME}-handover.{path,service}
               (enabled: re-runs the attach at the first desktop login
               so the boot pins land owned by your user)

The build runs as the current user; sudo escalates only the install
steps. Presets are compiled in: pass --preset on the CLI or export
ZYLAXION_PRESET (ZYLAXION_VOLUME works the same way).
EOF
}

case "${1:-}" in
    "") ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument: $1 (this script takes no options)" >&2; usage; exit 2 ;;
esac

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "error: ${PROJECT_NAME} is a Linux keyboard-sound tool; this host is not Linux." >&2
    exit 1
fi

ARCH="$(uname -m)"

# Exact-token membership in the space-separated cpuinfo flag list.
has_flag() {
    [[ " $1 " == *" $2 "* ]]
}

# x86-64 microarchitecture level from /proc/cpuinfo:
#   v3 requires avx, avx2, bmi1, bmi2, f16c, fma, lzcnt, movbe, sse4_2
#     (AMD reports lzcnt as abm - either token counts),
#   v4 additionally requires avx512f, avx512bw, avx512cd, avx512dq, avx512vl.
# The first "flags" line is one logical CPU; the levels are uniform
# across a socket, so one line is the honest read.
detect_amd64_level() {
    local flags
    # Read the FIRST flags line. The address must be the /pattern/
    # guard (print on the first line matching ^flags, then quit): the
    # line-number address `1s/.../p` reads the FILE's line 1 — the
    # "processor : 0" line — which never matches, left `flags` empty,
    # and made every machine classify as "base" (the v3/v4 lanes
    # never fired; caught by the night-hunter-5 script audit).
    flags="$(sed -n '/^flags[[:space:]]*:/{s/^flags[[:space:]]*:[[:space:]]*//p;q;}' /proc/cpuinfo)"
    if [[ -z "${flags}" ]]; then
        echo "base"
        return
    fi
    local f
    for f in avx avx2 bmi1 bmi2 f16c fma movbe sse4_2; do
        has_flag "${flags}" "${f}" || { echo "base"; return; }
    done
    if ! has_flag "${flags}" lzcnt && ! has_flag "${flags}" abm; then
        echo "base"
        return
    fi
    for f in avx512f avx512bw avx512cd avx512dq avx512vl; do
        has_flag "${flags}" "${f}" || { echo "v3"; return; }
    done
    echo "v4"
}

LEVEL="$(detect_amd64_level)"

if [[ $EUID -eq 0 ]]; then
    SUDO=""
else
    SUDO="sudo"
fi

cd "${REPO_ROOT}"

if [[ ! -f Cargo.toml ]]; then
    echo "error: Cargo.toml not found. Run this script from the repo root." >&2
    exit 1
fi
if [[ ! -f "${ATTACH_UNIT_SRC}" ]]; then
    echo "error: required file not found: ${ATTACH_UNIT_SRC}" >&2
    exit 1
fi
if [[ ! -f "${HANDOVER_PATH_SRC}" || ! -f "${HANDOVER_SERVICE_SRC}" ]]; then
    echo "error: required files not found: ${HANDOVER_PATH_SRC} / ${HANDOVER_SERVICE_SRC}" >&2
    exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found on PATH." >&2
    echo "  install the Rust toolchain first:  curl https://sh.rustup.rs -sSf | sh" >&2
    exit 1
fi

echo ">> [1/4] CPU: ${ARCH}, x86-64 level: ${LEVEL}"
BIN=""
case "${ARCH}:${LEVEL}" in
    x86_64:v4)
        echo ">> [2/4] Building the x86-64-v4 release shape (cargo pro-linux-amd64-v4-gnu)"
        cargo pro-linux-amd64-v4-gnu --locked
        BIN="target/x86_64-unknown-linux-gnu/pro-linux-amd64-v4-gnu/${PROJECT_NAME}"
        ;;
    x86_64:v3)
        echo ">> [2/4] Building the x86-64-v3 release shape (cargo pro-linux-amd64-v3-gnu)"
        cargo pro-linux-amd64-v3-gnu --locked
        BIN="target/x86_64-unknown-linux-gnu/pro-linux-amd64-v3-gnu/${PROJECT_NAME}"
        ;;
    x86_64:base)
        echo ">> [2/4] Building the baseline release shape (cargo pro-linux-amd64-gnu — no v3/v4 flags detected)"
        cargo pro-linux-amd64-gnu --locked
        BIN="target/x86_64-unknown-linux-gnu/release/${PROJECT_NAME}"
        ;;
    *)
        echo "error: no parity-checked alias exists for ${ARCH} — zylaxion ships the amd64 gnu shapes only." >&2
        echo "  build the host dev tier manually if you must: cargo build --release --locked" >&2
        exit 1
        ;;
esac

if [[ ! -f "${BIN}" ]]; then
    echo "error: build produced no binary at ${BIN}" >&2
    exit 1
fi

echo ">> [3/4] Installing the binary to /usr/bin/"
# Clean any stale user-local binary first so nothing shadows /usr/bin
# (the pre-night-master-2 default path was ~/.local/bin).
user_bin="${HOME}/.local/bin/${PROJECT_NAME}"
if [[ -f "${user_bin}" ]]; then
    rm -f "${user_bin}"
    echo "   cleaned stale user-local binary: ${user_bin}"
fi
"${SUDO}" install -Dm755 "${BIN}" "/usr/bin/${PROJECT_NAME}"
echo "   installed: /usr/bin/${PROJECT_NAME}"

echo ">> [4/4] Installing the boot oneshot + the login hand-over to /etc/systemd/system/"
# The system oneshot plants the kernel-side tap at every boot (bpffs
# pins do not survive a reboot). A oneshot, not a daemon: it runs
# `zylaxion attach`, exits, and stays dead.
"${SUDO}" install -Dm644 "${ATTACH_UNIT_SRC}" \
    "/etc/systemd/system/${PROJECT_NAME}-attach.service"
# The login hand-over (night-dinner-19): the boot oneshot runs before
# anyone logs in, so its pins are root-owned; the path unit watches
# /run/user and re-runs `zylaxion attach` at the first desktop login,
# handing the pins to that user and relaxing the bpffs front door.
"${SUDO}" install -Dm644 "${HANDOVER_SERVICE_SRC}" \
    "/etc/systemd/system/${PROJECT_NAME}-handover.service"
"${SUDO}" install -Dm644 "${HANDOVER_PATH_SRC}" \
    "/etc/systemd/system/${PROJECT_NAME}-handover.path"
if command -v systemctl >/dev/null 2>&1; then
    "${SUDO}" systemctl daemon-reload
    if "${SUDO}" systemctl enable "${PROJECT_NAME}-attach" >/dev/null 2>&1; then
        echo "   enabled: ${PROJECT_NAME}-attach (plants the tap at every boot)"
    else
        echo "   (enable manually: sudo systemctl enable ${PROJECT_NAME}-attach)"
    fi
    # --now: the watch fires immediately on a desktop host (a
    # /run/user entry already exists), so the just-planted or
    # already-planted pins land owned by this user without a reboot.
    if "${SUDO}" systemctl enable --now "${PROJECT_NAME}-handover.path" >/dev/null 2>&1; then
        echo "   enabled: ${PROJECT_NAME}-handover.path (hands the pins over at the first login)"
    else
        echo "   (enable manually: sudo systemctl enable --now ${PROJECT_NAME}-handover.path)"
    fi
else
    echo "   (systemd not present; the unit files are installed but not enabled)"
fi

echo ">> Done."
echo
echo "Next steps:"
echo "  - Plant the tap now (the oneshot is enabled for future boots):"
echo "      sudo systemctl start ${PROJECT_NAME}-attach"
echo "  - Then, as your user:"
echo "      ${PROJECT_NAME} on"
echo "  - Pick a preset (24 compiled in):"
echo "      ${PROJECT_NAME} list-presets"
echo "      ${PROJECT_NAME} preset --switch ibm_model_m   # live switch"
echo "  - Turn it off:"
echo "      ${PROJECT_NAME} off"
echo "  - Uninstall:"
echo "      sudo ./scripts/package/uninstall.sh"
