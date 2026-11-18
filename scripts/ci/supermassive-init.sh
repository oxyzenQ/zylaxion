#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
#
# The supermassive init (night-master-3, the zelynic lineage's shape):
# PID 1 inside the KVM micro-VM booted by
# .github/workflows/supermassive.yml. The bring-up half runs on the
# runner (the pinned toolchain, the eBPF prerequisites, the exact
# release-shape build — a guest cannot host a Rust toolchain); THIS
# script is the other half — the zero-to-hero E2E inside the guest:
# the kernel surfaces, the staged sound/input modules, the install
# step (the payload lands at /usr/bin/zylaxion the way
# install-on-my-machine.sh lands it), then the engine's --full
# battery — CLI/UX, the tap lifecycle, the night-dinner-3 repro (the
# mode-0700 front door, the attach self-heal, the verdict honesty
# law), and the real-usage chain with a real user who is in no input
# group.
#
# Contract: one MASS-RESULT line per probe plus a final MASS-VERDICT
# line, all on the serial console (the workflow's Verdict step greps
# these sentinels; the VM cannot relay exit codes through qemu, the
# sentinel lines ARE the relay — the zelynic kernel-floor contract,
# kept verbatim). The engine (scripts/supermassive/supermassive-test.py)
# prints its own MASS rows under the same contract; this init owns the
# surfaces the engine cannot see: the mounts, the module chain, the
# install step.
#
# PID-1 rules (the zelynic kernel-floor lessons, kept): no systemd
# here — this script owns every mount; it must NEVER exit without
# ending the VM, and the exit rides qemu's isa-debug-exit port device
# (reboot(2) poweroff silently no-ops in a direct-boot guest) — see
# guest_exit.

set -u
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

# The root-owned __pycache__ wall, shell layer (night-cafe-1): any
# interpreter this script spawns inherits dont-write-bytecode, so no
# repo-local bytecode lands beside the scripts at any privilege level
# (the sudo lane's per-file guards carry the same law — sudo's
# env_reset strips environment guards before the interpreter starts).
export PYTHONDONTWRITEBYTECODE=1

FAILURES=0

note() {
        echo "MASS-RESULT: $1 — $2"
        if [ "$2" != "PASS" ]; then
                FAILURES=$((FAILURES + 1))
        fi
}

guest_exit() {
        # The deterministic VM exit (the zelynic run-4 lesson): qemu's
        # isa-debug-exit device ends the VM the moment one byte lands on
        # port 0xf4; /dev/port is the root-writable port-I/O window
        # (seek = the port number), and the written value 0 makes qemu
        # exit with status 1 — the rc the workflow's boot step accepts as
        # the guest's deliberate exit, the MASS-* sentinel lines being
        # the verdict.
        #
        # The drain beat (night-improve-27): the tail rows and the
        # verdict ride the kernel's console ring and the emulated UART,
        # and qemu ends the guest the instant the port byte lands — on
        # a fast runner that race beat the serial drain, cutting vm.log
        # mid-row and hiding the verdict from the step that greps for
        # it (the flaky E2E leg, proven by reruns passing unchanged).
        # Two seconds lets every console byte land before the exit.
        sleep 2
        printf '\x00' | dd of=/dev/port bs=1 seek=244 count=1 2>/dev/null || true
        # Belts, in order: the raw poweroff syscall (if this guest ever
        # grows an ACPI S5 path), then the sleep loop (never spin the
        # runner's budget — the outer timeout owns the last resort).
        python3 -c 'import os; os.sync(); os.reboot(os.LINUX_REBOOT_CMD_POWER_OFF)' 2>/dev/null || true
        while true; do sleep 60; done
}

Z=/opt/zylaxion
if ! cd "$Z"; then
        # A PID 1 must never plain-exit (the guest would panic-wedge):
        # exit through the port device with the verdict already naming
        # the failure.
        echo "MASS-VERDICT: FAIL (cannot cd $Z — rootfs assembly bug)"
        guest_exit
