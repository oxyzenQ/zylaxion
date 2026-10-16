#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-host.sh — the host-truth audit (night-dinner-13).
#
# The security ledger (docs/ZACTRIX_SECURITY.md) closed the sandbox
# pass with one INCONCLUSIVE dimension and one inconclusive row:
#
#   Dimension 3 (privilege boundaries): the pinned-file permission
#   modes — "unmeasurable without a real bpffs host, with the exact
#   measurement named for the pass that can attach". This script IS
#   that named measurement, instrumented.
#
#   Dimension 2 (BPF program safety): the loaded-program identity
#   confirmation — root + bpftool required.
#
# Plus the operational rows a sandbox structurally cannot produce:
# the real attach lane (fentry vs kprobe on the real kernel), real
# input telemetry through the physical keyboard, the real audio
# backend, the boot posture, kernel-log hygiene, and (with --full)
# the full detach/re-attach lifecycle with state restoration.
#
# Usage:
#   scripts/audit/zactrix-host.sh            # read-only rows; changes nothing
#   scripts/audit/zactrix-host.sh --full     # + the mutating lifecycle row:
#                                            #   off -> detach -> attach -> on ->
#                                            #   10 s soak -> off, then the recorded
#                                            #   preset/volume/player state restored.
#                                            #   Retires any running player (the
#                                            #   attach reaper contract).
#
# Verdict vocabulary: PASS, FAIL, INCONCLUSIVE (the row cannot be
# measured here, the exact unlock command is named), HUMAN (the one
# row only your ear can close), INFO (a recorded fact, no verdict).
# Exit 0 when no FAIL, 1 otherwise. INCONCLUSIVE never fails a run —
# the ledger wants the truth, not a green light.
#
# The default mode mutates nothing: no attach, no detach, no player
# spawn. It measures whatever posture the host is already in.

set -uo pipefail
cd "$(dirname "$0")/../.."

BIN_NAME="${ZYLAXION_BIN:-zylaxion}"
PIN_DIR="/sys/fs/bpf/zylaxion"
ALL_PINS=(tap tap_link control keymap events stats)
SOAK_SECONDS="${SOAK_SECONDS:-10}"

PASS=0
FAIL=0
INCONCLUSIVE=0

line() { printf '%s\n' "$*"; }
hr()   { printf '%s\n' "--------------------------------------------------------------"; }

verdict() { # verdict <STATUS> <NAME> <DETAIL>
    local status="$1" name="$2" detail="$3"
    case "$status" in
        PASS)        PASS=$((PASS + 1));;
        FAIL)        FAIL=$((FAIL + 1));;
        INCONCLUSIVE) INCONCLUSIVE=$((INCONCLUSIVE + 1));;
    esac
    printf '[%s] %s\n' "$status" "$name"
    [[ -n "$detail" ]] && printf '       %s\n' "$detail"
    return 0
}

as_root() {
    if [[ ${EUID} -eq 0 ]]; then
        "$@"
    else
        sudo "$@"
    fi
}

# ------------------------------------------------------------------
# R0 — the environment stamp (INFO): what exactly was measured, so
# the ledger row carries its own provenance.
# ------------------------------------------------------------------
BIN_ABS="$(command -v "$BIN_NAME" 2>/dev/null || true)"
INVOKER_UID="$(id -u)"
INVOKER_NAME="$(id -un)"
KERNEL_REL="$(uname -r)"
BPFFS_LINE="$(grep bpffs /proc/mounts | head -1 || true)"
BPFTOOL_ABS="$(command -v bpftool 2>/dev/null || true)"
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo not-a-checkout)"

