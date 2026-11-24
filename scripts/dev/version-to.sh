#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
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
#                                  generalized — sibling_scan() honors every
#                                  such manifest the find turns up; today
#                                  that is ebpf/, fuzz/, and
#                                  scripts/audit/harness/, all carrying
#                                  their own version = line, all mirrored
#                                  on every bump)
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
# the same invocation: the validated root build (cargo
# pro-linux-amd64-gnu --locked, whose build.rs drives the prerequisite
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
# night-ledger-1: the bump also CARRIES the release detail — the notes the
# cycle accumulated under "## [Unreleased]" move verbatim into the version's
# own section ("## [vX.Y.Z] — tagline — date", newest first), spliced
# directly UNDER the "## [Released]" ledger and ABOVE the running section
# (night-ledger-3 placement): everything released sits above
# "## [Unreleased]", so a shipped record is never read from the unreleased
# zone (the v80.0.6 trap: nine releases piled up there before this,
# v80.0.3 through v80.2.1 shipped with no section of their own). The
# tagline is optional
# (--tagline "...", the same words the release commit carries, and the
# commit message rides it too); the date is read from the system clock in
# UTC, never hardcoded. Probed honestly: a missing file, a missing anchor,
# an ambiguous ledger (more than one "## [Unreleased]" heading), or a
# "## [Released]" section without this tool's own shape warns and writes
# nothing — a ledger this script did not create is not one it rewrites.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${SCRIPT_DIR}"

# Colors — the shell brand palette (night-cafe-1): the RED/GREEN/
# YELLOW/NC usage sites map onto the tiers inside the palette — the
# same bytes the CLI renders, plain text when piped. The palette is
# in-repo (scripts/lib/): SCRIPT_DIR is the repo ROOT here (../..),
# so the sibling `../lib` form — the zelynic dev-script shape, where
# SCRIPT_DIR is the scripts/ dir — resolves OUTSIDE this checkout and
# breaks any layout without a sibling lib/ (the freeze-lts lesson).
# shellcheck disable=SC1091 source=scripts/lib/ux_palette.sh
. "${SCRIPT_DIR}/scripts/lib/ux_palette.sh"

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

# Write the release record to CHANGELOG.md (night-improve-9, carried by
# night-ledger-1). Two steps, each with its own honest verdict:
#   1. the ledger line — a "## [Released]" heading with the bare version
#      line under it, newest first (the owner's shape, created above
#      "## [Unreleased]" when absent);
#   2. the detail carry — the cycle's notes move verbatim out of
#      "## [Unreleased]" into the version's own section, so the release
#      record never stays filed under "Unreleased".
# Honest probes: a missing file, a missing "## [Unreleased]" anchor, an
# ambiguous ledger, or a "## [Released]" section without this tool's own
# shape warns and writes nothing — a ledger this script did not create is
# not one it rewrites. A failed ledger line also skips the carry: the
# record lands whole or not at all.
changelog_release() {
    local ver="v$1"
    local tagline="${2:-}"
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
            return 0
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
            return 0
        fi
    fi
    # The ledger line landed — now carry the release detail. The raw
    # version rides across: the carry adds its own "v" prefix.
    changelog_carry_detail "$1" "${tagline}"
}

# Carry the unreleased detail into the released section (night-ledger-1,
# placement fixed by night-ledger-3). The bare ledger line used to be the
# whole bump record: the cycle's notes stayed under "## [Unreleased]"
# release after release, and the owner read a shipped change from the
# wrong section. A bump is a release event — the notes the running
# section holds move verbatim into the version's own section:
# "## [vX.Y.Z] — tagline — date", newest first, spliced directly UNDER
# the "## [Released]" ledger and ABOVE the running section, so nothing
# released ever sits below "## [Unreleased]" (the old placement put the
# version's own section below the running notes — a shipped record filed
# in the unreleased zone, the exact trap the v80.0.6 bump hit). The
# running header stays behind for the fresh cycle; the date comes from
# the system clock in UTC (the same law the freeze record runs on),
# never hardcoded. Probes: an ambiguous ledger (more than one
# "## [Released]" or "## [Unreleased]" heading) writes nothing; an empty
# running section is the expected steady state after a clean carry, not
# a failure. Bare stray anchors heal first (the heal documents its own
# law below); only content-bearing strays stay ambiguous.