fi

# ── the kernel surfaces, before anything that needs them ──────────────
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs dev /dev
exec >/dev/console 2>&1
# night-dragon-maintenance-4: the boot evidence line carries the REAL
# kernel release and the leg name (the old line was corrupted static
# text, "1001 1001uname -r)", evidence of nothing). The leg rides the
# kernel command line (massleg=), the same channel the console and
# panic flags ride - no rootfs rebuild per leg.
leg=floor-or-latest
for word in $(cat /proc/cmdline 2>/dev/null); do
        case "$word" in
        massleg=*) leg="${word#massleg=}" ;;
        esac
done
echo "MASS: init booted, kernel $(uname -r), leg ${leg}"

# A hostname for the guest (the zed lane's sudo prints an
# "unable to resolve host" line on every invocation in a hostless
# guest — noise in every captured row; one line kills it).
hostname zylaxion-vm 2>/dev/null || true
echo "127.0.0.1 zylaxion-vm" >> /etc/hosts 2>/dev/null || true

# The floor itself — the whole reason this VM exists. zylaxion's
# documented floor is 5.15 (docs and the doctor's verdict). The legs
# witness BOTH directions of the floor law (night-dragon-maintenance-4,
# the zejtron kernel-witnesses lineage):
#   floor       - the true 5.15 jammy kernel: at-floor, and the EXACT
#                 family witness (5.15.*) - a drift to a newer minor
#                 means the resolver broke and the leg says so loudly;
#   latest      - the newest kernel the archive serves: above-floor;
#   below-floor - the focal 5.4 GA kernel: BELOW the floor, and the
#                 leg's battery is the REFUSAL walk - doctor, attach,
#                 and on must all refuse honestly naming the floor,
#                 the direction every other lane can only compute.
# night-runner-1 adds the LTS-span witness seats, each with the same
# exact-family law (a resolver drift fails loudly instead of quietly
# testing the wrong kernel):
#   lts-6.1     - the Debian bookworm 6.1.0-50 family (the pinned
#                 artifact the BTF matrix already rides);
#   lts-6.6     - the Arch LTS 6.6.72-1 family (the second pin);
#   lts-6.8     - the noble GA 6.8.0-31 family (the frozen pocket).
kver=$(uname -r)
major=${kver%%.*}
minor=$(echo "$kver" | cut -d. -f2 | grep -oE '^[0-9]+' || echo 0)
at_or_above_floor=false
if [ "${major:-0}" -gt 5 ] || { [ "${major:-0}" -eq 5 ] && [ "${minor:-0}" -ge 15 ]; }; then
        at_or_above_floor=true
fi
case "$leg" in
below-floor)
        if [ "$at_or_above_floor" = false ]; then
                note "kernel $kver is below the 5.15 documented floor (the refusal world, expected)" PASS
        else
                note "kernel (uname -r = $kver, wanted BELOW 5.15 — the below-floor leg booted a supported kernel, the witness is void)" FAIL
        fi
        ;;
floor)
        if [ "$at_or_above_floor" = true ] && [ "${kver#5.15.}" != "$kver" ]; then
                note "kernel $kver (the 5.15 documented floor, the exact GA family)" PASS
        elif [ "$at_or_above_floor" = true ]; then
                note "kernel (uname -r = $kver, above the floor but NOT the 5.15 family — the floor leg's resolver drifted)" FAIL
        else
                note "kernel (uname -r = $kver, wanted >= 5.15)" FAIL
        fi
        ;;
lts-6.1)
        if [ "$at_or_above_floor" = true ] && [ "${kver#6.1.}" != "$kver" ]; then
                note "kernel $kver (the 6.1 LTS family, the Debian bookworm witness)" PASS
        elif [ "$at_or_above_floor" = true ]; then
                note "kernel (uname -r = $kver, above the floor but NOT the 6.1 family — the leg's staging drifted)" FAIL
        else
                note "kernel (uname -r = $kver, wanted >= 5.15)" FAIL
        fi
        ;;
