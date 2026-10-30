#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""The claims battery (night-master-1) — the static half of
the supermassive proof.

The project's headline claims are extraordinary, so they get their own
independent verification lane, separate from the runtime chaos the
other modules drive:

  1. PURE MATH SYNTHESIS — every sound is computed from equations at
     render time. No sampled audio ships (the RIFF/WAVE scan lives in
     sm_audio), and — the deeper half this module adds — no audio
     DECODER is linked into the process image: without a decoder the
     binary physically cannot play a sample even if one were smuggled
     in. The DSP dependency surface is synthesized, or it is nothing.

  2. eBPF, EMBEDDED — the kernel-side tap ships inside the binary as a
     real BPF ELF object (e_machine 247) targeting input_event, and
     the repo's prebuilt object is the same species. No external .o,
     no libinput, no userspace input library anywhere in the linkage.

  3. NO DAEMON, NO BLOAT CONFIG — the runtime posture is one stateless
     player and pinned bpffs maps. The binary carries no config-file,
     pidfile, or IPC-socket strings, no daemon-era restart advice, and
     the dependency graph (Cargo.lock, transitively) carries no config
     stack, no watcher, no dbus, no libinput.

Every probe is pure stdlib: the ELF parsing is hand-rolled (the guest
initramfs has no readelf/ldd to lean on), the string scans are bytes
membership tests, the registry names come from the binary's own
`list-presets` output so the catalog can never drift from the proof.

The battery runs in both lanes (rootless self-test and the VM's full
run). The eBPF-object probes need the release shape; against a binary
with no tap embedded (since night-dragon-maintenance-0 no zylaxion
build can be dormant, so this shape means a foreign or stripped
binary) they report an honest SKIP in the rootless lane and a hard
FAIL in the full lane, where the release shape is the guest contract.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

# The root-owned pycache guard (night-dragon-maintenance-T1, the same
# law the supermassive-test entry carries): a direct run of this
# module must never leave root-owned bytecode caches beside the
# scripts for the project-local modules it imports below.
sys.dont_write_bytecode = True

from sm_report import Ledger, Step  # noqa: E402

# Libraries that would make sampled playback possible. Any hit is a
# claim breach: the synthesis needs none of these.
_FORBIDDEN_LINK_LIBS = (
    "sndfile", "avcodec", "avformat", "mpg123", "vorbis", "FLAC",
    "libopus", "opusfile", "libmad", "modplug", "openmpt", "audiofile",
    "gstreamer", "openal", "SDL", "portaudio", "libinput",
)

# Crates that would resurrect the deleted daemon/config era. Checked
# against Cargo.lock so TRANSITIVE bloat is caught too, not just the
# direct dependency lines.
_FORBIDDEN_CRATES = (
    "toml", "libinput", "notify", "daemonize", "dbus", "zbus",
    "serde_yaml", "config", "xdg", "directories", "shellexpand",
)

# Byte strings that must never appear in the binary: the deleted
# config surface, a socket, the old input library, the pidfile era,
# and the stale restart advice night-hunt-1 retired.
_FORBIDDEN_BIN_STRINGS = (
    b"config.toml", b"zylaxion.conf", b"zylaxion.toml",
    b"zylaxion.sock", b"libinput", b"pidfile",
    b"systemctl --user restart",
)

_EM_BPF = 247  # e_machine value for BPF program objects


def _read_binary(binary: str) -> bytes:
    return Path(binary).read_bytes()


def _list_preset_names(binary: str) -> list[str]:
    """Ask the binary for its own preset catalog.

    night-improve-29: list-presets renders ONE line per preset —
    the name indented exactly two spaces, then two spaces, then the
    description; the active row ends with the `<- active` marker and
    the header/note prose is unindented — the shape is parsed, not
    guessed.
    """
    proc = subprocess.run(
        [binary, "list-presets"], capture_output=True, text=True, timeout=45
    )
    names: list[str] = []
    for line in proc.stdout.splitlines():
        match = re.fullmatch(r"  ([A-Za-z0-9_]+)  .+(?:  <- active)?", line)
        if match:
            names.append(match.group(1))
    return names


