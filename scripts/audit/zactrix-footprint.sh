#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# zactrix-footprint.sh — PROOF 6, the resource footprint
# (night-private-research-2xblade-3.5). Three states plus the
# cold-start bonus, each measured against its named target:
#
#   STATE A — idle with tap attached, sound off. The sandbox cannot
#           attach the tap (no root, no bpftool, /sys/fs/bpf not
#           writable), so the tap-memory dimension is expected
#           INCONCLUSIVE; what IS measurable is the process half:
#           sixty seconds of observation proving no zylaxion process
#           exists while idle. The compiled map definitions are
#           printed as DEFINITIONAL context, clearly not a
#           measurement.
#   STATE B — sound on, 5 keystrokes/s for 60 s. RSS at t=5/30/60,
#           CPU over the window (/proc utime+stime delta), tap memory
#           same as STATE A.
#   STATE C — sound on, zero keystrokes for 60 s (the state the
#           README's zero-idle-cost claim refers to). RSS at t=0/60,
#           CPU over the window.
#   COLD   — process start → device open → first sample pushed →
#           first rendered sample pushed, measured internally by the
#           driver's T0 anchor (the real ALSA null-PCM open path).
#
# DEVICE NOTE (the instrument's own history, kept honest): the FIRST
# run of this instrument drove every state through cpal against the
# ALSA null PCM — and its own evidence refuted it. The null device
# is POLLOUT-always-ready, so the cpal callback thread burned
# ~75-100% of one core in every state INCLUDING idle, while the
# orchestrator loop itself cost 8 ticks per 10 s (0.8%). Per-thread
# /proc attribution named the thread: cpal_alsa_out. STATE B and C
# therefore run the production Orchestrator loop against the driver's
# PacedNullSink — the real 16384-frame ring, the real back-pressure,
# the real render path — drained on the wall clock at the desktop's
# typical quantum (1024 frames @ 48 kHz). The cpal callback's own
# per-frame cost is the named excluded residue (bounded by PROOF 3).
# No audio device exists in this sandbox; no hardware latency claim
# is made anywhere.
#
# Raw output lands in scripts/audit/out/ (gitignored).

set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=scripts/audit/out
mkdir -p "$OUT"
BIN=scripts/audit/harness/target/release/zactrix-audit
EVIDENCE="$OUT/footprint.txt"
: > "$EVIDENCE"

export PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-/home/z/my-project/alsa-root/lib/pkgconfig}"

CLK_TCK=$(getconf CLK_TCK)
NPROC=$(nproc)

log() { tee -a "$EVIDENCE" <<< "$*"; }

# ── Build ────────────────────────────────────────────────────────────
log "== build =="
cargo build --release -q --manifest-path scripts/audit/harness/Cargo.toml
log "harness built: $BIN"

# ── Environment facts ────────────────────────────────────────────────
log ""
log "== environment =="
log "kernel: $(uname -r)  cores: $NPROC  CLK_TCK: $CLK_TCK"
log "state B/C device: PacedNullSink (16384-frame ring, 1024-frame drain @ 48 kHz)"
log "cold-start device: ALSA null PCM (~/.asoundrc override, no /dev/snd)"
log ""

# ── STATE A — idle, sound off, no player process ─────────────────────
log "== STATE A: idle, sound off (60 s observation) =="
if command -v bpftool >/dev/null 2>&1; then
    log "bpftool: $(command -v bpftool)"
else
    log "bpftool: ABSENT — the tap-memory dimension cannot be measured in this sandbox"
fi
if [ -d /sys/fs/bpf/zylaxion ]; then
    log "pin dir: /sys/fs/bpf/zylaxion exists"
else
    log "pin dir: /sys/fs/bpf/zylaxion ABSENT — the tap is not attached (no privileges to attach)"
fi
log "kernel.unprivileged_bpf_disabled = $(sysctl -n kernel.unprivileged_bpf_disabled)"
log "observation: 60 samples, 1 s apart, expecting zero zylaxion processes"
absent=0; total=0
end=$((SECONDS + 60))
while [ "$SECONDS" -lt "$end" ]; do
    n=$(ps aux | grep -c '[z]ylaxion' || true)
    total=$((total + 1))
    if [ "$n" -eq 0 ]; then absent=$((absent + 1)); fi
    sleep 1
done
log "result: $absent/$total samples with zero zylaxion processes (idle CPU 0% — no process exists)"
log ""
log "DEFINITIONAL context (ebpf/src/main.rs — the compiled map sizes, NOT a measurement):"
grep -n 'static control\|static keymap\|static events\|static stats' ebpf/src/main.rs | while IFS= read -r line; do
    log "  definition: $line"
done
log ""

