#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# ZYLAXION CLI UX END-TO-END BATTERY (night-zen-lts-6)
#
# The owner-machine E2E battery: eight batches over the whole CLI
# surface — discovery, write lane, boundaries, typo rescue, env lane,
# telemetry, privilege refusals, lifecycle idempotency — in a SAFE
# order with the session state restored at the end.
#
# Design laws this script keeps (each one fixes a flaw in the draft
# plan it grew out of):
#   1. The detach lane (b7) runs LAST — detaching before the
#      telemetry batch would starve it of keystroke events.
#   2. The 10000% force write (b3) is restored to 100% before any
#      later batch can spawn a player — the stored posture outlives
#      the test otherwise (lifecycle.rs writes VOLUME_PCT_SLOT at
#      spawn; the next `on` would render it).
#   3. The foreground `_play` probe runs under `timeout 5` — a live
#      takeover would otherwise hold the terminal forever.
#   4. Every expectation is written FROM the code it tests (file refs
#      inline), so a green output can never be misread as a bug.
#
# Usage:
#   ./scripts/audit/cli-e2e.sh            # all eight batches + restore
#   ./scripts/audit/cli-e2e.sh b3         # one batch only
#   ZY=./target/debug/zylaxion ./scripts/audit/cli-e2e.sh b1   # another binary
#   ZY_TYPE_SECONDS=40 ./scripts/audit/cli-e2e.sh b8           # longer typing window
#
# Batch 8 asks for ~20 keystrokes in ANOTHER terminal during a sleep
# window; everything else is hands-off. Paste the whole output back
# — the ═══ delimiters and [exit=N] markers make it self-annotated.

set +e  # every failure IS a test case; the script never bails

ZY="${ZY:-zylaxion}"
BATCH="${1:-all}"

r() { echo; echo "═══ $* ═══"; "$@"; echo "[exit=$?]"; }
expect() { echo "── expect: $*"; }
note() { echo "── note: $*"; }

# The state restore replays what the machine looked like before the
# battery: preset, volume (with --force-this above 200), and whether
# the sound was on.
capture_state() {
    local st
    st="$($ZY status 2>/dev/null)"
    ORIG_PRESET="$(printf '%s\n' "$st" | awk '/^  preset:/ {print $2}')"
    ORIG_VOLUME="$(printf '%s\n' "$st" | awk '/^  volume:/ {print $2}' | tr -d '%')"
    if printf '%s\n' "$st" | grep -q 'player:     running'; then
        ORIG_RUNNING=1
    else
        ORIG_RUNNING=0
    fi
    note "captured: preset=${ORIG_PRESET:-<none>} volume=${ORIG_VOLUME:-<none>}% running=$ORIG_RUNNING"
}

restore_state() {
    echo
    echo "═══ restore — back to the captured state ═══"
    if [ -z "${ORIG_PRESET:-}" ]; then
        note "the capture failed — restore manually: zylaxion preset --switch <name>; zylaxion volume -p <n>; zylaxion on"
        return
    fi
    if [ -n "${ORIG_VOLUME:-}" ] && [ "$ORIG_VOLUME" -gt 200 ] 2>/dev/null; then
        r $ZY volume -p "$ORIG_VOLUME" --force-this
    else
        r $ZY volume -p "${ORIG_VOLUME:-100}"
    fi
    r $ZY preset --switch "$ORIG_PRESET"
    if [ "$ORIG_RUNNING" = "1" ]; then
        r $ZY on
    fi
    r $ZY status
}

# ── Batch 1 — discovery + read surface (no state change) ──────────
b1() {
    expect "quick status overview, exit 0 — not an error dump"
    r $ZY
    r $ZY -h
    r $ZY --help
    r $ZY -V
    r $ZY --version
    expect "24 presets, the <- active marker on the live row"
    r $ZY list-presets
    r $ZY list-backends
    r $ZY status
    r $ZY status -v
    r $ZY status --verbose
    expect "build facts + kernel/eBPF verdict + audio; honest degraded rows where the host is short"
    r $ZY doctor
    expect "depth matrix D0-D3, status=verified, exit 0"
    r $ZY sig
    note "NO_COLOR must strip ANSI on every surface. Counted mechanically (a pipe may TTY-gate the colored lane — if both counts are 0, eyeball it on your terminal):"
    printf "── NO_COLOR=1 -h ansi-escape count (expect 0): "
    NO_COLOR=1 $ZY -h | grep -c $'\x1b'
    printf "── plain -h ansi-escape count (expect > 0, or TTY-gated): "
    $ZY -h | grep -c $'\x1b'
}

# ── Batch 2 — write surface + live switch ─────────────────────────
b2() {
    expect "off when already off: idempotent, exit 0 — the gate clears, no error (lifecycle.rs)"
    r $ZY off
    r $ZY status
    expect "on: spawns the player; status shows a fresh pid"
    r $ZY on
    sleep 1
    r $ZY status
    expect "live switch lands on BOTH spellings; status reflects without a restart"
    r $ZY preset --switch topre
    r $ZY preset -s ibm_model_m
    r $ZY status -v
    expect "volume 60 then 100 land; status reflects"
    r $ZY volume --percent 60
    r $ZY volume -p 100
    r $ZY status
    expect "off: player not running; pgrep shows NO zylaxion _play line (the stateless-player law)"
    r $ZY off
    r $ZY status
    r pgrep -af zylaxion
    note "pgrep [exit=1] with no output IS the green case here"
}