# night-release-2: the bare stray anchor heal. A "## [Released]" or
# "## [Unreleased]" heading whose whole body is blank lines is locally
# provable as litter — the v80.5.6 bump found one at the CHANGELOG tail
# (line 3648, riding since v80.5.5, stranded after the Frozen running
# notes archive section) and every carry bailed into "detail not
# carried" while the bump walked on to die at the stale lock. The heal
# retires bare strays before the honesty probe runs. The keep-one law:
# a content-bearing anchor keeps the post; when every anchor of the
# kind is bare, the first one keeps it (the probe still needs its
# single anchor). Two anchors that BOTH hold notes stay human work —
# that ambiguity the machine must not resolve by guess, so the probe
# below keeps refusing exactly as it always has.
heal_bare_ledger_anchors() {
    local kind scan tmp starts ends
    local total own_count content_bearing first_bare is_own i
    local -a s_lines s_own s_body
    for kind in Released Unreleased; do
        scan="$(awk -v kind="${kind}" '
            /^## / {
                if (hdr != "") print hdr, k, body
                hdr = NR
                k = ($0 == "## [" kind "]") ? "own" : "other"
                body = 0
                next
            }
            hdr != "" && $0 !~ /^[[:space:]]*$/ { body++ }
            END { if (hdr != "") print hdr, k, body }
        ' CHANGELOG.md)"
        total=0; own_count=0; content_bearing=0; first_bare=""
        s_lines=(); s_own=(); s_body=()
        while IFS=' ' read -r line s_kind s_body_count; do
            is_own=0
            [ "${s_kind}" = "own" ] && is_own=1
            s_lines+=("${line}"); s_own+=("${is_own}"); s_body+=("${s_body_count}")
            if [ "${is_own}" = "1" ]; then
                own_count=$((own_count + 1))
                if [ "${s_body_count}" -gt 0 ]; then
                    content_bearing=$((content_bearing + 1))
                elif [ -z "${first_bare}" ]; then
                    first_bare="${line}"
                fi
            fi
        done <<< "${scan}"
        total="${own_count}"
        [ "${total}" -gt 1 ] || continue
        [ "${content_bearing}" -lt 2 ] || continue
        starts=""
        ends=""
        total="$(wc -l < CHANGELOG.md)"
        for i in "${!s_lines[@]}"; do
            [ "${s_own[$i]}" = "1" ] || continue
            [ "${s_body[$i]}" = "0" ] || continue
            if [ "${content_bearing}" = "0" ] && [ "${s_lines[$i]}" = "${first_bare}" ]; then
                continue
            fi
            if [ $((i + 1)) -lt "${#s_lines[@]}" ]; then
                starts+="${s_lines[$i]} "
                ends+="$((s_lines[i + 1] - 1)) "
            else
                starts+="${s_lines[$i]} "
                ends+="${total} "
            fi
        done
        [ -n "${starts}" ] || continue
        tmp="$(mktemp)"
        ZYLAXION_HEAL_STARTS="${starts% }" ZYLAXION_HEAL_ENDS="${ends% }" awk '
            BEGIN {
                n = split(ENVIRON["ZYLAXION_HEAL_STARTS"], S, " ")
                m = split(ENVIRON["ZYLAXION_HEAL_ENDS"], E, " ")
                for (i = 1; i <= n && i <= m; i++) delstart[S[i]] = E[i]
            }
            skipping { if (NR <= skipto) next; skipping = 0 }
            NR in delstart { skipto = delstart[NR]; skipping = 1; next }
            { print }
        ' CHANGELOG.md >"${tmp}" && mv "${tmp}" CHANGELOG.md
        echo -e "  ${GREEN}OK${NC} CHANGELOG.md        → bare stray ## [${kind}] anchor(s) retired — the ledger heals to its one ${kind}"
    done
}

