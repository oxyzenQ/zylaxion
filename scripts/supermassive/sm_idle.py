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
        time.sleep(SILENT_WINDOW_S)
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
