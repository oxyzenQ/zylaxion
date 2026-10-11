#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux sound keyboard; no other OS is supported.
#
# ZYLAXION COMMIT-TIME ACTIONS-PIN HEALTH CHECK (night-improve-8)
#
# The actions-pin health contract, the zelynic engine's final form
# sized to this repo. Two seats, one engine:
#   - on-demand, maintainer seat: THIS SCRIPT — --dry-run to read the
#     verdict (the default), --apply to heal. Healed pins are committed
#     like any other change and the next read reports them current.
#   - advisory reads: warn mode (the default) prints the classified
#     table and exits 0; strict blocks on a KNOWN-stale verdict only;
#     auto re-verifies cold, heals minor/patch drift itself, stages
#     .github/workflows/ and fails once for review. MAJOR moves are a
#     human call — reported, never auto-healed.
#
# Version truth: GitHub releases/latest (stable releases only), with
# the newest semver tag as the fallback for release-less repos. The
# pin's identity is the SHA: a pin sitting on the latest release
# commit is CURRENT in every form its comment token takes.
#
# Honesty rules (the zelynic lineage): a verdict that cannot be
# reached (offline, rate limit, missing tool, overrun budget,
# unparsable answer) is an honest SKIP — never a fake "current", and
# never a block. Only a fully parsed behind-verdict can block
# (strict) or heal (auto). Pin freshness is a REMOTE fact; a gate
# that blocks a commit on facts it cannot reach would be a dishonest
# gate.
#
# Speed contract: a read must not wait on the network twice. The
# verdict is cached under $GIT_DIR/zylaxion/actions-pins/ keyed on
# the workflows' content hash (any workflow edit re-checks) with a
# TTL (upstream releases re-check even without edits). A cached
# non-current verdict re-verifies COLD before it can block or heal —
# upstream truth can move under a TTL-old cache. A cold sweep runs
# under a per-call timeout budget; a cached hit costs zero API calls.
#
# Tooling delta (documented, zelynic parity in contract not tooling):
# JSON parsing and the guarded line rewrite ride python3 — present on
# every ubuntu runner and dev box, one interpreter fewer to install
# than jq. curl carries the API reads.
#
# Modes (off | warn | strict | auto; default warn):
#   ZYLAXION_ACTIONS_HEALTH                 one-shot, beats git config
#   git config zylaxion.actionsHealthCheck  repo-local, persistent
#
# Tunables (env, optional):
#   ZYLAXION_ACTIONS_HEALTH_TTL       cache seconds (default 21600 = 6h)
#   ZYLAXION_ACTIONS_HEALTH_TIMEOUT   per-API-call seconds (default 10)
#   GITHUB_TOKEN                      authenticated API reads; when
#                                     empty it resolves from the
#                                     machine's own git credential
#                                     helper for github.com — no new
#                                     standing secret, the push
#                                     credential reuses itself
#
# Usage:
#   bash scripts/gates/check-actions-pins.sh             # read (warn)
#   bash scripts/gates/check-actions-pins.sh --apply     # heal seat
#   bash scripts/gates/check-actions-pins.sh --refresh-cache
#
# Exit codes: 0 healthy or honest skip; 1 known-stale block (strict /
# auto only); 2 indeterminate — never blocks, never heals.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

API_ROOT="https://api.github.com"
DEFAULT_TTL=21600
DEFAULT_TIMEOUT=10

MODE="${ZYLAXION_ACTIONS_HEALTH:-$(git config zylaxion.actionsHealthCheck 2> /dev/null || echo warn)}"
APPLY_REQUESTED=0
REFRESH_ONLY=0
for arg in "$@"; do
    case "${arg}" in
    --apply) APPLY_REQUESTED=1 ;;
    --refresh-cache) REFRESH_ONLY=1 ;;
    -h | --help)
        sed -n '2,65p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    *)
        echo "Unknown option: ${arg}" >&2
        exit 2
        ;;
    esac
