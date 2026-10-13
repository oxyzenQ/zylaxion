# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""Environment probes for the supermassive battery (night-master-3).

Pure /proc and /sys reads plus a couple of guarded shell outs — the
facts every stage needs before it can honestly run or honestly skip.
Nothing here mutates state; the batteries do that. The probe row names
carry the fact, not the tool: "kernel 6.18.0 (floor 5.15)" reads as a
verdict on any serial console.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess


def run(cmd: list[str], timeout: int = 60) -> tuple[int, str]:
    """A guarded shell out: (rc, combined output). Never raises."""
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return proc.returncode, (proc.stdout + proc.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, f"{type(exc).__name__}: {exc}"


def parse_kernel_release(release: str) -> tuple[int, int] | None:
    """The zylaxion session::kernel parse shape: (major, minor) or None."""
    match = re.match(r"^(\d+)\.(\d+)", release)
    if not match:
        return None
    return int(match.group(1)), int(match.group(2))


def kernel_floor_ok(release: str, floor: tuple[int, int] = (5, 15)) -> bool:
    """The zylaxion floor law: (major, minor) meets 5.15."""
    parsed = parse_kernel_release(release)
    if parsed is None:
        return False
    major, minor = parsed
    return major > floor[0] or (major == floor[0] and minor >= floor[1])


def is_root() -> bool:
    return os.geteuid() == 0


def bpffs_mounted() -> bool:
    try:
        with open("/proc/mounts", encoding="utf-8") as fh:
            for line in fh:
                parts = line.split()
                if len(parts) >= 3 and parts[1] == "/sys/fs/bpf" and parts[2] == "bpf":
                    return True
    except OSError:
        return False
    return False


def btf_present() -> bool:
    return os.path.exists("/sys/kernel/btf/vmlinux")


def lockdown_blocks_bpf() -> bool:
    """The confidentiality mode withdraws BPF entirely."""
    try:
        with open("/sys/kernel/security/lockdown", encoding="utf-8") as fh:
            content = fh.read()
    except OSError:
        return False
    return "[confidentiality]" in content


def asound_cards() -> list[str]:
    """The ALSA card ids present under /proc/asound (empty when none)."""
    cards: list[str] = []
    try:
        entries = os.listdir("/proc/asound")
    except OSError:
        return cards
    for entry in entries:
        if entry != "cards" and not entry.startswith("card") and entry != "timer":
            continue
        if re.fullmatch(r"card\d+", entry):
            cards.append(entry)
    return cards


def pcm_state(card: str = "card0", pcm: str = "pcm0p") -> str:
    """The PCM sub device state text ('RUNNING', 'OPEN', or '')."""
    try:
        with open(f"/proc/asound/{card}/{pcm}/sub0/status", encoding="utf-8") as fh:
            return fh.read().strip()
    except OSError:
        return ""


def user_in_group(user: str, group: str) -> bool:
    rc, out = run(["id", "-nG", user])
    if rc != 0:
        return False
    return group in out.split()


def can_read(path: str, user: str) -> bool:
    """Can `user` read `path`? (sudo -u test, the honest polarity)."""
    rc, _ = run(["sudo", "-u", user, "test", "-r", path], timeout=15)
    return rc == 0


def which(binary: str) -> str | None:
    return shutil.which(binary)