changelog_carry_detail() {
    heal_bare_ledger_anchors
    local ver="v$1"
    local tagline="${2:-}"
    local date_stamp
    date_stamp="$(date -u +%Y-%m-%d)"
    local heading
    if [ -n "${tagline}" ]; then
        heading="## [${ver}] — ${tagline} — ${date_stamp}"
    else
        heading="## [${ver}] — ${date_stamp}"
    fi
    local rcount ucount
    rcount="$(grep -c '^## \[Released\]$' CHANGELOG.md || true)"
    ucount="$(grep -c '^## \[Unreleased\]$' CHANGELOG.md || true)"
    if [ "${rcount}" != "1" ] || [ "${ucount}" != "1" ]; then
        echo -e "  ${YELLOW}!${NC} CHANGELOG.md        → ambiguous ledger (${rcount:-0} ## [Released], ${ucount:-0} ## [Unreleased] headings) — detail not carried"
        return 0
    fi
    # The running body: between the "## [Unreleased]" heading and the
    # next "## " heading, trimmed of its blank frame. A "## " line at
    # column zero reads as the boundary — the ledger prose keeps its
    # fences indented, the same shape every heading probe here trusts.
    local body
    body="$(awk '
        /^## \[Unreleased\]$/ { insec = 1; next }
        insec && /^## / { exit }
        insec { print }
    ' CHANGELOG.md | awk 'NF && !s { s = NR } NF { e = NR } { l[NR] = $0 } END { for (i = s; i <= e; i++) print l[i] }')"
    if [ -z "${body}" ]; then
        echo -e "  ${GREEN}OK${NC} CHANGELOG.md        → unreleased section already clean — nothing to carry"
        return 0
    fi
    # One pass, two splices: the carried section lands UNDER the
    # "## [Released]" ledger — after the heading, its spacer and the
    # bare version lines, before the first heading that follows (a
    # previous version section or "## [Unreleased]") — and the running
    # body drops out of "## [Unreleased]" (the header keeps one blank
    # spacer for the fresh cycle). Held blanks normalize to exactly one
    # blank line at every seam. Heading and body ride through the
    # environment: awk -v would eat backslash escapes, and prose
    # carries them.
    local tmp
    tmp="$(mktemp)"
    ZYLAXION_CARRY_HEADING="${heading}" ZYLAXION_CARRY_BODY="${body}" awk '
        BEGIN {
            h = ENVIRON["ZYLAXION_CARRY_HEADING"]
            b = ENVIRON["ZYLAXION_CARRY_BODY"]
        }
        /^## \[Released\]$/ && !rseen {
            rseen = 1
            print
            next
        }
        rseen && !rdone {
            if ($0 ~ /^## /) {
                print ""
                print h
                print ""
                print b
                print ""
                print
                if ($0 == "## [Unreleased]") useen = 1
                rdone = 1
                next
            }
            if ($0 ~ /^[ \t]*$/) {
                rpending = 1
                next
            }
            if (rpending) {
                print ""
                rpending = 0
            }
            print
            next
        }
        /^## \[Unreleased\]$/ && !useen {
            useen = 1
            print
            next
        }
        useen && !udone {
            if ($0 ~ /^## /) {
                print ""
                print
                udone = 1
                next
            }
            next
        }
        { print }
        END {
            if (rseen && !rdone) {
                print ""
                print h
                print ""
                print b
            }
            if (useen && !udone) {
                # The running section is the last section in the file:
                # the dropped body leaves the header alone at EOF - close
                # it with the one blank spacer the fresh cycle rides on.
                print ""
            }
        }
    ' CHANGELOG.md >"${tmp}"
    mv "${tmp}" CHANGELOG.md
    echo -e "  ${GREEN}OK${NC} CHANGELOG.md        → detail carried under ## [Released] as ## [${ver}] (${date_stamp})"
}

