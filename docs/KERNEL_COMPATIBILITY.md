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
