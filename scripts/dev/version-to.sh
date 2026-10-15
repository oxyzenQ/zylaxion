#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux sound keyboard; no other OS is supported.
# =============================================================================
# ZYLAXION VERSION MANAGER
# =============================================================================
# Centralized version management — update ALL files from a single command.
#
# Usage:
#   ./scripts/dev/version-to.sh v11.0.1          # Update to v11.0.1
#   ./scripts/dev/version-to.sh v11.1.0 --commit # Update and auto-commit
#   ./scripts/dev/version-to.sh                  # Show current version
#
# Single source of truth: the root Cargo.toml [package] version
# (night-improve-21: one package, the cosmic dragon unification).
# Files updated:
#   Cargo.toml                     (the source of truth itself)
#   Cargo.lock                     (every workspace member entry, cargo-synced)
#   CHANGELOG.md                   (night-improve-9: the release ledger line)
#   any sibling Cargo.toml that carries its OWN version line
#                                  (night-improve-9: the zelynic ebpf lane,
#                                  generalized — zylaxion has no such manifest
#                                  today, all members inherit
#                                  version.workspace = true; the scanner
#                                  honors one the day it appears)
# Files auto-derived: scripts/build.sh (reads from Cargo.toml), the binary
# (env!("CARGO_PKG_VERSION")). README.md is probed for a version surface and
# reported honestly either way (the zelynic NIGHT-hunt-32 rule: never claim a
# rewrite that did not happen — since night-dinner-7 the README carries the
# self-resolving download lanes and zero hardcoded versions, so the probe's
# honest no-surface verdict is the expected steady state here).
# The prebuilt lane rides the bump (night-dinner-8, the zelynic
# NIGHT-improve-70 shape): the ebpf crate's version is part of the tree
# ebpf-prebuilt/manifest.toml pins, so THIS script's own sibling rewrite
# stales the lane by construction — and when it does, the refresh runs in
# the same invocation: the validated root build (cargo build --release
# --locked --features ebpf, whose build.rs drives the prerequisite
# preflight, the nested nightly cross-compile and the structural ELF
# validation), then the one-writer regenerate, then the parity gate as the
# fail-closed verdict. One command leaves a committable tree; a refresh
# that cannot complete fails closed with the bootstrap recipe. A lane
# already stale BEFORE the bump keeps the honest manual verdict: this
# script auto-refreshes only the churn it created, never a WIP it found.
# The changelog ledger (night-improve-9, the zelynic NIGHT-improve-76 shape):
# a bump is a release event — the ledger records it the same way the Cargo
# files do: a "## [Released]" heading with the bare version line under it,
# newest first, pre-release and stable alike, above "## [Unreleased]".
# Probed honestly: a missing file, a missing anchor, or a "## [Released]"
# section without this tool's own shape warns and writes nothing — a ledger
# this script did not create is not one it rewrites.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${SCRIPT_DIR}"

