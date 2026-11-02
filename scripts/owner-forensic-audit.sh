#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zylaxion owner forensic audit — DEPTHBORE-1
#
# Five unprivileged tests against the real machine: real BPF tap, real
# audio, real desktop session. Run as the user who runs `zylaxion on`
# (never root — the player refuses root by design). The tap must
# already be attached (`sudo zylaxion attach`, once per boot).
#
# Every test prints its raw measurements next to the verdict, so the
# output is evidence, not a verdict to trust blindly. No jq, no
# nonstandard tools: bash, procfs, ps, getconf.
#
# What each test proves:
#   1. idle CPU       — the "zero idle cost" claim on YOUR hardware
#   2. active CPU     — the honest cost of actually producing sound
#   3. memory         — the player's footprint (RSS/PSS)
#   4. SIGKILL        — kill -9 recovery: no zombie, stale pid taken
#                       over, no second player ever spawned
#   5. signal stop    — SIGTERM exits cleanly AND status reads
#                       "sound: off" afterwards (the gate must not
#                       outlive the player it gates)
#
# Exit code: 0 when every test passes, 1 otherwise.

set -u

BINARY="${ZYLAXION_BIN:-zylaxion}"
EXE="$(command -v "$BINARY" 2>/dev/null || true)"
CLK_TCK="$(getconf CLK_TCK)"
FAILS=0

say()  { printf '%s\n' "$*"; }
rule() { printf '%s\n' "------------------------------------------------------------"; }

# cpu_ticks PID -> "<utime> <stime>" from /proc/PID/stat (fields 14,15)
cpu_ticks() {
    awk '{print $14, $15}' "/proc/$1/stat" 2>/dev/null || echo "0 0"
}

# pct_of_core <tick_delta> <seconds> -> percent of ONE core
pct_of_core() {
    awk -v t="$1" -v hz="$CLK_TCK" -v s="$2" 'BEGIN { if (s <= 0) { print "nan" } else { printf "%.2f", 100.0 * t / hz / s } }'
}

# resolve_player -> the live player pid, verified to carry the player cmdline
resolve_player() {
    local pid
    pid="$(pgrep -xf "$EXE _play" | head -1)"
    if [ -n "$pid" ] && tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null | grep -q "_play"; then
        printf '%s' "$pid"
    fi
}

check() { # check <label> <ok:0/1> <detail...>
    if [ "$2" -eq 0 ]; then
        say "  PASS: $1 — $3"
    else
        say "  FAIL: $1 — $3"
        FAILS=$((FAILS + 1))
    fi
}

[ -n "$EXE" ] || { say "zylaxion binary not found in PATH (set ZYLAXION_BIN)"; exit 1; }

say "zylaxion owner forensic audit"
say "binary: $EXE    CLK_TCK: $CLK_TCK    user: $(id -un)"
if [ "$(id -u)" -eq 0 ]; then
    say "WARNING: running as root — the player refuses root; re-run as the desktop user."
    exit 1
fi
rule

# ── TEST 1: idle CPU ────────────────────────────────────────────────
say "TEST 1: idle CPU — spawn, then a 10 s silent window (DO NOT TYPE)"
"$EXE" on >/dev/null || { say "  FAIL: zylaxion on failed"; exit 1; }
sleep 3
PID="$(resolve_player)"
if [ -z "$PID" ]; then
    say "  FAIL: no player process found after on"
    exit 1
fi
say "  player pid: $PID"
set -- $(cpu_ticks "$PID"); T1=$(( $1 + $2 ))
sleep 10
set -- $(cpu_ticks "$PID"); T2=$(( $1 + $2 ))
IDLE_PCT="$(pct_of_core $((T2 - T1)) 10)"
PS_CPU="$(ps -o %cpu= -p "$PID" 2>/dev/null | tr -d ' ')"
say "  measured: ${IDLE_PCT}% of one core over 10 s (ps reads ${PS_CPU}%)"
check "idle cost below 1% of one core (the fixed loop; a poll reads 5-100%)" \
      "$(awk -v v="$IDLE_PCT" 'BEGIN { print (v < 1.0) ? 0 : 1 }')" \
      "idle ${IDLE_PCT}%"
say "  thread inventory (every line must be S — sleeping, never R):"
ps -T -p "$PID" -o spid,state,comm= | sed 's/^/    /'
rule