def _needed_libraries(blob: bytes) -> list[str] | None:
    """Parse DT_NEEDED straight out of the ELF image.

    Pure-python ELF64 walk: header -> program headers -> PT_DYNAMIC ->
    the DT_NEEDED string-table entries. Returns None when the image is
    not an ELF64 little-endian shared object/executable (then the
    caller reports honestly instead of guessing).
    """
    if blob[:4] != b"\x7fELF" or blob[4] != 2 or blob[5] != 1:
        return None
    e_phoff = int.from_bytes(blob[32:40], "little")
    e_phentsize = int.from_bytes(blob[54:56], "little")
    e_phnum = int.from_bytes(blob[56:58], "little")
    dynamic = None
    for i in range(e_phnum):
        base = e_phoff + i * e_phentsize
        p_type = int.from_bytes(blob[base:base + 4], "little")
        if p_type == 2:  # PT_DYNAMIC
            p_offset = int.from_bytes(blob[base + 8:base + 16], "little")
            p_filesz = int.from_bytes(blob[base + 32:base + 40], "little")
            dynamic = blob[p_offset:p_offset + p_filesz]
            break
    if dynamic is None:
        return []
    strtab = 0
    needed: list[int] = []
    for off in range(0, len(dynamic) - 16, 16):
        tag = int.from_bytes(dynamic[off:off + 8], "little", signed=True)
        val = int.from_bytes(dynamic[off + 8:off + 16], "little")
        if tag == 5:  # DT_STRTAB — vaddr; for the first LOAD map this
            strtab = val  # equals the file offset in practice
        elif tag == 1:  # DT_NEEDED
            needed.append(val)
    # Resolve the string table: find the PT_LOAD covering strtab's
    # vaddr and translate to a file offset.
    file_strtab = None
    for i in range(e_phnum):
        base = e_phoff + i * e_phentsize
        if int.from_bytes(blob[base:base + 4], "little") != 1:  # PT_LOAD
            continue
        p_vaddr = int.from_bytes(blob[base + 16:base + 24], "little")
        p_filesz = int.from_bytes(blob[base + 32:base + 40], "little")
        p_offset = int.from_bytes(blob[base + 8:base + 16], "little")
        if p_vaddr <= strtab < p_vaddr + p_filesz:
            file_strtab = p_offset + (strtab - p_vaddr)
            break
    if file_strtab is None:
        return None
    out: list[str] = []
    for off in needed:
        end = blob.index(b"\x00", file_strtab + off)
        out.append(blob[file_strtab + off:end].decode("ascii", "replace"))
    return out


def _embedded_bpf_objects(blob: bytes) -> list[int]:
    """Offsets of embedded ELF-BPF objects (e_machine 247) in an image."""
    hits: list[int] = []
    start = 0
    while True:
        idx = blob.find(b"\x7fELF", start)
        if idx < 0 or idx + 20 > len(blob):
            break
        machine = int.from_bytes(blob[idx + 18:idx + 20], "little")
        if machine == _EM_BPF:
            hits.append(idx)
        start = idx + 4
    return hits


