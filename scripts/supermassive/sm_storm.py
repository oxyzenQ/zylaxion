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
  the storm itself — three uinput virtual keyboards typing concurrently
    (three devices, three writer threads: the storm's writers), and while
    they hammer: the telemetry poll that must RISE between two polls
    taken DURING the window (the realtime claim, never an
    after-the-fact reconstruction), eight live control-plane writes
    (the preset switch and the whole volume boundary lattice: 0, 42,
    the 200 cap edge, the 201 session-gate refusal, the 201 --force-this
    lift, the 10001 force-ceiling refusal, the restore), each asserted
    while the keystrokes cross the kernel, the PCM witness read
    mid-storm, and the whole window carried by the live heartbeat
    (night-dragon-maintenance-4, the loading indicator: one MASS:
    beat every couple of seconds naming the elapsed time and the
    telemetry captured, so the window's processing is visible and can
    never be mistaken for a hang);
  the brutality pass (night-dragon-maintenance-4, the detective
    proven, not described): the press-RATE probe (a dedicated quiet
    burst whose telemetry delta must clear the sustained-catch floor
    - silence is a defect, not a mood), the attach/detach CHURN (the
    tap torn down and replanted CHURN_CYCLES times while the writers
    keep typing - every cycle asserts the detach, the pin-less
    kernel, the replant, and a fresh counter catching again within
    seconds: the surveillance survives its own teardown), and the
    FREEZE witness (after the writers stop, the counter must not move
    a single press - telemetry reflects reality, it never runs away);
  the close — the injectors stop, the accumulated telemetry clears the
    three-writer floor, `off` stops the sound, the player self-exits, and
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
import threading
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
HEARTBEAT_INTERVAL = 2.0
HEARTBEAT_FLOOR = 2
# night-dragon-maintenance-4 (the brutality pass): the storm's own
# numbers, sized to the measured injector rate (five keys per writer
# at ~14 presses/s, so three writers sustain ~43/s) with the zelynic
# honest-margin law - floors catch REAL silence, they do not flake on
# a slow runner.
RATE_BURST_SECONDS = 2.0
RATE_FLOOR = 15.0
CHURN_CYCLES = 3
# The fresh-map catch window (seconds): the churn contract says the
# replanted tap "catches again within seconds", not instantly. Kernel
# trampoline/RCU settle time after an immediate detach/replant varies by
# kernel generation - first witnessed on the 7.0 archive head, where the
# catch lands late under load. The probe polls the fresh counter across
# this window and fails only when it closes empty: a persistently deaf
# replant is still a FAIL, a late catch is an honest pass with its
# latency on the ledger.
CATCH_WINDOW_SECONDS = 10.0
PIN_DIR = "/sys/fs/bpf/zylaxion"


class _StormHeartbeat:
    """The live indicator (night-dragon-maintenance-4, the owner's ask:
    the longest quiet span of the whole battery must never LOOK hung).

    The storm window is the battery's CCTV monitor: three writers hammer
    the kernel's input path and the window's verdicts ride the polls
    and the control writes around it - so for its full ~10 s the ledger
    used to print nothing, and a serial-console watcher (or a CI log
    reader) could not tell "processing events" from "wedged guest".
    The heartbeat closes that gap: a daemon thread prints one MASS:
    info line per beat - seconds elapsed, the writers' state, and the
    telemetry count READ LIVE through the tool's own status surface -
    so every beat is simultaneously the loading indicator AND one more
    mid-storm proof that the control plane answers under load. The
    sentinel contract is untouched: the heartbeat rides the MASS:
    information prefix, never the MASS-RESULT/MASS-VERDICT relay the
    workflow greps.

    The beats are counted: the lane's ledger carries one row pinning
    the indicator actually ran (HEARTBEAT_FLOOR beats over the
    measured window), so a future refactor cannot silently drop the
    indicator the owner asked for.
    """

    def __init__(self, binary: str, user: str) -> None:
        self._binary = binary
        self._user = user
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self.beats = 0
        self.peak_presses = 0

    def start(self) -> None:
        self._thread = threading.Thread(
            target=self._loop, name="storm-heartbeat", daemon=True
        )
        self._thread.start()

    def _loop(self) -> None:
        started = time.monotonic()
        while not self._stop.wait(HEARTBEAT_INTERVAL):
            elapsed = time.monotonic() - started
            presses = _presses(self._binary, self._user)
            if presses > self.peak_presses:
                self.peak_presses = presses
            self.beats += 1
            print(
                f"MASS: storm heartbeat — {elapsed:.0f}s into the window, "
                f"writers running, telemetry {presses} presses captured",
                flush=True,
            )

    def stop(self) -> None:
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=5)
            self._thread = None


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


