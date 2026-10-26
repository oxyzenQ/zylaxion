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
