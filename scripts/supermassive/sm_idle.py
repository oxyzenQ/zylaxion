#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
# OS: Linux only — zylaxion is a Linux keyboard-sound tool; no other OS is supported.
"""sm_idle — the parked-player witness (night-frontier-Z4).

The ZERO-timer law, witnessed end to end on the REAL player: the tap
attached, the player on, nothing typed. The three scheduling threads
are read from /proc/<pid>/task/<tid>/status before and after a silent
window, and the deltas are the verdict:

- zylaxion (the main thread — the render loop): ZERO voluntary
  switches. The silent wait is a park (the blocking select), no timer.
- zylaxion-bridge: ZERO. The events-ring poll is a forever block; the
  wind-down pipe is its one exit lane.
- zylaxion-watch: at most ONE. The 60 s backstop is the one timer left
  in the estate (it arms only when both evented lanes are dead — which
  they are not here), and a window shorter than 60 s can catch at most
  one of its wakes.

The Mouth is out of scope by physics: the cpal thread is woken by the
audio hardware's clock (the device's own interrupts), not by a timer
zylaxion owns — the interrupt-driven half the architecture claims.

The chain rides the same lane the journey stage drives: attach hands
the tap to the real user, `on` spawns the stateless player, the
window runs, `off` must still land through the evented stop, and the
detach closes the cycle. A dispatch-era kernel (5.15-5.18) that gates
every plain-user bpf() call defers the witness honestly — the era
gate is the verdict there, exactly as the chain stage rules.
"""

from __future__ import annotations

import os
import subprocess
import time

from sm_audio import (
    _narrowed_unpriv_gate,
    _player_pid,
    _run,
    _run_as_user,
    _unpriv_bpf_disabled,
)
from sm_report import Ledger, Step  # noqa: E402

# The silent window, in seconds. Long enough to average out scheduler
# noise around the ZERO bar, short enough to respect the E2E lane's
# ten minute job cap. A window shorter than 60 s can catch at most one
# of the watcher's backstop wakes.
SILENT_WINDOW_S = 20


def _thread_switches(pid: int) -> dict[str, tuple[int, int]]:
    """One snapshot of every thread's (voluntary, nonvoluntary)
    context switches, keyed by comm — /proc/<pid>/task/<tid>/status.
    The same mechanical surface the in-tree idle_park test reads; the
    E2E copy rides the REAL player, the real kernel tap, and the real
    silent window."""
    threads: dict[str, tuple[int, int]] = {}
    task_dir = f"/proc/{pid}/task"
    try:
        entries = os.listdir(task_dir)
    except OSError:
        return threads
    for tid in entries:
        try:
            with open(f"{task_dir}/{tid}/comm", encoding="utf-8") as fh:
                comm = fh.read().strip()
            with open(f"{task_dir}/{tid}/status", encoding="utf-8") as fh:
                vol: int | None = None
                nonvol: int | None = None
                for line in fh:
                    if line.startswith("voluntary_ctxt_switches:"):
                        vol = int(line.split(":", 1)[1].strip())
                    elif line.startswith("nonvoluntary_ctxt_switches:"):
                        nonvol = int(line.split(":", 1)[1].strip())
            if vol is not None and nonvol is not None:
                threads[comm] = (vol, nonvol)
        except (OSError, ValueError):
            continue
    return threads


def _cpu_idle_tick() -> float:
    """One idle-time tick from /proc/stat's aggregate cpu line: the
    idle + iowait jiffies converted to seconds. The kernel's own
    accounting of how much the CPUs rested — always present, unlike
    cpuidle sysfs, and the honest gate for the parked window (a
    spinning poll shows up as missing idle time, whatever the guest's
    idle-state driver exposes)."""
    with open("/proc/stat", encoding="utf-8") as fh:
        first = fh.readline()
    fields = first.split()
    if len(fields) < 5 or fields[0] != "cpu":
        return 0.0
    clk = os.sysconf("SC_CLK_TCK") or 100
    idle_jiffies = int(fields[4])
    iowait_jiffies = int(fields[5]) if len(fields) > 5 else 0
    return (idle_jiffies + iowait_jiffies) / clk