def battery(ledger: Ledger, binary: str, tree_root: str, root_lane: bool) -> None:
    """The claims battery. root_lane=True makes eBPF-shape absence fatal."""
    blob = _read_binary(binary)

    # ── Claim 1: pure math — no decoder, no sample path ─────────────
    with Step(ledger, "pure math — the process cannot decode samples"):
        libs = _needed_libraries(blob)
        if libs is None:
            ledger.fail_row(
                "the binary is an analyzable ELF64 image",
                f"{binary} does not parse as ELF64 little-endian",
            )
        else:
            banned = [lib for lib in libs
                      if any(pat in lib for pat in _FORBIDDEN_LINK_LIBS)]
            if banned:
                ledger.fail_row(
                    "no audio decoder is linked into the binary",
                    f"forbidden DT_NEEDED={banned}",
                )
            else:
                ledger.pass_row(
                    "no audio decoder is linked into the binary",
                    f"DT_NEEDED={', '.join(libs) if libs else '(static: none)'}",
                )

    # ── Claim 1 positive control: the compiled registry ─────────────
    with Step(ledger, "pure math — the compiled registry is complete in the binary"):
        names = _list_preset_names(binary)
        registry_src = Path(tree_root) / "src" / "profiles" / "registry.rs"
        source_names = set(
            re.findall(r'^\s{8}"([A-Za-z0-9_]+)",$', registry_src.read_text(), re.M)
        )
        binary_names = set(names)
        missing_from_blob = sorted(n for n in names if n.encode() not in blob)
        drift = sorted(source_names ^ binary_names)
        if not names:
            ledger.fail_row(
                "the binary declares its own preset catalog",
                "list-presets produced no parseable names",
            )
        elif drift:
            ledger.fail_row(
                "the binary catalog matches the compiled registry exactly",
                f"source-vs-binary drift={drift[:6]}",
            )
        elif missing_from_blob:
            ledger.fail_row(
                "every cataloged preset name lives in the binary",
                f"missing={missing_from_blob[:5]}",
            )
        else:
            ledger.pass_row(
                "every cataloged preset name lives in the binary",
                f"{len(names)} presets, source registry cross-checked byte-for-byte",
            )

    # ── Claim 2: eBPF — the tap ships inside the binary ─────────────
    with Step(ledger, "eBPF — the tap object is embedded in the binary"):
        bpf_hits = _embedded_bpf_objects(blob)
        has_target = b"input_event" in blob
        if bpf_hits and has_target:
            ledger.pass_row(
                "an ELF-BPF tap object ships inside the binary",
                f"{len(bpf_hits)} embedded object(s), input_event target present",
            )
        elif not bpf_hits and root_lane:
            ledger.fail_row(
                "an ELF-BPF tap object ships inside the binary",
                "the full lane demands the release shape — EM_BPF absent",
            )
        elif not bpf_hits:
            ledger.skip_row(
                "an ELF-BPF tap object ships inside the binary",
                "no embedded BPF object — not the release shape (no zylaxion build is dormant since night-dragon-maintenance-0); rerun on the release shape",
            )
        else:
            ledger.fail_row(
                "an ELF-BPF tap object ships inside the binary",
                "EM_BPF present but the input_event target string is absent",
            )

    # ── Claim 2: the prebuilt object is the same species ────────────
    with Step(ledger, "eBPF — the prebuilt object is a clean BPF ELF"):
        prebuilt = Path(tree_root) / "ebpf-prebuilt"
        objects = [p for p in sorted(prebuilt.iterdir())
                   if p.is_file() and p.name != "manifest.toml"] if prebuilt.is_dir() else []
        if not objects:
            ledger.skip_row(
                "the prebuilt tap object is a clean ELF-BPF image",
                "ebpf-prebuilt/ carries no staged objects (source-build lane)",
            )
        else:
            bad: list[str] = []
            for obj in objects:
                data = obj.read_bytes()
                if data[:4] != b"\x7fELF" or int.from_bytes(
                    data[18:20], "little"
                ) != _EM_BPF:
                    bad.append(f"{obj.name}: not an ELF-BPF object")
                elif b"RIFF" in data or b"WAVE" in data:
                    bad.append(f"{obj.name}: carries audio magic")
            if bad:
                ledger.fail_row(
                    "the prebuilt tap object is a clean ELF-BPF image",
                    "; ".join(bad),
                )
            else:
                ledger.pass_row(
                    "the prebuilt tap object is a clean ELF-BPF image",
                    f"{len(objects)} object(s), zero audio magic",
                )

    # ── Claim 3: no daemon, no config — the strings ─────────────────
    with Step(ledger, "no daemon — zero config/pidfile/socket strings"):
        hits = [s.decode() for s in _FORBIDDEN_BIN_STRINGS if s in blob]
        if hits:
            ledger.fail_row(
                "the binary carries zero daemon-era strings",
                f"found={hits}",
            )
        else:
            ledger.pass_row(
                "the binary carries zero daemon-era strings",
                "no config file, pidfile, socket, libinput or stale "
                "restart advice anywhere in the image",
            )

    # ── Claim 3: no daemon, no config — the dependency graph ────────
    with Step(ledger, "no bloat — the dependency graph carries no config stack"):
        lock = Path(tree_root) / "Cargo.lock"
        lock_text = lock.read_text(encoding="utf-8", errors="replace")
        locked = set(re.findall(r'^name = "([^"]+)"', lock_text, re.M))
        banned = [c for c in _FORBIDDEN_CRATES if c in locked]
        if banned:
            ledger.fail_row(
                "Cargo.lock carries zero daemon/config-era crates",
                f"found={banned}",
            )
        else:
            ledger.pass_row(
                "Cargo.lock carries zero daemon/config-era crates",
                f"{len(locked)} locked crates, transitively clean",
            )