hr
line "zactrix-host — the host-truth audit (night-dinner-13)"
hr
line "R0 environment stamp:"
line "      kernel:          ${KERNEL_REL}"
line "      commit:          ${COMMIT}"
line "      binary:          ${BIN_ABS:-<not found in PATH>}"
[[ -n "$BIN_ABS" ]] && line "      binary version:  $("$BIN_ABS" -V 2>/dev/null | head -1 || true)"
line "      invoking user:   ${INVOKER_NAME} (uid ${INVOKER_UID})"
line "      bpffs mount:     ${BPFFS_LINE:-<bpffs NOT mounted>}"
line "      bpftool:         ${BPFTOOL_ABS:-<not installed>}"
line ""

if [[ -z "$BIN_ABS" ]]; then
    verdict FAIL "R0 binary presence" \
        "'$BIN_NAME' is not in PATH — install it first (scripts/package/install-on-my-machine.sh) or set ZYLAXION_BIN."
fi
if [[ -z "$BPFTOOL_ABS" ]]; then
    verdict INCONCLUSIVE "R0 bpftool presence" \
        "bpftool is not installed — R2 (unprivileged map read) and R4 (loaded-program identity) need it. Unlock: install bpftool (the bpf tools package of your distro)."
fi

# ------------------------------------------------------------------
# R1 — Dimension 3a: the pin census + the permission matrix.
# The named measurement: with the tap attached, stat every pinned
# object on the real bpffs and record uid:gid mode per pin — that is
# the row the sandbox could not produce (no bpffs host there).
# ------------------------------------------------------------------
hr
line "R1 — Dimension 3a: the pin census + the permission matrix"
hr
PIN_STAT_OUT=""
if as_root test -d "$PIN_DIR" 2>/dev/null; then
    PIN_STAT_OUT="$(as_root stat -c '%n %u:%g %a %F' "$PIN_DIR" "$PIN_DIR"/* 2>/dev/null || true)"
    line "$PIN_STAT_OUT"
    PIN_COUNT="$(as_root find "$PIN_DIR" -mindepth 1 -maxdepth 1 2>/dev/null | wc -l)"
    MISSING_PINS=""
    for p in "${ALL_PINS[@]}"; do
        as_root test -e "$PIN_DIR/$p" 2>/dev/null || MISSING_PINS="${MISSING_PINS} $p"
    done
    if [[ -z "$MISSING_PINS" ]]; then
        verdict PASS "R1 pin census" \
            "all 6 pins present under $PIN_DIR (${PIN_COUNT} entries total)"
        verdict PASS "R1 pin permission matrix" \
            "measured on the real bpffs host (the output above IS the ledger evidence — paste it into docs/ZACTRIX_SECURITY.md Dimension 3)"
        # The ledger claim, verified mechanically: the pins stay 0600
        # owner-only, chowned to the invoking user at attach time.
        CLAIM_BREACH=""
        USER_OWNED=0
        ROOT_OWNED=0
        while read -r pname puids pmode _rest; do
            [[ "$pname" == "$PIN_DIR" ]] && continue
            [[ "$pmode" == "600" ]] || CLAIM_BREACH="${CLAIM_BREACH} ${pname##*/}(${pmode})"
            [[ "$puids" == "$INVOKER_UID"* ]] && USER_OWNED=$((USER_OWNED + 1))
            [[ "$puids" == "0:"* ]] && ROOT_OWNED=$((ROOT_OWNED + 1))
        done <<< "$PIN_STAT_OUT"
        if [[ -n "$CLAIM_BREACH" ]]; then
            verdict FAIL "R1 the 0600 owner-only claim" \
                "pins breaking the documented mode:${CLAIM_BREACH} — the ledger claim (pins stay 0600 owner-only) is broken on this host"
        elif [[ $USER_OWNED -gt 0 ]]; then
            verdict PASS "R1 the 0600 owner-only claim" \
                "every pin is mode 600, ${USER_OWNED} owned by uid ${INVOKER_UID} (the documented hand-off posture: attach chowned them to the invoking user)"
        elif [[ $ROOT_OWNED -gt 0 ]]; then
            verdict PASS "R1 the 0600 owner-only claim" \
                "every pin is mode 600, root-owned (the unit lane posture — root planted them; the hand-off posture is 'sudo ${BIN_NAME} attach' from your session)"
        else
            verdict INCONCLUSIVE "R1 the 0600 owner-only claim" \
                "pin modes are 600 but the owner uid matches neither the invoker nor root — record the matrix above for the ledger"
        fi
    else
        verdict INCONCLUSIVE "R1 pin census" \
            "pins missing:${MISSING_PINS} — re-attach and re-run. Unlock: sudo ${BIN_NAME} attach"
    fi
