# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""The realtime depth stresstest (night-dragon-maintenance-2, the
zejtron storm lineage).

The owner's ask: the ultimate real function of the supermassive E2E is
a REALTIME depth leg — concurrent keystroke writers hammering the
kernel's input path while the tool's own surfaces answer, every verdict
asserted UNDER LOAD, never after it. The zejtron storm surface proved
the shape on the introspection estate; this module carries it to the
tap estate:

  the full verb walk, sound on — the explicit-preset `on --preset`
    variant, the one-screen `status`, and the `doctor` report while the
    tap is live and the player runs (the summary-matches-rc contract,
    the same polarity law the CLI battery pins rootless);
  the storm itself — two uinput virtual keyboards typing concurrently
    (two devices, two writer threads: the storm's writers), and while
    they hammer: the telemetry poll that must RISE between two polls
    taken DURING the window (the realtime claim, never an
    after-the-fact reconstruction), eight live control-plane writes
    (the preset switch and the whole volume boundary lattice: 0, 42,
    the 200 cap edge, the 201 session-gate refusal, the 201 --force-this
    lift, the 10001 force-ceiling refusal, the restore), each asserted
    while the keystrokes cross the kernel, and the PCM witness read
    mid-storm;
  the close — the injectors stop, the accumulated telemetry clears the
    two-writer floor, `off` stops the sound, the player self-exits, and
    the lane ends detached: nothing of zylaxion remains.

Root lane only (it drives root's attach). It rides the micro-VM's
--full lane after the zero-to-hero chain, re-planting the tap the chain
detached. When the guest kernel's unprivileged-bpf posture locks the
user lane, the storm honestly rides the classified-refusal branch the
product ships for that world (the same two-worlds posture the
zero-to-hero chain carries) — the door still gets proven, the storm
just does not run.

The keyboard instruments are imported from sm_audio: one
implementation, two callers — the engine family's shared instruments
are the instruments the whole battery drives, never a private twin.
"""

from __future__ import annotations

import os
import re
import subprocess
import time

from sm_audio import (  # noqa: E402 - the engine family's shared instruments
    _KeyboardInjector,
    _pcm_state,
    _player_pid,
    _run,
    _run_as_user,
    _unpriv_bpf_disabled,
)
from sm_report import Ledger, Step

STORM_SECONDS = 8.0
FINAL_PRESSES_FLOOR = 50
RUN_TIMEOUT = 90


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


def _presses(binary: str, user: str) -> int:
    rc, out = _run_as_user(user, binary, ["status", "--verbose"])
    match = re.search(r"total presses:\s*(\d+)", out)
    return int(match.group(1)) if (rc == 0 and match) else 0


def battery(ledger: Ledger, binary: str, user: str) -> None:
    """The whole storm family; rows land on the ledger."""
    if os.geteuid() != 0:
        ledger.skip_row(
            "the realtime storm needs the root lane",
            "not root — the guest's --full lane owns it",
        )
        return

    posture = _unpriv_bpf_disabled()
    if not _open_the_door(ledger, binary, user, posture):
        return
    _storm(ledger, binary, user)


def _open_the_door(
    ledger: Ledger, binary: str, user: str, posture: int
) -> bool:
    """Replant the tap, start the sound with the explicit-preset
    variant, and walk the attached verbs. False = the lane ends here
    (the refusal world or an assembly failure), already reported."""
    with Step(ledger, "the storm stage — sound on, every verb answered"):
        rc, out = _attach_as_root(binary, user)
        if rc == 0 and "tap attached" in out:
            ledger.pass_row(
                "the storm replants the tap (the SUDO_USER handover)",
                out.splitlines()[0][:70],
            )
        else:
            ledger.fail_row(
                "the storm replants the tap (the SUDO_USER handover)",
                f"rc={rc}: {out[:200]}",
            )
            return False

        rc, out = _run_as_user(user, binary, ["on", "--preset", "topre"])
        if posture != 0:
            if rc == 1 and "unprivileged bpf" in out:
                ledger.pass_row(
                    "on --preset as the real user refuses at the kernel's "
                    "unprivileged-bpf gate",
                    f"posture {posture}: the classified refusal world, "
                    "the storm honestly does not run",
                )
            else:
                ledger.fail_row(
                    "on --preset as the real user refuses at the kernel's "
                    "unprivileged-bpf gate",
                    f"posture {posture}, rc={rc}: {out[:200]}",
                )
            _run([binary, "detach"])
            return False
        if rc == 0 and "sound on:" in out:
            ledger.pass_row(
                "on --preset topre (the explicit-preset variant) starts the sound",
                out.splitlines()[0][:70],
            )
        else:
            ledger.fail_row(
                "on --preset topre (the explicit-preset variant) starts the sound",
                f"rc={rc}: {out[:200]}",
            )
            _run([binary, "detach"])
            return False

        time.sleep(1.0)
        pid = _player_pid(user)
        if pid:
            ledger.pass_row(
                "the player renders under the explicit-preset start",
                f"pid {pid}",
            )
        else:
            ledger.fail_row(
                "the player renders under the explicit-preset start",
                "no zylaxion process under the user",
            )

        rc, out = _run_as_user(user, binary, ["status"])
        if rc == 0 and out:
            ledger.pass_row(
                "status (the one-screen truth) answers while the player runs",
                out.splitlines()[0][:70],
            )
        else:
            ledger.fail_row(
                "status (the one-screen truth) answers while the player runs",
                f"rc={rc}: {out[:120]}",
            )

        rc, out = _run_as_user(user, binary, ["doctor"])
        summary_ok = (rc == 0 and "All checks passed" in out) or (
            rc == 1 and "Some checks failed" in out
        )
        if summary_ok:
            verdict = "all checks passed" if rc == 0 else "some checks failed"
            ledger.pass_row(
                "doctor answers while the tap is live (the summary matches the rc)",
                f"rc {rc}: {verdict}",
            )
        else:
            ledger.fail_row(
                "doctor answers while the tap is live (the summary matches the rc)",
                f"rc={rc}: {out[:200]}",
            )
        return True


def _storm(ledger: Ledger, binary: str, user: str) -> None:
    """The storm window itself: two writers, polls and control writes
    mid-storm, then the close."""
    with Step(ledger, "the realtime storm — verdicts under load, never after"):
        storm_dir_ok = True
        injector_a = _KeyboardInjector()
        injector_b = _KeyboardInjector()
        started = injector_a.start() and injector_b.start()
        if not started:
            ledger.fail_row(
                "two uinput keyboards type concurrently (the storm's writers)",
                injector_a.error or injector_b.error or "could not create the devices",
            )
            injector_a.shutdown()
            injector_b.shutdown()
            _run_as_user(user, binary, ["off"])
            _run([binary, "detach"])
            return
        ledger.pass_row(
            "two uinput keyboards type concurrently (the storm's writers)",
            "two devices, two writer threads, one kernel input path",
        )

        poll_a = _presses(binary, user)
        if poll_a > 0:
            ledger.pass_row(
                "poll A reads the telemetry mid-storm",
                f"total presses: {poll_a}",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "poll A reads the telemetry mid-storm",
                "status --verbose never reported a press while the writers ran",
            )

        # The eight live control-plane writes, each asserted while the
        # keystrokes cross the kernel (the zejtron verdicts-under-load
        # law). The volume lattice walks the gate's own contract: the
        # session cap edge, the session refusal, the force lift, and
        # the force ceiling.
        _control_write(
            ledger, binary, user,
            "preset --switch cherry_mx_black (one map write mid-storm)",
            ["preset", "--switch", "cherry_mx_black"], 0, "",
        )
        _control_write(ledger, binary, user, "volume 42 mid-storm",
                       ["volume", "--percent", "42"], 0, "")
        _control_write(ledger, binary, user, "volume 0 (the silence floor) mid-storm",
                       ["volume", "--percent", "0"], 0, "")
        _control_write(ledger, binary, user, "volume 200 (the cap edge) mid-storm",
                       ["volume", "--percent", "200"], 0, "")
        _control_write(
            ledger, binary, user,
            "volume 201 refused by the session gate mid-storm",
            ["volume", "--percent", "201"], 1, "safety cap",
        )
        _control_write(
            ledger, binary, user,
            "volume 201 --force-this lifts the cap mid-storm",
            ["volume", "--percent", "201", "--force-this"], 0, "",
        )
        _control_write(
            ledger, binary, user,
            "volume 10001 --force-this refused at the force ceiling mid-storm",
            ["volume", "--percent", "10001", "--force-this"], 1, "force cap",
        )
        _control_write(ledger, binary, user, "volume restored to 42 mid-storm",
                       ["volume", "--percent", "42"], 0, "")

        time.sleep(1.5)
        poll_b = _presses(binary, user)
        if poll_b > poll_a:
            ledger.pass_row(
                "poll B reads the telemetry RISING between two mid-storm polls",
                f"{poll_a} -> {poll_b} while the writers still ran "
                "(the realtime claim, not a reconstruction)",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "poll B reads the telemetry RISING between two mid-storm polls",
                f"poll A {poll_a}, poll B {poll_b} — the count did not rise "
                "under the writers",
            )

        pcm_witness = ""
        for card in ("card0", "card1"):
            state = _pcm_state(card)
            if state in ("RUNNING", "XRUN"):
                pcm_witness = f"{card}/pcm0p ({state})"
                break
        if pcm_witness:
            ledger.pass_row(
                "the PCM witness reads mid-storm (frames render under load)",
                pcm_witness,
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "the PCM witness reads mid-storm (frames render under load)",
                "no ALSA PCM reached RUNNING or XRUN during the window",
            )

        injector_a.shutdown()
        injector_b.shutdown()
        final_presses = _presses(binary, user)
        if final_presses >= FINAL_PRESSES_FLOOR:
            ledger.pass_row(
                "the storm's accumulated telemetry clears the two-writer floor",
                f"total presses: {final_presses} (floor {FINAL_PRESSES_FLOOR})",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "the storm's accumulated telemetry clears the two-writer floor",
                f"total presses: {final_presses} (floor {FINAL_PRESSES_FLOOR})",
            )

        rc, out = _run_as_user(user, binary, ["off"])
        time.sleep(1.5)
        stopped = rc == 0 and not _player_pid(user)
        if stopped:
            ledger.pass_row(
                "off stops the storm's sound and the player self-exits",
                "zero processes remain after the storm",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "off stops the storm's sound and the player self-exits",
                f"rc={rc}, pid-still-alive={bool(_player_pid(user))}",
            )

        rc, out = _run([binary, "detach"])
        if rc == 0:
            ledger.pass_row(
                "the storm ends detached (nothing of zylaxion remains)",
                out.splitlines()[0][:70] if out else "",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "the storm ends detached (nothing of zylaxion remains)",
                f"rc={rc}: {out[:120]}",
            )

        if storm_dir_ok:
            ledger.info(
                "the realtime storm closes",
                f"window ~{STORM_SECONDS:.0f}s, two writers, ten mid-storm "
                "verdicts, every one asserted under load",
            )


def _control_write(
    ledger: Ledger,
    binary: str,
    user: str,
    name: str,
    args: list[str],
    want_rc: int,
    token: str,
) -> bool:
    """One mid-storm control-plane write, asserted on the ledger while
    the writers hammer. True = the verdict held."""
    rc, out = _run_as_user(user, binary, args)
    token_ok = (not token) or (token in out)
    if rc == want_rc and token_ok:
        ledger.pass_row(name, out.splitlines()[0][:70] if out else "rc ok")
        return True
    ledger.fail_row(
        name,
        f"rc={rc} (wanted {want_rc}), token={token!r}: {out[:120]}",
    )
    return False