# Colors
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[0;33m'
readonly NC='\033[0m'

# Read current version from the root Cargo.toml (the package version).
current_version() {
    grep -m1 '^version = ' Cargo.toml | sed 's/.*"\(.*\)".*/\1/'
}

# Scan non-root manifests that carry their OWN version line. A
# workspace-inherit member (version.workspace = true) has none and is
# correctly absent here — its version moves with the root automatically.
sibling_scan() {
    find . -name Cargo.toml -not -path './target/*' -not -path './.git/*' \
        -not -path './Cargo.toml' -print0 |
        while IFS= read -r -d '' f; do
            if grep -q '^version = ' "$f"; then
                local n v
                n="$(grep -m1 '^name = ' "$f" | sed 's/.*"\(.*\)".*/\1/')"
                v="$(grep -m1 '^version = ' "$f" | sed 's/.*"\(.*\)".*/\1/')"
                printf '%s\t%s\t%s\n' "$f" "$n" "$v"
            fi
        done
}

# Validate semver format (pre-release labels allowed: v11.1.0-rc.1 is
# the owner's shape; the old vMAJOR.MINOR.PATCH-only regex would have
# rejected the family's own beta/rc convention).
validate_version() {
    local ver="$1"
    if ! echo "${ver}" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
        echo -e "${RED}Error: Invalid version '${ver}'. Expected format: X.Y.Z or X.Y.Z-label${NC}" >&2
        exit 1
    fi
}

# Write the release line to CHANGELOG.md (night-improve-9). The ledger
# shape is the owner's: a "## [Released]" heading with the bare version
# line under it — newest first, created above "## [Unreleased]" when
# absent. Honest probes: a missing file, a missing "## [Unreleased]"
# anchor, or a "## [Released]" section without this tool's own shape
# warns and writes nothing.
changelog_release() {
    local ver="v$1"
    if [ ! -f CHANGELOG.md ]; then
        echo -e "  ${YELLOW}!${NC} CHANGELOG.md        → file absent, nothing to write"
        return 0
    fi
    # A version line never repeats: the re-run at a version the ledger
    # already carries (the usual stale-tree re-entry) writes nothing.
    if grep -qxF "${ver}" CHANGELOG.md; then
        echo -e "  ${YELLOW}!${NC} CHANGELOG.md        → ${ver} already recorded, nothing to write"
        return 0
    fi
    local tmp
    tmp="$(mktemp)"
    if grep -q '^## \[Released\]$' CHANGELOG.md; then
        # The section exists. This tool's own shape puts one blank
        # spacer between the heading and the version lines — the new
        # version splices in under it, above the current top line.
        if [ -z "$(sed -n '/^## \[Released\]$/{n;p;}' CHANGELOG.md)" ]; then
            awk -v ver="${ver}" '
                /^## \[Released\]$/ && !done {
                    print
                    getline spacer
                    print spacer
                    print ver
                    done = 1
                    next
                }
                { print }
            ' CHANGELOG.md >"${tmp}"
            mv "${tmp}" CHANGELOG.md
            echo -e "  ${GREEN}OK${NC} CHANGELOG.md        → ${ver} (## [Released], newest first)"
        else
            rm -f "${tmp}"
            echo -e "  ${YELLOW}!${NC} CHANGELOG.md        → unexpected ## [Released] shape (no blank spacer under the heading) — nothing to write"
        fi
    else
        # No section yet — create it above "## [Unreleased]" (the
        # unreleased work keeps its own section below the ledger).
        if grep -q '^## \[Unreleased\]$' CHANGELOG.md; then
            awk -v ver="${ver}" '
                /^## \[Unreleased\]$/ && !done {
                    print "## [Released]"
                    print ""
                    print ver
                    print ""
                    done = 1
                }
                { print }
            ' CHANGELOG.md >"${tmp}"
            mv "${tmp}" CHANGELOG.md
            echo -e "  ${GREEN}OK${NC} CHANGELOG.md        → ${ver} (section created above ## [Unreleased])"
        else
            rm -f "${tmp}"
            echo -e "  ${YELLOW}!${NC} CHANGELOG.md        → no ## [Unreleased] anchor found — nothing to write"
        fi
    fi
}

# Sync Cargo.lock to the bumped package (night-improve-9: the old
# six-member workspace materialized EVERY member's version into the
# lock — the root-only sed left stale entries for the next build to
# rewrite, dirtying the tree; night-improve-21 collapsed the members
# into one package, and the contract is the same for the root entry).
# cargo is the authority on its own lock format: `cargo update
# --workspace --offline` moves the package version and stays frozen on
# third-party deps. Verified per entry name afterwards; a verification
# miss warns honestly instead of failing the whole bump.
lock_sync_and_verify() {
    local target="$1"
    if ! command -v cargo > /dev/null 2>&1; then
        echo -e "  ${YELLOW}!${NC} Cargo.lock          → cargo not on PATH — run 'cargo update --workspace --offline' manually"
        return 0
    fi
    if ! cargo update --workspace --offline > /dev/null 2>&1; then
        echo -e "  ${YELLOW}!${NC} Cargo.lock          → cargo update --workspace --offline failed — run it manually to resync"
        return 0
    fi
    local member entry_ver all_ok=1
    while IFS= read -r member; do
        entry_ver="$(grep -A1 "^name = \"${member}\"$" Cargo.lock | grep '^version = "' | head -1 | sed 's/^version = "\(.*\)"/\1/')"
        if [ "${entry_ver}" = "${target}" ]; then
            echo -e "  ${GREEN}OK${NC} Cargo.lock          → ${member} version = ${target}"
        else
            echo -e "  ${YELLOW}!${NC} Cargo.lock          → ${member}: expected ${target}, got ${entry_ver:-absent} (run 'cargo update --workspace --offline' to fix)"
            all_ok=0
        fi
    done < <(if [ -d crates ]; then
        find crates -maxdepth 2 -name Cargo.toml -print0 |
            xargs -0 -I{} sh -c 'grep -m1 "^name = " "{}" | sed "s/.*\"\(.*\)\".*/\1/"'
    else
        # night-improve-21: no member manifests — verify the root
        # entry through the same manifest-path pipeline.
        printf './Cargo.toml\0'
    fi |
        xargs -0 -r grep -h -m1 '^name = ' |
        sed 's/name = "\(.*\)"/\1/')
    [ "${all_ok}" = "1" ]
}