done

# ── mode gate ────────────────────────────────────────────────────────────
case "${MODE}" in
off)
    exit 0
    ;;
warn | strict | auto) ;;
*)
    echo -e "\033[0;33m!\033[0m unknown actions-health mode '${MODE}' — falling back to warn"
    MODE=warn
    ;;
esac

# ── token resolution (env -> gh -> git credential helper) ───────────────
TOKEN=""
resolve_token() {
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        TOKEN="${GITHUB_TOKEN}"
        return 0
    fi
    if command -v gh > /dev/null 2>&1; then
        TOKEN="$(gh auth token 2> /dev/null || true)"
        [ -n "${TOKEN}" ] && return 0
    fi
    TOKEN="$(printf 'protocol=https\nhost=github.com\n\n' |
        git credential fill 2> /dev/null | sed -n 's/^password=//p' || true)"
    return 0
}

# ── API plumbing ─────────────────────────────────────────────────────────
API_STATUS=0
API_BODY=""
api_get() {
    local path="$1"
    local auth=()
    [ -n "${TOKEN}" ] && auth=(-H "Authorization: Bearer ${TOKEN}")
    API_STATUS=0
    API_BODY="$(curl -sS --max-time "${ZYLAXION_ACTIONS_HEALTH_TIMEOUT:-$DEFAULT_TIMEOUT}" \
        -H "Accept: application/vnd.github+json" ${auth[@]+"${auth[@]}"} \
        -w $'\n%{http_code}' "${API_ROOT}${path}" 2> /dev/null)" || {
        API_STATUS=000
        API_BODY=""
        return 1
    }
    API_STATUS="${API_BODY##*$'\n'}"
    API_BODY="${API_BODY%$'\n'*}"
    [ "${API_STATUS}" = "200" ]
}

# ── version math (the zelynic shapes, verbatim) ──────────────────────────
is_semver() {
    printf '%s' "$1" | grep -Eq '^v?[0-9]+(\.[0-9]+)*$'
}

version_tuple() {
    local v="${1#v}"
    local maj min patch
    IFS='.' read -r maj min patch <<< "$v"
    printf '%s %s %s' "${maj:-0}" "${min:-0}" "${patch:-0}"
}

version_cmp() {
    local a_maj a_min a_pat b_maj b_min b_pat
    read -r a_maj a_min a_pat <<< "$(version_tuple "$1")"
    read -r b_maj b_min b_pat <<< "$(version_tuple "$2")"
    if [ "$a_maj" -lt "$b_maj" ]; then
        printf '%s\n' -1
    elif [ "$a_maj" -gt "$b_maj" ]; then
        printf '%s\n' 1
    elif [ "$a_min" -lt "$b_min" ]; then
        printf '%s\n' -1
    elif [ "$a_min" -gt "$b_min" ]; then
        printf '%s\n' 1
    elif [ "$a_pat" -lt "$b_pat" ]; then
        printf '%s\n' -1
    elif [ "$a_pat" -gt "$b_pat" ]; then
        printf '%s\n' 1
    else
        printf '%s\n' 0
    fi
}

major_of() {
    local maj _min _pat
    read -r maj _min _pat <<< "$(version_tuple "$1")"
    printf '%s\n' "$maj"
}

# ── per-repo truth (releases/latest primary, semver tags fallback) ──────
declare -A LATEST_TAG=()
declare -A LATEST_SHA=()
declare -A REPO_VERDICT=()

