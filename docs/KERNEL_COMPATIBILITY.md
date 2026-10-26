<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion Kernel & Distro Compatibility

The kernel floor the tap requires and the per-distro layouts the tap
actually meets, each entry carrying its evidence. A resolved entry is
RESOLVED — not worked around: the fix lives in the code and the entry
records why, so nobody re-litigates it as a zylaxion bug later.

## The kernel floor

zylaxion requires Linux 5.15 or newer. The attach chain probes the
kernel's own surfaces at attach time (fentry when the kernel carries
BTF for the target function, kprobe otherwise) and refuses honestly
when nothing probes — `zylaxion doctor` names the verdict either way.

The floor is not a paper number: the Supermassive CI's e2e-vm job
crosses the WHOLE span live, one leg per direction (the zejtron
kernel-witnesses lineage). The floor leg boots the REAL minimum — the
jammy GA release-day 5.15.0 kernel, resolved from the jammy main
pocket only (frozen by archive policy; the weekly jammy-updates ABI
bumps are deliberately not configured, a newest-5.15 is the floor
family, not the floor) — inside the KVM micro-VM, on the low envelope
(a quarter of the runner's cores, an eighth of its RAM), and runs the
whole E2E battery on it: the tap lifecycle, the real-user chain, the
realtime storm. The at-floor direction of the compatibility verdict
is proven live on every push, not computed; the best-specs leg
crosses the other end of the span on the newest kernel the archive
serves; and the below-floor leg (night-dragon-maintenance-4) boots
the focal 5.4 GA kernel — BELOW the floor — to prove the refusal
direction live: doctor, attach, and on must all refuse on that
kernel, each naming the 5.15 minimum, while status still answers
honestly. Every leg's init also witnesses its own resolved kernel
family (5.15.* on the floor leg, sub-5.15 on the refusal leg), so a
resolver drift fails loudly instead of quietly testing the wrong
kernel.

## CachyOS: nested bpffs (bpf-on-bpf)

**Layout.** On CachyOS, `findmnt -t bpf` shows `/sys/fs/bpf` mounted
with a nested `/sys/fs/bpf` overmount — bpf-on-bpf. This is the
distro's own layout choice, not a zylaxion defect; the tap's pin
directory (`/sys/fs/bpf/zylaxion`) lives inside it like any other
bpffs consumer.

**Symptom.** The interaction made a half-state visible as a surprise:
a prior attach attempt that `mkdir`'d the pin directory and then failed
left the directory behind, and the NEXT first-ever `zylaxion attach`
failed with `EEXIST` at `/sys/fs/bpf/zylaxion` — the tap looked broken
on a host it had never touched. Before night-improve-26 the cleanup
unpinned files but never removed the parent directory, and the
attach-side self-heal only fired when a pin (not the bare directory)
was present — so the stale-EMPTY directory survived into the next
attach as the EEXIST.

**Fix.** night-improve-26's classified cleanup (the D3 fix), in code:

- `detach` unlinks every pin and the failure marker, then removes the
  pin directory itself through a classified rmdir: already-gone reads
  as success, foreign entries are left in place and named honestly
  (they are not ours to remove), anything unexpected is surfaced.
- `attach` cleans the stale directory before `mkdir` through the same
  classified path, so attach is idempotent from any prior state:
  fresh, stale-empty, half-attached, fully-attached. Foreign entries
  get the honest refusal naming them, never a silent deletion.
- The attach-failure marker the doctor reads back is deliberately not
  swept by the cleanup lanes: it is read, logged, and superseded —
  the evidence survives the cleanup.

**Status.** RESOLVED — in code, not worked around. No code branches
on the CachyOS layout: the cleanup is distro-agnostic, the layout only
made the pre-existing half-state visible.

**Provenance.** Observed live on CachyOS (kernel 6.18.50, the owner's
host): the first attach's EEXIST, then `detach` clearing the leftover
directory, then a clean attach — the evidence chain that produced this
entry. The attachment itself was proven live on that host: `bpftool
prog show` on the pinned id reports `prog_type tracing`, `attach_type
trace_fentry`, and the BTF id of `input_event` in that kernel.

## Debian 13: bpffs mounts mode 0700 (the front door)

**Layout.** Debian 13 (kernel 6.12.95+deb13) mounts bpffs at
`/sys/fs/bpf` with `mode=700` — `drwx-----T root root`, no
traversal for anyone but root. The pins attach plants are still
handed to the invoking user (`chown` to `$SUDO_USER`, files `0600`,
the pin directory `0755`), but the user cannot stat() THROUGH the
mount root, so every unprivileged verb's `bpf_obj_get` fails
`EACCES` before the kernel ever looks at the pins.

**Symptom.** The failure chain that surfaced it (live session,
v80.0.0-beta.1): `attach` reported "pins owned by 'zactrix' —
unprivileged surfaces can open them", `doctor` reported "All checks
passed — zylaxion is ready to run", and then `zylaxion on` refused
with "the tap is not attached (cannot open …/control): BPF_OBJ_GET
failed — tip: sudo zylaxion attach". All three verdicts were wrong
at once: the tap WAS attached, the pins WERE owned by the user, and
re-attaching fixed nothing — the mount root's missing `o+x` bit was
the whole story. `ls /sys/fs/bpf/zylaxion` as the user read
"Permission denied"; `namei -l` showed the `drwx-----T` root.

**Fix.** night-dinner-3, in code, from three directions:

- `attach` completes the hand-over it promises: after chowning the
  pins it stats the walk AS the sudo user and, when the mount denies
  traversal, adds the ONE missing bit — `o+x` on `/sys/fs/bpf`,
  never `o+r` (a user who knows a path may walk it; nobody gains
  listing; the pins stay `0600`). The relax is logged with its undo
  (`sudo chmod o-x /sys/fs/bpf`) and named in the attach summary.
- `doctor` gained the `bpffs reach` station: it evaluates the walk
  for the identity that will run the verbs (euid, or the sudo user
  under sudo) and fails with the exact fix while the walk is blocked
  — the "All checks passed" lie is gone.
- every "not attached" verdict (`on`, `off`, `status`, `preset`,
  `volume`) now stats the control-pin path first and separates a
  genuinely absent tap from a walk-blocked mount, each with its own
  honest message and fix.

**Status.** RESOLVED — in code, not worked around, and
distro-agnostic: the relax fires only when the walk is actually
blocked (a 0700 mount), is idempotent, and costs nothing on distros
that mount bpffs traversable. The bit resets on the next remount;
`attach` re-adds it on every attach. For a boot-persistent posture
without re-running attach first:
`echo 'z /sys/fs/bpf 0701 root root - -' | sudo tee /etc/tmpfiles.d/zylaxion-bpffs.conf`.

**Provenance.** The live Debian 13 session quoted above (zactrix
host, kernel 6.12.95+deb13-amd64, v80.0.0-beta.1 binary, build
97026cf): the manual `sudo chmod o+x /sys/fs/bpf` flipped the very
next `zylaxion on` to a working player (pid 2730), the preset swaps
ran live, and `sudo zylaxion detach` cleanly reaped the player
through the cleared gate — the full evidence chain for both this
entry and the attach-side fix.

<!-- ZYLAXION-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — perfect sync across every .md file is a
  known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of
  truth. Always cross-check against the actual source files before
  relying on any specific number (preset count, rate bound, LOC),
  file path, function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
