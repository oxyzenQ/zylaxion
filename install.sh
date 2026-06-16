#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Zylaxion installer — builds the release binary and installs it
# system-wide along with the default acoustic profile TOMLs.
#
# Usage: sudo ./install.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="${SCRIPT_DIR}/Cargo.toml"
PROFILE_DIR="${SCRIPT_DIR}/profiles"

INSTALL_BIN="/usr/local/bin/zylaxion"
INSTALL_PROFILES="/etc/zylaxion/profiles"

# ── Prerequisites ───────────────────────────────────────────────────

check_deps() {
    local missing=()

    if ! command -v cargo &>/dev/null; then
        missing+=("cargo (Rust toolchain)")
    fi

    if ! command -v pkg-config &>/dev/null; then
        missing+=("pkg-config")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        echo "error: missing dependencies:"
        for dep in "${missing[@]}"; do
            echo "  - $dep"
        done
        echo ""
        echo "Install Rust: https://rustup.rs/"
        echo "Install pkg-config: sudo apt install pkg-config (Debian/Ubuntu)"
        exit 1
    fi
}

# ── Build ──────────────────────────────────────────────────────────

build_release() {
    echo "==> Building release binary..."
    cargo build --release --manifest-path "$MANIFEST"
}

# ── Install ─────────────────────────────────────────────────────────

install_bin() {
    echo "==> Installing binary to ${INSTALL_BIN}..."
    install -m 0755 "${SCRIPT_DIR}/target/release/zylaxion" "$INSTALL_BIN"
}

install_profiles() {
    echo "==> Installing profile TOMLs to ${INSTALL_PROFILES}/..."
    mkdir -p "$INSTALL_PROFILES"
    for toml in "$PROFILE_DIR"/*.toml; do
        [ -f "$toml" ] || continue
        install -m 0644 "$toml" "$INSTALL_PROFILES/"
        echo "    installed $(basename "$toml")"
    done
}

# ── Post-install notes ──────────────────────────────────────────────

post_install() {
    echo ""
    echo "==> Zylaxion installed successfully!"
    echo ""

    # Check if user is in the input group.
    if ! groups 2>/dev/null | tr ' ' '\n' | grep -qx "input"; then
        echo "    ⚠  You are NOT in the 'input' group."
        echo "       Run:  sudo usermod -aG input \$USER"
        echo "       Then log out and back in."
    else
        echo "    ✓ User is in the 'input' group."
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