resolve_tag_sha() {
    local repo="$1" tag="$2" obj_sha obj_type tagobj
    api_get "/repos/${repo}/git/ref/tags/${tag}" || return 1
    obj_sha="$(printf '%s' "$API_BODY" | python3 -c 'import json,sys;print(json.load(sys.stdin).get("object",{}).get("sha","") or "")')"
    obj_type="$(printf '%s' "$API_BODY" | python3 -c 'import json,sys;print(json.load(sys.stdin).get("object",{}).get("type","") or "")')"
    [ -n "$obj_sha" ] || return 1
    if [ "$obj_type" = "tag" ]; then
        # Annotated tag object: the commit SHA lives one deref deeper.
        api_get "/repos/${repo}/git/tags/${obj_sha}" || return 1
        tagobj="$(printf '%s' "$API_BODY" | python3 -c 'import json,sys;print(json.load(sys.stdin).get("object",{}).get("sha","") or "")')"
        [ -n "$tagobj" ] || return 1
        printf '%s\n' "$tagobj"
    else
        printf '%s\n' "$obj_sha"
    fi
}

query_repo() {
    local repo="$1" tag
    if [ -n "${LATEST_TAG[$repo]:-}" ] || [ -n "${REPO_VERDICT[$repo]:-}" ]; then
        return 0
    fi
    if api_get "/repos/${repo}/releases/latest"; then
        tag="$(printf '%s' "$API_BODY" | python3 -c 'import json,sys;print(json.load(sys.stdin).get("tag_name","") or "")')"
        if [ -n "$tag" ] && is_semver "$tag"; then
            LATEST_TAG[$repo]="$tag"
            return 0
        fi
    fi
    if [ "${API_STATUS}" = "404" ] || [ "${API_STATUS}" = "200" ]; then
        if api_get "/repos/${repo}/tags?per_page=100"; then
            tag="$(printf '%s' "$API_BODY" | python3 -c '
import json, re, sys
names = [x.get("name", "") for x in json.load(sys.stdin)]
names = [n for n in names if re.fullmatch(r"v?[0-9]+(\.[0-9]+)*", n)]
names.sort(key=lambda s: [int(p) for p in s.lstrip("v").split(".")])
print(names[-1] if names else "")
' || true)"
            if [ -n "$tag" ]; then
                LATEST_TAG[$repo]="$tag"
                return 0
            fi
            REPO_VERDICT[$repo]="no releases and no semver tags"
            return 0
        fi
    fi
    if [ "${API_STATUS}" = "403" ]; then
        REPO_VERDICT[$repo]="api rate-limited (403)"
    else
        REPO_VERDICT[$repo]="api unreachable (status ${API_STATUS})"
    fi
}

ensure_latest_sha() {
    local repo="$1" sha
    [ -z "${LATEST_SHA[$repo]:-}" ] || return 0
    sha="$(resolve_tag_sha "$repo" "${LATEST_TAG[$repo]}")" || sha=""
    if [ -n "$sha" ]; then
        LATEST_SHA[$repo]="$sha"
    else
        REPO_VERDICT[$repo]="tag ${LATEST_TAG[$repo]} unresolvable"
    fi
}

# ── output helpers (red failure, yellow skip, green keep) ───────────────
readonly C_RED='\033[0;31m'
readonly C_GREEN='\033[0;32m'
readonly C_YELLOW='\033[0;33m'
readonly C_NC='\033[0m'

HEALED=0
KEPT=0
SKIPPED=0
INDETERMINATE=0
MAJOR_MOVES=0
EDIT_KEYS=""
declare -A EDIT_FILE=()
declare -A EDIT_LINE=()
declare -A EDIT_TEXT=()
declare -A EDIT_ORIG=()

note() { echo -e "  SWEEP: $*"; }
keep() { echo -e "  ${C_GREEN}keep${C_NC}: $*"; }
skip() { echo -e "  ${C_YELLOW}skip${C_NC}: $*"; }

record_edit() {
    local file="$1" lineno="$2" text="$3" orig="$4"
    EDIT_FILE[$EDIT_COUNT]="$file"
    EDIT_LINE[$EDIT_COUNT]="$lineno"
    EDIT_TEXT[$EDIT_COUNT]="$text"
    EDIT_ORIG[$EDIT_COUNT]="$orig"
    EDIT_COUNT=$((EDIT_COUNT + 1))
    return 0
}
EDIT_COUNT=0

