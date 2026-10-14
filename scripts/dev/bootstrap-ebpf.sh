#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux eBPF tool; no other OS is supported.
#
# Host bootstrap for the pure-Rust eBPF build (night-improve-23, the
# zelynic scripts/dev/bootstrap-ebpf.sh lineage, trimmed to zylaxion's
# ONE object and its aliases).
#
# Installs the two source-build prerequisites:
#   1. the dated nightly toolchain pinned by ebpf/rust-toolchain.toml
#      (minimal profile, with the rust-src + rustfmt components)
#   2. the bpf-linker 0.11.1 prebuilt static-musl binary, installed
#      into ~/.local/bin (no sudo, no system LLVM, no clang)
#
# Every git-checkout build needs both (night-badas-1 wording refresh):
# the ebpf feature is default since night-improve-23, so build.rs
# source-builds the kernel object and its preflight hard-requires the
# dated nightly pin and bpf-linker BEFORE any compile time is spent —
# a warm ebpf/target cache does not bypass the preflight (it exists to
# catch a missing bpf-linker even then). Only a registry tarball (cargo
# install from crates.io) rides the ebpf-prebuilt/ object instead and
# needs neither prerequisite. Re-running this script after a fresh
# clone is the one-command path to a green build.
#
# The nightly pin is READ from ebpf/rust-toolchain.toml and the
# bpf-linker pin from the BPF_LINKER_VERSION constant below, so bumping
# either in its own file re-targets this script.
#
# Idempotent: re-running skips whatever is already satisfied. Never
# silent: every step announces itself; the final line reports total
# wall-clock elapsed.
#
# Usage:
#   ./scripts/dev/bootstrap-ebpf.sh          install whatever is missing
#   ./scripts/dev/bootstrap-ebpf.sh --check  report status only (exit 1
#                                            if something is missing)

set -euo pipefail

# The rustup/cargo bin dir is not on PATH in every invocation context
# (non-login shells, cron) — the script drives rustup directly, so try
# the standard location before refusing.
if [[ -d "${HOME}/.cargo/bin" ]]; then
	PATH="${HOME}/.cargo/bin:${PATH}"
fi

BPF_LINKER_URL_BASE="https://github.com/aya-rs/bpf-linker/releases/download"
BPF_LINKER_VERSION="0.11.1"
# The artifact checksum pins (night-private-research-2xblade-3.6X, the
# security hardening pass): sha256 of the exact release asset, measured
# over HTTPS from the aya-rs release page and committed here. HTTPS
# authenticates the transport; this pin authenticates the bytes — a
# compromised release asset or a mirrored page now dies at the
# checksum gate, before anything is extracted or executed. A bumped
# BPF_LINKER_VERSION MUST re-measure and re-pin both hashes in the
# same commit: the version and the two hashes are one contract.
BPF_LINKER_SHA256_X86_64="e058a6aecc9e65fa4c977b298a8e4b738424d7629769fd352eed409fb57e16e8"
BPF_LINKER_SHA256_AARCH64="341ec1c595496877cae2b073544c2226d78a922739632b5732dbaa48507f1380"
CHECK_ONLY=false

case "${1:-}" in
"") ;;
--check) CHECK_ONLY=true ;;
*)
	echo "Usage: $0 [--check]" >&2
	exit 1
	;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TOOLCHAIN_FILE="${REPO_ROOT}/ebpf/rust-toolchain.toml"
LOCAL_BIN="${HOME}/.local/bin"
TMP_DIR=""

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

ok() { echo -e "${GREEN}[bootstrap-ebpf]${NC} $1"; }
warn() { echo -e "${YELLOW}[bootstrap-ebpf]${NC} $1"; }
die() {
	echo -e "${RED}[bootstrap-ebpf]${NC} $1" >&2
	exit 1
}

cleanup() {
	if [[ -n "${TMP_DIR}" ]]; then
		rm -rf "${TMP_DIR}"
	fi
}
trap cleanup EXIT

# ── Resolve the dated nightly pin from the crate's own file ────────────────
[[ -f "${TOOLCHAIN_FILE}" ]] || die "ebpf/rust-toolchain.toml not found (${TOOLCHAIN_FILE}) — run from inside the zylaxion repo."
command -v rustup >/dev/null 2>&1 || die "rustup is not installed — get it from https://rustup.rs first (this script drives it)."

EBPF_TOOLCHAIN="$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "${TOOLCHAIN_FILE}" | head -n 1)"
[[ -n "${EBPF_TOOLCHAIN}" ]] || die "could not read the toolchain channel from ${TOOLCHAIN_FILE}."

# ── Status probes (no side effects) ────────────────────────────────────────
TOOLCHAIN_OK=false
COMPONENTS_OK=false
LINKER_OK=false
LINKER_RESOLVED=""

