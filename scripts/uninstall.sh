#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# zylaxion uninstaller - removes the installed binary, config.toml, and
# systemd user unit.
#
# Usage: ./scripts/uninstall.sh
#
# Environment variables:
#   PREFIX   Installation prefix (default: ~/.local)
#   DESTDIR  Staging root (must match the value used during install)

set -euo pipefail

PREFIX="${PREFIX:-${HOME}/.local}"
DESTDIR="${DESTDIR:-}"

BIN_DST="${DESTDIR}${PREFIX}/bin/zylaxion"
SHARE_DST="${DESTDIR}${PREFIX}/share/zylaxion"
TARGET_USER="${USER}"

echo "==> Uninstalling Zylaxion..."

# ── Disable systemd user unit first (best-effort) ──────────────────
# This is best-effort because the user service may not be active.

if [ -n "${TARGET_USER}" ] && [ "$(id -u)" -ne 0 ]; then
    # Running as the target user — attempt disable.
    if systemctl --user is-enabled zylaxion.service >/dev/null 2>&1; then
        echo "    disabling systemd user unit..."
        systemctl --user disable --now zylaxion.service 2>/dev/null || true
    fi
fi

# ── Remove binary + share ──────────────────────────────────────────

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

# ── Remove systemd user unit ───────────────────────────────────────
# Check both locations: system-wide (/etc/systemd/user) and per-user
# (~/.config/systemd/user). DESTDIR is respected for packaging builds.

USER_UNIT="${DESTDIR}${HOME}/.config/systemd/user/zylaxion.service"

for unit in "$USER_UNIT"; do
    if [ -f "$unit" ]; then
        rm -f "$unit"
        echo "    removed ${unit}"
    fi
done

echo ""
echo "==> Zylaxion uninstalled."
echo "    Note: user config in ~/.config/zylaxion/ was NOT removed."
echo ""
echo "    If zylaxion was enabled as a systemd user service, run:"
echo "      systemctl --user disable zylaxion.service"
echo "      systemctl --user daemon-reload"