# ── sweep ────────────────────────────────────────────────────────────────
sweep_sha_pin() {
    local file="$1" lineno="$2" content="$3" repo="$4" action_part="$5"
    local ref="$6" value="$7" comment="$8"
    local cur_ver token latest cmp new_line new_sha new_token

    token=""
    if [[ $comment =~ v[0-9]+(\.[0-9]+)* ]]; then
        token="${BASH_REMATCH[0]}"
    fi
    if [ -z "$token" ]; then
        skip "${repo} — no version token in the pin comment; add \"# vN\" to enlist it"
        SKIPPED=$((SKIPPED + 1))
        return 0
    fi
    cur_ver="$token"

    query_repo "$repo"
    if [ -n "${REPO_VERDICT[$repo]:-}" ]; then
        skip "${repo} — ${REPO_VERDICT[$repo]}"
        SKIPPED=$((SKIPPED + 1))
        INDETERMINATE=$((INDETERMINATE + 1))
        return 0
    fi
    latest="${LATEST_TAG[$repo]}"
    ensure_latest_sha "$repo"
    if [ -n "${REPO_VERDICT[$repo]:-}" ]; then
        skip "${repo} — ${REPO_VERDICT[$repo]}"
        SKIPPED=$((SKIPPED + 1))
        INDETERMINATE=$((INDETERMINATE + 1))
        return 0
    fi
    new_sha="${LATEST_SHA[$repo]}"
    cmp="$(version_cmp "$cur_ver" "$latest")"

    if [ "$cmp" = "1" ]; then
        keep "${repo} — pinned ${cur_ver} is ahead of latest release ${latest}"
        KEPT=$((KEPT + 1))
        return 0
    fi
    # The pin's identity is the SHA, not the comment's token: a pin
    # already sitting on the release commit is CURRENT in every form
    # its token takes (the zelynic no-op-heal rule).
    if [ "$ref" = "$new_sha" ]; then
        keep "${repo} — ${cur_ver} pinned at the latest release commit"
        KEPT=$((KEPT + 1))
        return 0
    fi

    new_line="${content/${value}/${action_part}@${new_sha}}"
    if [ "$cmp" = "-1" ] && [ "$(major_of "$cur_ver")" != "$(major_of "$latest")" ]; then
        # Major moved: the comment's version token moves with it,
        # preserving the author's chosen form (major-only vs exact).
        # The move is still only ever WRITTEN by the maintainer seat;
        # AUTO refuses any batch that carries a major move.
        if [ "$token" = "v$(major_of "$cur_ver")" ]; then
            new_token="v$(major_of "$latest")"
        else
            new_token="v${latest#v}"
        fi
        new_line="${new_line/${token}/${new_token}}"
        MAJOR_MOVES=$((MAJOR_MOVES + 1))
    elif [ "$cmp" = "0" ]; then
        note "${repo} — pin drift: ${cur_ver} current but the SHA moved; re-pinning to the release commit"
    fi
    record_edit "$file" "$lineno" "$new_line" "$content"
    note "${repo} (${file#./}:${lineno}) — ${cur_ver} -> ${latest}, SHA ${ref:0:12} -> ${new_sha:0:12} HEAL"
    HEALED=$((HEALED + 1))
}