# Sync Cargo.lock to the bumped package (night-improve-9: the old
# six-member workspace materialized EVERY member's version into the
# lock — the root-only sed left stale entries for the next build to
# rewrite, dirtying the tree; night-improve-21 collapsed the members
# into one package, and the contract is the same for the root entry).
# cargo is the authority on its own lock format: `cargo update
# --workspace` moves the package version and stays frozen on
# third-party deps. Verified per entry name afterwards; a verification
# miss fails the bump at the honest wall (the caller owns the FAIL
# line) instead of letting a stale lock ambush the --locked builds
# downstream.
#
# night-release-2: the resync runs at two speeds. The weekly dep
# update rides CI — its Cargo.lock-only commit carries versions the
# pulling machine's registry index has never fetched, so the offline
# resync dies on resolution ("location searched: crates.io index";
# the v80.5.6 bump hit exactly that behind crc32fast 1.5.5 → 1.5.6)
# and the stale lock then ambushed both the lane refresh and
# install-on-my-machine with "cannot update the lock file ...
# because --locked was passed". The heal: offline first (the
# reproducibility door), then one named online pass — the registry
# is the source of truth for a version the lock already pins. Both
# speeds failing returns non-zero; the callers stop the bump with
# the tree's recovery named, before any --locked build can die.
cargo_resync_heal() {
    local where="${1:-root}"
    if cargo update --workspace --offline > /dev/null 2>&1; then
        return 0
    fi
    echo -e "  ${YELLOW}..${NC} Cargo.lock (${where})  → offline resync blocked (a cold index entry behind a lock-only dep bump) — one online pass warms the registry cache"
    if cargo update --workspace > /dev/null 2>&1; then
        return 0
    fi
    return 1
}

lock_sync_and_verify() {
    local target="$1"
    if ! command -v cargo > /dev/null 2>&1; then
        echo -e "  ${YELLOW}!${NC} Cargo.lock          → cargo not on PATH — run 'cargo update --workspace' manually"
        return 0
    fi
    cargo_resync_heal "root" || return 1
    local member entry_ver all_ok=1
    while IFS= read -r member; do
        entry_ver="$(grep -A1 "^name = \"${member}\"$" Cargo.lock | grep '^version = "' | head -1 | sed 's/^version = "\(.*\)"/\1/')"
        if [ "${entry_ver}" = "${target}" ]; then
            echo -e "  ${GREEN}OK${NC} Cargo.lock          → ${member} version = ${target}"
        else
            echo -e "  ${YELLOW}!${NC} Cargo.lock          → ${member}: expected ${target}, got ${entry_ver:-absent}"
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

# --- Sync each touched sibling's OWN Cargo.lock (night-improve-27) ---
# A sibling that carries its own version line can also carry its own
# DETACHED workspace lock (the ebpf lane does). A stale sibling lock
# fails every --locked build of that lane — the nested eBPF build
# refuses to rewrite the lock, which is exactly how the v80.0.0-alpha.1
# rehearsal died ("cannot update the lock file ... because --locked was
# passed"). cargo stays the authority on its own lock format, per lane;
# the rust-toolchain.toml beside the lock resolves the toolchain.
# night-release-2: the resync rides the same two-speed heal as the
# root lock, and a sibling that cannot resync at either speed stops
# the bump at the honest wall — a stale lane lock ships a CI failure,
# never a release.
sibling_lock_sync() {
    local manifest="$1"
    local dir name entry_ver
    dir="$(dirname "${manifest}")"
    [ -f "${dir}/Cargo.lock" ] || { echo -e "  ${YELLOW}!${NC} ${dir}/Cargo.lock  → absent, nothing to sync"; return 0; }
    name="$(grep -m1 '^name = ' "${manifest}" | sed 's/.*"\(.*\)".*/\1/')"
    if (cd "${dir}" && cargo_resync_heal "${dir}"); then
        entry_ver="$(grep -A1 "^name = \"${name}\"$" "${dir}/Cargo.lock" | grep '^version = "' | head -1 | sed 's/^version = "\(.*\)"/\1/')"
        if [ "${entry_ver}" = "${NEW_VERSION}" ]; then
            echo -e "  ${GREEN}OK${NC} ${dir}/Cargo.lock  → ${name} version = ${NEW_VERSION}"
        else
            echo -e "  ${YELLOW}!${NC} ${dir}/Cargo.lock  → ${name}: expected ${NEW_VERSION}, got ${entry_ver:-absent}"
        fi
    else
        echo -e "  ${RED}FAIL${NC} ${dir}/Cargo.lock  → resync impossible in ${dir}/ offline and online — the bump stops here ('git checkout -- .' discards the half-bump)"
        exit 1
    fi
}