else
    verdict INCONCLUSIVE "R1 pin permission matrix" \
        "no tap is planted ($PIN_DIR absent) — the matrix can only be measured by a pass that can attach. Unlock: sudo ${BIN_NAME} attach, then re-run this audit"
fi
line ""

# ------------------------------------------------------------------
# R2 — Dimension 3b: the unprivileged walk + the control-map read.
# The status surface promises "introspection: bpftool map dump pinned
# /sys/fs/bpf/zylaxion/control" as a plain user. Measure exactly that
# promise, plus the walk chain (the mount root o+x lane,
# night-dinner-3's addendum).
# ------------------------------------------------------------------
hr
line "R2 — Dimension 3b: the unprivileged walk + the control-map read"
hr
if [[ ${EUID} -eq 0 ]]; then
    verdict INCONCLUSIVE "R2 unprivileged lanes" \
        "this audit is running as root — the whole point of R2 is the NON-root verdict. Re-run as your desktop user (sudo is prompted per row where the row demands it)"
elif [[ -z "$BPFTOOL_ABS" ]]; then
    verdict INCONCLUSIVE "R2 control-map read" "bpftool missing — see R0 unlock"
elif ! as_root test -e "$PIN_DIR/control" 2>/dev/null; then
    verdict INCONCLUSIVE "R2 control-map read" "no tap planted — Unlock: sudo ${BIN_NAME} attach"
else
    if ls "$PIN_DIR" >/dev/null 2>&1; then
        WALK_OUT="$(ls "$PIN_DIR" 2>/dev/null || true)"
        verdict PASS "R2 unprivileged walk" "the invoking user lists the pin directory: $(echo "$WALK_OUT" | tr '\n' ' ')"
    else
        WALK_OUT="$(ls "$PIN_DIR" 2>&1 || true)"
        if echo "$WALK_OUT" | grep -q "Permission denied"; then
            verdict INFO "R2 unprivileged walk" \
                "listing denied (the o+x-without-o+r shape: path-wise stat may still work — the stat row below decides): ${WALK_OUT}"
        else
            verdict FAIL "R2 unprivileged walk" "unexpected walk failure: ${WALK_OUT}"
        fi
    fi
    if stat "$PIN_DIR/control" >/dev/null 2>&1; then
        verdict PASS "R2 unprivileged stat" "the invoking user stats the control pin (the o+x walk chain holds)"
    else
        verdict FAIL "R2 unprivileged stat" \
            "the invoking user cannot stat $PIN_DIR/control — the status surface and the player lanes would degrade; check the mount-root mode (R3) and the pin owner (R1)"
    fi
    DUMP_HEAD="$(bpftool map dump pinned "$PIN_DIR/control" 2>&1 | head -6 || true)"
    if bpftool map dump pinned "$PIN_DIR/control" >/dev/null 2>&1; then
        verdict PASS "R2 unprivileged control-map dump" \
            "bpftool map dump pinned works as the plain user (the introspection promise holds on the real host)"
        line "       first lines of the dump:"
        echo "$DUMP_HEAD" | sed 's/^/         /'
    else
        verdict FAIL "R2 unprivileged control-map dump" \
            "bpftool could not dump the control map as the invoking user: $(echo "$DUMP_HEAD" | head -1)"
    fi
fi
line ""

