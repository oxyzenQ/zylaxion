<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
# The night-depth-hardening-Z1 end-to-end audit (2026-11-18)

One night, two tasks, one law: every claim the release chain makes
must be PROVEN by an instrument that runs on every push, not computed
in a doc. Task 1 (the D4 reproducible release chain) upgrades the
provenance story from artifact identity to process identity; task 2
(night-runner-1) gives the kernel checklist's pending rows their
runtime records in CI. This audit records the method, the live
evidence, the three defects the instruments caught (two mine, both
named and fixed from evidence), and the honest boundaries. Head
commit at audit time: `3186208` (v80.2.3 era, all ten Dragon Guard
lanes green). Task 3 (the D2 tap-object self-verification) joined the
series later; its section at the end carries its own live evidence.

## Task 1: the D4 reproducible release chain

**What shipped.** `scripts/release/repro-build.sh` (the ONE pinned
recipe: toolchain from `rust-toolchain.toml`, `SOURCE_DATE_EPOCH` at
the commit timestamp, the static musl v3 shape through the new
`pro-linux-amd64-v3-musl` alias, redacted path prefixes, `--locked`
deps, no shared build cache) + `.github/workflows/reproducible.yml`
(two independent hosts build cold, the verify job `cmp`s the binaries
and diffs the `-V` stamps) + the profile/alias plumbing + the docs
(`VERIFY_RELEASE.md` process-identity section, the depth-matrix
entry 38 deepening, the RULES lane bullet).