# Show current version and exit
if [ $# -eq 0 ]; then
    echo -e "Current version: ${GREEN}v$(current_version)${NC}"
    echo ""
    echo "Usage: ./scripts/dev/version-to.sh v<VERSION> [--commit] [--tagline \"...\"]"
    exit 0
fi

NEW_VERSION="${1#v}"
shift

COMMIT=false
TAGLINE=""
while [ $# -gt 0 ]; do
    case "${1}" in
    --commit | -c) COMMIT=true ;;
    --tagline)
        if [ $# -lt 2 ]; then
            echo -e "${RED}Error: --tagline needs a value (the words the release commit carries)${NC}" >&2
            exit 1
        fi
        TAGLINE="${2}"
        shift
        ;;
    *)
        echo -e "${RED}Unknown option: ${1}${NC}" >&2
        exit 1
        ;;
    esac
    shift
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
    # night-release-2: the re-run is a recovery door, not a shrug. The
    # v80.5.6 bump died at the resync, the owner re-ran, and this
    # branch answered "nothing to change" while the tree stayed
    # half-bumped — manifests at target, locks stale, lane pin stale:
    # the exact wreckage the recovery had to heal. A re-run now runs
    # the same healing legs the fresh bump runs: locks resync at two
    # speeds, the ledger carries what it refused when the run died
    # (bare stray anchors heal first), the lane refreshes when its pin
    # is stale. An honest completed bump re-runs to a quiet no-op — an
    # empty running section carries nothing, a fresh lane passes
    # parity untouched.
    echo -e "${YELLOW}Already at v${NEW_VERSION} (root + all own-version siblings) — healing any half-bumped tree, not just reporting.${NC}"
    if [ -f Cargo.lock ]; then
        lock_sync_and_verify "${NEW_VERSION}" || {
            echo -e "  ${RED}FAIL${NC} Cargo.lock          → could not be synced to v${NEW_VERSION} offline and online — the bump stops here ('git checkout -- .' discards the half-bump)"
            exit 1
        }
    fi
    while IFS=$'\t' read -r f n v; do
        [ -z "${f}" ] && continue
        sibling_lock_sync "${f}"
    done <<< "${SIBLINGS}"
    changelog_carry_detail "${NEW_VERSION}" "${TAGLINE}"
    if ! bash scripts/gates/check-prebuilt-parity.sh >/dev/null 2>&1; then
        echo -e "  ${YELLOW}..${NC} ebpf-prebuilt/     → lane pin stale on the half-bumped tree — auto-refreshing through the validated build pipeline"
        if cargo pro-linux-amd64-gnu --locked \
            && bash scripts/dev/regenerate-ebpf-prebuilt.sh > /dev/null \
            && bash scripts/gates/check-prebuilt-parity.sh; then
            echo -e "  ${GREEN}OK${NC} ebpf-prebuilt/     → lane regenerated in parity (commit ./ebpf/ + ./ebpf-prebuilt/ together)"
        else
            echo -e "  ${YELLOW}!${NC} ebpf-prebuilt/     → the refresh did not complete — finish by hand and commit the lane with its sources:"
            echo "    cargo pro-linux-amd64-gnu --locked && ./scripts/dev/regenerate-ebpf-prebuilt.sh"
        fi
    fi
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
# night-release-2: a failed sync is a stop, not a warning — the stale
# lock's real death came later (the --locked lane refresh and
# install-on-my-machine), which is the ambush the bump now refuses to
# arrange.
if [ -f Cargo.lock ]; then
    lock_sync_and_verify "${NEW_VERSION}" || {
        echo -e "  ${RED}FAIL${NC} Cargo.lock          → could not be synced to v${NEW_VERSION} offline and online — the bump stops here ('git checkout -- .' discards the half-bump, fix the registry cause, re-run)"
        exit 1
    }
fi

