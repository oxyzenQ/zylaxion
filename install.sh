#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Zylaxion installer — builds the release binary and installs it
# system-wide along with the default acoustic profile TOMLs.
#
# Distro-agnostic: works on Arch, Fedora, Debian/Ubuntu, and any
# Linux distribution with a standard filesystem layout.
#
# Usage: sudo ./install.sh

set -euo pipefail

# ── Early PATH fix for sudo ─────────────────────────────────────────
# When run via `sudo`, the root shell has a minimal PATH that does
# NOT include the invoking user's ~/.cargo/bin.  Fix this by looking
# up the user's home directory from /etc/passwd and prepending their
# cargo bin to PATH.  This must happen before anything else.
if [ -n "${SUDO_USER:-}" ]; then
    USER_HOME="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
    export PATH="${USER_HOME}/.cargo/bin:${PATH}"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="${SCRIPT_DIR}/Cargo.toml"
PROFILE_DIR="${SCRIPT_DIR}/profiles"

INSTALL_BIN="/usr/local/bin/zylaxion"
INSTALL_PROFILES="/etc/zylaxion/profiles"

# ── Prerequisites ───────────────────────────────────────────────────

check_deps() {
    local missing=()

    if ! command -v cargo &>/dev/null; then
        missing+=("cargo")
    fi

    if ! command -v pkg-config &>/dev/null; then
        missing+=("pkg-config")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        echo "Error: missing dependencies:"
        for dep in "${missing[@]}"; do
            echo "  - ${dep}"
        done
        echo ""
        echo "Please install the above via your system's package manager."
        echo "Rust: https://rustup.rs/"
        exit 1
    fi
}

# ── Build ──────────────────────────────────────────────────────────

build_release() {
    echo "==> Building release binary..."
    cargo build --release --locked --manifest-path "$MANIFEST"
}

# ── Install ─────────────────────────────────────────────────────────

install_bin() {
    echo "==> Installing binary to ${INSTALL_BIN}..."
    install -Dm755 "${SCRIPT_DIR}/target/release/zylaxion" "$INSTALL_BIN"
}

install_profiles() {
    echo "==> Installing profile TOMLs to ${INSTALL_PROFILES}/"
    mkdir -p "$INSTALL_PROFILES"
    for toml in "$PROFILE_DIR"/*.toml; do
        [ -f "$toml" ] || continue
        install -m0644 "$toml" "$INSTALL_PROFILES/"
        echo "    installed $(basename "$toml")"
    done
}

# ── Post-install notes ──────────────────────────────────────────────

post_install() {
    local target_user="${SUDO_USER:-$USER}"

    echo ""
    echo "==> Zylaxion installed successfully!"
    echo ""

    # Check if the target user is in the input group.
    if id -nG "$target_user" 2>/dev/null | tr ' ' '\n' | grep -qx "input"; then
        echo "    OK  User '${target_user}' is in the 'input' group."
    else
        echo "    WARNING  User '${target_user}' is NOT in the 'input' group."
        echo "    Run:  sudo usermod -aG input ${target_user}"
        echo "    Then log out and back in for the change to take effect."
    fi

    echo ""
    echo "    Usage:"
    echo "      zylaxion start --profile technical   (foreground)"
    echo "      zylaxion daemon --profile classic    (background)"
    echo "      zylaxion list-profiles              (see all profiles)"
    echo "      zylaxion doctor                     (system check)"
    echo ""
    echo "    Uninstall:  sudo ${SCRIPT_DIR}/uninstall.sh"
}

# ── Main ────────────────────────────────────────────────────────────

check_deps
build_release
install_bin
install_profiles
post_install