lts-6.6)
        if [ "$at_or_above_floor" = true ] && [ "${kver#6.6.}" != "$kver" ]; then
                note "kernel $kver (the 6.6 LTS family, the Arch LTS witness)" PASS
        elif [ "$at_or_above_floor" = true ]; then
                note "kernel (uname -r = $kver, above the floor but NOT the 6.6 family — the leg's staging drifted)" FAIL
        else
                note "kernel (uname -r = $kver, wanted >= 5.15)" FAIL
        fi
        ;;
lts-6.8)
        if [ "$at_or_above_floor" = true ] && [ "${kver#6.8.}" != "$kver" ]; then
                note "kernel $kver (the 6.8 GA family, the noble witness)" PASS
        elif [ "$at_or_above_floor" = true ]; then
                note "kernel (uname -r = $kver, above the floor but NOT the 6.8 family — the leg's staging drifted)" FAIL
        else
                note "kernel (uname -r = $kver, wanted >= 5.15)" FAIL
        fi
        ;;
*)
        if [ "$at_or_above_floor" = true ]; then
                note "kernel $kver (the 5.15+ documented floor)" PASS
        else
                note "kernel (uname -r = $kver, wanted >= 5.15)" FAIL
        fi
        ;;
esac

# The below-floor leg is the refusal witness: no tap can attach, so
# the module/sound staging and the whole-chaos engine are void. The
# walk below proves the refusal direction end to end instead.
if [ "$leg" = "below-floor" ]; then
        if install -Dm755 "$Z/zylaxion" /usr/bin/zylaxion &&
                /usr/bin/zylaxion -V >/dev/null 2>&1; then
                note "the payload is installed and answers -V at /usr/bin/zylaxion" PASS
        else
                note "install -Dm755 \$Z/zylaxion /usr/bin/zylaxion" FAIL
        fi

        # The kernel surfaces are all the refusal walk needs.
        # doctor: rc 1 naming the floor (the kernel row is the first
        # failure; the report keeps its other rows honest).
        doctor_out=$(/usr/bin/zylaxion doctor 2>&1); doctor_rc=$?
        if [ "$doctor_rc" -eq 1 ] && echo "$doctor_out" | grep -q "below the 5.15 minimum"; then
                note "doctor refuses below the floor (rc 1, the 5.15 minimum named)" PASS
        else
                note "doctor refuses below the floor (rc $doctor_rc, wanted 1 + 'below the 5.15 minimum')" FAIL
        fi
        # attach: the kernel gate fires before any BPF syscall.
        attach_out=$(/usr/bin/zylaxion attach 2>&1); attach_rc=$?
        if [ "$attach_rc" -ne 0 ] && echo "$attach_out" | grep -q "below the 5.15 minimum"; then
                note "attach refuses below the floor (rc $attach_rc, the floor named before the chain)" PASS
        else
                note "attach refuses below the floor (rc $attach_rc, wanted non-zero + 'below the 5.15 minimum')" FAIL
        fi
        # on: the spawn door refuses with the canonical verdict. The
        # verb runs AS A REAL USER — as root the privilege refusal
        # would fire before the kernel gate ever speaks, and the
        # witness wants the KERNEL refusal direction, not the root
        # door (the battery's no-input-group law, the same shape).
        groupadd -f audio 2>/dev/null || true
        useradd -m -s /bin/bash zed 2>/dev/null || true
        usermod -aG audio zed 2>/dev/null || true
        on_out=$(sudo -u zed /usr/bin/zylaxion on 2>&1); on_rc=$?
        if [ "$on_rc" -eq 1 ] && echo "$on_out" | grep -q "verified unsupported"; then
                note "on refuses below the floor (rc 1, verified unsupported)" PASS
        else
                note "on refuses below the floor (rc $on_rc, wanted 1 + 'verified unsupported')" FAIL
        fi
        # status: still answers honestly on a dying kernel - the
        # posture report is the one surface that never refuses.
        status_out=$(/usr/bin/zylaxion status 2>&1); status_rc=$?
        if [ "$status_rc" -eq 0 ] && echo "$status_out" | grep -q "verified unsupported"; then
                note "status answers honestly below the floor (rc 0, the verdict named)" PASS
        else
                note "status answers honestly below the floor (rc $status_rc, wanted 0 + 'verified unsupported')" FAIL
        fi

        if [ "$FAILURES" -eq 0 ]; then
                echo "MASS-VERDICT: PASS (the refusal direction proven on kernel $(uname -r))"
        else
                echo "MASS-VERDICT: FAIL (refusal walk: $FAILURES failed)"
        fi
        guest_exit