sweep_tag_pin() {
    local file="$1" lineno="$2" content="$3" repo="$4" action_part="$5"
    local ref="$6" value="$7"
    local latest cmp new_ref new_line

    query_repo "$repo"
    if [ -n "${REPO_VERDICT[$repo]:-}" ]; then
        skip "${repo} — ${REPO_VERDICT[$repo]}"
        SKIPPED=$((SKIPPED + 1))
        INDETERMINATE=$((INDETERMINATE + 1))
        return 0
    fi
    latest="${LATEST_TAG[$repo]}"
    cmp="$(version_cmp "$ref" "$latest")"
    if [ "$cmp" != "-1" ]; then
        keep "${repo}@${ref} — latest release ${latest} needs no tag move"
        KEPT=$((KEPT + 1))
        return 0
    fi
    if [ "$(major_of "$ref")" = "$(major_of "$latest")" ]; then
        keep "${repo}@${ref} — the moving tag already carries ${latest}"
        KEPT=$((KEPT + 1))
        return 0
    fi
    if [[ $ref == *.* ]]; then
        new_ref="v${latest#v}"
    else
        new_ref="v$(major_of "$latest")"
    fi
    new_line="${content/${value}/${action_part}@${new_ref}}"
    record_edit "$file" "$lineno" "$new_line" "$content"
    note "${repo} (${file#./}:${lineno}) — ${ref} -> ${new_ref} HEAL (tag pin)"
    HEALED=$((HEALED + 1))
    MAJOR_MOVES=$((MAJOR_MOVES + 1))
}

