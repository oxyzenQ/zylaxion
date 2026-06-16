#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Zylaxion installer — installs a pre-built release binary and profile
# TOMLs to the system.  Does NOT build the project; run
# `cargo build --release --locked` first.
#
# Follows FHS: binary to $PREFIX/bin, profiles to $PREFIX/share.
#
# Usage: sudo ./scripts/install.sh
#
# Environment variables:
#   PREFIX   Installation prefix (default: /usr/local)
#   DESTDIR  Staging root for packaging (default: unset)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="${SCRIPT_DIR}/.."
PROFILE_DIR="${WORKSPACE_ROOT}/profiles"

PREFIX="${PREFIX:-/usr/local}"
DESTDIR="${DESTDIR:-}"

BIN_SRC="${WORKSPACE_ROOT}/target/release/zylaxion"
BIN_DST="${DESTDIR}${PREFIX}/bin/zylaxion"
SHARE_DST="${DESTDIR}${PREFIX}/share/zylaxion/profiles"

TARGET_USER="${SUDO_USER:-$USER}"

# ── Pre-flight ─────────────────────────────────────────────────────

if [ ! -f "$BIN_SRC" ]; then
    echo "Error: release binary not found at ${BIN_SRC}"
    echo ""
    echo "Build it first:"
    echo "  cargo build --release --locked"
    exit 1
fi

if [ ! -d "$PROFILE_DIR" ] || [ -z "$(shopt -s nullglob; echo "$PROFILE_DIR"/*.toml)" ]; then
    echo "Error: no profile TOMLs found in ${PROFILE_DIR}/"
    exit 1
fi

# ── Install ─────────────────────────────────────────────────────────

echo "==> Installing binary to ${BIN_DST}"
install -Dm755 "$BIN_SRC" "$BIN_DST"

echo "==> Installing profile TOMLs to ${SHARE_DST}/"
install -dm755 "$SHARE_DST"
for toml in "$PROFILE_DIR"/*.toml; do
    install -m0644 "$toml" "$SHARE_DST/"
    echo "    $(basename "$toml")"
done

# ── Post-install ─────────────────────────────────────────────────────

echo ""
echo "==> Zylaxion installed to ${PREFIX}"
echo ""

if id -nG "$TARGET_USER" 2>/dev/null | tr ' ' '\n' | grep -qx "input"; then
    echo "    OK  User '${TARGET_USER}' is in the 'input' group."
else
    echo "    WARNING  User '${TARGET_USER}' is NOT in the 'input' group."
    echo "    Zylaxion needs raw keyboard access via evdev."
    echo "    Run:  sudo usermod -aG input ${TARGET_USER}"
    echo "    Then log out and back in."
fi

echo ""
echo "    Usage:"
echo "      zylaxion start --profile technical   (foreground)"
echo "      zylaxion daemon --profile classic    (background)"
echo "      zylaxion doctor                     (system check)"
echo ""
echo "    Uninstall:  sudo ./scripts/uninstall.sh"