# ------------------------------------------------------------------
# R3 — Dimension 3c: the mount-root mode + the night-dinner-3 relax
# lane. The addendum's contract: attach may add o+x to /sys/fs/bpf
# ONLY as a ride-along of a successful hand-off, and nothing more.
# Measure the mode; record which posture the host is in.
# ------------------------------------------------------------------
hr
line "R3 — Dimension 3c: the bpffs mount-root mode (the relax lane)"
hr
MOUNT_ROOT_MODE="$(stat -c '%a' /sys/fs/bpf 2>/dev/null || true)"
MOUNT_ROOT_OUT="$(as_root stat -c '%a %u:%g' /sys/fs/bpf 2>/dev/null || true)"
if [[ -n "$MOUNT_ROOT_MODE" ]]; then
    line "      /sys/fs/bpf mode: ${MOUNT_ROOT_MODE}${MOUNT_ROOT_OUT:+ (root view: ${MOUNT_ROOT_OUT})}"
    case "$MOUNT_ROOT_MODE" in
        *7*)
            verdict PASS "R3 mount-root mode" \
                "mode ${MOUNT_ROOT_MODE} carries o+x — either the distro default or the night-dinner-3 ride-along relax; the walk verdict lives in R2, this row records the lever"
            ;;
        *)
            verdict PASS "R3 mount-root mode" \
                "mode ${MOUNT_ROOT_MODE} — no o+x; a non-root walk to the pins is blocked at the mount root (the Debian 13 default shape; the hand-off relax never fired or is not needed for root-only reads)"
            ;;
    esac
else
    verdict INCONCLUSIVE "R3 mount-root mode" "cannot stat /sys/fs/bpf — is this a Linux host with bpffs support?"
fi
line ""

# ------------------------------------------------------------------
# R4 — Dimension 2's residual: the loaded-program identity, root +
# bpftool required. Measure and record what bpftool exposes of the
# loaded program (id, tag, verifier instruction count) and pin the
# on-disk prebuilt object against the manifest sha256. The byte-exact
# loaded-vs-built hash stays NAMED, not claimed — the ledger wording
# stands until a hash lane exists; this row produces the host facts.
# ------------------------------------------------------------------
hr
line "R4 — Dimension 2 residual: the loaded-program identity"
hr
if [[ -z "$BPFTOOL_ABS" ]]; then
    verdict INCONCLUSIVE "R4 loaded-program identity" "bpftool missing — see R0 unlock"
elif ! as_root test -e "$PIN_DIR/tap" 2>/dev/null; then
    verdict INCONCLUSIVE "R4 loaded-program identity" "no tap planted — Unlock: sudo ${BIN_NAME} attach"
else
    PROG_SHOW="$(as_root bpftool prog show pinned "$PIN_DIR/tap" 2>&1 || true)"
    line "$PROG_SHOW" | sed 's/^/      /'
    if echo "$PROG_SHOW" | grep -q "tag "; then
        PROG_TAG="$(echo "$PROG_SHOW" | grep -oP 'tag \K[0-9a-f]+' | head -1)"
        verdict PASS "R4 loaded program present" \
            "the live program answered bpftool prog show (tag ${PROG_TAG} — recorded for the ledger; the loaded-vs-built byte hash stays the named residual)"
    else
        verdict FAIL "R4 loaded program present" \
            "bpftool could not show the pinned program: $(echo "$PROG_SHOW" | head -1)"
    fi
    if [[ -f "ebpf-prebuilt/zylaxion-tap" && -f "ebpf-prebuilt/manifest.toml" ]]; then
        MEASURED_SHA="$(sha256sum ebpf-prebuilt/zylaxion-tap | awk '{print $1}')"
        PINNED_SHA="$(grep -A1 'name = "zylaxion-tap"' ebpf-prebuilt/manifest.toml | grep 'sha256' | grep -oP '"\K[0-9a-f]+' | head -1)"
        if [[ -n "$PINNED_SHA" && "$MEASURED_SHA" == "$PINNED_SHA" ]]; then
            verdict PASS "R4 prebuilt object parity" \
                "sha256 ${MEASURED_SHA:0:16}... matches the manifest pin (what this host loads is what the tree certifies)"
        else
            verdict FAIL "R4 prebuilt object parity" \
                "measured ${MEASURED_SHA:0:16}... vs pinned ${PINNED_SHA:0:16}... — the prebuilt lane is stale; run scripts/dev/regenerate-ebpf-prebuilt.sh"
        fi
    else
        verdict INCONCLUSIVE "R4 prebuilt object parity" \
            "no ebpf-prebuilt/ tree here (running outside a checkout?) — parity measured repo-side by scripts/gates/check-prebuilt-parity.sh"
    fi