def _cpuidle_snapshot() -> dict[str, int]:
    """One cpuidle residency snapshot, aggregated by state name across
    cpus: /sys/devices/system/cpu/cpu*/cpuidle/state*/time (us). The
    deep-state half of the Z6 witness — RECORDED, never gated: a
    micro-VM guest exposes whatever idle states its kernel and machine
    model carry, and the host's real C-states live above the guest's
    view entirely. Absent sysfs returns an empty dict and the row is
    an honest skip."""
    rows: dict[str, int] = {}
    try:
        cpus = os.listdir("/sys/devices/system/cpu")
    except OSError:
        return rows
    for cpu in sorted(cpus):
        if not cpu.startswith("cpu") or cpu == "cpuidle":
            continue
        base = f"/sys/devices/system/cpu/{cpu}/cpuidle"
        try:
            states = os.listdir(base)
        except OSError:
            continue
        for state in sorted(states):
            try:
                with open(f"{base}/{state}/name", encoding="utf-8") as fh:
                    name = fh.read().strip()
                with open(f"{base}/{state}/time", encoding="utf-8") as fh:
                    time_us = int(fh.read().strip())
            except (OSError, ValueError):
                continue
            if name:
                rows[name] = rows.get(name, 0) + time_us
    return rows


def battery(ledger: Ledger, binary: str, user: str) -> None:
    """The parked-player witness; rows land on the ledger."""
    with Step(ledger, "the parked player — zero timer wakeups (night-frontier-Z4)"):
        env_attach = dict(os.environ)
        env_attach["SUDO_USER"] = user
        try:
            proc = subprocess.run(
                [binary, "attach"],
                capture_output=True,
                text=True,
                env=env_attach,
                timeout=90,
            )
            rc, out = proc.returncode, (proc.stdout + proc.stderr).strip()
        except (OSError, subprocess.TimeoutExpired) as exc:
            rc, out = 127, f"{type(exc).__name__}: {exc}"
        if rc == 0 and "tap attached" in out:
            ledger.pass_row("the witness's attach hands the tap to the real user", user)
        else:
            ledger.fail_row(
                "the witness's attach hands the tap to the real user", f"rc={rc}: {out[:300]}"
            )
            return

        rc, out = _run_as_user(user, binary, ["on"])
        if not (rc == 0 and "sound on" in out):
            posture = _unpriv_bpf_disabled()
            narrowed = _narrowed_unpriv_gate()
            if posture != 0 and not narrowed and "unprivileged bpf" in out:
                ledger.pass_row(
                    "the parked-player witness defers to the kernel's dispatch gate",
                    f"posture {posture}: the era gate is the verdict on this kernel "
                    "(the narrowed-gate and posture-0 legs carry the witness)",
                )
            else:
                ledger.fail_row(
                    "the witness's player turns on (the parked chain)",
                    f"rc={rc}: {out[:300]}",
                )
            _run([binary, "detach"])
            return

        time.sleep(1.5)
        pid = _player_pid(user)
        if not pid:
            ledger.fail_row(
                "the witness's player runs", "no zylaxion process under the user"
            )
            _run([binary, "detach"])
            return

        before = _thread_switches(pid)
        # The Z6 C-state witness brackets the SAME silent window: the
        # parked loop's idle-time delta and the cpuidle residency rows
        # describe the exact regime the zero-wakeup claim rides. No
        # extra wall time — the snapshots ride the window that exists.
        idle_before = _cpu_idle_tick()
        cstates_before = _cpuidle_snapshot()
        time.sleep(SILENT_WINDOW_S)
        idle_after = _cpu_idle_tick()
        cstates_after = _cpuidle_snapshot()
        after = _thread_switches(pid)

        # The player must still be alive: a forever poll that degenerates
        # into a busy loop, an early exit, or a missed wake shows here
        # first — the park has to SURVIVE its own silence.
        if _player_pid(user) == pid:
            ledger.pass_row(
                "the player survives the silent window (the park holds)",
                f"pid {pid} still running after {SILENT_WINDOW_S}s of silence",
            )
        else:
            ledger.fail_row(
                "the player survives the silent window (the park holds)",
                f"pid {pid} left the process table mid-witness",
            )

        def delta(comm: str) -> tuple[int, int] | None:
            b = before.get(comm)
            a = after.get(comm)
            if b is None or a is None:
                return None
            return a[0] - b[0], a[1] - b[1]

        render = delta("zylaxion")
        bridge = delta("zylaxion-bridge")
        watch = delta("zylaxion-watch")

        if render is None:
            ledger.fail_row(
                "the render loop emits ZERO timer wakeups while silent",
                "the main thread's /proc status row is missing",
            )
        elif render[0] == 0:
            ledger.pass_row(
                "the render loop emits ZERO timer wakeups while silent",
                f"voluntary +0, nonvoluntary +{render[1]} over {SILENT_WINDOW_S}s (the park)",
            )
        else:
            ledger.fail_row(
                "the render loop emits ZERO timer wakeups while silent",
                f"voluntary +{render[0]}, nonvoluntary +{render[1]} — a timer hides in "
                "the idle wait",
            )

        if bridge is None:
            ledger.fail_row(
                "the bridge emits ZERO timer wakeups while silent",
                "the bridge thread's /proc status row is missing",
            )
        elif bridge[0] == 0:
            ledger.pass_row(
                "the bridge emits ZERO timer wakeups while silent",
                f"voluntary +0, nonvoluntary +{bridge[1]} over {SILENT_WINDOW_S}s "
                "(the forever poll)",
            )
        else:
            ledger.fail_row(
                "the bridge emits ZERO timer wakeups while silent",
                f"voluntary +{bridge[0]}, nonvoluntary +{bridge[1]} — a timer hides in "
                "the ring wait",
            )

        if watch is None:
            ledger.fail_row(
                "the watcher stays at the 60 s backstop while silent",
                "the watcher thread's /proc status row is missing",
            )
        elif watch[0] <= 1:
            ledger.pass_row(
                "the watcher stays at the 60 s backstop while silent",
                f"voluntary +{watch[0]} (<= 1, the last-resort backstop) over "
                f"{SILENT_WINDOW_S}s",
            )
        else:
            ledger.fail_row(
                "the watcher stays at the 60 s backstop while silent",
                f"voluntary +{watch[0]} — the backstop is polling, not parking",
            )

        # The Z6 C-state witness (night-frontier-Z6): the physics half
        # of the idle claim. The GATE is the kernel's own idle-time
        # accounting — a parked loop leaves the CPUs idle for essentially
        # the whole window (the bar sits at 90 percent for scheduler
        # noise); a spinning poll would eat the delta and fail here. The
        # cpuidle residency rows are the RECORDED half: whatever states
        # this guest exposes, their residency grew — and the host's real
        # deep C-states stay the host instrument's (idle_cpu_sanity).
        idle_delta_s = idle_after - idle_before
        if idle_delta_s >= 0.9 * SILENT_WINDOW_S:
            ledger.pass_row(
                "the parked window's CPU rests (the idle-time witness)",
                f"the kernel's idle time grew {idle_delta_s:.1f}s of the "
                f"{SILENT_WINDOW_S}s window — the park rests the core, it does not spin it",
            )
        else:
            ledger.fail_row(
                "the parked window's CPU rests (the idle-time witness)",
                f"idle time grew only {idle_delta_s:.1f}s of the "
                f"{SILENT_WINDOW_S}s window — something burned CPU in the silence",
            )

        if cstates_before or cstates_after:
            deltas = {
                name: cstates_after.get(name, 0) - cstates_before.get(name, 0)
                for name in sorted(set(cstates_before) | set(cstates_after))
            }
            pretty = ", ".join(
                f"{name} +{d_us}us" for name, d_us in deltas.items() if d_us > 0
            )
            ledger.info(
                "the cpuidle residency rows (measured, recorded — not gated)",
                pretty or "no residency growth in the exposed states",
            )
        else:
            ledger.skip_row(
                "the cpuidle residency rows (measured, recorded — not gated)",
                "cpuidle sysfs is absent in this guest — the deep-state half is the "
                "host instrument's (idle_cpu_sanity), honestly skipped here",
            )

        rc, out = _run_as_user(user, binary, ["off"])
        time.sleep(1.5)
        if rc == 0 and not _player_pid(user):
            ledger.pass_row(
                "off still lands on the parked player (the evented stop)",
                "the ControlStop wake, witnessed end to end",
            )
        else:
            ledger.fail_row(
                "off still lands on the parked player (the evented stop)",
                f"rc={rc}, player still alive: {_player_pid(user)}",
            )

        _run([binary, "detach"])