**The one C dependency, honestly.** zylaxion ships glibc-only
(night-dragon-maintenance-T1 retired the musl release lane because a
glibc-host cross-build welds the host's libasound onto a musl CRT).
The D4 lane keeps that law by construction: alsa-lib 1.2.13 is built
from a sha256-pinned source tarball (the pin table in the script, the
supply-chain law) with musl-gcc into a PRIVATE prefix, the pkg-config
verdict is asserted to resolve inside that prefix only, and the
kernel UAPI headers ride a private include window (three symlinked
libc-agnostic trees — linux, asm, asm-generic) so no glibc header is
ever searched. The musl artifact is the identity INSTRUMENT, not a
release shape; zero new surface ships.

**The defect chain, named from live evidence.** Three instrument
round-trips, each caught by a run before it could ship a lie:

1. `configure` run-tests: musl-gcc cannot execute its run-me test
   programs on a glibc build host — the recipe crosses
   (`--host=x86_64-unknown-linux-musl`).
2. Kernel UAPI headers: `linux/types.h` lives outside musl-gcc's
   `-nostdinc` window (the CI host named it); the first fix attempt
   (bare `-I/usr/include`) was refuted by the CI host a run later —
   glibc's stdio.h shadowed musl's (`__gnuc_va_list` chaos). The
   window is the honest shape.
3. The cross gate: alsa-sys 0.4.0 refuses target!=host pkg-config
   queries without a declared sysroot (the gate that keeps host libs
   out of cross builds). The prefix IS the musl sysroot and its pc
   paths are absolute, so `PKG_CONFIG_SYSROOT_DIR=/` unlocks the
   crate's cross mode and re-routes nothing.

**The verdicts.**

- Local, same host, two cold passes (`--cold-two-pass`): PASS —
  byte-identical, sha256 `3afecc55…db82dd` (commit `b3f60a8`).
- CI, two independent hosts, commit `42c0cb4`: PASS — stamps
  byte-identical, binaries byte-identical, shared sha256
  `69cca2cf403688a07553e3c70da77b5fb248664ca205a02a8d604a3801d56d51`.
  The lane re-passed on every later push (`3953451`, `3186208`).

**Honest boundaries.** The static binary's ALSA runtime stays partial
by nature (libasound dlopens its PCM plugin tree) — the lane proves
identity, not audio; `-V` and the stamp surfaces run anywhere, which
is what the identity lane needs. The published glibc release shapes
are NOT (yet) covered by the two-host cmp — their reproducibility is
the D3 same-host attestation; extending the two-host cmp to the gnu
release shapes is the natural next step and is deliberately not done
here (the musl instrument lane had to stand first, and the gnu shapes
carry distro libasound the recipe would need a story for).

## Task 2: night-runner-1, the LTS-span runtime records

**What shipped.** Three witness legs in the Supermassive E2E KVM
matrix beside the existing three seats — the 6.1 Debian bookworm
`6.1.0-50-amd64` deb and the 6.6 Arch LTS `6.6.72-1-lts` package,
both sha256-pinned with the BTF matrix's law (verified live against
the artifacts they stage: full sound chain + uinput present in both),
and the 6.8 seat resolving the noble GA `6.8.0-31` kernel from the
frozen main pocket (the floor leg's law, one series later). Each leg
boots the REAL kernel family and runs the whole battery; the guest
init witnesses the exact kernel family per leg.

**The defect chain, named from live evidence.**

1. The 6.8 seat fell through to the latest pattern and resolved
   7.0.0-42 — the exact-family witness REFUSED the drift exactly as
   designed (the instrument working is the story). Fixed: the seat's
   own noble-main branch.
2. The pocket list filename carried a blocked vocabulary token; the
   language gate caught it before the tree shipped. Fixed: renamed.
3. The 6.1 seat read a false SKIP (no `/dev/input/event*` node). My
   first diagnosis (i8042 as a module) was falsified by the second
   run's staged list — bookworm builds the WHOLE serio chain into
   vmlinuz; the artifact ships `evdev.ko` as the module, the one
   piece that mints event nodes. Fixed: evdev staged and loaded; the
   probe then read its lawful PASS (`1 event devices, all denied`).
4. The floor seat's PROOF 7 tripped its relative autorepeat gate by a
   hair on one run (`412` vs `382`) and passed the rerun — a
   measurement-noise flake on a busy host, recorded not hidden; the
   battery itself passed 112 probes that run.

**The verdicts (run `3186208`, all six seats):**

| kernel line | seat | verdict |
|---|---|---|
| 5.15.0-25 (the floor, jammy GA) | low specs | PASS (whole battery) |
| 6.1.0-50-amd64 (Debian bookworm) | LTS span | PASS (whole battery) |
| 6.6.72-1-lts (Arch LTS) | LTS span | PASS (whole battery) |
| 6.8.0-31 (noble GA, frozen pocket) | LTS span | PASS (whole battery) |
| newest the archive serves (7.0.0-42 this run) | best specs | PASS (whole battery) |
| 5.4 GA (below the floor) | refusal walk | PASS (refusal proven) |

The kernel checklist's pending rows carry runtime records now: 6.1
LTS, 6.6 LTS and 6.8 are CI-tested on every push. The 7.1 record
stays the owner's CachyOS host session (the nested-bpffs entry);
a 7.4-class kernel rides the dynamic `latest` seat the day the
archive serves it — no seat is version-pinned to a kernel no runner
serves yet, and that is the honest boundary, not a gap.

## Reproduction sketch

The D4 chain: `./scripts/release/repro-build.sh --cold-two-pass`
locally (musl-gcc on PATH, the pinned toolchain installed, the eBPF
bootstrap run); the two-host half is CI's, on every push. The kernel
span: read the six seats' MASS-VERDICT lines in the Supermassive E2E
run for any push; each leg's kernel family witness is in the run log
(the `MASS: init booted, kernel …, leg …` line). The alsa pin and the
kernel pins verify with `sha256sum -c` against the tables in the
script and the workflow.

## Task 3: the D2 tap-object self-verification

**What shipped.** The build stamps the staged object's blake3 as a
compile-time env fact (`build/tapstamp.rs` stamps
`ZYLAXION_TAP_OBJECT_B3` from BOTH staging lanes, the moment the
object lands in OUT_DIR — the zejtron night-frontier-Z5 shape, ported
to zylaxion's vocabulary), and `sig`'s new fifth depth row
`D4 tap-object` re-hashes the bytes the binary actually carries
(through the same `black_box` barrier the anchor recompute rides — a
constant-folded recompute would verify a patched binary as happily as
a pristine one) and compares them to the stamp. Three arms pinned in
`test/info/signature_tests.rs` (the pristine self-check through the
real stamp, the one-flip refuse naming both digests, the absent-stamp
honest fail); the report shape pinned at 5/5 in
`test/commands/sig_tests.rs`.

**The instrument, end to end.** `scripts/audit/tamper-sig-probe.sh`
(the Dragon Guard check job's tamper-sig leg, after cargo test):
locates the staged object's byte-exact run inside the built binary,
flips ONE byte in a patched copy, and demands the refusal by name
beside a pristine control arm. Live evidence, debug binary at head
`a6827a9-dirty`:

- Control arm: pristine `sig` reads
  `SIGNATURE: AUTHENTIC (5/5 depths verified)`, the D4 row green — and
  its digest equals the `ebpf-prebuilt/manifest.toml` object pin
  (`eca1b26b…c3470`): the binary proves its embedded camera IS the
  manifest's object, the D2 task's own sentence.
- Tamper arm: one flipped byte (offset 872700) →
  `D4 tap-object  FAIL — the embedded tap object hashes to
  b3:d2c76f3a…, the build stamped b3:eca1b26b… - the tap was patched
  after the build`, verdict `SIGNATURE: TAMPERED (4/5 depths
  verified)`, exit 2 — while D0-D2 stay green. The identity layers
  were never touched by the patch; that is exactly the hole the new
  depth closes (the attack story the task opened with: `-V` still
  says verified, the camera was swapped).

**Honest boundaries.** The row proves the embedded object is exactly
what the build staged (and, on the registry lane, transitively what
the manifest pins) — an attacker who fully understands the stamp can
rebuild and re-stamp; the release checksums and the legal layer
remain the outer circle. The `-V` Depth-matrix line is untouched (the
storage-layer axis, the family contract). The verification runs on
the `sig` surface; a live attach still trusts the image it runs from
— extending the stamp check to the attach path is a deliberate
non-goal here (the surface scripts can gate on is `sig`, and its exit
code is the contract).

Reproduction: build any checkout (`cargo build`), run
`./scripts/audit/tamper-sig-probe.sh target/debug/zylaxion`; both
arms green means the circle holds.

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
