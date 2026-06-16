#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Zylaxion uninstaller — removes the installed binary and profile TOMLs.
#
# Usage: sudo ./scripts/uninstall.sh

set -euo pipefail

INSTALL_BIN="/usr/local/bin/zylaxion"
INSTALL_DIR="/etc/zylaxion"

echo "==> Uninstalling Zylaxion..."

if [ -f "$INSTALL_BIN" ]; then
    rm -f "$INSTALL_BIN"
    echo "    removed ${INSTALL_BIN}"
else
    echo "    ${INSTALL_BIN} not found (already removed?)"
fi

if [ -d "$INSTALL_DIR" ]; then
    rm -rf "$INSTALL_DIR"
    echo "    removed ${INSTALL_DIR}/"
else
    echo "    ${INSTALL_DIR} not found (already removed?)"
fi

echo ""
echo "==> Zylaxion uninstalled."
echo "    Note: user config in ~/.config/zylaxion/ was NOT removed."