probe_all() {
	TOOLCHAIN_OK=false
	if rustup toolchain list 2>/dev/null | grep -q "^${EBPF_TOOLCHAIN}-"; then
		TOOLCHAIN_OK=true
	fi

	COMPONENTS_OK=true
	if [[ "${TOOLCHAIN_OK}" != true ]]; then
		COMPONENTS_OK=false
	else
		local listing
		if listing="$(rustup component list --toolchain "${EBPF_TOOLCHAIN}" 2>/dev/null)"; then
			for component in rust-src rustfmt; do
				if ! grep -qE "^${component}(-[^ ]*)?[[:space:]]+\(installed\)" <<<"${listing}"; then
					COMPONENTS_OK=false
				fi
			done
		else
			COMPONENTS_OK=false
		fi
	fi

	LINKER_OK=false
	LINKER_RESOLVED=""
	if command -v bpf-linker >/dev/null 2>&1; then
		LINKER_RESOLVED="$(command -v bpf-linker)"
	elif [[ -x "${LOCAL_BIN}/bpf-linker" ]]; then
		LINKER_RESOLVED="${LOCAL_BIN}/bpf-linker"
	fi
	local reported
	reported="$("${LINKER_RESOLVED}" --version 2>/dev/null || echo "unknown")"
	if [[ "${reported}" == *"${BPF_LINKER_VERSION}"* ]]; then
		LINKER_OK=true
	fi
}

report() {
	if [[ "${TOOLCHAIN_OK}" == true && "${COMPONENTS_OK}" == true ]]; then
		ok "nightly pin ${EBPF_TOOLCHAIN}: installed (rust-src, rustfmt present)"
	else
		warn "nightly pin ${EBPF_TOOLCHAIN}: not fully installed"
	fi
	if [[ "${LINKER_OK}" == true ]]; then
		ok "bpf-linker ${BPF_LINKER_VERSION}: ${LINKER_RESOLVED}"
	elif [[ -n "${LINKER_RESOLVED}" ]]; then
		warn "bpf-linker: ${LINKER_RESOLVED} reports the wrong version (expected ${BPF_LINKER_VERSION})"
	else
		warn "bpf-linker: not on PATH"
	fi
}

install_bpf_linker() {
	local arch asset url archive extracted expected_hash
	arch="$(uname -m)"
	case "${arch}" in
	x86_64)
		asset="bpf-linker-x86_64-unknown-linux-musl.tar.zst"
		expected_hash="${BPF_LINKER_SHA256_X86_64}"
		;;
	aarch64)
		asset="bpf-linker-aarch64-unknown-linux-musl.tar.zst"
		expected_hash="${BPF_LINKER_SHA256_AARCH64}"
		;;
	*)
		die "no bpf-linker ${BPF_LINKER_VERSION} prebuilt for ${arch} — build it from source: https://github.com/aya-rs/bpf-linker"
		;;
	esac
	url="${BPF_LINKER_URL_BASE}/v${BPF_LINKER_VERSION}/${asset}"

	TMP_DIR="$(mktemp -d)"
	archive="${TMP_DIR}/${asset}"
	extracted="${TMP_DIR}/extract"
	mkdir -p "${extracted}"

	ok "downloading ${url} (~30 MB — the slow step)"
	if command -v curl >/dev/null 2>&1; then
		curl -fsSL --retry 3 -o "${archive}" "${url}" ||
			die "download failed — check network access, or fetch ${url} by hand and extract bpf-linker into ${LOCAL_BIN}"
	elif command -v wget >/dev/null 2>&1; then
		wget -q -O "${archive}" "${url}" ||
			die "download failed — check network access, or fetch ${url} by hand and extract bpf-linker into ${LOCAL_BIN}"
	else
		die "neither curl nor wget is available — fetch ${url} by hand and extract bpf-linker into ${dest}"
	fi
	[[ -s "${archive}" ]] || die "downloaded file is empty: ${archive}"

	# The artifact pin (night-private-research-2xblade-3.6X): the
	# downloaded archive must match the sha256 committed beside the
	# version constant — the checksum gate fires before extraction,
	# before the ELF check, before anything on this host executes
	# bytes it did not build. A mismatch here is tampering until
	# proven otherwise: refuse loudly, name both hashes.
	command -v sha256sum >/dev/null 2>&1 ||
		die "sha256sum is not available — cannot verify the pinned bpf-linker checksum."
	local measured_hash
	measured_hash="$(sha256sum "${archive}" | awk '{print $1}')"
	[[ "${measured_hash}" == "${expected_hash}" ]] ||
		die "downloaded ${asset} sha256 ${measured_hash} does not match the pinned checksum ${expected_hash} — refusing to install an artifact the repo cannot vouch for"
	ok "checksum verified: ${measured_hash}"

	# Extract with whichever zstd-capable decompressor the host has:
	# GNU tar with zstd, python3 with the zstandard module, or explicit
	# manual instructions — never a silent skip.
	if command -v zstd >/dev/null 2>&1 && tar --zstd -tf "$archive" >/dev/null 2>&1; then
		ok "extracting (tar --zstd)..."
		tar --zstd -xf "$archive" -C "$extracted"
	elif command -v python3 >/dev/null 2>&1 && python3 -c 'import zstandard' >/dev/null 2>&1; then
		ok "extracting (python3 zstandard, then tar)..."
		# The release asset is a zstd stream wrapping a POSIX tar that
		# contains the binary — two layers, so decompress first and unpack
		# the tar second (writing the raw stream straight to the binary
		# name installs a tar, not an ELF).
		python3 - "$archive" "${TMP_DIR}/payload.tar" <<'PYEOF'