# Show current version and exit
if [ $# -eq 0 ]; then
    echo -e "Current version: ${GREEN}v$(current_version)${NC}"
    echo ""
    echo "Usage: ./scripts/dev/version-to.sh v<VERSION> [--commit]"
    exit 0
fi

NEW_VERSION="${1#v}"
shift

COMMIT=false
for arg in "$@"; do
    case "${arg}" in
    --commit | -c) COMMIT=true ;;
    *)
        echo -e "${RED}Unknown option: ${arg}${NC}" >&2
        exit 1
        ;;
    esac
done

validate_version "${NEW_VERSION}"
CURRENT=$(current_version)

# Sibling manifests with their own versions (the zelynic ebpf lane,
# generalized). Empty today — the report line proves the scan ran.
SIBLINGS="$(sibling_scan)"
ALREADY=true
if [ "${CURRENT}" != "${NEW_VERSION}" ]; then
    ALREADY=false
else
    while IFS=$'\t' read -r f n v; do
        [ -z "$f" ] && continue
        if [ "${v}" != "${NEW_VERSION}" ]; then
            ALREADY=false
            break
        fi
    done <<< "${SIBLINGS}"
fi

if [ "${ALREADY}" = "true" ]; then
    echo -e "${YELLOW}Already at v${NEW_VERSION} (root + all own-version siblings), nothing to change.${NC}"
    exit 0
fi

echo -e "Updating version: ${YELLOW}v${CURRENT}${NC} → ${GREEN}v${NEW_VERSION}${NC}"
echo ""

# --- Update root Cargo.toml (source of truth) ---
# Only the [workspace.package] version line, never dependency versions.
if [ "${CURRENT}" = "${NEW_VERSION}" ]; then
    echo -e "  ${YELLOW}!${NC} Cargo.toml          → already at v${NEW_VERSION} (a sibling lane alone drifted)"
else
    sed -i "0,/^version = \".*\"/s//version = \"${NEW_VERSION}\"/" Cargo.toml
    echo -e "  ${GREEN}OK${NC} Cargo.toml          → ${NEW_VERSION}"
fi

# --- Sibling manifests that carry their own version (night-improve-9) ---
# The zelynic script bumps ebpf/Cargo.toml alongside the root; the
# zylaxion generalization honors any manifest with its own version
# line, wherever a future lane appears. Members inheriting
# version.workspace = true need (and get) nothing here.
SIBLING_TOUCHED=()
if [ -z "${SIBLINGS}" ]; then
    echo -e "  ${GREEN}OK${NC} sibling manifests   → none carry own versions (members inherit workspace.package)"
else
    while IFS=$'\t' read -r f n v; do
        [ -z "$f" ] && continue
        if [ "${v}" != "${NEW_VERSION}" ]; then
            sed -i "0,/^version = \".*\"/s//version = \"${NEW_VERSION}\"/" "$f"
            echo -e "  ${GREEN}OK${NC} ${f} → ${NEW_VERSION}"
            SIBLING_TOUCHED+=("${f}")
        else
            echo -e "  ${YELLOW}!${NC} ${f} → already at v${NEW_VERSION}"
        fi
    done <<< "${SIBLINGS}"
fi

# --- Sync + verify Cargo.lock (every workspace member entry) ---
if [ -f Cargo.lock ]; then
    lock_sync_and_verify "${NEW_VERSION}" || true
fi