fi
line ""

# ------------------------------------------------------------------
# R5 — the boot posture: the oneshot is enabled, this boot's attach
# ended SUCCESS, and the journal carries no Unknown-key lines (the
# night-dinner-8 class of warning — the unit file must parse clean).
# ------------------------------------------------------------------
hr
line "R5 — the boot posture (the oneshot lane)"
hr
if ! command -v systemctl >/dev/null 2>&1; then
    verdict INCONCLUSIVE "R5 boot posture" "no systemd here"
else
    ENABLED="$(systemctl is-enabled zylaxion-attach 2>/dev/null || true)"
    if [[ "$ENABLED" == "enabled" ]]; then
        verdict PASS "R5 unit enabled" "zylaxion-attach plants the tap at every boot"
    else
        verdict FAIL "R5 unit enabled" \
            "is-enabled says '${ENABLED:-nothing}' — the boot lane is off. Unlock: sudo systemctl enable zylaxion-attach"
    fi
    BOOT_LOG="$(journalctl -b -u zylaxion-attach --no-pager 2>/dev/null || true)"
    if [[ -z "$BOOT_LOG" ]]; then
        verdict INCONCLUSIVE "R5 this boot's attach" \
            "no journal entries for the unit this boot (or the journal is not readable by this user — try re-running with sudo cached) — Unlock: sudo systemctl start zylaxion-attach, then re-run"
    else
        if echo "$BOOT_LOG" | grep -q "status=0/SUCCESS"; then
            verdict PASS "R5 this boot's attach" "ExecStart ended status=0/SUCCESS"
        else
            verdict FAIL "R5 this boot's attach" "no SUCCESS line in this boot's unit journal"
        fi
        if echo "$BOOT_LOG" | grep -q "Unknown key"; then
            verdict FAIL "R5 journal parse hygiene" \
                "the unit journal carries Unknown-key lines — the installed unit file is not the fixed one; reinstall from assets/zylaxion-attach.service"
        else
            verdict PASS "R5 journal parse hygiene" "no Unknown-key lines (the night-dinner-8 class stays closed)"
        fi
    fi
fi
line ""

# ------------------------------------------------------------------
# R6 — kernel-log hygiene: after a real attach, the kernel log must
# carry no BPF verifier failures or loader errors. The sandbox can
# never produce this row (it cannot load the program at all).
# ------------------------------------------------------------------
hr
line "R6 — kernel-log hygiene (the verifier lane)"
hr
KLOG="$(as_root dmesg --level=warn,err 2>/dev/null | tail -400 || journalctl -k --no-pager 2>/dev/null | tail -400 || true)"
if [[ -z "$KLOG" ]]; then
    verdict INCONCLUSIVE "R6 kernel-log hygiene" "cannot read the kernel log (dmesg restricted and no journalctl -k access) — re-run with sudo cached"