run_sweep() {
    local line file rest lineno content raw value comment
    local action_part ref repo
    while IFS= read -r line; do
        file="${line%%:[0-9]*}"
        rest="${line#"$file":}"
        lineno="${rest%%:*}"
        content="${rest#*:}"
        raw="${content#*uses:}"
        raw="${raw#"${raw%%[![:space:]]*}"}" # trim leading spaces
        comment=""
        if [[ $raw == *"#"* ]]; then
            comment="#${raw#*#}"
            raw="${raw%%#*}"
            raw="${raw%"${raw##*[![:space:]]}"}" # trim trailing spaces
        fi
        value="$raw"
        [ -n "$value" ] || continue
        action_part="${value%@*}"
        ref="${value##*@}"
        if [[ $action_part == .* || $action_part == docker:* ]]; then
            skip "${value} — local/docker reference, not a version pin"
            SKIPPED=$((SKIPPED + 1))
            continue
        fi
        IFS='/' read -r owner name _rest <<< "$action_part"
        repo="${owner}/${name}"
        if ! [[ $repo =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
            skip "${value} — unparsable action reference"
            SKIPPED=$((SKIPPED + 1))
            continue
        fi
        if [[ $ref =~ ^[0-9a-fA-F]{40}$ ]]; then
            sweep_sha_pin "$file" "$lineno" "$content" \
                "$repo" "$action_part" "$ref" "$value" "$comment"
        elif [[ $ref =~ ^v[0-9]+(\.[0-9]+)*$ ]]; then
            sweep_tag_pin "$file" "$lineno" "$content" \
                "$repo" "$action_part" "$ref" "$value"
        else
            skip "${repo}@${ref} — branch-style ref, not a version pin"
            SKIPPED=$((SKIPPED + 1))
        fi
    done < <(grep -nE '^[[:space:]]*(-[[:space:]]+)?uses:' .github/workflows/*.yml 2> /dev/null || true)
    return 0
}

# ── guarded apply (python rewrite with drift verification) ──────────────
apply_edits() {
    [ "${EDIT_COUNT}" -gt 0 ] || return 0
    local backup pre_nonworkflow post_nonworkflow apply_out i
    backup="$(mktemp -d)"
    cp -a .github/workflows "${backup}/workflows"
    pre_nonworkflow="$(git status --porcelain 2> /dev/null | grep -v '\.github/workflows/' || true)"

    # One TSV record per edit (tab never appears inside a YAML line —
    # yamllint enforces spaces): file, 1-based line, original, new.
    local tsv=""
    for ((i = 0; i < EDIT_COUNT; i++)); do
        tsv+="${EDIT_FILE[$i]}"$'\t'"${EDIT_LINE[$i]}"$'\t'"${EDIT_ORIG[$i]}"$'\t'"${EDIT_TEXT[$i]}"$'\n'
    done

    apply_out="$(PIN_EDITS="${tsv}" python3 - << 'PYEOF'
import os, sys
edits = []
for rec in os.environ["PIN_EDITS"].splitlines():
    if not rec:
        continue
    f, ln, orig, new = rec.split("\t")
    edits.append((f, int(ln), orig, new))
per_file = {}
for f, ln, orig, new in edits:
    per_file.setdefault(f, []).append((ln, orig, new))
for f, items in per_file.items():
    with open(f, "r", encoding="utf-8") as fh:
        lines = fh.read().split("\n")
    for ln, orig, new in sorted(items, key=lambda x: -x[0]):
        idx = ln - 1
        if idx >= len(lines):
            print(f"DRIFT: {f}:{ln} beyond EOF", file=sys.stderr)
            sys.exit(1)
        if lines[idx] != orig:
            print(f"DRIFT: {f}:{ln} changed since the sweep", file=sys.stderr)
            sys.exit(1)
        lines[idx] = new
    with open(f, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))
print("APPLIED")
PYEOF
)" || true

    if [ "${apply_out}" != "APPLIED" ]; then
        echo -e "  ${C_RED}FAIL${C_NC}: the apply could not prove itself clean — restoring the pre-apply tree"
        rm -rf .github/workflows
        cp -a "${backup}/workflows" .github/workflows
        rm -rf "${backup}"
        return 1
    fi
    # The delta-shaped proof: nothing outside .github/workflows/ may
    # appear new in the tree after the apply.
    post_nonworkflow="$(git status --porcelain 2> /dev/null | grep -v '\.github/workflows/' || true)"
    if [ "${pre_nonworkflow}" != "${post_nonworkflow}" ]; then
        echo -e "  ${C_RED}FAIL${C_NC}: writes escaped .github/workflows/ — restoring the pre-apply tree"
        rm -rf .github/workflows
        cp -a "${backup}/workflows" .github/workflows
        rm -rf "${backup}"
        return 1
    fi
    rm -rf "${backup}"
    return 0
}

# ── cache ────────────────────────────────────────────────────────────────
CACHE_DIR="$(git rev-parse --git-dir 2> /dev/null || echo .git)/zylaxion/actions-pins"
workflows_hash() {
    cat .github/workflows/*.yml 2> /dev/null | sha256sum | cut -d' ' -f1
}
cache_fresh() {
    local f="$1" now mtime ttl
    [ -f "$f" ] || return 1
    ttl="${ZYLAXION_ACTIONS_HEALTH_TTL:-$DEFAULT_TTL}"
    now="$(date +%s)"
    mtime="$(stat -c %Y "$f" 2> /dev/null || echo 0)"
    [ "$((now - mtime))" -lt "$ttl" ]
}
write_cache() {
    local f="$1" final="$2"
    mkdir -p "$(dirname "$f")"
    {
        echo "CACHE-FINAL: ${final}"
        echo "HEALED: ${HEALED}"
        echo "KEPT: ${KEPT}"
        echo "SKIPPED: ${SKIPPED}"
        echo "INDETERMINATE: ${INDETERMINATE}"
        echo "MAJOR_MOVES: ${MAJOR_MOVES}"
        echo "GENERATED: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    } > "$f"
}

classify() {
    if [ "${INDETERMINATE}" -gt 0 ]; then
        FINAL="indeterminate"
    elif [ "${HEALED}" -gt 0 ]; then
        FINAL="behind"
    else
        FINAL="current"
    fi
}

cold_run() {
    HEALED=0
    KEPT=0
    SKIPPED=0
    INDETERMINATE=0
    MAJOR_MOVES=0
    EDIT_COUNT=0
    resolve_token
    run_sweep
    classify
    write_cache "${CACHE_DIR}/${WF_HASH}.verdict" "${FINAL}"
}

# ═════════════════════════════════════════════════════════════════════════
# main
# ═════════════════════════════════════════════════════════════════════════
WF_HASH="$(workflows_hash)"
CACHE_FILE="${CACHE_DIR}/${WF_HASH}.verdict"
FINAL="current"
CACHED=0

if cache_fresh "$CACHE_FILE"; then
    CACHED=1
    FINAL="$(sed -n 's/^CACHE-FINAL: //p' "$CACHE_FILE")"
    HEALED="$(sed -n 's/^HEALED: //p' "$CACHE_FILE")"
    KEPT="$(sed -n 's/^KEPT: //p' "$CACHE_FILE")"
    SKIPPED="$(sed -n 's/^SKIPPED: //p' "$CACHE_FILE")"
    INDETERMINATE="$(sed -n 's/^INDETERMINATE: //p' "$CACHE_FILE")"
    MAJOR_MOVES="$(sed -n 's/^MAJOR_MOVES: //p' "$CACHE_FILE")"
    # A cached non-current verdict is neither a block mandate nor a
    # write mandate: re-verify COLD in every mode before anything
    # downstream may act on it.
    if [ "${FINAL}" != "current" ]; then
        CACHED=0
        cold_run
    fi
else
    cold_run
fi

if [ "${REFRESH_ONLY}" = "1" ]; then
    echo "pin cache refreshed: ${FINAL} (healed ${HEALED}, kept ${KEPT}, skipped ${SKIPPED})"
    exit 0
fi

# The verdict, always classified, never silent in warn/strict. Auto's
# silence is the reward for healthy pins.
if [ "${MODE}" != "auto" ] || [ "${FINAL}" != "current" ]; then
    [ "${CACHED}" = "1" ] &&
        echo -e "  sweep verdict: ${HEALED} healed, ${KEPT} kept, ${SKIPPED} skipped (mode: ${MODE}, cached)"
    [ "${CACHED}" = "0" ] &&
        echo -e "  sweep verdict: ${HEALED} healed, ${KEPT} kept, ${SKIPPED} skipped (mode: ${MODE})"
fi

if [ "${FINAL}" = "current" ]; then
    [ "${MODE}" = "auto" ] || echo -e "  ${C_GREEN}all action pins current${C_NC}"
    exit 0
fi

if [ "${FINAL}" = "indeterminate" ]; then
    echo -e "  ${C_YELLOW}pin freshness indeterminate this run${C_NC} — advisory only, never a block"
    exit 2
fi

# FINAL == behind. MAJOR moves ride the table for the human; the
# strict seat blocks on them, AUTO refuses to write them.
if [ "${MAJOR_MOVES}" -gt 0 ] && [ "${MODE}" != "warn" ] && [ "${APPLY_REQUESTED}" = "0" ]; then
    echo -e "  ${C_RED}major version moves pending${C_NC} — a human call: review the table, then run with --apply to take them"
    exit 1
fi

if [ "${MODE}" = "warn" ] && [ "${APPLY_REQUESTED}" = "0" ]; then
    echo -e "  ${C_YELLOW}stale pins found${C_NC} — heal with: bash scripts/gates/check-actions-pins.sh --apply"
    exit 0
fi

if [ "${HEALED}" -gt 0 ] && apply_edits; then
    if [ "${MODE}" = "auto" ]; then
        git add .github/workflows/ 2> /dev/null || true
        echo -e "  ${C_GREEN}healed and staged${C_NC}: review git diff --cached, then re-commit — the re-run reads the healed pins as current"
        exit 1
    fi
    echo -e "  ${C_GREEN}healed${C_NC}: ${HEALED} pin(s) rewritten in .github/workflows/ — review and commit"
    exit 0
fi

if [ "${MODE}" = "strict" ]; then
    echo -e "  ${C_RED}stale pins, nothing healed${C_NC} — resolve the table above, then re-run"
    exit 1
fi

# Warn-with-explicit-apply or a failed heal: advisory exit, the table
# already printed. Remote facts never block a commit.
exit 0
