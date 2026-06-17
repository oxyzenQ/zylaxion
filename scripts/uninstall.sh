#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Zylaxion uninstaller — removes the installed binary and config.toml.
#
# Usage: sudo ./scripts/uninstall.sh
#
# Environment variables:
#   PREFIX   Installation prefix (default: /usr/local)
#   DESTDIR  Staging root (must match the value used during install)

set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
DESTDIR="${DESTDIR:-}"

BIN_DST="${DESTDIR}${PREFIX}/bin/zylaxion"
SHARE_DST="${DESTDIR}${PREFIX}/share/zylaxion"

echo "==> Uninstalling Zylaxion..."

if [ -f "$BIN_DST" ]; then
    rm -f "$BIN_DST"
    echo "    removed ${BIN_DST}"
else
    echo "    ${BIN_DST} not found (already removed?)"
fi

if [ -d "$SHARE_DST" ]; then
    rm -rf "$SHARE_DST"
    echo "    removed ${SHARE_DST}/"
else
    echo "    ${SHARE_DST} not found (already removed?)"
fi

echo ""
echo "==> Zylaxion uninstalled."
echo "    Note: user config in ~/.config/zylaxion/ was NOT removed."