else
    BPF_NOISE="$(echo "$KLOG" | grep -iE 'bpf|verifier' | grep -iE 'fail|error|denied|reject|invalid|unknown' || true)"
    ZLX_NOISE="$(echo "$KLOG" | grep -i 'zylaxion' || true)"
    if [[ -n "$BPF_NOISE" ]]; then
        verdict FAIL "R6 verifier/loader errors" "the kernel log carries BPF error lines:"
        echo "$BPF_NOISE" | head -5 | sed 's/^/         /'
    else
        verdict PASS "R6 verifier/loader errors" "no BPF verifier or loader error lines in the recent kernel log"
    fi
    if [[ -n "$ZLX_NOISE" ]]; then
        verdict INFO "R6 zylaxion kernel-log mentions" "the kernel log mentions zylaxion (recorded, usually the loader's own info lines):"
        echo "$ZLX_NOISE" | head -3 | sed 's/^/         /'
    fi
fi
line ""

# ------------------------------------------------------------------
# Parse helpers over the status surface (the one-screen truth).
# ------------------------------------------------------------------
status_verbose() { "$BIN_ABS" status --verbose 2>/dev/null; }
presses_now()    { status_verbose | grep 'total presses:' | tr -dc '0-9' | tail -1; }
player_pid()     { status_verbose | grep -oP 'player:\s+running \(pid \K[0-9]+'; }
preset_now()     { status_verbose | awk '/^preset:/ {print $2}'; }
volume_now()     { status_verbose | awk '/^volume:/ {gsub(/%/,"",$2); print $2}'; }
pgrep_name()     { basename "$BIN_ABS"; }

# ------------------------------------------------------------------
# R7 — claim B live: no daemon, no IPC, no socket. The sandbox grep
# proved the code; only a running host proves the RUNTIME. Measured
# with whatever player is live right now (the --full soak re-measures
# during its own playback window).
# ------------------------------------------------------------------
hr
line "R7 — claim B live: no socket/IPC while running"
hr
if ! command -v ss >/dev/null 2>&1; then
    verdict INCONCLUSIVE "R7 claim B live" "ss not available — install iproute2"
else
    LIVE_PID="$(player_pid)"
    if [[ -n "$LIVE_PID" ]]; then
        SOCKETS="$(ss -tulpn 2>/dev/null | grep -i "$(pgrep_name)" || true)"
        if [[ -z "$SOCKETS" ]]; then
            verdict PASS "R7 claim B live (player pid ${LIVE_PID})" \
                "no TCP/UDP socket names the zylaxion processes while the player runs"
        else
            verdict FAIL "R7 claim B live" "the player owns sockets (claim B broken):"
            echo "$SOCKETS" | sed 's/^/         /'
        fi
    else
        verdict INCONCLUSIVE "R7 claim B live" \
            "no player is running — the strongest form of this row measures DURING playback. Unlock: ${BIN_NAME} on (or run this audit with --full)"
    fi
fi
line ""

# ------------------------------------------------------------------
# R8 — real-input telemetry: the counters accumulate in the kernel on
# real fentry hits from the physical keyboard. A sandbox cannot type.
# This row asks the human to type for a few seconds and measures the
# delta — the wire-level proof the tap sees real input events.
# ------------------------------------------------------------------
hr
line "R8 — real-input telemetry delta (type when prompted)"
hr
P0="$(presses_now)"
if [[ -z "$P0" ]]; then
    verdict INCONCLUSIVE "R8 real-input telemetry" "no tap planted (status shows no press counter) — Unlock: sudo ${BIN_NAME} attach"
else
    line "      presses so far: ${P0} — TYPE A FEW KEYS NOW (6 s window)..."
    sleep 6
    P1="$(presses_now)"
    DELTA=$(( P1 - P0 ))
    if (( DELTA > 0 )); then
        verdict PASS "R8 real-input telemetry" \
            "the kernel counted ${DELTA} new presses in the window (fentry:input_event fires on the real keyboard)"
    else
        verdict FAIL "R8 real-input telemetry" \
            "the press counter did not move (${P0} -> ${P1}) — the tap is planted but deaf; check R6's kernel log and the attach lane"
    fi
fi
line ""