# ── Batch 3 — volume boundaries (sound off: writes only, no ears) ─
b3() {
    note "the player is off by design: boundary writes land in the map, nothing renders"
    expect "rc 2 refusal naming the 200% cap and --force-this (preset.rs volume_gate)"
    r $ZY volume -p 201
    expect "rc 2: nothing above the force ceiling passes (the 10000 law)"
    r $ZY volume -p 10001 --force-this
    expect "rc 2 clap parse error: unexpected argument"
    r $ZY volume -p -1
    expect "rc 2 clap value parse error"
    r $ZY volume -p abc
    expect "rc 0: 0% is a legal mute — the synopsis's <0-200> holds at the floor"
    r $ZY volume -p 0
    expect "rc 0: the 200% cap boundary itself is legal"
    r $ZY volume -p 200
    expect "rc 0: 250 lands with --force-this"
    r $ZY volume -p 250 --force-this
    expect "rc 0: the 10000 force-ceiling boundary itself lands (map stores gain 550.0, render-time clamped)"
    r $ZY volume -p 10000 --force-this
    note "RESTORE to 100% NOW — a stored 10000% would render on the next spawn (b5/b8 both spawn)"
    r $ZY volume -p 100
}

# ── Batch 4 — typo rescue ─────────────────────────────────────────
b4() {
    expect "tip: did you mean 'topre'? (preset.rs: closest_value_match + the tip line)"
    r $ZY preset --switch topr
    r $ZY preset --switch toprre
    expect "NOT a silent success — the registry is EXACT-CASE; the rescue is the tip, the verdict is still an error"
    r $ZY preset --switch TOPRE
    expect "the clap flag rescue (jaro_ci): did you mean --switch?"
    r $ZY preset --swich topre
    expect "subcommand rescue: did you mean volume?"
    r $ZY volum -p 50
    expect "subcommand rescue: did you mean status?"
    r $ZY stat
}

# ── Batch 5 — env surface (the no-config law) ─────────────────────
b5() {
    expect "on succeeds from the env preset; status reflects it; off"
    r env ZYLAXION_PRESET=topre $ZY on
    sleep 1
    r $ZY status
    r $ZY off
    expect "rc != 0 hard error — unknown env preset (pinned by preset_tests.rs)"
    r env ZYLAXION_PRESET=notreal $ZY on
    expect "on succeeds; the spawn writes the env gain into the map (lifecycle.rs:126), so status shows 182%"
    r env ZYLAXION_VOLUME=10.0 $ZY on
    sleep 1
    r $ZY status
    r $ZY off
    expect "rc != 0 hard error above the env ceiling (MAX_VOLUME_GAIN 10.0; no force escape on this lane)"
    r env ZYLAXION_VOLUME=10.01 $ZY on
    expect "rc != 0 hard error: invalid ZYLAXION_VOLUME value 'abc'"
    r env ZYLAXION_VOLUME=abc $ZY on
    note "RESTORE the map volume — the 10.0 spawn left 182% stored"
    r $ZY volume -p 100
}

# ── Batch 8 — telemetry + the foreground player (run BEFORE b7!) ──
b8() {
    r $ZY on
    sleep 1
    note "NOW: type ~20 keystrokes in ANOTHER terminal — the tap feeds the kernel counters as you type"
    echo "── waiting ${ZY_TYPE_SECONDS:-20}s for your keystrokes..."
    sleep "${ZY_TYPE_SECONDS:-20}"
    expect "total presses >= your keystrokes; top keys rendered; kernel-side accumulation visible"
    r $ZY status -v
    r $ZY off
    expect "the player dies to pkill; the pid is real"
    r $ZY on
    sleep 1
    r pkill -f 'zylaxion _play'
    sleep 1
    expect "foreground _play under timeout 5: either a clean takeover run ended by the timeout, or a FULL death reason — never silence, never an infinite hold"
    r timeout 5 $ZY _play
    r $ZY off
}

# ── Batch 6 — privilege refusals (root, on purpose) ───────────────
b6() {
    expect "REFUSE: the do-not-use-sudo verdict (entry 26; pinned by sound_tests.rs) — on AND _play"
    sudo -u root $ZY on
    sudo -u root $ZY _play
    expect "allowed: rescue verbs work as root, exit 0"
    sudo $ZY off
    sudo $ZY status
}

# ── Batch 7 — lifecycle idempotency (runs LAST; leaves the tap planted) ──
b7() {
    expect "clean attach — pins present stay planted, telemetry preserved (attach.rs)"
    sudo systemctl start zylaxion-attach 2>/dev/null
    sudo $ZY attach
    expect "second attach: idempotent, no error"
    sudo $ZY attach
    expect "not attached, with the honest errno reason (entry 28: ENOENT, not EACCES)"
    sudo $ZY detach
    r $ZY status
    expect "detach again: the idempotent no-op success (privilege.rs: pins absent = no-op, whatever the euid)"
    sudo $ZY detach
    note "RESTORE: re-plant the tap — the machine must be left exactly as it was found"
    sudo $ZY attach
}

case "$BATCH" in
    all)
        capture_state
        b1; b2; b3; b4; b5; b8; b6; b7
        restore_state
        echo
        echo "═══ battery complete — paste the whole output back ═══"
        ;;
    b1|b2|b3|b4|b5|b6|b7|b8)
        capture_state
        "$BATCH"
        echo
        echo "═══ batch $BATCH complete (state restore ran only in the full battery) ═══"
        ;;
    *)
        echo "usage: $0 [all|b1|b2|b3|b4|b5|b6|b7|b8]" >&2
        exit 2
        ;;
esac