# --- Sync each touched sibling's OWN Cargo.lock (night-improve-27) ---
# (the sibling_lock_sync implementation lives beside
# lock_sync_and_verify, where the ALREADY-branch recovery door calls
# it too; the per-touched-sibling walk stays with the bump flow)
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
    # The literal-version law (night-dragon-maintenance-3): swap the
    # exact OLD version string for the NEW one — no regex guesses at
    # suffix shapes. The retired pattern carried a hardcoded
    # (alpha|beta|rc) channel list, and its night-dinner-7 ancestor
    # (-(alpha|beta|rc).N)? existed because the former
    # (-[A-Za-z0-9.]+)? ate the first -token- after the semver — the
    # -linux- platform segment vanished from the rc.1 README URLs.
    # A literal swap handles ANY suffix — beta.1, rc.9, example.1 —
    # because the version is KNOWN at bump time, not patterned, and
    # URLs pinned to OLDER releases keep their own history.
    sed -i "s|releases/download/v${CURRENT}|releases/download/v${NEW_VERSION}|g" README.md
    sed -i "s|zylaxion-v${CURRENT}-|zylaxion-v${NEW_VERSION}-|g" README.md
    sed -i "s|Version: v.*|Version: v${NEW_VERSION}|" README.md
    echo -e "  ${GREEN}OK${NC} README.md           → v${NEW_VERSION} (release URLs + versioned examples)"
else
    echo -e "  ${YELLOW}!${NC} README.md           → no version surface found (no release URL / versioned example / Version line) — nothing to rewrite"
fi

# --- Update CHANGELOG.md (night-improve-9: the ledger line; night-ledger-1: the detail carry) ---
changelog_release "${NEW_VERSION}" "${TAGLINE}"

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
    if cargo pro-linux-amd64-gnu --locked \
        && bash scripts/dev/regenerate-ebpf-prebuilt.sh > /dev/null \
        && bash scripts/gates/check-prebuilt-parity.sh; then
        echo -e "  ${GREEN}OK${NC} ebpf-prebuilt/     → lane regenerated in parity (commit ./ebpf/ + ./ebpf-prebuilt/ together)"
    else
        echo -e "${RED}FAIL:${NC} the prebuilt refresh did not complete — the tree is NOT committable in this state."
        echo -e "  the refresh drives the eBPF build; its once-per-machine prerequisites:"
        echo -e "    ./scripts/dev/bootstrap-ebpf.sh"
        echo -e "  then finish the bump manually and commit the lane with its sources:"
        echo -e "    cargo pro-linux-amd64-gnu --locked && ./scripts/dev/regenerate-ebpf-prebuilt.sh"
        echo -e "    git add ./ebpf/Cargo.toml ./ebpf/Cargo.lock ./ebpf-prebuilt/ && git commit"
        exit 1
    fi
elif bash scripts/gates/check-prebuilt-parity.sh >/dev/null 2>&1; then
    echo -e "  ${GREEN}OK${NC} ebpf-prebuilt/     → parity green (the lane pins the live ebpf/ tree)"
else
    echo -e "  ${YELLOW}!${NC} ebpf-prebuilt/     → STALE: ebpf/ changed after the lane was generated (not by this run)"
    echo -e "     run 'cargo pro-linux-amd64-gnu --locked && ./scripts/dev/regenerate-ebpf-prebuilt.sh' and commit ./ebpf/ + ./ebpf-prebuilt/ together (the parity gate fails the commit until the refresh lands)"
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
        echo -e "  1. cargo pro-linux-amd64-gnu --locked && ./scripts/dev/regenerate-ebpf-prebuilt.sh"
        echo -e "  2. git add ${STAGE_FILES[*]} ./ebpf/Cargo.toml ./ebpf/Cargo.lock ./ebpf-prebuilt/"
        echo -e "  3. git commit -m \"release: v${NEW_VERSION}\""
        exit 1
    fi
    for f in "${STAGE_FILES[@]}"; do
        # -e, not -f: the stage set can carry ./ebpf-prebuilt/, a directory.
        [ -e "$f" ] && git add "$f"
    done
    # The tagline rides the commit subject (the owner's release shape:
    # "release: v80.2.1 growth naturally"), so the amend-after-bump the
    # tagline used to need is gone.
    if [ -n "${TAGLINE}" ]; then
        git commit -m "release: v${NEW_VERSION} ${TAGLINE}"
        echo -e "${GREEN}OK Committed: release: v${NEW_VERSION} ${TAGLINE}${NC}"
    else
        git commit -m "release: v${NEW_VERSION}"
        echo -e "${GREEN}OK Committed: release: v${NEW_VERSION}${NC}"
    fi
fi