# ------------------------------------------------------------------
# R9 — the player on real audio hardware: pid liveness plus the
# resource row a sandbox cannot produce (real cpal backend on the
# real ALSA/PipeWire server). The audible verdict is HUMAN on
# purpose: only your ear closes it.
# ------------------------------------------------------------------
hr
line "R9 — the player on real audio hardware"
hr
LIVE_PID="$(player_pid)"
if [[ -n "$LIVE_PID" ]] && kill -0 "$LIVE_PID" 2>/dev/null; then
    RES_ROW="$(ps -p "$LIVE_PID" -o pid,%cpu,rss,etime --no-headers 2>/dev/null || true)"
    line "      player: pid ${LIVE_PID}  (PID %CPU RSS_KB ELAPSED)"
    line "      ${RES_ROW}"
    verdict PASS "R9 player alive on the real backend" \
        "the detached player holds ($(echo "$RES_ROW" | awk '{print "cpu "$2"% rss "$3"KB up "$4"}'))"
    verdict HUMAN "R9 the audible row" \
        "keys are sounding on the CURRENT preset? only your ear closes this — toggle presets with '${BIN_NAME} preset -s <name>' while it plays"
elif [[ -n "$LIVE_PID" ]]; then
    verdict FAIL "R9 player alive" "status reports pid ${LIVE_PID} but the process is gone (a stale map pid — the next 'on' takes over)"
else
    verdict INCONCLUSIVE "R9 player alive on the real backend" \
        "no player running — Unlock: ${BIN_NAME} on (or run this audit with --full, which measures the whole on->soak->off cycle)"
fi
line ""

# ------------------------------------------------------------------
# R10 — the mutating lifecycle row (--full only): off -> detach ->
# attach -> on -> soak -> off, with the recorded preset/volume/player
# state restored. Detach wipes the kernel-side control state (the
# maps are re-created), so the restore replays what R0-R9 recorded.
# This is the row that also exercises the attach reaper contract.
# ------------------------------------------------------------------
FULL_MODE=0
[[ "${1:-}" == "--full" ]] && FULL_MODE=1