# ── TEST 2: active CPU ──────────────────────────────────────────────
say "TEST 2: ACTIVE CPU — TYPE CONTINUOUSLY ANYWHERE FOR 10 SECONDS"
say "  (start typing as soon as you press Enter; sound must be audible)"
printf '%s' "  press Enter to start the window: "
read -r _
set -- $(cpu_ticks "$PID"); T1=$(( $1 + $2 ))
sleep 10
set -- $(cpu_ticks "$PID"); T2=$(( $1 + $2 ))
ACTIVE_PCT="$(pct_of_core $((T2 - T1)) 10)"
say "  measured: ${ACTIVE_PCT}% of one core while producing sound"
say "  (sound production is real work: the LTS certification measured"
say "   2.15% on its Xeon; a spin would read near 100%)"
check "active cost below 10% of one core (regression catcher, not a perf gate)" \
      "$(awk -v v="$ACTIVE_PCT" 'BEGIN { print (v < 10.0) ? 0 : 1 }')" \
      "active ${ACTIVE_PCT}%"
rule

# ── TEST 3: memory footprint ────────────────────────────────────────
say "TEST 3: memory footprint of the player"
RSS_KB="$(ps -o rss= -p "$PID" 2>/dev/null | tr -d ' ')"
PSS_KB="$(awk '/^Pss:/ {print $2}' "/proc/$PID/smaps_rollup" 2>/dev/null)"
say "  RSS: ${RSS_KB} KiB    PSS: ${PSS_KB:-n/a} KiB"
check "RSS below 50 MB (the LTS target)" \
      "$(awk -v v="${RSS_KB:-999999}" 'BEGIN { print (v < 51200) ? 0 : 1 }')" \
      "RSS ${RSS_KB} KiB"
rule

# ── TEST 4: SIGKILL recovery ────────────────────────────────────────
say "TEST 4: SIGKILL — kill -9 the player mid-flight"
OLD_PID="$PID"
kill -9 "$PID" 2>/dev/null
sleep 1
STATE="$(ps -o stat= -p "$PID" 2>/dev/null | tr -d ' ')"
if [ -z "$STATE" ]; then
    say "  the killed pid is fully gone (reaped by init/the session manager — no zombie)"
    check "no zombie left behind" 0 "state: gone"
elif [ "$STATE" = "Z" ]; then
    check "no zombie left behind" 1 "pid $PID is a ZOMBIE — its parent never reaped it"
else
    check "no zombie left behind" 1 "pid $PID still exists with state $STATE"
fi
say "  map truth after the kill:"
"$EXE" status 2>/dev/null | grep -E "sound:|player:" | sed 's/^/    /'
say "  recovery: on takes over from the stale registration"
"$EXE" on >/dev/null || { say "  FAIL: recovery on failed"; FAILS=$((FAILS + 1)); }
sleep 2
NEW_PID="$(resolve_player)"
if [ -n "$NEW_PID" ] && [ "$NEW_PID" != "$OLD_PID" ]; then
    check "a fresh player took over (new pid $NEW_PID, old $OLD_PID)" 0 "single player"
elif [ -n "$NEW_PID" ]; then
    check "a fresh player took over" 1 "pid unchanged ($NEW_PID) — takeover did not spawn a new player"
else
    check "a fresh player took over" 1 "no player running after recovery on"
fi
COUNT="$(pgrep -xf "$EXE _play" | wc -l)"
check "exactly one player exists" "$(( COUNT == 1 ? 0 : 1 ))" "count: $COUNT"
rule

# ── TEST 5: signal-driven stop truth ────────────────────────────────
say "TEST 5: SIGTERM stop — the gate must not outlive the player"
PID="$(resolve_player)"
[ -n "$PID" ] || PID="$NEW_PID"
kill -TERM "$PID" 2>/dev/null
sleep 2
if [ -n "$(resolve_player)" ]; then
    check "player exited on SIGTERM" 1 "pid $PID still running"
else
    check "player exited on SIGTERM" 0 "process gone within 2 s (fade-out + unregister)"
fi
SOUND_LINE="$("$EXE" status 2>/dev/null | grep 'sound:')"
say "  status after the stop: $SOUND_LINE"
check "status reads sound: off after a signal stop" \
      "$(printf '%s' "$SOUND_LINE" | grep -q 'off' && echo 0 || echo 1)" \
      "$SOUND_LINE"
"$EXE" off >/dev/null 2>&1
LEFT="$(pgrep -xf "$EXE _play" | wc -l)"
check "zero zylaxion processes remain" "$(( LEFT == 0 ? 0 : 1 ))" "remaining: $LEFT"
rule

say "summary: $FAILS failed check(s)"
say "(optional deep lane, from a source checkout:"
say "  cargo run --release --example idle_cpu_sanity  — idle + active regime guards)"
[ "$FAILS" -eq 0 ] || exit 1
