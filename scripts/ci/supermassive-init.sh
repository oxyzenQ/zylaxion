#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
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
# battery — CLI/UX, the tap lifecycle, and the real-usage chain with a
# real user who is in no input group.
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
echo "MASS: init on $(uname -r)"

# The floor itself — the whole reason this VM exists. zylaxion's
# documented floor is 5.15 (docs and the doctor's verdict): the floor
# leg boots the true 5.15 jammy kernel, the latest leg boots the
# newest kernel the archive serves, so one numeric check serves both
# ends of the span.
kver=$(uname -r)
major=${kver%%.*}
minor=$(echo "$kver" | cut -d. -f2 | grep -oE '^[0-9]+' || echo 0)
if [ "${major:-0}" -gt 5 ] || { [ "${major:-0}" -eq 5 ] && [ "${minor:-0}" -ge 15 ]; }; then
	note "kernel $kver (the 5.15+ documented floor)" PASS
else
	note "kernel (uname -r = $kver, wanted >= 5.15)" FAIL
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

# ── the staged module chain (the guest's sound and keyboard) ──────────
# The workflow extracts the .ko files from the SAME kernel's module
# debs (same-suite vermagic by construction) into $Z/modules/. The
# insmod order is the dependency order: soundcore -> snd -> snd-timer
# -> snd-pcm -> snd-dummy, and uinput (no dependencies beyond the
# input core, which is built-in on every ubuntu kernel).
MODDIR="$Z/modules"
for mod in soundcore snd snd-timer snd-pcm snd-dummy uinput; do
	KO=$(find "$MODDIR" -name "${mod}.ko*" 2>/dev/null | head -1)
	if [ -z "$KO" ]; then
		note "module $mod staged from the kernel's own debs" FAIL
		continue
	fi
	if insmod "$KO" 2>/dev/null || grep -q "$mod" /proc/modules; then
		note "module $mod loaded" PASS
	else
		note "module $mod (insmod $(basename "$KO"))" FAIL
	fi
done

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
