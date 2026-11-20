# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
"""The CLI, UX, and fatal-input battery (night-master-3).

The rootless-safe lane: every probe here exercises a surface that is
honest without the tap attached — the identity render, the curated
help, the discovery verbs, the parser's fatal-input refusals, the
loading lane's narration contract (night-ux-orion-1), the tap-less
doors, and the root doors both polarities. This module runs
in BOTH engine modes: --self-test rides it against a locally built
binary (no root, no tap), --full rides it inside the micro-VM before
the tap stages begin. A probe asserts the contract the source pins —
exit codes, the diagnostic strings, the suggestion UX — never a
snapshot of prose that will drift.
"""

from __future__ import annotations

import os
import pty
import re
import subprocess
import sys
import time

# The root-owned pycache guard (night-cafe-1, the same law the
# supermassive-test entry and sm_static carry): a direct run of this
# module must never leave root-owned bytecode caches beside the
# scripts for the project-local modules it imports below.
sys.dont_write_bytecode = True

from sm_report import Ledger, Step  # noqa: E402


def run_cli(
    binary: str,
    args: list[str],
    env_extra: dict[str, str] | None = None,
    timeout: int = 45,
) -> tuple[int, str]:
    """One canonical invocation, clean environment, captured output."""
    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "LANG": "C.UTF-8",
        "TERM": "dumb",
    }
    if env_extra:
        env.update(env_extra)
    try:
        proc = subprocess.run(
            [binary, *args],
            capture_output=True,
            text=True,
            env=env,
            timeout=timeout,
        )
        return proc.returncode, (proc.stdout + proc.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, f"{type(exc).__name__}: {exc}"


def run_as(
    user: str,
    binary: str,
    args: list[str],
    timeout: int = 45,
) -> tuple[int, str]:
    """One canonical invocation AS another user (a root caller)."""
    try:
        proc = subprocess.run(
            ["sudo", "-u", user, binary, *args],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return proc.returncode, (proc.stdout + proc.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, f"{type(exc).__name__}: {exc}"


def battery(ledger: Ledger, binary: str, full_mode: bool) -> None:
    """The whole CLI/UX family; rows land on the ledger."""
    _identity(ledger, binary)
    _discovery(ledger, binary)
    _fatal_input(ledger, binary)
    _loading_lane(ledger, binary)
    _tapless_doors(ledger, binary, full_mode)
    _root_doors(ledger, binary, full_mode)


def _identity(ledger: Ledger, binary: str) -> None:
    with Step(ledger, "CLI identity surfaces"):
        rc, out = run_cli(binary, ["-V"])
        if rc == 0 and "zylaxion" in out:
            ledger.pass_row("-V prints the build identity", out.splitlines()[0][:70])
        else:
            ledger.fail_row("-V prints the build identity", f"rc={rc}: {out[:90]}")

        rc, out = run_cli(binary, [])
        if rc == 0 and out:
            ledger.pass_row("no-args overview prints and exits 0", f"{len(out)} bytes")
        else:
            ledger.fail_row("no-args overview prints and exits 0", f"rc={rc}: {out[:90]}")

        rc, out = run_cli(binary, ["-h"])
        needed = ("attach", "detach", "doctor", "status")
        missing = [token for token in needed if token not in out]
        if rc == 0 and not missing:
            ledger.pass_row("-h carries the curated reference", "all verbs present")
        else:
            ledger.fail_row(
                "-h carries the curated reference",
                f"rc={rc}, missing={missing}",
            )


def _discovery(ledger: Ledger, binary: str) -> None:
    with Step(ledger, "CLI discovery surfaces"):
        rc, out = run_cli(binary, ["list-presets"])
        names = [line for line in out.splitlines() if line.strip()]
        if rc == 0 and len(names) >= 20:
            ledger.pass_row(
                "list-presets renders the compiled registry",
                f"{len(names)} rows",
            )
        else:
            ledger.fail_row(
                "list-presets renders the compiled registry",
                f"rc={rc}, rows={len(names)}",
            )

        rc, out = run_cli(binary, ["list-backends"])
        if rc == 0 and out:
            ledger.pass_row("list-backends renders", out.splitlines()[0][:60])
        else:
            ledger.fail_row("list-backends renders", f"rc={rc}: {out[:90]}")

        rc, out = run_cli(binary, ["doctor"])
        # night-badas-4: the probe used to equate the doctor's rc with
        # the KERNEL verdict's polarity ("verified supported" -> rc 0).
        # The doctor's actual contract is the WHOLE report's health:
        # rc 0 <=> the "All checks passed" summary line, rc 1 <=> the
        # "Some checks failed" summary line — and on a runner with no
        # audio device (and no XDG_RUNTIME_DIR) the kernel verdict can
        # honestly be "verified supported" while the report still exits
        # 1. The probe therefore pins the real contract: the eBPF row
        # is present with its canonical verdict word (the zelynic row
        # geometry, night-dragon-maintenance-1 — the row key carries
        # the subject, the canonical verdict string carries the
        # polarity), AND the summary line matches the rc.
        ebpf_row = next(
            (line for line in out.splitlines() if line.strip().startswith("eBPF:")),
            "",
        )
        supported = "verified supported" in ebpf_row
        unsupported = "verified unsupported" in ebpf_row
        verdict_ok = supported or unsupported
        summary_ok = (rc == 0 and "All checks passed" in out) or (
            rc == 1 and "Some checks failed" in out
        )
        if verdict_ok and summary_ok:
            verdict = (
                "kernel verified supported"
                if supported
                else "kernel verified unsupported"
            )
            ledger.pass_row("doctor prints the kernel verdict", f"{verdict}, rc {rc} matches the summary")
        else:
            ledger.fail_row(
                "doctor prints the kernel verdict",
                f"rc={rc}, kernel-verdict-line={verdict_ok}, "
                f"summary-matches-rc={summary_ok}: {out[:80]}",
            )


def _fatal_input(ledger: Ledger, binary: str) -> None:
    """The parser's fatal-input matrix: every row must refuse, exit 2."""
    with Step(ledger, "fatal input — the parser refuses"):
        cases = [
            ("unknown verb refused", ["bogus-verb"]),
            ("unknown flag refused", ["--bogus-flag"]),
            ("bad volume text refused", ["volume", "--percent", "abc"]),
            ("volume percent overflow refused", ["volume", "--percent", "4294967296"]),
            ("negative volume refused", ["volume", "--percent", "-1"]),
            ("preset --switch without value refused", ["preset", "--switch"]),
        ]
        for name, args in cases:
            rc, out = run_cli(binary, args)
            if rc == 2:
                ledger.pass_row(name, "exit 2")
            else:
                ledger.fail_row(name, f"rc={rc}: {out[:80]}")

        rc, out = run_cli(binary, ["sttus"])
        suggestion = (
            "did you mean" in out or "a similar subcommand exists" in out
        )
        if rc == 2 and suggestion:
            ledger.pass_row("typo verb gets the did-you-mean tip", "suggestion carried")
        else:
            ledger.fail_row(
                "typo verb gets the did-you-mean tip",
                f"rc={rc}, tip={suggestion}",
            )

        rc, out = run_cli(binary, ["on", "--preset"])
        if rc == 2 and "a value is required" in out:
            ledger.pass_row(
                "on --preset without a value refused at parse", "exit 2"
            )
        else:
            ledger.fail_row(
                "on --preset without a value refused at parse",
                f"rc={rc}: {out[:80]}",
            )


def _pty_stderr_run(
    binary: str,
    args: list[str],
    env_extra: dict[str, str] | None = None,
    timeout: int = 45,
) -> tuple[int, str, str]:
    """One invocation with a REAL pty behind stderr; stdout captured.

    The loading lane's pty door: the spinner narrates only on a
    terminal, so the probe gives stderr a true one — the frames land
    on the pty master while stdout (the verdict) rides a plain pipe,
    exactly the two-stream split the contract pins. Returns
    (rc, stdout_text, pty_text); the pty text carries the raw draw
    bytes, ANSI escapes included.
    """
    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "LANG": "C.UTF-8",
        "TERM": "dumb",
    }
    if env_extra:
        env.update(env_extra)
    master, slave = pty.openpty()
    os.set_blocking(master, False)
    try:
        proc = subprocess.Popen(
            [binary, *args],
            stdout=subprocess.PIPE,
            stderr=slave,
            env=env,
        )
    finally:
        os.close(slave)
    chunks: list[bytes] = []
    deadline = time.monotonic() + timeout
    while True:
        try:
            data = os.read(master, 65536)
        except BlockingIOError:
            data = None
        except OSError:
            break  # EIO on Linux: every slave fd closed — the EOF door
        if data:
            chunks.append(data)
            continue
        if time.monotonic() > deadline:
            proc.kill()
            proc.wait()
            os.close(master)
            return 124, "", ""
        if proc.poll() is not None:
            # Exited: the last writes may still sit in the pty buffer —
            # one more pass drains them (or hits the EOF door).
            time.sleep(0.02)
            continue
        time.sleep(0.01)
    os.close(master)
    stdout_text = proc.stdout.read().decode("utf-8", "replace")
    rc = proc.wait()
    return rc, stdout_text, b"".join(chunks).decode("utf-8", "replace")


def _loading_lane(ledger: Ledger, binary: str) -> None:
    """The loading lane (night-ux-orion-1): the spinner's real bytes.

    The fetch behind --check-update is the one indeterminate wait on a
    drive-by surface. Three doors, all honest without the tap, and the
    lane skips NOTHING — both caller polarities carry their own rows:

    - the piped door: a captured stderr must NEVER see narration — the
      inert machine's byte silence (no frames, no clears, no carriage
      returns anywhere). Under a root caller the contract is the
      refusal: the privilege guard exits 1 before the machine can arm,
      and the refusal rides a spinner-free stream;
    - the pty ASCII door: a real plain terminal draws the rotating dash
      if the fetch crossed the lazy threshold, or stays silent if it
      did not — both honest; the contract is the seat and the clear;
    - the pty helix door: a color-capable UTF-8 terminal draws the
      braille strands under the same terms.

    The seat and the clear are the pins: every drawn frame must match
    the style ladder's seat, and the LAST drawn frame must be followed
    by the full-width clear — the verdict never lands on a line the
    spinner left dirty. stdout must carry zero spinner bytes in every
    door and every polarity.
    """
    label_prefix = "[checking for updates... "
    with Step(ledger, "the loading lane — the wait narrates itself"):
        # The piped door first: byte silence on either polarity.
        rc, out = run_cli(binary, ["--check-update"], env_extra={"NO_COLOR": "1"})
        clean = "\r" not in out and label_prefix not in out
        if rc == 1 and "do not use sudo" in out and clean:
            ledger.pass_row(
                "the loading lane never arms under the root refusal (piped)",
                "the guard exits 1 before the arm, the stream stays clean",
            )
        elif rc == 0 and clean:
            ledger.pass_row(
                "the loading lane stays off a piped stderr",
                "byte-silent: no frames, no clears, no carriage returns",
            )
        else:
            ledger.fail_row(
                "the loading lane stays honest on a piped stderr",
                f"rc={rc}: {out[:90]}",
            )

        # The pty doors. Frames drew or did not — both honest; the
        # network's speed is not the contract. A root caller never
        # reaches the lane (the guard refuses first), and that door is
        # its own row, never a skip.
        legs = [
            (
                "the ASCII floor draws the rotating dash on a plain pty",
                {"TERM": "dumb", "LANG": "C", "NO_COLOR": "1"},
                "-",
            ),
            (
                "the helix draws braille strands on a capable pty",
                {"TERM": "xterm-256color", "LANG": "C.UTF-8"},
                "braille",
            ),
        ]
        for name, env_extra, seat in legs:
            rc, out_stdout, pty_text = _pty_stderr_run(
                binary, ["--check-update"], env_extra=env_extra
            )
            plain = re.sub(r"\x1b\[[0-9;]*m", "", pty_text)
            starts = [
                i
                for i in range(len(plain))
                if plain.startswith("\r" + label_prefix, i)
            ]
            if rc == 1 and "do not use sudo" in out_stdout + plain and not starts:
                ledger.pass_row(
                    f"{name} — the root polarity",
                    "the guard refuses before the lane arms, the pty stays frame-free",
                )
                continue
            stdout_clean = (
                rc == 0 and "\r" not in out_stdout and label_prefix not in out_stdout
            )
            seat_ok = True
            clear_ok = True
            if starts:
                draw_head = len("\r" + label_prefix)
                frames: list[str] = []
                for i in starts:
                    close = plain.find("]", i + draw_head)
                    if close == -1:
                        seat_ok = False
                        break
                    frames.append(plain[i + draw_head : close])
                if seat == "-":
                    seat_ok = seat_ok and bool(frames) and all(
                        ch in "-\\|/" for frame in frames for ch in frame
                    )
                    # The drawn line's cells: the bracket prefix, the
                    # frame's one cell, the closing bracket.
                    clear_width = len(label_prefix) + 1 + 1
                else:
                    seat_ok = seat_ok and bool(frames) and all(
                        0x2800 <= ord(ch) <= 0x28FF
                        for frame in frames
                        for ch in frame
                    )
                    clear_width = len(label_prefix) + 2 + 1
                clear = "\r" + " " * clear_width + "\r"
                clear_ok = clear in plain[starts[-1] :]
            verdict = f"frames={len(starts)}, seat={'ok' if seat_ok else 'drift'}, clear={'ok' if clear_ok else 'MISSING'}"
            if stdout_clean and seat_ok and clear_ok:
                ledger.pass_row(name, verdict)
            else:
                ledger.fail_row(name, f"rc={rc}: {verdict}: {plain[:90]!r}")


def _tapless_doors(ledger: Ledger, binary: str, full_mode: bool) -> None:
    """Every verb must answer honestly with the tap absent."""
    with Step(ledger, "honest refusals — the tap-less door"):
        rc, out = run_cli(binary, ["on"])
        # The door the caller meets is the honest contract for ITS
        # lane: the full lane's caller is root, so `on` meets the root
        # refusal (a root player would open audio in root's session —
        # the parent owns the door before anything spawns); the
        # self-test lane is unprivileged and meets the kernel gate,
        # the tap gate, the unprivileged-bpf posture gate (a distro
        # that builds CONFIG_BPF_UNPRIV_DEFAULT_OFF=y locks the verbs
        # out for the whole boot), or — on a narrowed-era kernel
        # (5.19+, night-ebpf-era-1) where the posture gate no longer
        # fires — the walk-block door (the verb reached the pin open
        # honestly and the host's bpffs mode refused the walk). All
        # honest environment verdicts. The user's tap-less door is
        # asserted separately in the real-usage lane (on after
        # detach, as the real user).
        root_door = "do not use sudo" in out
        kernel_door = "kernel eBPF verdict" in out
        tap_door = "the tap is not attached" in out and "zylaxion attach" in out
        posture_door = "unprivileged bpf" in out
        walk_door = "cannot walk to" in out and "chmod o+x" in out
        if full_mode:
            ok = rc == 1 and root_door
            shape = "root door"
        else:
            ok = rc == 1 and (tap_door or kernel_door or posture_door or walk_door)
            if tap_door:
                shape = "tap door"
            elif kernel_door:
                shape = "kernel door"
            elif posture_door:
                shape = "posture door"
            else:
                shape = "walk door"
        if ok:
            ledger.pass_row(
                "on without the ready environment answers honestly",
                f"rc 1, {shape} carried",
            )
        else:
            ledger.fail_row(
                "on without the ready environment answers honestly",
                f"rc={rc}, root-door={root_door}, tap-door={tap_door}, "
                f"kernel-door={kernel_door}, posture-door={posture_door}, "
                f"walk-door={walk_door}: {out[:90]}",
            )

        rc, out = run_cli(binary, ["_play"])
        if full_mode:
            # The full lane's caller is root — the player-side guard
            # owns the door before anything else.
            if rc == 1 and "do not use sudo" in out:
                ledger.pass_row(
                    "_play as root refuses before opening anything",
                    "do-not-use-sudo carried",
                )
            else:
                ledger.fail_row(
                    "_play as root refuses before opening anything",
                    f"rc={rc}: {out[:90]}",
                )
        else:
            # Unprivileged, tap-less: the player's own honest startup
            # failure — the pinned map is unreadable (the message names
            # the tap and the attach fix), or the kernel's
            # unprivileged-bpf posture gate ends the run classified
            # (night-badas-4 round 5).
            honest = (
                "cannot open the pinned control map" in out
                or "unprivileged bpf" in out
                or "zylaxion doctor" in out
            )
            if rc == 1 and honest:
                ledger.pass_row(
                    "_play startup failure names the tap and the fix",
                    "honest startup diagnostic carried",
                )
            else:
                ledger.fail_row(
                    "_play startup failure names the tap and the fix",
                    f"rc={rc}, honest={honest}: {out[:90]}",
                )

        rc, out = run_cli(binary, ["off"])
        if rc == 0 and "sound is off" in out:
            first = out.splitlines()[0][:60]
            ledger.pass_row("off without tap is idempotent honest", first)
        else:
            ledger.fail_row(
                "off without tap is idempotent honest",
                f"rc={rc}: {out[:90]}",
            )

        rc, out = run_cli(binary, ["status"])
        if rc == 0 and out:
            ledger.pass_row(
                "status without tap prints the honest posture", "rc 0"
            )
        else:
            ledger.fail_row(
                "status without tap prints the honest posture",
                f"rc={rc}: {out[:90]}",
            )

        rc, out = run_cli(binary, ["detach"])
        if rc == 0:
            first = out.splitlines()[0][:60] if out else ""
            ledger.pass_row("detach without tap is idempotent honest", first)
        else:
            ledger.fail_row(
                "detach without tap is idempotent honest",
                f"rc={rc}: {out[:90]}",
            )


def _root_doors(ledger: Ledger, binary: str, full_mode: bool) -> None:
    """The remaining root-refusal family (the `on` root door lives in
    the tap-less step above; this step owns the network door and the
    attach polarity)."""
    with Step(ledger, "root doors"):
        caller_is_root = os.geteuid() == 0

        if caller_is_root and full_mode:
            rc, out = run_cli(binary, ["--check-update"])
            if rc == 1 and "do not use sudo" in out:
                ledger.pass_row(
                    "--check-update as root refuses before any network I/O",
                    "do-not-use-sudo",
                )
            else:
                ledger.fail_row(
                    "--check-update as root refuses before any network I/O",
                    f"rc={rc}: {out[:90]}",
                )
        else:
            ledger.skip_row(
                "--check-update as root refuses before any network I/O",
                "this lane runs unprivileged — the root lane owns the door",
            )

        if caller_is_root and full_mode:
            rc, out = run_as("zed", binary, ["attach"])
        else:
            rc, out = run_cli(binary, ["attach"])
        if rc == 1 and "sudo" in out:
            ledger.pass_row(
                "attach as a plain user refuses (the one root surface)",
                "run-under-sudo carried",
            )
        else:
            ledger.fail_row(
                "attach as a plain user refuses (the one root surface)",
                f"rc={rc}: {out[:120]}",
            )