fi

# Everything the batteries need (PID 1 owns them all): tmpfs scratch,
# bpffs (the pin surface), devpts (the pseudo-terminal shape sudo's
# PAM stack probes), loopback. No egress: the battery is offline by
# design — zylaxion needs no network, and a deterministic guest beats
# a wired one.
if mount -t tmpfs tmp /tmp &&
        mount -t tmpfs run /run &&
        mkdir -p /sys/fs/cgroup /sys/fs/bpf /dev/pts &&
        mount -t cgroup2 none /sys/fs/cgroup 2>/dev/null &&
        mount -t bpf bpf /sys/fs/bpf &&
        mount -t devpts devpts /dev/pts &&
        ip link set lo up 2>/dev/null; then
        note "kernel surfaces (proc, sysfs, devtmpfs, bpffs, devpts, loopback)" PASS
else
        note "kernel surfaces" FAIL
fi

# The unprivileged-bpf posture, named for the archive (informational —
# a kernel policy is not a defect): Ubuntu builds the gate in
# (CONFIG_BPF_UNPRIV_DEFAULT_OFF=y on both reference kernels), so the
# guest boots with kernel.unprivileged_bpf_disabled=2 — unprivileged
# bpf() is refused for the whole boot and root cannot re-enable it.
# The engine's real-user lane reads the same fact and tests what the
# kernel honestly allows (night-badas-4 round 5).
echo "MASS: unprivileged bpf posture: kernel.unprivileged_bpf_disabled=$(cat /proc/sys/kernel/unprivileged_bpf_disabled 2>/dev/null || echo absent)"

# ── the staged module chain (the guest's sound and keyboard) ──────────
# The workflow extracts the .ko files from the SAME kernel's module
# debs (same-suite vermagic by construction) into $Z/modules/. The
# insmod order is the dependency order: soundcore -> snd -> snd-timer
# -> snd-pcm -> snd-dummy. uinput is the special case (night-badas-4
# round 2, verified against jammy 5.15.0-198 and noble 7.0.0-38): both
# kernels build uinput INTO vmlinuz (modules.builtin carries
# kernel/drivers/input/misc/uinput.ko in both; no .ko ships in any
# deb), so a missing uinput.ko is the expected shape. Round 5 (same
# two kernels, run 38375322712): a built-in module WITHOUT module
# parameters does not populate /sys/module at all — the driver is
# demonstrably alive (/dev/uinput exists via devtmpfs) while
# /sys/module/uinput is absent. The honest evidence is therefore the
# driver's device surfaces: /dev/uinput or /sys/class/misc/uinput,
# with /sys/module/uinput kept for kernels that do populate it. The
# workflow stages a uinput.ko when the archive ships one (a future
# series may un-inline it).
MODDIR="$Z/modules"
for mod in soundcore snd snd-timer snd-pcm snd-dummy uinput; do
        KO=$(find "$MODDIR" -name "${mod}.ko*" 2>/dev/null | head -1)
        if [ -z "$KO" ]; then
                if [ "$mod" = "uinput" ] &&
                        { [ -e /sys/module/uinput ] || [ -e /dev/uinput ] ||
                                [ -e /sys/class/misc/uinput ]; }; then
                        note "module uinput built into the kernel (no .ko staged)" PASS
                        continue
                fi
                note "module $mod staged from the kernel's own debs" FAIL
                continue
        fi
        if insmod "$KO" 2>/dev/null || grep -q "$mod" /proc/modules; then
                note "module $mod loaded" PASS
        else
                note "module $mod (insmod $(basename "$KO"))" FAIL
        fi
