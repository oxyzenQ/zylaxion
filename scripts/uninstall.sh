#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky
#
# Uninstall zylaxion: binary + config.toml + systemd user service.
# Auto-detects and removes from all known locations:
#   binary:  /usr/bin/, ~/.local/bin/
#   config:  /etc/zylaxion/, ~/.config/zylaxion/
#   service: /etc/systemd/user/, ~/.config/systemd/user/
# User config (~/.config/zylaxion/) is PRESERVED — pass --purge to remove.
# Sudo is used ONLY for system paths. Run WITHOUT sudo.

set -uo pipefail

PROJECT_NAME="zylaxion"

usage() {
    cat <<EOF
Usage: $0 [--system|--user|--all] [--purge]

  (default)  Auto-detect: scan all known locations and remove every
             ${PROJECT_NAME} artifact found. Sudo only for system paths.
  --system   Remove only system paths (/usr/bin,
             /etc/${PROJECT_NAME}, /etc/systemd/user).
  --user     Remove only user paths (~/.local/bin,
             ~/.config/systemd/user). No sudo.
  --all      Same as default.
  --purge    Also remove user config at ~/.config/${PROJECT_NAME}/.

Sudo is used only for system paths. Run WITHOUT sudo.
EOF
}

MODE="--all"
PURGE=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --system) MODE="--system"; shift ;;
        --user)   MODE="--user";   shift ;;
        --all)    MODE="--all";    shift ;;
        --purge)  PURGE=1;         shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

# Best-effort: disable systemd user unit first (only if running as non-root
# and a user service is detected).
if [[ "${MODE}" != "--system" ]] && [[ "$(id -u)" -ne 0 ]]; then
    if systemctl --user is-enabled "${PROJECT_NAME}.service" >/dev/null 2>&1; then
        echo ">> Disabling systemd user unit..."
        systemctl --user disable --now "${PROJECT_NAME}.service" 2>/dev/null || true
    fi
fi

removed=0

remove_at() {
    local target="$1"
    local need_sudo="$2"
    if [[ -e "${target}" ]]; then
        if [[ "${need_sudo}" == "yes" ]]; then
            sudo rm -rf "${target}"
        else
            rm -rf "${target}"
        fi
        echo "   removed: ${target}"
        removed=$((removed+1))
    fi
}

echo ">> Uninstalling ${PROJECT_NAME}"

case "${MODE}" in
    --system)
        remove_at "/usr/bin/${PROJECT_NAME}" yes
        remove_at "/etc/${PROJECT_NAME}" yes
        remove_at "/etc/systemd/user/${PROJECT_NAME}.service" yes
        ;;
    --user)
        remove_at "${HOME}/.local/bin/${PROJECT_NAME}" no
        remove_at "${HOME}/.config/systemd/user/${PROJECT_NAME}.service" no
        if [[ ${PURGE} -eq 1 ]]; then
            remove_at "${HOME}/.config/${PROJECT_NAME}" no
        elif [[ -f "${HOME}/.config/${PROJECT_NAME}/config.toml" ]]; then
            echo "   NOTE: user config preserved at ~/.config/${PROJECT_NAME}/config.toml"
            echo "         remove with: ./scripts/uninstall.sh --purge"
        fi
        ;;
    --all)
        # Binary
        remove_at "/usr/bin/${PROJECT_NAME}" yes
        remove_at "${HOME}/.local/bin/${PROJECT_NAME}" no
        # Config (system paths removed; user config preserved unless --purge)
        remove_at "/etc/${PROJECT_NAME}" yes
        if [[ ${PURGE} -eq 1 ]]; then
            remove_at "${HOME}/.config/${PROJECT_NAME}" no
        elif [[ -f "${HOME}/.config/${PROJECT_NAME}/config.toml" ]]; then
            echo "   NOTE: user config preserved at ~/.config/${PROJECT_NAME}/config.toml"
            echo "         remove with: ./scripts/uninstall.sh --purge"
        fi
        # Service
        remove_at "/etc/systemd/user/${PROJECT_NAME}.service" yes
        remove_at "${HOME}/.config/systemd/user/${PROJECT_NAME}.service" no
        ;;
esac

if [[ ${removed} -eq 0 ]]; then
    echo "   (nothing found to remove)"
    exit 0
fi

echo
echo ">> Done. Removed ${removed} artifact(s)."
echo "   Reload systemd to reflect changes:"
echo "     systemctl --user daemon-reload"