import sys

import zstandard

with open(sys.argv[1], "rb") as src, open(sys.argv[2], "wb") as dst:
    zstandard.ZstdDecompressor().copy_stream(src, dst)
PYEOF
		tar -xf "${TMP_DIR}/payload.tar" -C "$extracted"
	else
		die "no zstd-capable extractor on this host (zstd binary, or python3 with the zstandard module) — extract ${archive} by hand into ${extracted}"
	fi
	[[ -f "${extracted}/bpf-linker" ]] ||
		die "the archive did not contain a top-level bpf-linker binary — inspect ${archive}"
	# The staged name must be an ELF executable before it lands on PATH:
	# a corrupt extraction would otherwise survive this script and only
	# explode later, far from the cause.
	elf_magic="$(dd if="${extracted}/bpf-linker" bs=1 count=4 2>/dev/null | od -An -tx1 | tr -d ' \n')"
	[[ "${elf_magic}" == "7f454c46" ]] ||
		die "extracted bpf-linker is not an ELF executable (file magic: ${elf_magic:-none}) — the extraction is corrupt; remove it and retry"

	mkdir -p "${LOCAL_BIN}"
	install -m755 "${extracted}/bpf-linker" "${LOCAL_BIN}/bpf-linker"
	local installed_version
	installed_version="$("${LOCAL_BIN}/bpf-linker" --version 2>/dev/null || echo "unknown")"
	[[ "${installed_version}" == *"${BPF_LINKER_VERSION}"* ]] ||
		die "installed bpf-linker reports '${installed_version}', expected ${BPF_LINKER_VERSION}"
	ok "bpf-linker ${installed_version} installed to ${LOCAL_BIN}/bpf-linker"
}

probe_all
report

if [[ "${CHECK_ONLY}" == true ]]; then
	if [[ "${TOOLCHAIN_OK}" == true && "${COMPONENTS_OK}" == true && "${LINKER_OK}" == true ]]; then
		exit 0
	fi
	die "--check: prerequisites not satisfied — run $0 (without --check) to install, and make sure ${LOCAL_BIN} is on PATH."
fi

CHANGED=false

if [[ "${TOOLCHAIN_OK}" != true ]]; then
	ok "installing nightly ${EBPF_TOOLCHAIN} (minimal profile, rust-src + rustfmt)..."
	rustup toolchain install "${EBPF_TOOLCHAIN}" --profile minimal --component rust-src --component rustfmt
	CHANGED=true
elif [[ "${COMPONENTS_OK}" != true ]]; then
	ok "adding missing components to ${EBPF_TOOLCHAIN}..."
	rustup component add rust-src rustfmt --toolchain "${EBPF_TOOLCHAIN}"
	CHANGED=true
fi

if [[ "${LINKER_OK}" != true ]]; then
	install_bpf_linker
	CHANGED=true
fi

# ── Re-probe: verify what changed, then trust it ──────────────────────────
if [[ "${CHANGED}" == true ]]; then
	probe_all
	report
	if [[ "${TOOLCHAIN_OK}" != true || "${COMPONENTS_OK}" != true || "${LINKER_OK}" != true ]]; then
		die "prerequisites are still not satisfied — see the messages above."
	fi
fi

# ── PATH: persist for future bash login shells, fix this session ──────────
if [[ ":${PATH}:" != *":${LOCAL_BIN}:"* ]]; then
	if grep -Eq '^[[:space:]]*export[[:space:]]+PATH=.*\.local/bin' "${HOME}/.profile" 2>/dev/null; then
		ok "${HOME}/.profile already exports ${LOCAL_BIN}."
	else
		# shellcheck disable=SC2016 # literal by design: expanded by the login shell that reads it
		printf '\n# added by zylaxion scripts/dev/bootstrap-ebpf.sh — bpf-linker lives here\nexport PATH="$HOME/.local/bin:$PATH"\n' >>"${HOME}/.profile"
		ok "persisted the PATH export to ${HOME}/.profile (bash login shells)"
		warn "fish/zsh login shells do not read ~/.profile — add that export line to your shell config yourself."
	fi
	PATH="${LOCAL_BIN}:${PATH}"
	export PATH
fi

# Functional sanity: the pin must be able to run rustc at all — this is
# the exact entry build.rs uses for the nested cross-build.
RUSTC_VERSION_REPORTED="$(rustup run "${EBPF_TOOLCHAIN}" rustc --version 2>/dev/null || echo "unknown")"
[[ "${RUSTC_VERSION_REPORTED}" != "unknown" ]] || die "rustup run ${EBPF_TOOLCHAIN} rustc --version failed — the toolchain is listed but broken."
ok "sanity: ${RUSTC_VERSION_REPORTED}"

ok "all eBPF build prerequisites are satisfied."
ok "bootstrap finished in ${SECONDS}s"
ok "next: change ebpf/src freely — the parent build cross-compiles it (bpf-linker must stay on PATH)"