done

# night-runner-1, the Debian serio split: i8042 rides as a module on
# bookworm while atkbd is builtin — without it the PS/2 port binds
# nothing and no /dev/input event node ever exists, so the real-user
# law reads a false SKIP. Load it when the workflow staged one; the
# kernels that build the chain in stage nothing, which is the honest
# no-op shape here.
I8042_KO=$(find "$MODDIR" -name "i8042.ko*" 2>/dev/null | head -1)
if [ -n "$I8042_KO" ]; then
        if insmod "$I8042_KO" 2>/dev/null || grep -q i8042 /proc/modules; then
                note "module i8042 loaded (the Debian serio split, atkbd builtin binds the port)" PASS
        else
                note "module i8042 (insmod $(basename "$I8042_KO"))" FAIL
        fi
else
        echo "MASS: no i8042.ko staged (this kernel builds the serio chain in)"
fi

# The sound card is the PCM witness's stage: snd-dummy must have
# registered card 0 before the engine's audio probes run.
sleep 1
if [ -e /proc/asound/card0 ]; then
        note "the ALSA sound card registered (/proc/asound/card0)" PASS
else
        note "the ALSA sound card registered (/proc/asound/card0)" FAIL
fi
if [ -e /dev/uinput ]; then
        note "the uinput device node is present (/dev/uinput)" PASS
else
        note "the uinput device node is present (/dev/uinput)" FAIL
fi

# ── the sound node posture (night-dinner-5d) ──────────────────────────
# udev never runs in an initramfs guest, and the desktop's /dev/snd
# posture (root:audio 0660) is a udev rule's work on a real machine —
# without it the nodes land root:root 0600 and every user-side ALSA
# open answers "cannot find card '0'" no matter which groups the user
# carries (the escalator row proved the exact string on the first
# live chain). A udev-less appliance applies the posture by hand; the
# engine's audio group (the user half) needs exactly this node half.
groupadd -f audio 2>/dev/null || true
if [ -d /dev/snd ] && ls /dev/snd/* >/dev/null 2>&1; then
        if chgrp audio /dev/snd/* 2>/dev/null &&
                chmod 660 /dev/snd/* 2>/dev/null; then
                note "the sound nodes carry the desktop posture (root:audio 0660)" PASS
        else
                note "the sound nodes carry the desktop posture (root:audio 0660)" FAIL
        fi
else
        note "the sound nodes exist under /dev/snd" FAIL
fi

# ── the install step (zero to hero: the payload lands in /usr/bin) ────
# The way install-on-my-machine.sh lands it, minus systemd (no init
# system lives in an initramfs guest — the oneshot unit's presence in
# the staged tree is asserted by the engine's tree walk instead).
if install -Dm755 "$Z/zylaxion" /usr/bin/zylaxion; then
        if /usr/bin/zylaxion -V >/dev/null 2>&1; then
                note "the payload is installed and answers at /usr/bin/zylaxion" PASS
        else
                note "the payload answers -V at /usr/bin/zylaxion" FAIL
        fi
else
        note "install -Dm755 \$Z/zylaxion /usr/bin/zylaxion" FAIL
fi

# ── the counter witness (night-dragon-lts) ─────────────────────────────
# The churn catch reads the tap's run counters (sm_storm calls
# `bpftool prog show pinned ...`), but noble ships bpftool as a
# uname-keyed wrapper: /usr/sbin/bpftool execs
# /usr/lib/linux-tools/$(uname -r)/bpftool and exits 2 for any other
# kernel — and this guest boots ARCHIVE kernels no noble tools package
# names. The versioned binary itself is kernel-agnostic (prog show
# pinned / link show pinned ride stable ioctls), so the init promotes
# the highest installed versioned binary onto PATH; sm_storm's bare
# `bpftool` resolves through /usr/local/bin and the instrument's
# evidence lines stay verbatim. Best-effort: a guest image without any
# versioned binary keeps today's rc=127 degradation, named as the
# informational line it is.
BPFTOOL_BIN="$(ls /usr/lib/linux-tools/*/bpftool 2>/dev/null | sort -V | tail -1)"
if [ -n "$BPFTOOL_BIN" ]; then
        if ln -sf "$BPFTOOL_BIN" /usr/local/bin/bpftool; then
                note "the counter witness rides bpftool ($BPFTOOL_BIN)" PASS
        else
                note "bpftool symlink onto /usr/local/bin" FAIL
        fi
