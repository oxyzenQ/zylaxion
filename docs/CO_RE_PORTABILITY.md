<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# zylaxion CO-RE Portability: the compile-once verdict

The frontier question, as posed: the eBPF object is compiled for a
specific kernel target, and BPF CO-RE (Compile Once — Run Everywhere)
would let one object run across kernel versions without a recompile —
a game-changer for binary distribution, at the price of migrating to a
newer aya. This page records the forensic answer, layer by layer, and
the one property that would change it.

## The verdict

The premise is inverted for this codebase: the shipped tap already IS
a compile-once object. One 2960-byte `zylaxion-tap.o` is built by the
prebuilt lane, embedded into the binary, and loads on every supported
kernel without a per-kernel recompile — the property CO-RE is famous
for is already the shipping posture. What the tap does not use is
CO-RE's specific machinery (.BTF.ext field relocations), and the
reason is structural, not accidental: the tap's kernel contract is a
FUNCTION ABI, not a struct layout, so there is nothing for field
relocations to do. The rest of this page carries the evidence, and
the exact recipe for the day the premise changes.

## The evidence, layer by layer

**Binary.** The shipped object carries zero BTF sections: `readelf -S
ebpf-prebuilt/zylaxion-tap.o` lists `.text` and `.rel.text` (plus
standard furniture) and no `.BTF` / `.BTF.ext`. No BTF.ext means no
type-relocation records — the object holds no compiled-in assumptions
about any kernel's type layout to relocate. The byte-identical object
sails into every host via the registry lane and the parity gate
(blake3 + tree pin) vouches it never drifts from the sources.

**Source.** The tap never dereferences a kernel struct — the single
thing CO-RE relocations exist to make safe. The argument shim
(`src/ebpf/args.rs`, single-sourced into the ebpf crate via `#[path]`)
extracts the four arguments of the exported `input_event()` function:
the fentry lane reads the BTF trampoline's typed slots, the kprobe
lane reads `PT_REGS_PARM1..PARM4`, and both funnel into one tested
body. The `struct input_dev *` arrives as argument 0 and is recorded
but never dereferenced — the passive-tap law. Reading arguments
instead of struct fields is what makes the relocation surface empty.

**Load.** The one genuinely kernel-specific fact the tap needs — the
`attach_btf_id` of the function it fentry-attaches to — is resolved at
LOAD time from the target kernel's own BTF
(`Btf::from_sys_fs()` feeding `prog.load(..., &btf)` in the attach
chain, `src/ebpf/loader/chain.rs`). Nothing about the target kernel is
baked in at compile time; the loader adapts to whatever kernel the
binary lands on, which is the run-everywhere half of CO-RE, delivered
by aya's loader instead of by object-file relocations.

**Empirical.** The Supermassive E2E lane boots the same binary (same
embedded object) inside KVM micro-VMs across the whole span on every
push: the floor leg on the real jammy GA release-day 5.15.0 kernel,
the best-specs leg on the newest kernel the archive serves, and the
below-floor leg on the focal 5.4 kernel — which must REFUSE
honestly. The refusal leg is the sharper half of the claim: the
boundary is enforced by the doctor/attach/status walk naming the 5.15
minimum, not hoped for. Compile-once-run-everywhere is therefore not a
paper property here; it is re-proven live per push, within the
documented 5.15 floor.

**Variance absorption.** Kernel symbol drift has its own seat: the
attach chain steps fentry:input_event → kprobe:input_event →
kprobe:input_handle_event, and the TAP_KIND control slot records which
step owns the live tap. A fentry attach needs the target's BTF at load
time; a kprobe does not. The chain descending from typed trampoline to
raw probe is how one object survives kernels that differ in exactly
the surfaces this tap touches.

## Why CO-RE is the wrong name today (and when it becomes right)

CO-RE's field relocations solve one problem: internal kernel structs
(`task_struct`, `input_dev`, ...) change layout between versions, so
compiled-in field offsets rot. The tap reads none of them — its whole
kernel interface is four register-width function arguments whose ABI
the shim pins and the tests freeze. That is why the object ships
without BTF sections and still ports: an empty relocation surface
needs no relocation engine. The aya 0.16 stack (night-enhance-1) is
already in place with its full aya-obj relocation support, so the
precondition the frontier question worried about is retired.

The day the tap wants to read an INTERNAL kernel struct — in-kernel
pid verification against `task_struct`, per-device state from
`input_dev` — the compile-once property continues, but through CO-RE
properly so: build with `RUSTFLAGS='-Cdebuginfo=2
-Clink-arg=--btf'` (the recipe already documented in
`ebpf/.cargo/config.toml`), and aya-obj applies the target kernel's
field relocations at load time. That flip is a conscious event, not a
silent one: the prebuilt parity gate carries a BTF-posture row that
fails the moment BTF sections appear in the shipped object, so the
compile-once claim and the object cannot drift apart — the gate,
this page, and the recipe move together, in one commit.

## The gate

`scripts/gates/check-prebuilt-parity.sh` pins the posture with one row
per object: the shipped ELF must carry no `.BTF`/`.BTF.ext` sections.
A future build that enables the BTF recipe trips the row loudly, with
this page named — exactly the estate's shape of choosing the frontier
deliberately instead of drifting into it.

<!--
  This page is a point-in-time forensic record, not a promise of
  future behavior. Source code (`src/**/*.rs`, `ebpf/src/**/*.rs`)
  is the single source of truth. Always cross-check against the
  actual source files before relying on any specific number (object
  size, section list, attach-chain steps), file path, or command.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->

## The kernel matrix, measured (night-frontier-2B)

The "no BTF.ext on the shipped object" verdict above is now measured
on every push, against real kernel BTF: the kernel matrix
(`scripts/audit/zactrix-kernel-matrix.sh` + the harness's
`--btf-matrix` mode) extracts the embedded `.BTF` from sha256-pinned
distro kernel images of the three LTS lines (Ubuntu jammy 5.15,
Debian bookworm 6.1, Arch LTS 6.6), parses the shipped
`zylaxion-tap.o` through the loader's own aya-obj path, and asserts,
per snapshot: `has_btf_relocations() == false` (the posture this
page documents, verified on the real object), the relocation chain
clean (`relocate_btf`, `relocate_externs`, `relocate_calls`), and
the fentry target `input_event` resolving to a BTF `Func` id — the
exact lookup `FEntry::load` performs. 3/3 PASS measured; the
map-fd patching stays a live-kernel fact (the snapshot cannot mint
fds), recorded as the map inventory instead. The day the tap grows
real field relocations, this matrix becomes the gate that says so —
the instrument already rides every push.

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
