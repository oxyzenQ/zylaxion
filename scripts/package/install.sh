#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only - zylaxion is a Linux keyboard-sound tool; no other OS is supported.
# Copyright (C) 2026 rezky_nightky
#
# Install zylaxion: binary + the kernel-side tap's boot oneshot.
# Supports --system (system-wide) and --user (default, ~/.local).
# Run WITHOUT sudo: the script escalates via sudo ONLY for --system install steps.
#
# night-improve-22: no config file exists anymore. All presets are
# compiled into the binary and selected with --preset or the
# ZYLAXION_PRESET environment variable.
# night-improve-23: there is NO daemon and NO user service. The
# kernel-side tap is planted once per boot by the SYSTEM oneshot unit
# (zylaxion-attach.service, --system mode) or by hand with
# `sudo zylaxion attach` (--user mode) — a oneshot that exits, not a
# supervised process.

set -euo pipefail

PROJECT_NAME="zylaxion"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ATTACH_UNIT_SRC="${REPO_ROOT}/assets/zylaxion-attach.service"

usage() {
    cat <<EOF
Usage: $0 [--system|--user]

  --system   Install system-wide:
               binary  → /usr/bin/${PROJECT_NAME}
               oneshot → /etc/systemd/system/${PROJECT_NAME}-attach.service
                         (enabled: plants the tap at every boot)
             (script invokes sudo for the install steps)
  --user     Install to user-local (default, no sudo):
               binary  → ~/.local/bin/${PROJECT_NAME}
               no unit — plant the tap by hand: sudo zylaxion attach

Preset selection is compiled in: pass --preset <name> on the CLI or
export ZYLAXION_PRESET=<name> (systemd units take Environment=).
ZYLAXION_VOLUME overrides the master volume the same way. Run
'${PROJECT_NAME} list-presets' to see the catalog.

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

if [[ ! -f "${ATTACH_UNIT_SRC}" ]]; then
    echo "error: required file not found: ${ATTACH_UNIT_SRC}" >&2
    exit 1
fi

echo ">> [1/3] Building ${PROJECT_NAME} (release, locked)"
cargo build --release --locked

BINARY="target/release/${PROJECT_NAME}"
if [[ ! -f "${BINARY}" ]]; then
    echo "error: build produced no binary at ${BINARY}" >&2
    exit 1
fi

echo ">> [2/3] Installing binary (${MODE})"
case "${MODE}" in
    --system)
        # v10.2.0: if a user-local install exists, clean it up first
        # so there are no stale binaries in ~ that would shadow the
        # system-wide install.
        user_bin="${HOME}/.local/bin/${PROJECT_NAME}"
        if [[ -f "${user_bin}" ]]; then
            echo "   cleaning existing user-local binary..."
            rm -f "${user_bin}"
            echo "   cleaned: ${user_bin}"
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

echo ">> [3/3] Kernel-side tap posture (${MODE})"
case "${MODE}" in
    --system)
        # The system oneshot: plants the tap at every boot (bpffs pins
        # do not survive a reboot). A ONESHOT, not a daemon — it exits
        # and stays dead; the kernel side remains.
        "${SUDO}" install -Dm644 "${ATTACH_UNIT_SRC}" \
            "/etc/systemd/system/${PROJECT_NAME}-attach.service"
        "${SUDO}" systemctl daemon-reload
        "${SUDO}" systemctl enable "${PROJECT_NAME}-attach" >/dev/null 2>&1 && \
            echo "   enabled: ${PROJECT_NAME}-attach (plants the tap at every boot)" ||
            echo "   (enable manually: sudo systemctl enable ${PROJECT_NAME}-attach)"
        echo "   start it now: sudo systemctl start ${PROJECT_NAME}-attach"
        ;;
    --user)
        echo "   no unit in user mode — plant the tap by hand (once per boot):"
        echo "     sudo zylaxion attach"
        echo "   or install the system oneshot: $0 --system"
        ;;
esac

echo ">> Done."
echo
echo "Next steps:"
if [[ "${MODE}" == "--system" ]]; then
echo "  - Plant the tap now (the oneshot is enabled for future boots):"
echo "      sudo systemctl start ${PROJECT_NAME}-attach"
echo "  - Then, as your user:"
echo "      ${PROJECT_NAME} on"
else
echo "  - Plant the tap (once per boot):"
echo "      sudo ${PROJECT_NAME} attach"
echo "  - Then:"
echo "      ${PROJECT_NAME} on"
fi
echo "  - Pick a preset (24 compiled in):"
echo "      ${PROJECT_NAME} list-presets"
echo "      ${PROJECT_NAME} preset --switch ibm_model_m   # live switch"
echo "  - Turn it off:"
echo "      ${PROJECT_NAME} off"
echo "  - Uninstall:"
case "${MODE}" in
    --system) echo "      sudo ./scripts/package/uninstall.sh --system" ;;
    --user)   echo "      ./scripts/package/uninstall.sh" ;;
esac