else
        echo "MASS: bpftool absent in the guest image — the churn catch degrades to rc=127 evidence"
fi

# ── PROOF 7: the tap's in-kernel cost, measured by the kernel itself ──
# (night-frontier-Z1, the harness-native BPF_PROG_TEST_RUN lane) — the
# one kernel-side runtime measurement the local gates cannot reach:
# the payload's harness binary drives the production tap object (the
# ebpf-prebuilt bytes the binary embeds) through the loader's own
# aya-obj parser, raw bpf() syscalls and BPF_PROG_TEST_RUN with
# synthetic input_event args in a real pt_regs; the kernel reports the
# durations and the lane gates the RELATIVE law (gate-off, unmapped,
# autorepeat all below the full press path). The body rides
# BPF_PROG_TYPE_SYSCALL (a KPROBE program answers ENOTSUPP to TEST_RUN
# on every mainline kernel — no bpf_prog_test_run_kprobe exists), the
# one program type whose test run the 5.15 floor already answers, so
# the floor MEASURES; a kernel that refuses the test run outright is
# classified honestly as a recorded skip, never a fake row. The
# attach/detach pair here is the same idempotent
# self-healing lane the engine chaos below exercises — the measurement
# never leaves state behind (the harness's maps are fresh instances,
# not the pinned ones).
if [ -x /usr/bin/zylaxion ] && [ -r "$Z/ebpf-prebuilt/zylaxion-tap.o" ]; then
	if /usr/bin/zylaxion attach >/dev/null 2>&1; then
		if [ -x "$Z/zactrix-audit" ]; then
			if "$Z/zactrix-audit" --prog-run "$Z/ebpf-prebuilt/zylaxion-tap.o"; then
				note "PROOF 7 prog-run lane (the kernel measured the tap, or the kernel is honestly classified)" PASS
			else
				note "PROOF 7 prog-run lane" FAIL
			fi
		else
			echo "MASS-RESULT: PROOF 7 prog-run lane — SKIP (the harness binary is not in the payload)"
		fi
		/usr/bin/zylaxion detach >/dev/null 2>&1 || true
	else
		note "PROOF 7 setup: attach on an at-floor leg" FAIL
	fi
else
	echo "MASS-RESULT: PROOF 7 prog-run lane — SKIP (no payload or no tap object staged)"
fi