def _status_verbose(binary: str, user: str) -> tuple[int, str]:
    """The raw status read: rc AND the full output, never swallowed.
    The parsed _presses reports a flat 0 for both 'the counter reads
    zero' and 'the status read failed outright' - the churn evidence
    needs the difference, so the raw pair is available here."""
    return _run_as_user(user, binary, ["status", "--verbose"])


def _presses(binary: str, user: str) -> int:
    rc, out = _status_verbose(binary, user)
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
    """The storm window itself: three writers, polls and control writes
    mid-storm, the rate probe, the churn, then the close."""
    with Step(ledger, "the realtime storm — verdicts under load, never after"):
        storm_dir_ok = True
        injector_a = _KeyboardInjector()
        injector_b = _KeyboardInjector()
        injector_c = _KeyboardInjector()
        started = injector_a.start() and injector_b.start() and injector_c.start()
        if not started:
            ledger.fail_row(
                "three uinput keyboards type concurrently (the storm's writers)",
                injector_a.error or injector_b.error or injector_c.error
                or "could not create the devices",
            )
            injector_a.shutdown()
            injector_b.shutdown()
            injector_c.shutdown()
            _run_as_user(user, binary, ["off"])
            _run([binary, "detach"])
            return
        ledger.pass_row(
            "three uinput keyboards type concurrently (the storm's writers)",
            "three devices, three writer threads, one kernel input path",
        )

        # The live indicator (night-dragon-maintenance-4): the window
        # stops being a silent span from this line on - every beat
        # names the elapsed time and the telemetry the tap captured,
        # read live through the tool's own status surface.
        heartbeat = _StormHeartbeat(binary, user)
        heartbeat.start()

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
            # The gate refusal is fatal INPUT (the parser's exit-2
            # convention, the same polarity the fatal-input matrix
            # pins): the CI VM legs caught the probe wanting rc 1 -
            # the probe drifted from the source it audits, the product
            # did not move.
            ["volume", "--percent", "201"], 2, "safety cap",
        )
        _control_write(
            ledger, binary, user,
            "volume 201 --force-this lifts the cap mid-storm",
            ["volume", "--percent", "201", "--force-this"], 0, "",
        )
        _control_write(
            ledger, binary, user,
            "volume 10001 --force-this refused at the force ceiling mid-storm",
            ["volume", "--percent", "10001", "--force-this"], 2, "force cap",
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

        # The press-RATE probe (night-dragon-maintenance-4, the
        # brutality pass): a dedicated quiet burst - no control writes
        # inside it - whose telemetry delta must clear the
        # sustained-catch floor. The polls prove the count RISES; the
        # rate probe proves it rises FAST ENOUGH: a detective that
        # catches one event a minute is not watching the same crime.
        rate_a = _presses(binary, user)
        time.sleep(RATE_BURST_SECONDS)
        rate_b = _presses(binary, user)
        rate = (rate_b - rate_a) / RATE_BURST_SECONDS
        if rate >= RATE_FLOOR:
            ledger.pass_row(
                "the press-rate probe clears the sustained-catch floor",
                f"{rate:.1f} presses/s over the {RATE_BURST_SECONDS:.0f}s burst "
                f"(floor {RATE_FLOOR:.0f})",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "the press-rate probe clears the sustained-catch floor",
                f"{rate:.1f} presses/s over the {RATE_BURST_SECONDS:.0f}s burst "
                f"(floor {RATE_FLOOR:.0f}) — the tap is not keeping up with "
                "the writers",
            )

        # The attach/detach CHURN (night-dragon-maintenance-4, the
        # brutality pass): the tap is torn down and replanted while
        # the writers keep typing - the surveillance must survive its
        # own teardown. Every cycle asserts four facts: the detach
        # succeeds under load, the kernel holds no residue, the
        # replant succeeds, and the fresh counter catches again within
        # seconds (the maps are brand new after a replant - the
        # counter starts at zero and must still find the crime).
        for cycle in range(1, CHURN_CYCLES + 1):
            rc, out = _run([binary, "detach"])
            pins_gone = not os.path.exists(PIN_DIR)
            if rc == 0 and pins_gone:
                ledger.pass_row(
                    f"the churn cycle {cycle} tears the tap down under load",
                    "detach rc 0, the kernel holds no residue while the "
                    "writers still type",
                )
            else:
                storm_dir_ok = False
                ledger.fail_row(
                    f"the churn cycle {cycle} tears the tap down under load",
                    f"rc={rc}, pins-gone={pins_gone}: {out[:120]}",
                )
                break
            rc, out = _attach_as_root(binary, user)
            if rc == 0 and "tap attached" in out:
                ledger.pass_row(
                    f"the churn cycle {cycle} replants the tap (the storm goes on)",
                    out.splitlines()[0][:70],
                )
                # Baseline the emit counters at replant: the catch
                # evidence reports the DELTA - presses emitted by each
                # writer SINCE the fresh maps landed.
                presses_at_replant = (
                    injector_a.presses,
                    injector_b.presses,
                    injector_c.presses,
                )
            else:
                storm_dir_ok = False
                ledger.fail_row(
                    f"the churn cycle {cycle} replants the tap (the storm goes on)",
                    f"rc={rc}: {out[:200]}",
                )
                break
            time.sleep(1.5)
            # Poll the fresh counter across the bounded catch window -
            # one instantaneous read after a fixed nap would measure the
            # kernel's settle time, not the tap's survival (the contract
            # under test: the replanted tap catches again, at all). The
            # RAW status pair rides along: a flat zero is ambiguous
            # between 'the tap is deaf' and 'the status read itself
            # refuses' - the failure evidence must name which.
            window_start = time.monotonic()
            catch_deadline = window_start + CATCH_WINDOW_SECONDS
            fresh = 0
            last_rc, last_out = 1, ""
            while True:
                last_rc, last_out = _status_verbose(binary, user)
                match = re.search(r"total presses:\s*(\d+)", last_out)
                fresh = int(match.group(1)) if (last_rc == 0 and match) else 0
                if fresh > 0 or time.monotonic() >= catch_deadline:
                    break
                time.sleep(0.5)
            if fresh > 0:
                latency = time.monotonic() - window_start
                ledger.pass_row(
                    f"the churn cycle {cycle} catches again on the fresh maps",
                    f"the replanted counter caught {fresh} presses "
                    f"{latency:.1f}s into the {CATCH_WINDOW_SECONDS:.0f}s "
                    "catch window",
                )
            else:
                storm_dir_ok = False
                writers = "; ".join(
                    f"{name} {inj.summary()} +{inj.presses - since_replant} "
                    f"since replant"
                    for name, inj, since_replant in (
                        ("a", injector_a, presses_at_replant[0]),
                        ("b", injector_b, presses_at_replant[1]),
                        ("c", injector_c, presses_at_replant[2]),
                    )
                )
                pins = (
                    sorted(os.listdir(PIN_DIR)) if os.path.isdir(PIN_DIR) else []
                )
                status_body = " | ".join(
                    line.strip()[:64]
                    for line in last_out.splitlines()[:8]
                    if line.strip()
                )
                # The execution witness: run_cnt on the pinned program
                # separates 'the program never runs' (link/trampoline
                # problem) from 'the program runs but the counters
                # freeze' (map-identity problem). Absent bpftool says so
                # and costs nothing.
                prog_rc, prog_out = _run(
                    ["bpftool", "prog", "show", "pinned", f"{PIN_DIR}/tap"]
                )
                prog_witness = (
                    prog_out.replace("\n", " ")[:180]
                    if prog_rc == 0
                    else f"bpftool rc={prog_rc}"
                )
                link_rc, link_out = _run(
                    ["bpftool", "link", "show", "pinned", f"{PIN_DIR}/tap_link"]
                )
                link_witness = (
                    link_out.replace("\n", " ")[:140]
                    if link_rc == 0
                    else f"bpftool rc={link_rc}"
                )
                ledger.fail_row(
                    f"the churn cycle {cycle} catches again on the fresh maps",
                    f"the fresh counter never moved within "
                    f"{CATCH_WINDOW_SECONDS:.0f}s while the writers ran - "
                    f"evidence: writers [{writers[:200]}]; "
                    f"status[{status_body[:220]}]; pins={pins}; "
                    f"prog[{prog_witness}]; link[{link_witness}]",
                )
                break

        # The sound returns (night-dragon-maintenance-4): the churn's
        # detach reaps the running player by contract (a running
        # player never survives a teardown), so the storm explicitly
        # brings the sound back after the final replant - the
        # surveillance AND the synth survive the teardown together,
        # and the close's `off` row stops a real player, not a
        # vacuous shape.
        if storm_dir_ok:
            rc, out = _run_as_user(user, binary, ["on", "--preset", "topre"])
            time.sleep(1.0)
            pid = _player_pid(user)
            if rc == 0 and "sound on:" in out and pid:
                ledger.pass_row(
                    "the storm's sound returns after the churn (the player respawns)",
                    f"pid {pid} renders under the replanted tap",
                )
            else:
                storm_dir_ok = False
                ledger.fail_row(
                    "the storm's sound returns after the churn (the player respawns)",
                    f"rc={rc}: {out[:200]}, pid={pid}",
                )

        injector_a.shutdown()
        injector_b.shutdown()
        injector_c.shutdown()
        final_presses = _presses(binary, user)
        heartbeat.stop()
        if final_presses >= FINAL_PRESSES_FLOOR:
            ledger.pass_row(
                "the storm's accumulated telemetry clears the three-writer floor",
                f"total presses: {final_presses} (floor {FINAL_PRESSES_FLOOR})",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "the storm's accumulated telemetry clears the three-writer floor",
                f"total presses: {final_presses} (floor {FINAL_PRESSES_FLOOR})",
            )

        # The indicator verdict (night-dragon-maintenance-4): the beats
        # are pinned so the loading indicator cannot silently regress.
        if heartbeat.beats >= HEARTBEAT_FLOOR:
            ledger.pass_row(
                "the storm window carried the live heartbeat (the loading indicator)",
                f"{heartbeat.beats} beats every {HEARTBEAT_INTERVAL:.0f}s, "
                f"peak telemetry {heartbeat.peak_presses} presses",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "the storm window carried the live heartbeat (the loading indicator)",
                f"{heartbeat.beats} beats, wanted >= {HEARTBEAT_FLOOR} "
                f"(the window went silent again)",
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

        # The FREEZE witness (night-dragon-maintenance-4, the
        # brutality pass): the writers are stopped, the player is
        # gone, the tap is still attached - and the counter must not
        # move a single press across the freeze span. Telemetry
        # reflects reality: a counter that keeps counting after the
        # crime stopped is a detective inventing evidence.
        freeze_a = _presses(binary, user)
        time.sleep(1.0)
        freeze_b = _presses(binary, user)
        if freeze_a == final_presses and freeze_b == freeze_a:
            ledger.pass_row(
                "the telemetry freezes when the writers stop (the counter reflects reality)",
                f"{freeze_a} presses held across the 1s freeze span",
            )
        else:
            storm_dir_ok = False
            ledger.fail_row(
                "the telemetry freezes when the writers stop (the counter reflects reality)",
                f"final {final_presses}, freeze reads {freeze_a} then {freeze_b} "
                "- the counter moved after the writers stopped",
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
                f"window ~{STORM_SECONDS:.0f}s, three writers, the rate probe, "
                f"{CHURN_CYCLES} churn cycles, the freeze witness, "
                f"{heartbeat.beats} live heartbeat beats, every verdict "
                "asserted under load",
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
