#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only - zylaxion is a Linux keyboard-sound tool; no other OS is supported.
# Copyright (C) 2026 rezky_nightky
#
# Uninstall zylaxion: binary + the tap's boot oneshot (night-improve-23:
# there is no daemon user service anymore) + a kernel-side sweep.
# Auto-detects and removes from all known locations:
#   binary:  /usr/bin/, ~/.local/bin/
#   oneshot: /etc/systemd/system/zylaxion-attach.service
#   kernel:  the pinned tap (sudo zylaxion detach)
#   legacy:  /etc/zylaxion/, ~/.config/zylaxion/ (config-era leftovers;
#            night-improve-22 retired the config file — the binary no
#            longer reads either directory, uninstall still cleans them)
# Legacy user dir (~/.config/zylaxion/) is PRESERVED — pass --purge to remove.
# Sudo is used ONLY for system paths. Run WITHOUT sudo.
#
# night-master-2: the install posture is SYSTEM-ONLY now (binary ->
# /usr/bin, oneshot -> /etc/systemd/system). The user-path removals
# below are LEGACY CLEANUP for installs made by the pre-night-master-2
# dual-mode installer; no current installer produces them.

set -uo pipefail

PROJECT_NAME="zylaxion"

usage() {
    cat <<EOF
Usage: $0 [--system|--user|--all] [--purge]

  (default)  Auto-detect: scan all known locations and remove every
             ${PROJECT_NAME} artifact found. Sudo only for system paths.
  --system   Remove only system paths (/usr/bin,
             /etc/${PROJECT_NAME}, /etc/systemd/system).
  --user     Remove only user paths (~/.local/bin,
             ~/.config/systemd/user). No sudo. Legacy cleanup only -
             the current install posture is system-only (night-master-2).
  --all      Same as default.
  --purge    Also remove the legacy user dir at ~/.config/${PROJECT_NAME}/.

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
    if systemctl is-enabled "${PROJECT_NAME}-attach" >/dev/null 2>&1; then
        echo ">> Disabling systemd user unit..."
        systemctl disable --now "${PROJECT_NAME}-attach" 2>/dev/null || true
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

# Kernel-side sweep FIRST (the binary must still exist to run it): the
# pinned tap outlives the binary otherwise — night-improve-23's detach
# is the only clean unpinner. Best-effort: a host without the tap (or
# without sudo) skips this and bpftool remains the manual escape hatch.
if command -v "${PROJECT_NAME}" >/dev/null 2>&1; then
    echo ">> detaching the kernel-side tap (best effort)"
    sudo "${PROJECT_NAME}" detach 2>/dev/null ||         echo "   (detach skipped — no tap attached, or no sudo; manual: sudo bpftool path delete, or ignore: a reboot clears bpffs)"
fi

case "${MODE}" in
    --system)
        remove_at "/usr/bin/${PROJECT_NAME}" yes
        remove_at "/etc/${PROJECT_NAME}" yes
        remove_at "/etc/systemd/system/${PROJECT_NAME}-attach.service" yes
        ;;
    --user)
        remove_at "${HOME}/.local/bin/${PROJECT_NAME}" no
        if [[ ${PURGE} -eq 1 ]]; then
            remove_at "${HOME}/.config/${PROJECT_NAME}" no
        elif [[ -d "${HOME}/.config/${PROJECT_NAME}" ]]; then
            echo "   NOTE: legacy config-era dir preserved at ~/.config/${PROJECT_NAME}/"
            echo "         (the binary no longer reads it; remove with: ./scripts/package/uninstall.sh --purge)"
        fi
        ;;
    --all)
        # Binary
        remove_at "/usr/bin/${PROJECT_NAME}" yes
        remove_at "${HOME}/.local/bin/${PROJECT_NAME}" no
        # Legacy config-era dirs (system removed; user preserved unless --purge)
        remove_at "/etc/${PROJECT_NAME}" yes
        if [[ ${PURGE} -eq 1 ]]; then
            remove_at "${HOME}/.config/${PROJECT_NAME}" no
        elif [[ -d "${HOME}/.config/${PROJECT_NAME}" ]]; then
            echo "   NOTE: legacy config-era dir preserved at ~/.config/${PROJECT_NAME}/"
            echo "         (the binary no longer reads it; remove with: ./scripts/package/uninstall.sh --purge)"
        fi
        # Service
        remove_at "/etc/systemd/system/${PROJECT_NAME}-attach.service" yes
        ;;
esac

if [[ ${removed} -eq 0 ]]; then
    echo "   (nothing found to remove)"
    exit 0
fi

echo
echo ">> Done. Removed ${removed} artifact(s)."
echo "   Reload systemd to reflect changes:"
echo "     systemctl daemon-reload"
