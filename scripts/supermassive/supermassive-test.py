#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""supermassive — the whole E2E on the micro-VM floor (night-master-3,
the world-first claims battery rides since night-master-1).

The zylaxion supermassive engine, the zelynic lineage's shape: one
Python engine, two lanes.

  --self-test [--binary PATH]   the rootless lane: the CLI/UX battery,
                                the pure-math proof, the zero-config
                                proof, and the world-first claims
                                battery (no decoder linked, the eBPF
                                tap embedded as an ELF-BPF object,
                                zero daemon-era strings, a
                                transitively clean dependency graph)
                                — everything honest without a tap, a
                                sound card, or root. Runs anywhere,
                                including a sandbox.
  --full     --binary PATH      the root lane (the micro-VM's PID 1
                                calls this): the whole chaos — env
                                probes, the CLI/UX battery, the tap
                                lifecycle (attach, handover,
                                idempotency, foreign entries, detach),
                                the night-dinner-3 repro (the
                                mode-0700 front door, the attach
                                self-heal, the verdict honesty law),
                                the real-usage chain (a real user with
                                no input group, uinput keystrokes, the
                                kernel-side telemetry, the PCM
                                witness, live preset/volume map
                                writes, off, the zero-process state).

Every probe lands on one ledger; the ledger prints a human row and the
MASS-RESULT sentinel per probe plus one final MASS-VERDICT line (the
CI relay — the serial console carries them, the workflow's Verdict
step greps them; the VM cannot pass exit codes through qemu).

Exit: 0 only when every row passed. The --full lane is strict: a SKIP
there means an environment the battery demanded is absent, which the
guest contract does not accept (the init stages everything the engine
needs; a missing piece is an assembly bug, never a skip).
"""

from __future__ import annotations

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from sm_audio import battery as audio_battery  # noqa: E402
from sm_env import (  # noqa: E402
    asound_cards,
    bpffs_mounted,
    btf_present,
    is_root,
    kernel_floor_ok,
    lockdown_blocks_bpf,
    parse_kernel_release,
    run,
)
from sm_journey import battery as journey_battery  # noqa: E402
from sm_report import Ledger, Step  # noqa: E402
from sm_static import battery as static_battery  # noqa: E402
from sm_tap import battery as tap_battery  # noqa: E402
from sm_ux import battery as ux_battery  # noqa: E402

GUEST_USER = "zed"
REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def _env_probes(ledger: Ledger) -> None:
    with Step(ledger, "environment probes"):
        release = ""
        try:
            with open("/proc/sys/kernel/osrelease", encoding="utf-8") as fh:
                release = fh.read().strip()
        except OSError:
            pass
        parsed = parse_kernel_release(release)
        if release and kernel_floor_ok(release):
            ledger.pass_row(
                f"kernel {release} meets the documented 5.15 floor",
                f"parsed {parsed[0]}.{parsed[1]}",
            )
        else:
            ledger.fail_row(
                "kernel meets the documented 5.15 floor",
                f"uname={release or 'unreadable'}",
            )

        if bpffs_mounted():
            ledger.pass_row("bpffs is mounted at /sys/fs/bpf", "the pin surface lives")
        else:
            ledger.fail_row(
                "bpffs is mounted at /sys/fs/bpf",
                "attach cannot pin — mount -t bpf bpf /sys/fs/bpf",
            )

        if btf_present():
            ledger.pass_row("vmlinux BTF is present", "the fentry chain step is available")
        else:
            ledger.info(
                "vmlinux BTF is absent", "the chain falls back to kprobes — still proven"
            )

        if lockdown_blocks_bpf():
            ledger.fail_row(
                "kernel lockdown does not withdraw BPF",
                "confidentiality mode blocks every bpf(2) surface",
            )
        else:
            ledger.pass_row("kernel lockdown does not withdraw BPF", "")

        cards = asound_cards()
        if cards:
            ledger.pass_row(
                "an ALSA sound card is present",
                f"{','.join(cards)} (the PCM witness can run)",
            )
        else:
            ledger.fail_row(
                "an ALSA sound card is present",
                "no card under /proc/asound — the guest must load snd-dummy",
            )

        rc, out = run(["which", "sudo"])
        if rc == 0:
            ledger.pass_row("sudo is available for the user lanes", "")
        else:
            ledger.fail_row("sudo is available for the user lanes", "the real-usage lane needs it")


def _ensure_user(ledger: Ledger, user: str) -> None:
    with Step(ledger, f"the real user ({user})"):
        rc, out = run(["id", "-u", user])
        if rc != 0:
            rc, out = run(
                ["useradd", "-m", "-s", "/bin/bash", user], timeout=30
            )
            if rc != 0:
                ledger.fail_row(
                    f"the real user {user} exists",
                    f"useradd rc={rc}: {out[:100]}",
                )
                return
        ledger.pass_row(f"the real user {user} exists", "")


def self_test(binary: str) -> int:
    """The rootless lane: everything honest without a tap or root."""
    ledger = Ledger("supermassive self-test (rootless)")
    if is_root():
        print(
            "the self-test lane refuses root — it proves the unprivileged "
            "surface; use --full on the root lane"
        )
        return 2
    ux_battery(ledger, binary, full_mode=False)
    audio_battery(ledger, binary, user="", tree_root=REPO_ROOT, root_lane=False)
    static_battery(ledger, binary, REPO_ROOT, root_lane=False)
    return ledger.finish()


def full(binary: str) -> int:
    """The root lane: the whole chaos, in order."""
    ledger = Ledger("supermassive full battery (the whole chaos)")
    ledger.strict_skips = True
    if not is_root():
        print("the full battery requires root — the guest init runs it as PID 1's child")
        return 2

    _env_probes(ledger)
    ux_battery(ledger, binary, full_mode=True)
    _ensure_user(ledger, GUEST_USER)
    tap_battery(ledger, binary, GUEST_USER)
    journey_battery(ledger, binary, GUEST_USER)
    audio_battery(ledger, binary, user=GUEST_USER, tree_root=REPO_ROOT)
    static_battery(ledger, binary, REPO_ROOT, root_lane=True)
    return ledger.finish()


def main() -> int:
    parser = argparse.ArgumentParser(
        prog="supermassive-test.py",
        description="the zylaxion supermassive E2E engine (night-master-3)",
    )
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument(
        "--self-test",
        action="store_true",
        help="the rootless lane: CLI/UX battery + pure-math + zero-config "
        "proofs + the world-first claims battery",
    )
    mode.add_argument(
        "--full",
        action="store_true",
        help="the root lane: the whole chaos (the micro-VM init calls this)",
    )
    parser.add_argument(
        "--binary",
        default=None,
        help="the zylaxion binary under test (default: target/release/zylaxion)",
    )
    args = parser.parse_args()

    binary = args.binary
    if not binary:
        binary = os.path.join(REPO_ROOT, "target", "release", "zylaxion")
    binary = os.path.abspath(binary)
    if not os.path.isfile(binary) or not os.access(binary, os.X_OK):
        print(f"the binary under test is missing or not executable: {binary}")
        print("build it first (./scripts/build.sh release) or pass --binary PATH")
        return 2

    print(f"supermassive — binary under test: {binary}")
    print(f"supermassive — repo root: {REPO_ROOT}")
    if args.self_test:
        return self_test(binary)
    return full(binary)


if __name__ == "__main__":
    sys.exit(main())