# ── PROOF 8: the evented control plane (night-frontier-Z2 B1) ──
# The whole loop a control change rides, on a real kernel with a real
# player: verb → control map write → ONE TEST_RUN on the pinned relay
# → the KERNEL produces the record into the pinned notices ring → the
# watcher's poll(2) wakes → the control map re-read (the ONLY
# authority). Four rows carry the proof:
#   - the player's wake row: a real `volume` verb lands in the player
#     log with the kernel-timestamped latency, gated far under the
#     10 s heartbeat (the old 100 ms poll is retired to the bridge);
#   - the apply row: the retune lands through the evented lane;
#   - the authority row: a SECOND verb that writes the SAME value
#     wakes the player, re-reads the map, changes NOTHING, and SAYS
#     so ("authority held") — a notice is a hint, the map is the law;
#     this is the gimmick door, welded shut;
#   - the kernel rows: the harness's notice-drive lane proves the
#     relay RUNS (TEST_RUN accepted), the ring WAKES a poller, and
#     the mmap drain echoes the frame byte-for-byte — run AFTER the
#     player stops, so the two consumers never race one ring.
# The player half needs its recipient to exist and to RECEIVE the
# pins: the zed user is created here (idempotent — the below-floor
# branch and the chaos battery both may have made it already), and
# the attach rides SUDO_USER=zed — the exact environment `sudo
# zylaxion attach` produces, the same hand-over law the zero-to-hero
# step rides. Without it the guest has no /run/user for the desktop
# scan to find, the pins stay root-owned, and the player could not
# open the control plane it is supposed to be woken by (the Z2 CI
# guest named both: 'unknown user zed', then the root-owned pins).
groupadd -f audio 2>/dev/null || true
useradd -m -s /bin/bash zed 2>/dev/null || true
usermod -aG audio zed 2>/dev/null || true
# The player's control-plane ops (BPF_OBJ_GET on the pins, the map
# lookups the authority law rides) cross the kernel's unprivileged
# bpf gate; the distro default shuts it (unprivileged_bpf_disabled=2)
# and the engine's own error names the boot-scoped remedy. Root opens
# the gate for this boot — the below-floor leg refuses at attach
# anyway, and the verb probes above run as root, past the gate.
sysctl -w kernel.unprivileged_bpf_disabled=0 >/dev/null 2>&1 || true
if [ -x /usr/bin/zylaxion ] && SUDO_USER=zed /usr/bin/zylaxion attach >/dev/null 2>&1; then
	PLOG=/tmp/zylaxion-player-p8.log
	rm -f "$PLOG"
	p8_started=no
	sudo -u zed env RUST_LOG=info /usr/bin/zylaxion _play >"$PLOG" 2>&1 &
	PLAY8=$!
	ready=no
	for i in $(seq 1 30); do
		grep -aq "ready" "$PLOG" 2>/dev/null && { ready=yes; break; }
		sleep 0.5
	done
	if [ "$ready" = yes ]; then
		p8_started=yes
		# The player's own startup law zeroes the enabled gate (the
		# control defaults put the tap in a defined idle posture), so
		# without `on` the wake's authority re-read sees the cleared
		# flag and the player stops before any retune can apply (the
		# guest witness named it: the notice row printed, then
		# 'control map cleared the enabled flag — stopping'). The
		# sound goes ON first; the retune and the authority rows ride
		# a live gate.
		sudo -u zed /usr/bin/zylaxion on >"$PLOG.on" 2>&1 || true
		# The verb's own words go where the battery can read them
		# back: a swallowed verb turns every downstream FAIL into a
		# guess (the Z2 CI guest named this — the wake rows failed
		# with the verb's error invisible).
		sudo -u zed /usr/bin/zylaxion volume --percent 61 >"$PLOG.verb1" 2>&1 || true
		saw_notice=no
		saw_apply=no
		for i in $(seq 1 40); do
			grep -aq "control notice kind" "$PLOG" && saw_notice=yes
			grep -aq "volume retuned to 61%" "$PLOG" && { saw_apply=yes; break; }
			sleep 0.1
		done
		wake_ns=$(grep -ao 'wake [0-9]* ns' "$PLOG" | head -1 | grep -o '[0-9]*')
		if [ "$saw_notice" = yes ] && [ -n "$wake_ns" ] && [ "$wake_ns" -lt 50000000 ]; then
			note "PROOF 8: the player woke on the notice in ${wake_ns} ns (the 100 ms control poll is retired)" PASS
		else
			note "PROOF 8: the player's evented wake (latency ${wake_ns:-none} ns)" FAIL
			grep -a "control notice\|volume retuned" "$PLOG" | head -4 || true
			echo "── the verbs' own words (the wake row's witness) ──"
			cat "$PLOG.on" 2>/dev/null | head -3 || true
			cat "$PLOG.verb1" 2>/dev/null | head -6 || true
		fi
		if [ "$saw_apply" = yes ]; then
			note "PROOF 8: the retune applied through the evented lane" PASS
		else
			note "PROOF 8: the retune apply" FAIL
			echo "── the player's last lines (the apply row's witness) ──"
			tail -10 "$PLOG" 2>/dev/null || true
			echo "── the verb's own words ──"
			cat "$PLOG.verb1" 2>/dev/null | head -6 || true
		fi
		# The authority row: the same verb, the SAME value — the map
		# truth does not move, so the wake must apply nothing and say
		# so. A notice that changed the sound here would be the
		# gimmick this proof exists to refuse.
		sudo -u zed /usr/bin/zylaxion volume --percent 61 >"$PLOG.verb2" 2>&1 || true
		auth=no
		for i in $(seq 1 20); do
			grep -aq "authority held" "$PLOG" && { auth=yes; break; }
			sleep 0.1
		done
		if [ "$auth" = yes ]; then
			note "PROOF 8: a wake with no pending change applied NOTHING — authority held" PASS
		else
			note "PROOF 8: the authority row (notice-as-hint)" FAIL
			tail -6 "$PLOG" 2>/dev/null || true
			cat "$PLOG.verb2" 2>/dev/null | head -4 || true
		fi
	else
		note "PROOF 8: the player never reached ready (the guest audio/input posture)" FAIL
		tail -5 "$PLOG" 2>/dev/null || true
	fi
	# The player stops through the same evented lane (off = one map
	# write + one notice), then the kernel-side lane runs ALONE.
	if [ "$p8_started" = yes ]; then
		sudo -u zed /usr/bin/zylaxion off >/dev/null 2>&1 || true
		for i in $(seq 1 20); do
			kill -0 "$PLAY8" 2>/dev/null || break
			sleep 0.25
		done
		kill -0 "$PLAY8" 2>/dev/null && pkill -f "/usr/bin/zylaxion _play" 2>/dev/null || true
	fi
	if [ -x "$Z/zactrix-audit" ]; then
		"$Z/zactrix-audit" --notice-drive >/tmp/p8-harness.out 2>&1
		if grep -aq "MASS-RESULT: PROOF 8 notice-drive lane (the kernel produced the wake, or the lane is honestly classified) — PASS" /tmp/p8-harness.out; then
			note "PROOF 8: the kernel notice-drive lane (relay runs, ring wakes, bytes echo the frame)" PASS
		else
			note "PROOF 8: the kernel notice-drive lane" FAIL
			grep -a "PROOF 8" /tmp/p8-harness.out | head -14 || true
			# The harness's own last words, unfiltered: a panic line does
			# not start with the lane prefix, and a hidden panic turns a
			# verdict into a guess.
			tail -6 /tmp/p8-harness.out 2>/dev/null || true
		fi
	else
		echo "MASS-RESULT: PROOF 8 notice-drive — SKIP (the harness binary is not in the payload)"
	fi
	/usr/bin/zylaxion detach >/dev/null 2>&1 || true
else
	echo "MASS-RESULT: PROOF 8 notice plane — SKIP (no payload or attach refused on this leg)"
fi

# ── the engine: the whole chaos ────────────────────────────────────────
# The engine prints its own MASS rows (same contract); its exit code
# folds into this init's verdict line — the ONLY line the workflow's
# Verdict step treats as authoritative.
ENGINE="$Z/scripts/supermassive/supermassive-test.py"
if python3 "$ENGINE" --full --binary /usr/bin/zylaxion; then
        ENGINE_RC=0
else
        ENGINE_RC=$?
fi

if [ "$ENGINE_RC" -eq 0 ] && [ "$FAILURES" -eq 0 ]; then
        echo "MASS-VERDICT: PASS (init probes $FAILURES failed, engine rc $ENGINE_RC)"
else
        echo "MASS-VERDICT: FAIL (init probes: $FAILURES failed, engine rc: $ENGINE_RC)"
fi
guest_exit