# --- Sync each touched sibling's OWN Cargo.lock (night-improve-27) ---
# A sibling that carries its own version line can also carry its own
# DETACHED workspace lock (the ebpf lane does). A stale sibling lock
# fails every --locked build of that lane — the nested eBPF build
# refuses to rewrite the lock, which is exactly how the v80.0.0-alpha.1
# rehearsal died ("cannot update the lock file ... because --locked was
# passed"). cargo stays the authority on its own lock format, per lane;
# the rust-toolchain.toml beside the lock resolves the toolchain.
sibling_lock_sync() {
    local manifest="$1"
    local dir name entry_ver
    dir="$(dirname "${manifest}")"
    [ -f "${dir}/Cargo.lock" ] || { echo -e "  ${YELLOW}!${NC} ${dir}/Cargo.lock  → absent, nothing to sync"; return 0; }
    name="$(grep -m1 '^name = ' "${manifest}" | sed 's/.*"\(.*\)".*/\1/')"
    if (cd "${dir}" && cargo update --workspace --offline > /dev/null 2>&1); then
        entry_ver="$(grep -A1 "^name = \"${name}\"$" "${dir}/Cargo.lock" | grep '^version = "' | head -1 | sed 's/^version = "\(.*\)"/\1/')"
        if [ "${entry_ver}" = "${NEW_VERSION}" ]; then
            echo -e "  ${GREEN}OK${NC} ${dir}/Cargo.lock  → ${name} version = ${NEW_VERSION}"
        else
            echo -e "  ${YELLOW}!${NC} ${dir}/Cargo.lock  → ${name}: expected ${NEW_VERSION}, got ${entry_ver:-absent}"
        fi
    else
        echo -e "  ${YELLOW}!${NC} ${dir}/Cargo.lock  → cargo update --workspace --offline failed in ${dir}/ — run it manually to resync"
    fi
}
SIBLING_LOCKS=()
if [ "${#SIBLING_TOUCHED[@]}" -gt 0 ]; then
    for f in "${SIBLING_TOUCHED[@]}"; do
        [ -n "$f" ] || continue
        sibling_lock_sync "$f"
        dir="$(dirname "$f")"
        if [ -f "${dir}/Cargo.lock" ]; then
            SIBLING_LOCKS+=("${dir}/Cargo.lock")
        fi
    done
fi

# --- Update README.md (probed first — the zelynic NIGHT-hunt-32 rule) ---
README_HITS="$(grep -cE 'releases/download/v[0-9]|zylaxion-v[0-9]+\.[0-9]+|^Version: v[0-9]' README.md 2>/dev/null || true)"
if [ "${README_HITS:-0}" -gt 0 ]; then
    sed -i -E "s|releases/download/v[0-9]+\.[0-9]+\.[0-9]+|releases/download/v${NEW_VERSION}|g" README.md
    # night-dinner-7: the optional group strips KNOWN prerelease
    # suffixes only. The former (-[A-Za-z0-9.]+)? ate the first
    # -token- after the semver — that is how the -linux- platform
    # segment vanished from the rc.1 README URLs at the bump.
    sed -i -E "s#zylaxion-v[0-9]+\.[0-9]+\.[0-9]+(-(alpha|beta|rc)\.[0-9]+)?-#zylaxion-v${NEW_VERSION}-#g" README.md
    sed -i "s|Version: v.*|Version: v${NEW_VERSION}|" README.md
    echo -e "  ${GREEN}OK${NC} README.md           → v${NEW_VERSION} (release URLs + versioned examples)"
else
    echo -e "  ${YELLOW}!${NC} README.md           → no version surface found (no release URL / versioned example / Version line) — nothing to rewrite"
fi

# --- Update CHANGELOG.md (night-improve-9: the release ledger line) ---
changelog_release "${NEW_VERSION}"

# --- scripts/build.sh reads dynamically from Cargo.toml, no update needed ---
echo -e "  ${GREEN}OK${NC} scripts/build.sh    → auto (reads from Cargo.toml)"

# --- Binary reads from Cargo.toml via env!("CARGO_PKG_VERSION"), no update needed ---
echo -e "  ${GREEN}OK${NC} Binary (zylaxion)   → auto (reads from Cargo.toml)"

# --- Prebuilt-lane refresh + parity verdict (night-dinner-8, the zelynic ---
# --- NIGHT-improve-70 shape) ---
# The gate is the one implementation of the verdict; this script only
# READS it (the anti-drift discipline the gate's header documents).
# When THIS script's own rewrite touched ./ebpf/Cargo.toml, the refresh
# runs in the same invocation so one command leaves a committable tree —
# the manual two-step (bump, failed commit, refresh, re-commit) was the
# exact friction the owner refused to keep typing. A lane stale for
# reasons this run did not create keeps the honest manual verdict.
EBPF_CHANGED=false
for f in "${SIBLING_TOUCHED[@]:-}"; do
    [ "$f" = "./ebpf/Cargo.toml" ] && EBPF_CHANGED=true
