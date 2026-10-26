# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""The night-dinner-3 repro battery — the first-run frictions, staged on
demand (night-dinner-5).

The live transcript that commissioned night-dinner-3 (zactrix@debian13,
release v80.0.0-beta.1): Debian 13 mounts bpffs at mode 0700, attach
handed the pins to the user, and every unprivileged verb misread the
walk-block as "the tap is not attached"; the doctor printed "All checks
passed" over an unusable mount; the unprivileged-bpf verdict claimed a
boot lock that the transcript's very next sysctl disproved.
night-dinner-3 fixed the product; this battery replays that posture on
the guest so the fixes can never regress silently:

  the walk-block door (tap-less) — at mode 0700 the doctor must FAIL
    the bpffs reach station (never the old blanket all-clear) and `on`
    must name the walk and its one-command fix (never "the tap is not
    attached").
  the attach self-heal — root's attach (the SUDO_USER handover)
    relaxes the front door by exactly one bit, says so in its own
    output, and the real user's `on` then reaches a live player and
    stops it again. The player's sound output is snd-dummy — the
    code-chosen output device for a machine with no audio hardware,
    the same substitution a VM (or the transcript's headless box)
    needs to prove frames without speakers.
  the verdict honesty law — at sysctl value 2 with no boot parameter
    on the command line, the verdict must be the runtime-default arm
    (the sysctl remedy), never the boot-lock claim the transcript
    refuted; with the parameter planted, the boot-lock arm is the
    honest one; at value 0 the allowed arm is.

Root lane only (it chmods the bpffs mount and drives root's attach).
It rides the micro-VM's --full lane between the tap lifecycle and the
real-usage chain, and it ends detached with the front door left at the
one-way relaxed mode the product itself leaves behind — the chain
after it attaches into a healthy mount. The posture flip it attempts
(the transcript's own remedy, proven on Debian 13) is also what lets
the real-usage chain after it run its full depth when the guest
kernel's default allows the runtime write; when the kernel refuses,
both batteries honestly ride the classified-refusal branches the
product ships for that world.
"""

from __future__ import annotations

import os
import subprocess
import time

from sm_report import Ledger, Step

BPFFS = "/sys/fs/bpf"
BOOT_LOCK_TOKEN = "bpf.unprivileged_bpf_disabled"
RUN_TIMEOUT = 90


def _run(cmd: list[str], timeout: int = RUN_TIMEOUT) -> tuple[int, str]:
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return proc.returncode, (proc.stdout + proc.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, f"{type(exc).__name__}: {exc}"


def _run_as_user(user: str, binary: str, args: list[str]) -> tuple[int, str]:
    return _run(["sudo", "-u", user, binary, *args])


def _attach_as_root(binary: str, user: str) -> tuple[int, str]:
    env = dict(os.environ)
    env["SUDO_USER"] = user
    try:
        proc = subprocess.run(
            [binary, "attach"],
            capture_output=True,
            text=True,
            env=env,
            timeout=RUN_TIMEOUT,
        )
        return proc.returncode, (proc.stdout + proc.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, f"{type(exc).__name__}: {exc}"


def _mount_mode() -> int | None:
    try:
        return os.stat(BPFFS).st_mode & 0o777
    except OSError:
        return None


def _unpriv_posture() -> int:
    try:
        with open("/proc/sys/kernel/unprivileged_bpf_disabled", encoding="utf-8") as fh:
            return int(fh.read().strip() or "0")
    except (OSError, ValueError):
        return 0


def _boot_param_locked() -> bool:
    try:
        with open("/proc/cmdline", encoding="utf-8") as fh:
            return BOOT_LOCK_TOKEN in fh.read()
    except OSError:
        return False


def _flip_posture_runtime_default() -> bool:
    """The transcript's remedy: a value-2 runtime default is one root
    write away from 0 (proven live on Debian 13's 6.12). Writes the
    procfs knob directly — no sysctl binary dependency in the guest.
    Returns whether the kernel actually took the new value."""
    try:
        with open("/proc/sys/kernel/unprivileged_bpf_disabled", "w", encoding="utf-8") as fh:
            fh.write("0")
    except OSError:
        return False
    return _unpriv_posture() == 0


def _player_pid(user: str) -> int:
    rc, out = _run(["pgrep", "-u", user, "-x", "zylaxion"], timeout=15)
    if rc == 0 and out:
        return int(out.splitlines()[0])
    return 0


def battery(ledger: Ledger, binary: str, user: str) -> None:
    """The whole repro family; rows land on the ledger. Root lane only."""
    if os.geteuid() != 0:
        ledger.skip_row(
            "the night-dinner-3 repro needs the root lane",
            "not root — the guest's --full lane owns it",
        )
        return

    posture_before = _unpriv_posture()
    if posture_before == 2 and not _boot_param_locked():
        flipped = _flip_posture_runtime_default()
        ledger.info(
            "the unprivileged-bpf posture at journey start",
            f"was 2; the runtime-default flip (the transcript's remedy) "
            f"{'succeeded — the full chains run this boot' if flipped else 'was refused by the kernel'}",
        )
    else:
        ledger.info(
            "the unprivileged-bpf posture at journey start",
            f"value {posture_before}, boot-param lock={_boot_param_locked()}",
        )

    _stage_walk_block(ledger, binary, user)
    _stage_self_heal(ledger, binary, user)
    _stage_verdict_honesty(ledger, binary, user)


def _stage_walk_block(ledger: Ledger, binary: str, user: str) -> None:
    """The tap-less world at mode 0700: the exact shape the transcript
    hit first — doctor's blanket all-clear and the verbs' false
    not-attached. Both must name the walk instead."""
    with Step(ledger, "the walk-block door — the Debian 13 posture, tap-less"):
        _run([binary, "detach"])  # idempotent; the repro needs the tap-less world
        mode_before = _mount_mode()
        try:
            os.chmod(BPFFS, 0o700)
        except OSError as exc:
            ledger.fail_row("the journey set the front door to mode 0700", str(exc))
            return
        ledger.info(
            "the front door sits at mode 0700",
            f"was {mode_before:o} — the transcript's Debian 13 default",
        )

        rc, out = _run_as_user(user, binary, ["doctor"])
        # The zelynic row shape (night-dragon-maintenance-1): the reach
        # FAIL reads "uid ... cannot walk to the pins" under the
        # "pin reach:" row key; the fix row keeps the chmod remedy.
        failed_reach = "cannot walk to the pins" in out and "chmod o+x" in out
        all_clear = "All checks passed" in out
        if rc == 1 and failed_reach and not all_clear:
            ledger.pass_row(
                "doctor at mode 0700 fails the reach station (no blanket all-clear)",
                f"rc 1, the fix named: sudo chmod o+x {BPFFS}",
            )
        else:
            ledger.fail_row(
                "doctor at mode 0700 fails the reach station (no blanket all-clear)",
                f"rc={rc}, reach-fail={failed_reach}, all-clear={all_clear}: {out[:120]}",
            )

        rc, out = _run_as_user(user, binary, ["on"])
        walk_door = "cannot walk to" in out and "chmod o+x" in out
        misdiagnosed = "the tap is not attached" in out
        if rc == 1 and walk_door and not misdiagnosed:
            ledger.pass_row(
                "on at mode 0700 names the walk-block (never a false not-attached)",
                "the transcript's misdiagnosis is fixed",
            )
        else:
            ledger.fail_row(
                "on at mode 0700 names the walk-block (never a false not-attached)",
                f"rc={rc}, walk-door={walk_door}, misdiagnosed={misdiagnosed}: {out[:400]}",
            )


def _stage_self_heal(ledger: Ledger, binary: str, user: str) -> None:
    """The attach self-heal: the one-bit relax, named in attach's own
    output, then the real user's door opens and a player lives and
    stops behind it. When the kernel refuses every unprivileged bpf()
    this boot, the door opening is still proven — the refusal moves
    from the walk to the map open, a shape only an owned, traversable
    pin set produces."""
    with Step(ledger, "the attach self-heal — one bit, then the sound"):
        rc, out = _attach_as_root(binary, user)
        mode_now = _mount_mode()
        relaxed_note = "relaxed o+x" in out
        door_open = mode_now is not None and (mode_now & 0o001) == 1
        if rc == 0 and "tap attached" in out and relaxed_note and door_open:
            ledger.pass_row(
                "attach relaxes the front door by one bit and says so",
                f"mode {mode_now:o}, the relax note carried",
            )
        elif rc == 0 and "tap attached" in out and door_open:
            ledger.pass_row(
                "attach finds the front door already traversable",
                f"mode {mode_now:o}",
            )
        else:
            ledger.fail_row(
                "attach leaves the front door traversable for the user",
                f"rc={rc}, mode={oct(mode_now) if mode_now is not None else 'unreadable'}, "
                f"relax-note={relaxed_note}: {out[:400]}",
            )
            _run([binary, "detach"])
            try:
                os.chmod(BPFFS, 0o755)
            except OSError:
                pass
            return

        rc, out = _run_as_user(user, binary, ["on"])
        if rc == 0 and "sound on:" in out:
            ledger.pass_row(
                f"{user} turns the sound on through the relaxed door",
                out.splitlines()[0][:70],
            )
        elif rc == 1 and "unprivileged bpf" in out and "refused to open" in out:
            ledger.pass_row(
                "the door is proven open — the refusal moved to the kernel's bpf gate",
                "the walk-block is gone; the posture wall is the honest remainder",
            )
            _run([binary, "detach"])
            return
        else:
            # The escalator (night-dinner-5b's lesson, honored in
            # night-dinner-5c): the detached player's stdio is null by
            # design, so the parent's error cannot carry the reason —
            # the product itself names the surface that can: _play in
            # the foreground. Eight seconds of it either outlives the
            # probe (alive — the parent's spawn path is the defect)
            # or dies with its own stderr on the ledger (the evidence).
            if _unpriv_posture() == 0:
                prc, pout = _run(
                    ["timeout", "8", "sudo", "-u", user, binary, "_play"],
                    timeout=20,
                )
                if prc == 124:
                    ledger.fail_row(
                        f"{user} turns the sound on through the relaxed door",
                        f"rc={rc}: {out[:200]} — but a foreground player runs as "
                        f"{user} (8s timeout outlived it): the detached-spawn "
                        f"path, not the player, is the defect",
                    )
                else:
                    ledger.fail_row(
                        f"{user} turns the sound on through the relaxed door",
                        f"rc={rc}: {out[:200]} — the foreground player's own "
                        f"stderr: {pout[:600]}",
                    )
            else:
                ledger.fail_row(
                    f"{user} turns the sound on through the relaxed door",
                    f"rc={rc}: {out[:400]}",
                )
            _run([binary, "detach"])
            return

        time.sleep(1.0)
        pid = _player_pid(user)
        if pid:
            ledger.pass_row(
                "the player lives behind the relaxed door (snd-dummy carries the output)",
                f"pid {pid}, the code-chosen sound output of a machine with no speakers",
            )
        else:
            ledger.fail_row(
                "the player lives behind the relaxed door (snd-dummy carries the output)",
                "no zylaxion process under the user",
            )

        rc, out = _run_as_user(user, binary, ["off"])
        time.sleep(1.5)
        if rc == 0 and not _player_pid(user):
            ledger.pass_row(
                "off stops the sound and the player self-exits",
                "the repro cycle closes clean",
            )
        else:
            ledger.fail_row(
                "off stops the sound and the player self-exits",
                f"rc={rc}, pid-still-alive={bool(_player_pid(user))}",
            )

        _run([binary, "detach"])


def _stage_verdict_honesty(ledger: Ledger, binary: str, user: str) -> None:
    """The value-2 verdict has two truths (night-dinner-3): the boot
    parameter lock the transcript never had, and the runtime default
    the transcript disproved with one sysctl. The doctor must speak
    the one the machine actually wears."""
    with Step(ledger, "the verdict honesty law — value 2, two truths"):
        value = _unpriv_posture()
        boot_locked = _boot_param_locked()
        rc, out = _run_as_user(user, binary, ["doctor"])
        if value == 0:
            # The zelynic row shape (night-dragon-maintenance-1): the
            # allowed arm reads under the "unpriv bpf:" row key.
            if "unpriv bpf:" in out and "allowed" in out:
                ledger.pass_row(
                    "the verdict speaks the allowed arm at value 0",
                    "the posture the flip produced",
                )
            else:
                ledger.fail_row(
                    "the verdict speaks the allowed arm at value 0",
                    f"rc={rc}: {out[:400]}",
                )
        elif boot_locked:
            if "locked (2)" in out and "cannot be re-enabled" in out:
                ledger.pass_row(
                    "the verdict speaks the boot-lock arm when the parameter plants it",
                    f"{BOOT_LOCK_TOKEN} is on the command line",
                )
            else:
                ledger.fail_row(
                    "the verdict speaks the boot-lock arm when the parameter plants it",
                    f"rc={rc}: {out[:400]}",
                )
        else:
            runtime_arm = "disabled (2)" in out and (
                "sysctl -w kernel.unprivileged_bpf_disabled=0" in out
            )
            boot_claim = "cannot be re-enabled" in out
            if runtime_arm and not boot_claim:
                ledger.pass_row(
                    "the value-2 verdict is the runtime-default arm (the refuted "
                    "boot-lock claim stays gone)",
                    "the sysctl remedy named",
                )
            else:
                ledger.fail_row(
                    "the value-2 verdict is the runtime-default arm (the refuted "
                    "boot-lock claim stays gone)",
                    f"rc={rc}, runtime-arm={runtime_arm}, boot-claim={boot_claim}: {out[:400]}",
                )
