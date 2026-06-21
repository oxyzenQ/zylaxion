#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# zylaxion installer - installs a pre-built release binary and the
# central config.toml for the current user. Does NOT build the project; run
# `cargo build --release --locked` first.
#
# Binary goes to $PREFIX/bin, config to $PREFIX/share.
#
# Usage: ./scripts/install.sh
#
# Environment variables:
#   PREFIX   Installation prefix (default: ~/.local)
#   DESTDIR  Staging root for packaging (default: unset)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="${SCRIPT_DIR}/.."
CONFIG_SRC="${WORKSPACE_ROOT}/config.toml"
SERVICE_SRC="${WORKSPACE_ROOT}/assets/zylaxion.service"

PREFIX="${PREFIX:-${HOME}/.local}"
DESTDIR="${DESTDIR:-}"

BIN_SRC="${WORKSPACE_ROOT}/target/release/zylaxion"
BIN_DST="${DESTDIR}${PREFIX}/bin/zylaxion"
CONFIG_DST="${DESTDIR}${PREFIX}/share/zylaxion/config.toml"

TARGET_USER="${USER}"

# ── Pre-flight ─────────────────────────────────────────────────────

if [ ! -f "$BIN_SRC" ]; then
    echo "Error: release binary not found at ${BIN_SRC}"
    echo ""
    echo "Build it first:"
    echo "  cargo build --release --locked"
    exit 1
fi

if [ ! -f "$CONFIG_SRC" ]; then
    echo "Error: config.toml not found at ${CONFIG_SRC}"
    exit 1
fi

if [ ! -f "$SERVICE_SRC" ]; then
    echo "Error: systemd unit not found at ${SERVICE_SRC}"
    exit 1
fi

# ── Install ─────────────────────────────────────────────────────────

echo "==> Installing binary to ${BIN_DST}"
install -Dm755 "$BIN_SRC" "$BIN_DST"

echo "==> Installing config.toml to ${CONFIG_DST}"
install -Dm0644 "$CONFIG_SRC" "$CONFIG_DST"

# ── systemd user unit ───────────────────────────────────────────────
SERVICE_DST="${DESTDIR}${HOME}/.config/systemd/user/zylaxion.service"

echo "==> Installing systemd user unit to ${SERVICE_DST}"
install -Dm0644 "$SERVICE_SRC" "$SERVICE_DST"

# ── Post-install ─────────────────────────────────────────────────────

echo ""
echo "==> Zylaxion installed to ${PREFIX}"
echo ""

if id -nG "$TARGET_USER" 2>/dev/null | tr ' ' '\n' | grep -qx "input"; then
    echo "    OK  User '${TARGET_USER}' is in the 'input' group."
else
    echo "    WARNING  User '${TARGET_USER}' is NOT in the 'input' group."
    echo "    Zylaxion needs raw keyboard access via evdev."
    echo "    Add '${TARGET_USER}' to the 'input' group, then log out and back in."
fi

echo ""
echo "    Usage:"
echo "      zylaxion start                          (foreground)"
echo "      zylaxion daemon                         (background)"
echo "      zylaxion testconf                       (validate config)"
echo "      zylaxion doctor                         (system check)"
echo ""
echo "    Config: ${PREFIX}/share/zylaxion/config.toml"
echo "            (copy to ~/.config/zylaxion/config.toml for user overrides)"
echo "            Edit and save — daemon auto-reloads within 1 second."
echo ""
echo "    systemd (auto-start on login):"
echo "      systemctl --user daemon-reload"
echo "      systemctl --user enable --now zylaxion"
echo "      journalctl --user -u zylaxion -f        (live logs)"
echo ""
echo "    Uninstall:  ./scripts/uninstall.sh"