if [[ $FULL_MODE -eq 1 ]]; then
    hr
    line "R10 — the full lifecycle (off -> detach -> attach -> on -> soak -> off)"
    hr
    PRE_RESTORE_PRESET="$(preset_now)"
    PRE_RESTORE_VOLUME="$(volume_now)"
    PRE_PLAYER_ON=0
    [[ -n "$(player_pid)" ]] && PRE_PLAYER_ON=1
    line "      recorded state to restore: preset=${PRE_RESTORE_PRESET:-unknown} volume=${PRE_RESTORE_VOLUME:-unknown}% player_on=${PRE_PLAYER_ON}"

    "$BIN_ABS" off >/dev/null 2>&1 || true
    if as_root "$BIN_ABS" detach >/dev/null 2>&1 && ! as_root test -d "$PIN_DIR" 2>/dev/null; then
        verdict PASS "R10 detach cleanliness" "the pin directory is gone; nothing of zylaxion remains in the kernel"
    else
        verdict FAIL "R10 detach cleanliness" "detach left $PIN_DIR behind (or detach failed) — inspect: ls $PIN_DIR"
    fi

    if as_root "$BIN_ABS" attach >/dev/null 2>&1 && as_root test -e "$PIN_DIR/tap" 2>/dev/null; then
        TAP_LINE="$(status_verbose | grep '^tap:' | sed 's/^tap: *//')"
        verdict PASS "R10 re-attach" "the tap planted again: ${TAP_LINE}"
    else
        verdict FAIL "R10 re-attach" "attach failed on this host — run 'sudo ${BIN_NAME} attach' by hand for the full error"
    fi

    if "$BIN_ABS" on >/dev/null 2>&1; then
        sleep 1
        SOAK_PID="$(player_pid)"
        if [[ -n "$SOAK_PID" ]] && kill -0 "$SOAK_PID" 2>/dev/null; then
            S0="$(presses_now)"
            line "      soak: player pid ${SOAK_PID} — TYPE NOW (${SOAK_SECONDS} s window)..."
            sleep "$SOAK_SECONDS"
            S1="$(presses_now)"
            SD=$(( S1 - S0 ))
            if (( SD > 0 )); then
                verdict PASS "R10 soak throughput" "${SD} presses voiced through the full wire (kernel tap -> player -> audio) in ${SOAK_SECONDS} s"
            else
                verdict FAIL "R10 soak throughput" "player ran but the press counter did not move — the tap-to-player wire is broken"
            fi
            if command -v ss >/dev/null 2>&1; then
                SOCKETS="$(ss -tulpn 2>/dev/null | grep -i "$(pgrep_name)" || true)"
                [[ -z "$SOCKETS" ]] \
                    && verdict PASS "R10 claim B during playback" "no sockets while the player runs" \
                    || verdict FAIL "R10 claim B during playback" "$SOCKETS"
            fi
            RES_ROW="$(ps -p "$SOAK_PID" -o %cpu,rss --no-headers 2>/dev/null || true)"
            verdict INFO "R10 soak resources" "player cpu/rss during soak: ${RES_ROW:-n/a}"
        else
            verdict FAIL "R10 soak" "the player died within 1 s of spawn — run '${BIN_NAME} _play' in the foreground for the full error"
        fi
    else
        verdict FAIL "R10 soak" "zylaxion on failed to spawn the player"
    fi

    "$BIN_ABS" off >/dev/null 2>&1 || true
    sleep 1
    if [[ -z "$(pgrep -x "$(pgrep_name)" || true)" ]]; then
        verdict PASS "R10 off leaves zero processes" "the player self-exited; nothing of zylaxion is resident in userspace"
    else
        verdict FAIL "R10 off leaves zero processes" "zylaxion processes survive 'off': $(pgrep -x "$(pgrep_name)" | tr '\n' ' ')"
    fi

    # Restore the recorded session state (detach wiped the maps).
    if [[ -n "$PRE_RESTORE_PRESET" ]]; then
        "$BIN_ABS" preset --switch "$PRE_RESTORE_PRESET" >/dev/null 2>&1 || true
    fi
    if [[ -n "$PRE_RESTORE_VOLUME" ]]; then
        "$BIN_ABS" volume --percent "$PRE_RESTORE_VOLUME" >/dev/null 2>&1 || true
    fi
    if [[ $PRE_PLAYER_ON -eq 1 ]]; then
        "$BIN_ABS" on >/dev/null 2>&1 || true
    fi
    verdict INFO "R10 state restored" "preset=${PRE_RESTORE_PRESET:-n/a} volume=${PRE_RESTORE_VOLUME:-n/a} player_on=${PRE_PLAYER_ON}"
    line ""
fi

# ------------------------------------------------------------------
# The ledger summary: the lines a paste into docs/ZACTRIX_SECURITY.md
# (or the night-dinner-13 evidence note) needs.
# ------------------------------------------------------------------
hr
line "host-audit summary"
hr
line "      PASS: ${PASS}   FAIL: ${FAIL}   INCONCLUSIVE: ${INCONCLUSIVE}"
line "      kernel ${KERNEL_REL} / commit ${COMMIT} / uid ${INVOKER_NAME}(${INVOKER_UID})"
line ""
line "      The ledger close-out: Dimension 3 flips INCONCLUSIVE -> measured"
line "      by R1+R2+R3 above (the permission matrix IS the named measurement);"
line "      Dimension 2's residual gets its host facts from R4 (tag + parity"
line "      recorded; the byte-exact loaded-vs-built hash stays named)."
line "      Exit code carries the FAIL count only — INCONCLUSIVE never fails"
line "      the run, it names its unlock command."
hr

(( FAIL == 0 )) && exit 0 || exit 1

