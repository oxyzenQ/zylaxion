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
# Usage: sudo ./scripts/install.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="${SCRIPT_DIR}/.."
MANIFEST="${WORKSPACE_ROOT}/Cargo.toml"
PROFILE_DIR="${WORKSPACE_ROOT}/profiles"

INSTALL_BIN="/usr/local/bin/zylaxion"
INSTALL_PROFILES="/etc/zylaxion/profiles"

# ── Resolve the invoking user (if run via sudo) ──────────────────────
TARGET_USER="${SUDO_USER:-$USER}"

# ── Prerequisites ───────────────────────────────────────────────────

check_deps() {
    local missing=()

    # When run via sudo, check cargo as the original user since root
    # may not have it in PATH.
    if [ -n "${SUDO_USER:-}" ]; then
        if ! sudo -Eu "$SUDO_USER" cargo --version &>/dev/null; then
            missing+=("cargo")
        fi
    else
        if ! command -v cargo &>/dev/null; then
            missing+=("cargo")
        fi
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
    if [ -n "${SUDO_USER:-}" ]; then
        # Run cargo as the invoking user to inherit RUSTUP_HOME,
        # CARGO_HOME, and the correct toolchain.  --manifest-path
        # uses the absolute path computed above.
        sudo -Eu "$SUDO_USER" cargo build --release --locked \
            --manifest-path "$MANIFEST"
    else
        cargo build --release --locked --manifest-path "$MANIFEST"
    fi
}

# ── Install ─────────────────────────────────────────────────────────

install_bin() {
    echo "==> Installing binary to ${INSTALL_BIN}..."
    install -Dm755 "${WORKSPACE_ROOT}/target/release/zylaxion" "$INSTALL_BIN"
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
    echo ""
    echo "==> Zylaxion installed successfully!"
    echo ""

    # Check if the target user is in the input group.
    if id -nG "$TARGET_USER" 2>/dev/null | tr ' ' '\n' | grep -qx "input"; then
        echo "    OK  User '${TARGET_USER}' is in the 'input' group."
    else
        echo "    WARNING  User '${TARGET_USER}' is NOT in the 'input' group."
        echo "    Run:  sudo usermod -aG input ${TARGET_USER}"
        echo "    Then log out and back in for the change to take effect."
    fi

    echo ""
    echo "    Usage:"
    echo "      zylaxion start --profile technical   (foreground)"
    echo "      zylaxion daemon --profile classic    (background)"
    echo "      zylaxion list-profiles              (see all profiles)"
    echo "      zylaxion doctor                     (system check)"
    echo ""
    echo "    Uninstall:  sudo ./scripts/uninstall.sh"
}

# ── Main ────────────────────────────────────────────────────────────

check_deps
build_release
install_bin
install_profiles
post_install
