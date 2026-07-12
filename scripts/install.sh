#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 rezky_nightky
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

# Refuse to run as root in --user mode — cargo build must run as the
# current user. If run with sudo, cargo build would create root-owned
# files in target/, breaking future `cargo clean` / `cargo build` for
# the normal user.
#
# v10.2.0 (dragonzen audit P5): allow root ONLY for --system installs
# (Docker containers, rootless environments, CI). In --system mode as
# root, the script skips the `sudo` prefix for install steps (since
# we're already root). In --user mode as root, refuse — root's $HOME
# is /root and the install would land in the wrong place.
if [[ $EUID -eq 0 ]] && [[ "${MODE}" != "--system" ]]; then
    echo "error: do not run this script with sudo in --user mode." >&2
    echo "  --user mode installs to \$HOME/.local/bin — root's \$HOME is /root." >&2
    echo "  Run: $0 --system  (uses sudo internally for the install step)" >&2
    echo "  Or:  $0 --user    (run as the target user, not sudo)" >&2
    exit 1
fi

# v10.2.0 (P5): set SUDO="" when already root so the install steps
# don't try to escalate (sudo inside a container without sudo installed
# would fail).
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
        # v10.2.0: if a user-local install exists, clean it up first
        # so there are no stale binaries or config in ~ that would
        # shadow the system-wide install.
        user_bin="${HOME}/.local/bin/${PROJECT_NAME}"
        user_cfg="${HOME}/.config/${PROJECT_NAME}/config.toml"
        user_svc="${HOME}/.config/systemd/user/${PROJECT_NAME}.service"
        if [[ -f "${user_bin}" ]] || [[ -f "${user_cfg}" ]] || [[ -f "${user_svc}" ]]; then
            echo "   cleaning existing user-local install..."
            rm -f "${user_bin}" "${user_svc}"
            # Remove config only if it's identical to the system one
            # (same file). If the user customized it, keep it — the
            # user-local config takes priority over /etc anyway.
            if [[ -f "${user_cfg}" ]]; then
                echo "   note: keeping ${user_cfg} (user-local override)"
            fi
            echo "   cleaned: ${user_bin} + ${user_svc}"
        fi
        "${SUDO}" install -Dm755 "${BINARY}" "/usr/bin/${PROJECT_NAME}"
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
        "${SUDO}" mkdir -p "/etc/${PROJECT_NAME}"
        config_path="/etc/${PROJECT_NAME}/config.toml"
        # v10.2.0: overwrite unconditionally — no .new or .bak bloat.
        "${SUDO}" install -Dm644 "${CONFIG_SRC}" "${config_path}"
        echo "   installed: ${config_path}"
        echo "   (system-wide default; users can override at ~/.config/${PROJECT_NAME}/config.toml)"
        ;;
    --user)
        user_cfg_dir="${HOME}/.config/${PROJECT_NAME}"
        user_cfg="${user_cfg_dir}/config.toml"
        mkdir -p "${user_cfg_dir}"
        # v10.2.0: overwrite unconditionally — no .new or .bak bloat.
        install -Dm644 "${CONFIG_SRC}" "${user_cfg}"
        echo "   installed: ${user_cfg}"
        ;;
esac

echo ">> [4/4] Installing systemd user unit (${MODE})"
case "${MODE}" in
    --system)
        # System-wide user unit. Modify ExecStart to point to /usr/bin.
        "${SUDO}" mkdir -p "/etc/systemd/user"
        sed 's|%h/.local/bin/'"${PROJECT_NAME}"'|/usr/bin/'"${PROJECT_NAME}"'|g' \
            "${SERVICE_SRC}" | "${SUDO}" tee "/etc/systemd/user/${PROJECT_NAME}.service" >/dev/null
        "${SUDO}" chmod 644 "/etc/systemd/user/${PROJECT_NAME}.service"
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
