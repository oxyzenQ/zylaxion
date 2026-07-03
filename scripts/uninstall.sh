#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky (oxyzenQ)
#
# Uninstall script for zylaxion.
# Auto-detects and removes the binary from any of:
#   /usr/bin/, /usr/local/bin/, ~/.local/bin/
# Sudo is used ONLY for system paths. Run WITHOUT sudo.

set -uo pipefail

zylaxion="zylaxion"
REPO_URL="https://github.com/oxyzenQ/zylaxion"

usage() {
    cat <<EOF
Usage: $0 [--system|--user|--all]

  (default)  Auto-detect: scan /usr/bin, /usr/local/bin, ~/.local/bin
             and remove every ${zylaxion} found. Sudo only for system paths.
  --system   Remove only from /usr/bin and /usr/local/bin (uses sudo).
  --user     Remove only from ~/.local/bin (no sudo).
  --all      Same as default.

EOF
}

MODE="--all"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --system) MODE="--system"; shift ;;
        --user)   MODE="--user";   shift ;;
        --all)    MODE="--all";    shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

SYSTEM_PATHS=(/usr/bin /usr/local/bin)
USER_PATH="${HOME}/.local/bin"
removed=0

remove_at() {
    local target="$1"
    local need_sudo="$2"
    if [[ -f "${target}" ]]; then
        if [[ "${need_sudo}" == "yes" ]]; then
            sudo rm -f "${target}"
        else
            rm -f "${target}"
        fi
        echo "   removed: ${target}"
        removed=$((removed+1))
    fi
}

echo ">> Uninstalling ${zylaxion}"

case "${MODE}" in
    --system)
        for p in "${SYSTEM_PATHS[@]}"; do
            remove_at "${p}/${zylaxion}" yes
        done
        ;;
    --user)
        remove_at "${USER_PATH}/${zylaxion}" no
        ;;
    --all)
        for p in "${SYSTEM_PATHS[@]}"; do
            remove_at "${p}/${zylaxion}" yes
        done
        remove_at "${USER_PATH}/${zylaxion}" no
        ;;
esac

if [[ ${removed} -eq 0 ]]; then
    echo "   (nothing found to remove)"
    exit 0
fi

echo ">> Done. Removed ${removed} copy/copies."
