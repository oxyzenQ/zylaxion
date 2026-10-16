#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only - zylaxion is a Linux keyboard-sound tool; no other OS is supported.
# Copyright (C) 2026 rezky_nightky
#
# Install zylaxion: binary + the kernel-side tap's boot oneshot + the
# login hand-over.
# night-master-2: the install posture is SYSTEM-ONLY. There are no
# --system/--user flags anymore - one posture, one destination:
#   binary    -> /usr/bin/zylaxion
#   oneshot   -> /etc/systemd/system/zylaxion-attach.service
#   hand-over -> /etc/systemd/system/zylaxion-handover.{path,service}
# The build is the portable baseline shape (cargo build --release
# --locked); the CPU-tuned personal lane is the sibling
# install-on-my-machine.sh in this directory.
#
# Run WITHOUT sudo: the script escalates via sudo ONLY for the install
# steps (already-root hosts skip the prefix - container posture).
#
# night-improve-22: no config file exists anymore. All presets are
# compiled into the binary and selected with --preset or the
# ZYLAXION_PRESET environment variable.
# night-improve-23: there is NO daemon and NO user service. The
# kernel-side tap is planted once per boot by the SYSTEM oneshot unit
# (zylaxion-attach.service) - a oneshot that exits, not a supervised
# process.

set -euo pipefail

PROJECT_NAME="zylaxion"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ATTACH_UNIT_SRC="${REPO_ROOT}/assets/${PROJECT_NAME}-attach.service"
HANDOVER_PATH_SRC="${REPO_ROOT}/assets/${PROJECT_NAME}-handover.path"
HANDOVER_SERVICE_SRC="${REPO_ROOT}/assets/${PROJECT_NAME}-handover.service"

usage() {
    cat <<EOF
Usage: $0

Installs the system posture, no flags:
  binary    -> /usr/bin/${PROJECT_NAME}
  oneshot   -> /etc/systemd/system/${PROJECT_NAME}-attach.service
               (enabled: plants the tap at every boot)
  hand-over -> /etc/systemd/system/${PROJECT_NAME}-handover.{path,service}
               (enabled: re-runs the attach at the first desktop login
               so the boot pins land owned by your user)

The build step (cargo build --release --locked) ALWAYS runs as the
current user; sudo is used only for the install steps. For a CPU-tuned
build (amd64 v3/v4 detection) use install-on-my-machine.sh (same
directory as this script) instead - it lands the same destinations.

Preset selection is compiled in: pass --preset <name> on the CLI or
export ZYLAXION_PRESET=<name> (systemd units take Environment=).
ZYLAXION_VOLUME overrides the master volume the same way. Run
'${PROJECT_NAME} list-presets' to see the catalog.
EOF
}

case "${1:-}" in
    "") ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument: $1 (this script takes no options)" >&2; usage; exit 2 ;;
esac

# v10.2.0 (dragonzen audit P5): allow root (Docker containers, rootless
# environments, CI) - as root the script skips the sudo prefix for the
# install steps. The build then runs as root too, which leaves
# root-owned files in target/; non-root invocation stays the
# recommended shape.
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
if [[ ! -f "${HANDOVER_PATH_SRC}" || ! -f "${HANDOVER_SERVICE_SRC}" ]]; then
    echo "error: required files not found: ${HANDOVER_PATH_SRC} / ${HANDOVER_SERVICE_SRC}" >&2
    exit 1
fi

echo ">> [1/3] Building ${PROJECT_NAME} (release, locked)"
cargo build --release --locked

BINARY="target/release/${PROJECT_NAME}"
if [[ ! -f "${BINARY}" ]]; then
    echo "error: build produced no binary at ${BINARY}" >&2
    exit 1
fi

echo ">> [2/3] Installing the binary"
# Clean any stale user-local binary first so nothing shadows /usr/bin
# (the pre-night-master-2 default path was ~/.local/bin).
user_bin="${HOME}/.local/bin/${PROJECT_NAME}"
if [[ -f "${user_bin}" ]]; then
    echo "   cleaning stale user-local binary..."
    rm -f "${user_bin}"
    echo "   cleaned: ${user_bin}"
fi
"${SUDO}" install -Dm755 "${BINARY}" "/usr/bin/${PROJECT_NAME}"
echo "   installed: /usr/bin/${PROJECT_NAME}"

echo ">> [3/3] Installing the boot oneshot + the login hand-over"
# The system oneshot: plants the tap at every boot (bpffs pins do not
# survive a reboot). A ONESHOT, not a daemon - it exits and stays dead;
# the kernel side remains.
"${SUDO}" install -Dm644 "${ATTACH_UNIT_SRC}" \
    "/etc/systemd/system/${PROJECT_NAME}-attach.service"
# The login hand-over (night-dinner-19): the boot oneshot runs before
# anyone logs in, so its pins are root-owned; the path unit watches
# /run/user and re-runs `zylaxion attach` at the first desktop login,
# handing the pins to that user and relaxing the bpffs front door.
"${SUDO}" install -Dm644 "${HANDOVER_SERVICE_SRC}" \
    "/etc/systemd/system/${PROJECT_NAME}-handover.service"
"${SUDO}" install -Dm644 "${HANDOVER_PATH_SRC}" \
    "/etc/systemd/system/${PROJECT_NAME}-handover.path"
if command -v systemctl >/dev/null 2>&1; then
    "${SUDO}" systemctl daemon-reload
    "${SUDO}" systemctl enable "${PROJECT_NAME}-attach" >/dev/null 2>&1 && \
        echo "   enabled: ${PROJECT_NAME}-attach (plants the tap at every boot)" ||
        echo "   (enable manually: sudo systemctl enable ${PROJECT_NAME}-attach)"
    # --now: the watch fires immediately on a desktop host (a
    # /run/user entry already exists), so the just-planted or
    # already-planted pins land owned by this user without a reboot.
    "${SUDO}" systemctl enable --now "${PROJECT_NAME}-handover.path" >/dev/null 2>&1 && \
        echo "   enabled: ${PROJECT_NAME}-handover.path (hands the pins over at the first login)" ||
        echo "   (enable manually: sudo systemctl enable --now ${PROJECT_NAME}-handover.path)"
else
    echo "   (systemd not present; the unit files are installed but not enabled)"
fi

echo ">> Done."
echo
echo "Next steps:"
echo "  - Plant the tap now (the oneshot is enabled for future boots):"
echo "      sudo systemctl start ${PROJECT_NAME}-attach"
echo "  - Then, as your user:"
echo "      ${PROJECT_NAME} on"
echo "  - Pick a preset (24 compiled in):"
echo "      ${PROJECT_NAME} list-presets"
echo "      ${PROJECT_NAME} preset --switch ibm_model_m   # live switch"
echo "  - Turn it off:"
echo "      ${PROJECT_NAME} off"
echo "  - Uninstall:"
echo "      sudo ./scripts/package/uninstall.sh"
