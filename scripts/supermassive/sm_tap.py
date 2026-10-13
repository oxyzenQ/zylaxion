# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""The tap lifecycle battery (night-master-3).

The kernel-side control plane, walked end to end on a root lane:
attach (the one privileged surface), the pin layout and its ownership
handover, the idempotency and foreign-entry classifications the
night-improve-26 cleanup landed, the unprivileged read-back, and the
detach contract (after this, nothing of zylaxion remains in the
kernel). Full mode only — every probe here mutates kernel state.

The battery drives the SAME canonical invocations the owner runs:
`zylaxion attach` / `zylaxion detach`, never a bespoke syscall side
door. The handover is the sudo contract itself (SUDO_USER set, the
exact environment `sudo zylaxion attach` produces for a real user).
"""

from __future__ import annotations

import os
import subprocess
import time

from sm_report import Ledger, Step

PIN_DIR = "/sys/fs/bpf/zylaxion"
PIN_NAMES = ("tap", "tap_link", "control", "keymap", "events", "stats")


def _run(binary: str, args: list[str], sudo_user: str | None = None) -> tuple[int, str]:
    env = dict(os.environ)
    if sudo_user:
        env["SUDO_USER"] = sudo_user
    try:
        proc = subprocess.run(
            [binary, *args],
            capture_output=True,
            text=True,
            env=env,
            timeout=90,
        )
        return proc.returncode, (proc.stdout + proc.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, f"{type(exc).__name__}: {exc}"


def _pins_present() -> list[str]:
    try:
        return sorted(os.listdir(PIN_DIR))
    except OSError:
        return []


def battery(ledger: Ledger, binary: str, handover_user: str) -> None:
    """The whole tap family; rows land on the ledger."""
    with Step(ledger, "tap attach — the one privileged surface"):
        rc, out = _run(binary, ["attach"], sudo_user=handover_user)
        if rc == 0 and "tap attached" in out:
            kind = out.splitlines()[0].replace("attach: ", "")[:60]
            ledger.pass_row("attach plants the tap", kind)
        else:
            ledger.fail_row(
                "attach plants the tap",
                f"rc={rc}: {out[:160]}",
            )
            # Nothing after this can pass honestly — the whole stage
            # family is downstream of a planted tap. Report the state
            # and bail out of this module.
            return

        present = _pins_present()
        missing = [name for name in PIN_NAMES if name not in present]
        if not missing:
            ledger.pass_row(
                "the pinned control plane layout is complete",
                f"{len(present)} pins at {PIN_DIR}",
            )
        else:
            ledger.fail_row(
                "the pinned control plane layout is complete",
                f"missing={missing}",
            )

        uid_probe = subprocess.run(
            ["id", "-u", handover_user], capture_output=True, text=True, timeout=15
        )
        expected_uid = int(uid_probe.stdout.strip()) if uid_probe.returncode == 0 else -1
        handed = expected_uid > 0
        for name in PIN_NAMES:
            try:
                stat = os.stat(f"{PIN_DIR}/{name}")
            except OSError:
                handed = False
                break
            if stat.st_uid != expected_uid:
                handed = False
                break
        if handed:
            ledger.pass_row(
                "the pins are handed to the invoking user",
                f"owner={handover_user}",
            )
        else:
            ledger.fail_row(
                "the pins are handed to the invoking user",
                "a pin kept root ownership — unprivileged verbs would fail",
            )

    with Step(ledger, "tap idempotency and classifications"):
        rc, out = _run(binary, ["attach"], sudo_user=handover_user)
        if rc != 0 and "entries" in out:
            ledger.pass_row(
                "re-attach over a live tap refuses honestly",
                out.splitlines()[0][:70],
            )
        else:
            ledger.fail_row(
                "re-attach over a live tap refuses honestly",
                f"rc={rc}: {out[:120]}",
            )

        foreign = f"{PIN_DIR}/foreign-probe"
        planted = False
        try:
            with open(foreign, "wb") as fh:
                fh.write(b"supermassive")
            planted = True
        except OSError:
            pass
        if planted:
            rc, out = _run(binary, ["detach"])
            note = out if rc != 0 else ""
            still_there = os.path.exists(foreign)
            if rc == 0 and still_there:
                ledger.pass_row(
                    "detach keeps foreign entries (handled-not-fatal)",
                    "foreign probe survived, verdict honest",
                )
            else:
                ledger.fail_row(
                    "detach keeps foreign entries (handled-not-fatal)",
                    f"rc={rc}, foreign-survives={still_there} {note[:80]}",
                )
            try:
                os.unlink(foreign)
            except OSError:
                pass
        else:
            ledger.skip_row(
                "detach keeps foreign entries (handled-not-fatal)",
                "the foreign probe could not be planted (bpffs permission shape)",
            )

    with Step(ledger, "unprivileged read-back — the handover's whole point"):
        probe = subprocess.run(
            ["sudo", "-u", handover_user, "test", "-r", f"{PIN_DIR}/control"],
            capture_output=True,
            text=True,
            timeout=15,
        )
        rc = probe.returncode
        if rc == 0:
            ledger.pass_row(
                "the handover user can open the control map",
                f"{handover_user} reads {PIN_DIR}/control",
            )
        else:
            ledger.fail_row(
                "the handover user can open the control map",
                f"{handover_user} cannot read {PIN_DIR}/control",
            )

    with Step(ledger, "detach — nothing of zylaxion remains"):
        rc, out = _run(binary, ["detach"])
        if rc == 0:
            ledger.pass_row("detach unpins everything", out.splitlines()[0][:70])
        else:
            ledger.fail_row("detach unpins everything", f"rc={rc}: {out[:120]}")

        residue = _pins_present()
        if not residue:
            ledger.pass_row("the kernel holds no zylaxion residue", f"{PIN_DIR} absent")
        else:
            ledger.fail_row(
                "the kernel holds no zylaxion residue",
                f"residue={residue}",
            )

        rc, out = _run(binary, ["detach"])
        if rc == 0:
            ledger.pass_row("second detach is idempotent honest", out.splitlines()[0][:70])
        else:
            ledger.fail_row(
                "second detach is idempotent honest",
                f"rc={rc}: {out[:120]}",
            )

        cycles_ok = True
        for index in range(5):
            rc_a, out_a = _run(binary, ["attach"], sudo_user=handover_user)
            rc_d, out_d = _run(binary, ["detach"])
            if rc_a != 0 or rc_d != 0:
                cycles_ok = False
                ledger.fail_row(
                    "rapid attach/detach cycles stay clean",
                    f"cycle {index}: attach rc={rc_a} ({out_a[:60]}), detach rc={rc_d}",
                )
                break
            time.sleep(0.2)
        if cycles_ok:
            ledger.pass_row("rapid attach/detach cycles stay clean", "5 cycles, all clean")
