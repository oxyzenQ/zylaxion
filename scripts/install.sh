#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky (oxyzenQ)
#
# Install zylaxion: binary + config.toml + systemd user service.
# Supports --system (system-wide) and --user (default, ~/.local).
# Run WITHOUT sudo: the script escalates via sudo ONLY for --system install steps.

set -euo pipefail

PROJECT_NAME="zylaxion"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG_SRC="${REPO_ROOT}/config.toml"
SERVICE_SRC="${REPO_ROOT}/assets/zylaxion.service"

usage() {
    cat <<EOF
Usage: $0 [--system|--user]

  --system   Install system-wide:
               binary  → /usr/bin/${PROJECT_NAME}
               config  → /etc/${PROJECT_NAME}/config.toml (FHS default)
               service → /etc/systemd/user/${PROJECT_NAME}.service
             (script invokes sudo for the install steps)
  --user     Install to user-local (default, no sudo):
               binary  → ~/.local/bin/${PROJECT_NAME}
               config  → ~/.config/${PROJECT_NAME}/config.toml (only if not present)
               service → ~/.config/systemd/user/${PROJECT_NAME}.service

The system config file is NEVER overwritten. If it already exists, a
timestamped backup is created as config.bak.<epoch> and the new template
is installed as config.new for manual review.

The build step (cargo build --release --locked) ALWAYS runs as the current user.
EOF
}

MODE="--user"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --system) MODE="--system"; shift ;;
        --user)   MODE="--user";   shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

cd "${REPO_ROOT}"

if [[ ! -f Cargo.toml ]]; then
    echo "error: Cargo.toml not found. Run this script from the repo root." >&2
    exit 1
fi

for f in "${CONFIG_SRC}" "${SERVICE_SRC}"; do
    if [[ ! -f "${f}" ]]; then
        echo "error: required file not found: ${f}" >&2
        exit 1
    fi
done

echo ">> [1/4] Building ${PROJECT_NAME} (release, locked)"
cargo build --release --locked

BINARY="target/release/${PROJECT_NAME}"
if [[ ! -f "${BINARY}" ]]; then
    echo "error: build produced no binary at ${BINARY}" >&2
    exit 1
fi

echo ">> [2/4] Installing binary (${MODE})"
case "${MODE}" in
    --system)
        sudo install -Dm755 "${BINARY}" "/usr/bin/${PROJECT_NAME}"
        echo "   installed: /usr/bin/${PROJECT_NAME}"
        ;;
    --user)
        user_bin="${HOME}/.local/bin"
        mkdir -p "${user_bin}"
        install -Dm755 "${BINARY}" "${user_bin}/${PROJECT_NAME}"
        echo "   installed: ${user_bin}/${PROJECT_NAME}"
        ;;
esac

echo ">> [3/4] Installing config.toml (${MODE})"
case "${MODE}" in
    --system)
        # FHS default location — in zylaxion's config search path.
        sudo mkdir -p "/etc/${PROJECT_NAME}"
        config_path="/etc/${PROJECT_NAME}/config.toml"
        if sudo test -f "${config_path}"; then
            backup="${config_path}.bak.$(date +%s)"
            sudo cp -p "${config_path}" "${backup}"
            sudo install -m 644 "${CONFIG_SRC}" "${config_path}.new"
            echo "   existing config preserved: ${config_path}"
            echo "   backup created at:            ${backup}"
            echo "   new template installed at:    ${config_path}.new (review and merge manually)"
        else
            sudo install -Dm644 "${CONFIG_SRC}" "${config_path}"
            echo "   installed: ${config_path}"
            echo "   (system-wide default; users can override at ~/.config/${PROJECT_NAME}/config.toml)"
        fi
        ;;
    --user)
        user_cfg_dir="${HOME}/.config/${PROJECT_NAME}"
        user_cfg="${user_cfg_dir}/config.toml"
        mkdir -p "${user_cfg_dir}"
        if [[ -f "${user_cfg}" ]]; then
            # Preserve user customizations — install as .new for review.
            install -Dm644 "${CONFIG_SRC}" "${user_cfg}.new"
            echo "   existing config preserved: ${user_cfg}"
            echo "   new template installed at: ${user_cfg}.new (review and merge manually)"
        else
            install -Dm644 "${CONFIG_SRC}" "${user_cfg}"
            echo "   installed: ${user_cfg}"
        fi
        ;;
esac

echo ">> [4/4] Installing systemd user unit (${MODE})"
case "${MODE}" in
    --system)
        # System-wide user unit. Modify ExecStart to point to /usr/bin.
        sudo mkdir -p "/etc/systemd/user"
        sed 's|%h/.local/bin/'"${PROJECT_NAME}"'|/usr/bin/'"${PROJECT_NAME}"'|g' \
            "${SERVICE_SRC}" | sudo tee "/etc/systemd/user/${PROJECT_NAME}.service" >/dev/null
        sudo chmod 644 "/etc/systemd/user/${PROJECT_NAME}.service"
        echo "   installed: /etc/systemd/user/${PROJECT_NAME}.service"
        ;;
    --user)
        user_svc_dir="${HOME}/.config/systemd/user"
        mkdir -p "${user_svc_dir}"
        install -Dm644 "${SERVICE_SRC}" \
            "${user_svc_dir}/${PROJECT_NAME}.service"
        echo "   installed: ${user_svc_dir}/${PROJECT_NAME}.service"
        ;;
esac

# ── Post-install checks ─────────────────────────────────────────────
echo
echo ">> Post-install checks"
if id -nG "${USER}" 2>/dev/null | tr ' ' '\n' | grep -qx "input"; then
    echo "   OK  User '${USER}' is in the 'input' group."
else
    echo "   WARNING  User '${USER}' is NOT in the 'input' group."
    echo "            ${PROJECT_NAME} needs raw keyboard access via evdev."
    echo "            Add '${USER}' to the 'input' group, then log out and back in:"
    echo "              sudo usermod -aG input ${USER}"
fi

echo
echo ">> Done."
echo
echo "Next steps:"
echo "  - Reload systemd user units:"
echo "      systemctl --user daemon-reload"
echo "  - Enable + start ${PROJECT_NAME}:"
echo "      systemctl --user enable --now ${PROJECT_NAME}"
echo "  - Live logs:"
echo "      journalctl --user -u ${PROJECT_NAME} -f"
echo "  - Verify config:"
echo "      ${PROJECT_NAME} testconf"
echo "  - Uninstall:"
case "${MODE}" in
    --system) echo "      sudo ./scripts/uninstall.sh --system" ;;
    --user)   echo "      ./scripts/uninstall.sh" ;;
esac