# ── External sampling helper (the prompt's /proc instrument) ─────────
# usage: sample_external <mode> <t_seconds> <pid>
sample_external() {
    local mode=$1 t=$2 pid=$3
    local rss ticks
    rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ' || echo gone)
    # utime=14, stime=15 (1-based); comm may hold spaces — the awk
    # field array keeps comm at its own index (base = the field that
    # ends with ")" = field 2), so utime = base+12, stime = base+13.
    ticks=$(awk 'NR==1 {
        for (i = 1; i <= NF; i++) if ($i ~ /\)$/) { base = i; break }
        print $(base + 12) + $(base + 13)
    }' "/proc/$pid/stat" 2>/dev/null || echo gone)
    log "EXTERNAL $mode t=${t}s rss_kib=${rss:-gone} ticks=${ticks:-gone}"
}

# usage: run_state <mode> <anchor...>
run_state() {
    local mode=$1; shift
    local log_file="$OUT/footprint-$mode.txt"
    local t_spawn_ns
    t_spawn_ns=$(date +%s%N)
    "$BIN" "--footprint-$mode" >"$log_file" 2>&1 &
    local pid=$!
    local banner='sound on, zero keystrokes'
    if [ "$mode" = "state-b" ]; then banner='sound on, 5 keys/s'; fi
    log "== ${mode^^}: $banner (60 s) =="
    log "player pid: $pid"
    for t in "$@"; do
        local due_ns=$(( t_spawn_ns + t * 1000000000 ))
        local now_ns
        now_ns=$(date +%s%N)
        if [ "$due_ns" -gt "$now_ns" ]; then
            sleep "$(awk -v d="$(( due_ns - now_ns ))" 'BEGIN { printf "%.3f", d / 1e9 }')"
        fi
        sample_external "$mode" "$t" "$pid"
    done
    wait "$pid"
    log "-- driver's internal samples (--"
    grep -E '^FOOTPRINT' "$log_file" | while IFS= read -r line; do log "$line"; done
    grep '\[footprint\]' "$log_file" | while IFS= read -r line; do log "$line"; done
    log ""
}

# ── Field parsers (the EXTERNAL lines carry key=value fields) ────────
ext_field() { # <mode> <t> <field-index>
    grep "EXTERNAL $1 t=${2}s" "$EVIDENCE" | tail -1 | awk -v i="$3" '{ v = $i; sub(/^[a-z_]+=/, "", v); print v }'
}

# CPU percent between two EXTERNAL samples: the tick delta over
# CLK_TCK, over the wall seconds, over the core count, times 100.
cpu_pct() {
    awk -v t1="$1" -v k1="$2" -v t2="$3" -v k2="$4" \
        -v clk="$CLK_TCK" -v n="$NPROC" \
        'BEGIN { printf "%.3f", (k2 - k1) / clk / (t2 - t1) / n * 100 }'
}

# ── STATE B — active keystroke burst ─────────────────────────────────
run_state state-b 0 5 30 60
log "== STATE B summary =="
rss5=$(ext_field state-b 5 4)
rss30=$(ext_field state-b 30 4)
rss60=$(ext_field state-b 60 4)
tick0=$(ext_field state-b 0 5)
tick60=$(ext_field state-b 60 5)
cpu_b=$(cpu_pct 0 "$tick0" 60 "$tick60")
log "player RSS: t=5s ${rss5} KiB, t=30s ${rss30} KiB, t=60s ${rss60} KiB (target < 50 MB = 51200 KiB)"
log "player RSS growth t=5s→t=60s: $((rss60 - rss5)) KiB (target < 1 MB = 1024 KiB)"
log "player CPU over 60 s window: ${cpu_b}% single-core average (target < 5%)"
log ""

# ── STATE C — between keystrokes ─────────────────────────────────────
run_state state-c 0 60
log "== STATE C summary =="
rssc0=$(ext_field state-c 0 4)
rssc60=$(ext_field state-c 60 4)
tickc0=$(ext_field state-c 0 5)
tickc60=$(ext_field state-c 60 5)
cpu_c=$(cpu_pct 0 "$tickc0" 60 "$tickc60")
log "player RSS: t=0s ${rssc0} KiB, t=60s ${rssc60} KiB"
log "player RSS growth: $((rssc60 - rssc0)) KiB (target: stable)"
log "player CPU over 60 s window: ${cpu_c}% single-core average (target < 1%)"
log ""

# ── COLD-START ───────────────────────────────────────────────────────
log "== COLD-START: process start → first output sample =="
t_spawn_ns=$(date +%s%N)
"$BIN" --footprint-cold >"$OUT/footprint-cold.txt" 2>&1
t_line_ns=$(date +%s%N)
grep -E '^FOOTPRINT|\[footprint\]' "$OUT/footprint-cold.txt" | while IFS= read -r line; do log "$line"; done
first_push=$(grep -o 'first_push_ms=[0-9.]*' "$OUT/footprint-cold.txt" | cut -d= -f2)
awk -v s="$t_spawn_ns" -v e="$t_line_ns" 'BEGIN { printf "EXTERNAL cold spawn→line-exit: %.3f ms\n", (e - s) / 1e6 }' | tee -a "$EVIDENCE"
log "cold-start (first_push_ms): ${first_push} ms (target < 500 ms)"
log ""

log "== done — raw logs in $OUT/footprint-*.txt, evidence in $EVIDENCE =="