done
if [ "${EBPF_CHANGED}" = "true" ] && ! bash scripts/gates/check-prebuilt-parity.sh >/dev/null 2>&1; then
    echo -e "  ${YELLOW}..${NC} ebpf-prebuilt/     → the version rewrite staled the lane's tree pin — auto-refreshing through the validated build pipeline"
    if cargo build --release --locked --features ebpf \
        && bash scripts/dev/regenerate-ebpf-prebuilt.sh > /dev/null \
        && bash scripts/gates/check-prebuilt-parity.sh; then
        echo -e "  ${GREEN}OK${NC} ebpf-prebuilt/     → lane regenerated in parity (commit ./ebpf/ + ./ebpf-prebuilt/ together)"
    else
        echo -e "${RED}FAIL:${NC} the prebuilt refresh did not complete — the tree is NOT committable in this state."
        echo -e "  the refresh drives the eBPF build; its once-per-machine prerequisites:"
        echo -e "    ./scripts/dev/bootstrap-ebpf.sh"
        echo -e "  then finish the bump manually and commit the lane with its sources:"
        echo -e "    cargo build --release --locked --features ebpf && ./scripts/dev/regenerate-ebpf-prebuilt.sh"
        echo -e "    git add ./ebpf/Cargo.toml ./ebpf/Cargo.lock ./ebpf-prebuilt/ && git commit"
        exit 1
    fi
elif bash scripts/gates/check-prebuilt-parity.sh >/dev/null 2>&1; then
    echo -e "  ${GREEN}OK${NC} ebpf-prebuilt/     → parity green (the lane pins the live ebpf/ tree)"
else
    echo -e "  ${YELLOW}!${NC} ebpf-prebuilt/     → STALE: ebpf/ changed after the lane was generated (not by this run)"
    echo -e "     run 'cargo build --release --locked --features ebpf && ./scripts/dev/regenerate-ebpf-prebuilt.sh' and commit ./ebpf/ + ./ebpf-prebuilt/ together (the parity gate fails the commit until the refresh lands)"
fi

echo ""
echo -e "${GREEN}Version updated to v${NEW_VERSION}${NC}"
# The staging hint names every file the run actually touched — the
# night-improve-27 rule: a version bump that touches a detached
# sibling's manifest AND lock must never hint a partial `git add`.
STAGE_FILES=(Cargo.toml Cargo.lock CHANGELOG.md)
[ "${README_HITS:-0}" -gt 0 ] && STAGE_FILES+=(README.md)
for f in "${SIBLING_TOUCHED[@]:-}"; do
    [ -n "$f" ] && STAGE_FILES+=("$f")
done
for f in "${SIBLING_LOCKS[@]:-}"; do
    [ -n "$f" ] && STAGE_FILES+=("$f")
done
# The lane rides the bump it staled (night-dinner-8): whenever this run
# touched the ebpf sibling, the refreshed lane is part of the commit set.
[ "${EBPF_CHANGED}" = "true" ] && STAGE_FILES+=("./ebpf-prebuilt/")
echo "next: git add ${STAGE_FILES[*]} && git commit"

if [ "${COMMIT}" = "true" ]; then
    # Fail closed (night-dinner-8): a commit that stages the ebpf version
    # bump WITHOUT the refreshed lane would ride a stale tree pin straight
    # into CI's parity gate — refuse and name the refresh. With the
    # auto-refresh above, this refusal is reachable only when the lane was
    # stale for reasons this run did not create (that shape already failed
    # closed in the refresh block for its own verdict).
    if ! bash scripts/gates/check-prebuilt-parity.sh >/dev/null 2>&1; then
        echo -e "${RED}FAIL:${NC} refusing to commit — the prebuilt lane is stale (ebpf/ changed, ebpf-prebuilt/ has not)."
        echo -e "  1. cargo build --release --locked --features ebpf && ./scripts/dev/regenerate-ebpf-prebuilt.sh"
        echo -e "  2. git add ${STAGE_FILES[*]} ./ebpf/Cargo.toml ./ebpf/Cargo.lock ./ebpf-prebuilt/"
        echo -e "  3. git commit -m \"release: v${NEW_VERSION}\""
        exit 1
    fi
    for f in "${STAGE_FILES[@]}"; do
        # -e, not -f: the stage set can carry ./ebpf-prebuilt/, a directory.
        [ -e "$f" ] && git add "$f"
    done
    git commit -m "release: v${NEW_VERSION}"
    echo -e "${GREEN}OK Committed: release: v${NEW_VERSION}${NC}"
fi
