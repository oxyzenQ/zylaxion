<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# The unprivileged-BPF stance — what the kernel buried, what zylaxion rides

The owner's question, stated honestly: the world is closing
`kernel.unprivileged_bpf_disabled` shut — mainline builds the gate
closed by default, the enterprise distros have had it closed for
years, and the August 2025 oss-security advisory tells every distro
to keep it closed. Is zylaxion's IP invested in a surface the kernel
is burying? This record measures the claim against the mainline
source (fetched, not remembered), against the tree's own code, and
against the estate's existing witnesses — and lands the decision with
the follow-ups named. Measured at the `afb0af5` era, mainline master
as of 2026-11-18.

## The kernel facts, from the source

**What the sysctl actually gates changed under the estate's feet.**

- v5.15 (`kernel/bpf/syscall.c`, `__sys_bpf`): the gate sits at the
  DISPATCH level — `if (sysctl_unprivileged_bpf_disabled &&
  !bpf_capable()) return -EPERM;` before the command switch. At 1 or
  2, EVERY plain-user bpf() call is refused: lookup, update, every
  op on an existing pinned fd included.
- v6.1 (Debian 12's kernel): already narrowed — `capable =
  bpf_capable() || !sysctl_unprivileged_bpf_disabled;` feeds only the
  object-creation commands, with the intent comment on record: the
  sysctl blocks "key object creation commands"; "other actions depend
  of fd availability and access to bpffs".
- v6.5 through master: the structural shape — exactly two gate sites,
  `map_create` and `bpf_prog_load`. Element ops ride
  `map_get_sys_perms(map, f) & FMODE_CAN_READ / FMODE_CAN_WRITE`:
  the FILE-PERMISSION model, keyed on how the pinned bpffs file was
  opened, never on the sysctl and never on a capability.

**The transition law** (`bpf_unpriv_handler`, mainline):
`locked_state = unpriv_enable == 1` — once the knob reads 1, no
transition away from 1 is accepted. A 2 (which is what
`CONFIG_BPF_UNPRIV_DEFAULT_OFF=y`, default y in mainline `kernel/bpf/Kconfig`,
boots into) is ROOT-RE-OPENABLE: `sysctl -w kernel.unprivileged_bpf_disabled=0`
succeeds. The Kconfig help text says the same in prose. This matches
the live witness already recorded in `src/session/kernel.rs`
(night-dinner-3: Debian 13 / 6.12, the distro-default 2 flipped to 0
by the very next sysctl) — and it corrects the folklore: 1 is the
permanent lock, 2 is the soft default.

**The world closing — the owner's research, verified:**

- `CONFIG_BPF_UNPRIV_DEFAULT_OFF` defaults to y in mainline
  (kernel/bpf/Kconfig), so stock mainline boots at 2.
- SUSE: "Unprivileged BPF has been disabled by default to reduce
  attack surface" — the December 6, 2021 SLE security update
  (jsc#SLE-22573), all SUSE kernels since early December 2021.
- Ubuntu: "Disable unprivileged BPF by default" — Launchpad bug
  1961338, the jammy-era kernel config.
- RHEL: the enterprise kernel "prevents unprivileged users from
  being able to use eBPF" via the sysctl (stated in the
  CVE-2025-21851 record).
- The August 2, 2025 oss-security advisory ("Linux kernel: eBPF
  vulnerabilities"): "Distros and systems should make sure they set
  (or keep the default of) kernel.unprivileged_bpf_disabled at 1 or
  2", naming the speculative-execution side-channel class unprivileged
  BPF exposes on unmitigated hardware.
- The historical cause: the 2021 verifier LPE wave — CVE-2021-3490
  (ALU32 bounds tracking), CVE-2021-31440, CVE-2021-33200 (pointer
  arithmetic limits) — the pressure that moved the distros first and
  mainline's default second.

## What zylaxion rides, from the code

The attach is the ONE privileged surface, and the tree pins that law
in a test (`test/session/privilege_tests.rs`): non-root is refused
naming `CAP_BPF/CAP_PERFMON` with the `sudo zylaxion attach` tip and
the promise that "every other surface stays unprivileged". After the
attach, the pins are chowned to the desktop user (the hand-over
service), and every verb opens the pinned maps with
`MapData::from_pin` — a bpffs FILE open — and rides element ops on
that fd. The events ring is consumed through its mmap. At no point
does the product call `BPF_MAP_CREATE` or `BPF_PROG_LOAD` as a plain
user; there is no unprivileged object creation anywhere in the tree.

That is precisely the pattern the mainline gate preserves: object
CREATION is what the sysctl buries (it is the verifier- and
specex-exposed surface the 2021 wave and the 2025 advisory are about);
consumers of an already-created, already-pinned map are governed by
bpffs file permissions — a filesystem question, not a capability one.
The Kconfig's choice of 2 (re-openable) over 1 (permanent) is itself
the kernel keeping room for exactly this consumer pattern. BPF tokens
(6.8+) continue the same direction: delegated, grantable access —
not a reopening of the old unprivileged creation lane.

**Verdict on the strategic worry: no pivot is warranted.** The
deprecated surface is unprivileged CREATION, and zylaxion never
touches it; the eBPF-only bet rides the privileged-load /
unprivileged-consumer shape the kernel actively keeps working. The
"unprivileged" in zylaxion's promise was never "unprivileged BPF
loading" — it is "unprivileged VERBS", and that is the surviving side.

## The honest finding — the tree's own gate lags the kernel

The posture law (night-badas-4 round 5,
`src/session/kernel.rs::unpriv_map_write_refusal`, consumed by
`player.rs` and `status.rs`) refuses EVERY plain-user map op whenever
the sysctl reads 1 or 2. That law is EXACT for the 5.15-era dispatch
gate — the floor kernel genuinely refuses every plain-user bpf()
call. But it is over-conservative for 6.1+ kernels, where the kernel
itself allows pinned-fd element ops on chowned pins at 1 or 2. On a
modern distro booting at the default 2 (trixie, noble, Arch — the
mainstream), the product therefore refuses UX the kernel permits: the
"sudo once per boot, then everything is unprivileged" promise dies at
zylaxion's own gate, not at the kernel's.

The engine's existing probe (sm_audio.py, the unprivileged-bpf gate
row) tests zylaxion's refusal, not the kernel's raw answer — so the
tree had no witness for the split. Two smaller drifts rode along:
the doctor's "2 with a boot-parameter lock" arm modeled a kernel
shape the mainline handler does not carry, and the refusal law
modeled the dispatch era for every kernel.

**The follow-up, shipped as night-ebpf-era-1** (the boundary measured
before any edit): the source walk pinned the gate-scope flip at
**5.19** — v5.15 through v5.18 dispatch-gate every plain-user bpf()
call, v5.19 computes the gate for the creation commands only (the
kernel's own intent comment names the shape), v6.5 moves the two
checks into `map_create` and `bpf_prog_load` with the pinned-fd ops
on `map_get_sys_perms` — and the transition law proved UNIFORM
across the eras: the handler at 5.15, 5.18 and master carries the
same `locked_state = unpriv_enable == 1` (1 permanent, 2
root-writable to 0), so night-dinner-3's boot-param premise is
re-worded, not re-witnessed: the verdict now reports the planted
parameter and names the handler's law. The tree ships the era split
end to end: `narrowed_unpriv_gate()` keyed on the parsed release in
`src/session/kernel.rs`, the verbs' gate and the doctor's posture
line era-aware (`unpriv_map_ops_allowed` /
`unpriv_map_ops_refusal`), the README's limitation truthful per
era, and the witness instrument in the engine — a raw pinned-fd
open-plus-lookup as the plain user and a two-way sysctl flip with
restore (a 1 is never touched: its lock cannot be undone this
boot), asserting the kernel's measured answer against the era law
on every leg. The promised UX returns on every 5.19+ distro without
touching the buried surface; the 5.15 floor keeps the honest
limitation, and the E2E is the arbiter — a leg whose kernel
contradicts the law reds the battery instead of the operator
finding it.

## The decision

1. **Keep the bet.** The eBPF-only, attach-once, pinned-map design is
   on the side of the fence the kernel maintains. No architecture
   change, no daemon resurrection, no unprivileged-loading detour —
   the deprecated lane stays untouched by construction.
2. **The witness instrument** (the raw-op E2E probe above) shipped
   as night-ebpf-era-1 together with the era-aware law, the boundary
   measured from the source before any edit; the E2E legs arbitrate
   the law from here on.
3. **Keep the honest limitations truthful**: the README's
   unprivileged-bpf limitation names the era split now
   (night-ebpf-era-1) — the dispatch-era floor keeps its refusal,
   the 5.19+ world rides the file-permission model; the doctor's
   posture line is the single surface operators see, and it carries
   the same split.
4. **Never market around the closed door.** No claim that zylaxion
   "works without privileges where others cannot" — the design works
   WITH the kernel's model (privileged load once, file-permission
   consumers), which is the only story that survives the advisory
   era on an LTS project.

## Sources (opened for this record)

- Mainline source, fetched and grepped:
  kernel/bpf/syscall.c at v5.15, v6.1, v6.4, v6.5, v6.7, v6.8 and
  master (raw.githubusercontent.com/torvalds/linux);
  kernel/bpf/Kconfig at master (the `default y` of
  CONFIG_BPF_UNPRIV_DEFAULT_OFF and the transition prose);
  Documentation/admin-guide/kernel-parameters.txt at master.
- docs.kernel.org/admin-guide/sysctl/kernel.html (the knob's
  documentation page).
- openwall.com/lists/oss-security/2025/08/03/2 (the August 2025
  advisory and the "1 or 2" recommendation).
- lists.suse.com SLE security update December 6, 2021 and
  support.scc.suse.com KB (the SUSE default-off record).
- Launchpad bug 1961338 (the Ubuntu default-off record).
- NVD CVE-2021-3490, cve.org CVE-2021-31440,
  access.redhat.com CVE-2021-33200 (the 2021 verifier wave).
- The kernsec.org linux-security-module archive, the June 2023
  series that moved the checks into map_create()/bpf_prog_load().

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
