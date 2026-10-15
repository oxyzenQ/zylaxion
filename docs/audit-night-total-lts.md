<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# The night-total-lts Depth Audit — the skill and the infra (2026-10-15)

Two task IDs, one tree, five dimensions each, perstage, with the
peak-skip rule honored in writing: **night-total-lts-1** reads the
skill (the product itself — `src/`, `ebpf/`, the frozen Zactrix
Engine, the loader/teardown lifecycle), **night-total-lts-2** reads
all infra (`scripts/`, `.github/`, `build.rs`, `build/`, the gates,
the release lane, the docs law). Honest verdicts only: every skip
names why, every finding names its fix or its owner, and every
measurement names the sandbox that produced it.

## Method and sandbox honesty

The repo is MEDIUM weight at this tree (101 `.rs` files; `src/` ~11.8k
lines, `scripts/` ~8.4k, `.github/` ~2.1k, `ebpf/` ~0.4k), small
enough to read the security-critical files end to end instead of
sampling. The sandbox: non-root uid, no `sudo`, no ALSA headers
(`pkg-config --exists alsa` fails, and `libasound2-dev` cannot be
installed), no bpffs, no bpftool, no audio device. Consequences,
named up front:

- The main crate and the audit harness cannot compile here (cpal →
  alsa-sys needs the headers), so `build.sh check-all`,
  `cargo clippy -p zylaxion`, `cargo test`, and the six Zactrix
  proof wrappers could not be re-run locally. The substituted
  instruments: `cargo fmt --check` (green), the full
  `gate-keepers.sh` wholesale run (green), the LOC/layout gate
  (green), the commit-time prebuilt gate (green, live), and — for
  the one code fix — a scratch crate that compiles the REAL
  `src/session/orphan.rs` with its REAL test tree against the
  workspace's pinned `nix`/`log` versions and runs the suite
  (7/7 pass, including the live stub-reap leg). The full
  fmt/clippy/test battery is GitHub Actions' job on the pushed
  commits, where the ALSA lane exists.
- The stress dimension rides the recorded proofs plus a drift
  audit, not fresh runs (Dimension 1 below).

## Dimension 1 — Stability & Crash

**Surface.** The render path and its callers: panics, coredumps,
engine failure modes, the teardown lifecycle, the half-attach
states.

**Evidence.**

- **Engine drift, the LTS certification's standing.** The six
  proofs in `docs/ZACTRIX_LTS.md` (soak, rates, RT-safety,
  numerical stability, hot-reload, footprint) all PASS at
  `3dedb8ab`. This audit verified the claim still binds: `git diff`
  across `3dedb8ab → c028ae6f → HEAD` shows `src/engine/pool/`,
  `src/engine/voice/`, `src/profiles/` byte-identical at every step
  — the proofs' measurand never moved. The only engine-tree
  movement is the freeze note and its freeze-test wiring in
  `src/engine/mod.rs` (the re-stamp bookkeeping, now reconciled in
  the LTS doc), plus logging/doc lanes in `src/core/` and
  `src/session/` that no proof measures. Skipped, with the why: a
  fresh stress battery would re-measure a byte-identical DSP with
  instruments this sandbox cannot build — the drift check is the
  honest equivalent here.
- **Panic surface, end-to-end classified.** The audio callback
  (`src/audio/mod.rs`) and the per-sample path
  (`process_sample`, `render_sample`) carry zero
  unwrap/expect/panic sites. Every panic-capable expression in
  `src/` is either cold-path fail-fast on compile-time invariants
  (the registry `DEFAULT_PRESET` lookups, `MAX_POLYPHONY > 0`,
  thread spawns at startup), exhaustive-match guards
  (`unreachable!` after matched arms), or `#[cfg(test)]` support
  (the WAV writer, the CLI harness). No RT-hot-path panic exists.
- **Coredumps.** The 3.6X hardening's `force_no_coredump()`
  (RLIMIT_CORE 0/0, hard limit included) is present in the player
  bootstrap; the piped-`core_pattern` lane remains the operator's
  policy, as recorded.
- **Failure modes, the lifecycle seams.** The half-attach
  self-heal (partial pin set cleaned and replanted; foreign entries
  refused, never deleted), the stale-pid takeover ("cattle, not
  pet"), the ringbuf backpressure (a failed reservation drops the
  event, never blocks the input path), and the detach order
  (gate-clear → reap → unpin, link first) all read correct in
  source, and night-improve-26/27's own test pins back them.

**Finding: PASS** — no crash, no regression, no engine failure
found; one security-adjacent lifecycle gap surfaced and is recorded
under Dimension 4 (it does not crash anything; it points a kill).

## Dimension 2 — Code Hygiene (the dragon hunt)

**Surface.** Stale markers, zombie code, undocumented escapes,
orphaned scripts, duplication — perstage (security-critical dirs
first), not a full blind scan.

**Evidence.**

- **Stale markers:** zero TODO/FIXME/XXX/HACK in `src/`, `ebpf/src/`,
  and `scripts/` (the one grep hit is the word "ENOTEMPTY" in a doc
  comment; none are real).
- **Orphaned scripts:** every `.sh` outside `gates/` and `audit/`
  is referenced from another script, a workflow, `build.rs`, `src/`,
  README, or docs — zero orphans.
- **The allow() inventory:** all 21 `#[allow(dead_code)]` /
  `#[allow(unused_imports)]` instances across 12 files carry a
  written reason (test-only helpers, cross-crate `#[path]`
  contracts, the zero-warning gate's honest scopes); two were
  spot-verified against their claimed consumers and hold. No
  zombie code hides under an unexplained allow. The newer files use
  `#[expect(...)]` where the toolchain supports it — the tree is
  migrating in the right direction.
- **The gates agree:** check-loc green (101 files under the 600
  cap, entry-file and Pattern-C layout laws hold), name-case green
  (967 tokens), headers/language/emoji green via gate-keepers.

**Finding: PEAK — skipped.** The dragons are not here: no
spaghetti, no duplicate logic, no stale or zombie code found in the
stages scanned. The repo's own gate system polices the classes this
dimension hunts, continuously, which is why the hunt comes back
empty.

## Dimension 3 — Optimization

**Surface.** Hot paths (the render path, the audio callback, the
eBPF tap) and the infra scripts.

**Evidence.** The hot paths are measured, not guessed: PROOF 3's
jitter (max/p50 2.60x natural, 3.58x saturated, worst block 2.23 ms
against the 5 ms target), PROOF 6's footprint (flat RSS, 1.075%/0.642%
CPU by the locked formula, cold start under 0.5 ms to first sample),
and the tap's off-state cost (one branch — the enabled-slot gate is
the first check after the two event filters). The engine is frozen
under the three-path admission bar, and no measured regression or
unmeasured claim exists to answer.

**Finding: PEAK — skipped.** For the engine, the freeze bar itself
is the reason: a change needs a measured improvement proven by the
audit harness, and no profile here names a deficit. The infra
scripts run seconds, are not hot, and carry no repeated-work
pattern worth a diff. Micro-optimizing either would violate the
no-over-engineering rule the tree itself states.

## Dimension 4 — Security Hardening (white-hat, kernel-hacker mode)

**Surface.** The eBPF program, the loader FFI, the pin lifecycle,
the privilege boundaries, the scancode law, CI, and the one network
call.

**Evidence.**

- **The BPF program (`ebpf/src/main.rs`), read end to end:** every
  map access bounds-checked by construction (`Array::get` →
  `Option`; a keycode past the table is uncounted, never a fault),
  no loops, no attacker-controlled pointer arithmetic, the `dev`
  pointer extracted per ABI and never dereferenced (the passive-tap
  law), stats ride BPF_ATOMIC fetch-add, a full ringbuf drops
  silently, the failure path returns 0 (never fails the input
  path), and the GPL section keeps the verifier honest. The
  argument shim is single-sourced with the userspace tests.
- **The loader FFI (`src/ebpf/bpf_syscall.rs`), read end to end:**
  every attribute struct is `#[repr(C)]` sized to a kernel-accepted
  head (no E2BIG overstatement), `CString` lifetimes cover each
  syscall, every error path closes every fd (no leaks), pin paths
  are compile-time constants (no injection surface), and the
  `cpu 0` kprobe handle is correct — the BPF program binds at the
  trace_event_call level and fires on all CPUs, exactly as the
  code's comment claims.
- **The scancode law:** the RULES.md verification grep returns zero
  matches in production paths; the only keycode-naming render is
  the disclosed `status --verbose` aggregate, as the security
  record already pins.
- **CI:** all third-party actions SHA-pinned, least-privilege
  `permissions:` per workflow (write only where a job needs it),
  the gate-keepers lane runs unfiltered on every push, Dependabot
  absent per policy. `--check-update` remains one constant-URL curl
  GET, root-refused before any I/O.
- **FINDING, fixed in night-total-lts-1 (commit `24b6c73`):** the
  detach-side player reaper trusted the pid in the control map —
  writable by the pin-owning user — and drove the SIGTERM/SIGKILL
  ladder at it with no identity check, while the attach-side sweep
  validated its pids with the exact cmdline matcher. A planted
  `PLAYER_PID` slot turned `sudo zylaxion detach` into a
  root-mediated kill aimed by map data at an arbitrary process. The
  fix (`orphan::pid_is_our_player`, the identity gate before any
  signal) and its honest residual (the microseconds-wide
  validate-then-kill pid-recycling window, refusal on the safe
  side) are recorded in the security doc's Dimension 3 addendum.
  The fix touches lifecycle code only — the frozen DSP is
  byte-identical through the commit, the freeze bar's security-fix
  path does not apply to it, and the admission is (a): a
  vulnerability fix.

**Finding: PASS** with the finding closed in the same audit; the
two standing INCONCLUSIVE residuals carry forward with their named
measurements (the bpffs pin modes and the loaded-program hash —
both need a real root host).

## Dimension 5 — LTS Stability

**Surface.** The long-term failure modes: toolchain drift, doc
rot, bookkeeping inconsistency, the self-enforcement chain.

**Evidence.**

- **The freeze chain, reconciled.** The LTS doc pinned `3dedb8ab`,
  the engine module doc and the CHANGELOG pinned `c028ae6f` — the
  two certification pins disagreed on paper, the exact rot class
  that misleads the next maintainer. The LTS doc now carries the
  re-stamp with the drift audit that verified it (Dimension 1).
- **Stale doc claims, fixed in night-total-lts-2.** RULES.md
  claimed the tool writes `$XDG_RUNTIME_DIR/zylaxion.pid` and a
  `.sock` — both died with the daemon (night-improve-23); the
  security record's own "Disk: nothing is ever written" line was
  already correct. RULES.md also pointed at a nonexistent
  `docs/trademark.md` (the file is `TRADEMARK.md` at the root), and
  the GLOSSARY still defined the IPC socket in present tense. All
  three corrected; the doctor's XDG_RUNTIME_DIR check gained the
  intent comment its absence invited (it checks the audio-server
  session posture, not zylaxion files — zylaxion writes none).
- **The self-enforcement chain holds:** the commit gate verified
  live during both commits of this audit (prebuilt parity + pairing
  fail-closed), the gates self-install `core.hooksPath` on clone,
  the toolchains are pinned (`rust-toolchain.toml` nightly
  2026-09-18 for the tap; stable for the workspace), and the
  release lane renders in the reference format end to end
  (night-improve-30).

**Finding: PASS.** The hidden failure mode this dimension hunts is
quiet divergence between what the docs claim and what the tree
does — three instances found, three fixed, and the remaining
inconclusives are measurement gaps a real host closes, not rot.

## The fix accounting

- **night-total-lts-1 (`24b6c73`)**: `src/session/orphan.rs` (the
  pure identity gate + docs), `src/ebpf/teardown.rs` (the gate in
  `reap_player_from_map`, after the gate-clear, before the ladder),
  `test/session/orphan_tests.rs` (eleven-assertion pin), the LTS
  doc's reconciliation section, the security doc's Dimension 3
  addendum, the CHANGELOG Security entry. Verified: fmt green,
  scratch-crate suite 7/7 (real module, pinned deps, live stub-reap
  leg), gate-keepers wholesale green, LOC gate green, commit gate
  green live.
- **night-total-lts-2 (this commit)**: RULES.md's disk-inventory and
  trademark rows, the GLOSSARY socket entry, the doctor intent
  comment, this audit report, the CHANGELOG Changed entry. No
  behavior change anywhere — the docs now state what the tree
  already did.
- **Benchmark, skipped with the why:** no render-path byte changed
  (the drift audit proves it across the whole span, including both
  commits), the changed code executes only inside a detach, and the
  host cannot build the instrument — the Zactrix metrics have
  nothing here to measure.
- **Version:** untouched, owner-decided, as always.

## The carried residuals (the next audit's worklist)

1. The bpffs pin-mode measurement (Dimension 3 of the security
   record, INCONCLUSIVE since 3.6): one command on a real root host
   — `stat /sys/fs/bpf/zylaxion/*` plus a second-user dump probe.
2. The loaded-program hash confirmation (needs root + bpftool).
3. The full fmt/clippy/test/cargo-audit battery and the six proof
   wrappers on an ALSA-capable host — CI runs the first on every
   push; the six wrappers are the owner's release ritual.

---

# The second pass — night-total-lts-3 (the skill) and -4 (the infra), 2026-10-16

The same pair of surfaces one day later, deeper, with two
instruments the first pass did not have and one day of fresh tree
that no pass had seen:

- **The sandbox can now build and run the whole battery locally.**
  The staged ALSA userspace (`libasound2t64` + `libasound2-dev`
  1.2.14 extracted from the Debian pool into `~/.local/lib`,
  reached through `PKG_CONFIG_PATH` + `LD_LIBRARY_PATH` +
  `RUSTFLAGS="-L native=..."`) closes the alsa-sys gap this file
  recorded as a hard sandbox limit on 2026-10-15, and `bpf-linker`
  rides the same PATH. The full `cargo fmt` / `cargo clippy
  --all-targets --all-features -- -D warnings` / `cargo test`
  battery is now a LOCAL instrument, not a CI-only one — the first
  audit in this series able to say so.
- **shellcheck 0.11.0** (static binary, no root needed) sweeps
  every repo shell script at severity >= warning.
- **The fresh tree**: `8aa0b7e..3fe6648` (night-dinner-3 through
  -6b; 39 files, ~1.6k insertions) had never been under an audit
  pass — the nd3 privilege/verdict surfaces (the o+x relax, the
  reach probe, the sudo-user identity), the nd4 auto-refresh lane,
  the nd5 VM journey and its guest init, and the nd6/6b Miri
  reshape.

## Dimension 1 — Stability & Crash (second pass)

**Evidence.** The full local battery is green: 224 tests, 0 failed
(including the eleven new nd3 kernel/pin pins and the new
classification law pin), fmt clean, clippy `-D warnings` clean
across `--all-targets --all-features` — the residual item 3 from
the first pass, closed to the extent a non-ALSA-device host can
close it. The stress dimension ran LIVE this time, against the real
binary: 18 hostile argument shapes (traversal, command
substitution, backticks, unicode, empty, unknown verbs), a 100 KB
env value, a traversal-shaped env, an injection-shaped env, and the
broken-pipe law (`-V | head -1`) — zero panics, zero rc 101, the
pipe contract holds. The fresh nd3 surface reads correct end to
end: the o+x relax adds exactly one bit (setuid/sticky preserved,
other-read never granted, root-only path, undo logged), the cmdline
lock probe is a whole-token match, and the mode triad follows the
kernel's no-fall-through order.

**Finding: PASS** — no crash, no regression; the battery is now
stronger than the first pass could measure.

## Dimension 2 — Code Hygiene (second pass)

**Evidence.** The fresh diff adds ZERO `unsafe` bytes. The
allow-inventory shape is unchanged. Shellcheck across the repo
scripts found exactly three warning-class items, all fixed in
night-total-lts-4: the wholesale gate's `cd` without a fail-fast
(SC2164 — a failed cd would have left every gate policing the wrong
tree silently), a dead `EDIT_KEYS` variable in the actions-pin gate
(SC2034), and the parity gate's single-element loop without a
written reason (SC2043 — deliberate shape, now carries the
directive and the why). No stale markers, no orphaned scripts, no
new duplication in the fresh surface.

**Finding: PASS** — three dragons found, three closed, all in gate
infra (the perstage priority surface).

## Dimension 3 — Optimization (second pass)

**Finding: PEAK — skipped.** The engine remains frozen with no
profile naming a deficit. The infra scripts run seconds, not hot.
The one new steady-state cost (the build-time `--check` verdict
subprocess, night-dinner-4) was measured incidentally: a warm full
build finishes in ~2.2 s with it in the path — nothing to answer.

## Dimension 4 — Security Hardening (second pass)

**Evidence.** The fresh privilege surfaces, read end to end:
`reach_identity` trusts `SUDO_UID`/`SUDO_GID` only inside an
euid-root session, where the environment is root's own or sudo's —
a non-root caller cannot reach that branch, so the spoof shape has
no door; the o+x relax grants walk, never list, and the pins stay
0600 under their owner; the nd5 python harness carries no
`shell=True`/`os.system`/`eval` anywhere and its root guard is
intact; every workflow's `run:` blocks are free of
`${{ github.event.* }}` interpolation (no script-injection class);
the release lane's checksum loop is pinned (b3sum@1.8.7) and
quoted; the guest init's privileged ops are scoped to `/dev/snd/*`
inside the throwaway VM. `mode_grants_x` reads only the primary-gid
triad — a supplementary-group grant would read as blocked: the
fail-safe direction (never a false "walkable"), recorded as a
known simplification.

**FINDING, fixed in night-total-lts-3 (this commit):** the
night-dinner-3 classifier folded EVERY unknown errno to Absent —
`EIO` on a dying bpffs mount or `ELOOP` on a tampered path read as
"not attached" (`status`) and "sound is off — nothing to stop"
(`off`) while a player kept playing: the exact lie family nd3
closed for EACCES, resurrected for the rare errnos. The fix is the
`classify_pin_stat` law — only NotFound earns the absent verdict,
only PermissionDenied earns the walk-block verdict, every other
errno defers to the syscall that follows (the classified open,
which reports its own errno verbatim); `status` gained the matching
third arm ("cannot be seen"), `off` now routes the unknown world to
the honest hard error. Pinned by `pin_stat_classification_law`.

**Finding: PASS** with the finding closed in the same audit.

## Dimension 5 — LTS Stability (second pass)

**Evidence.** The self-enforcement chain was exercised live during
this pass: the wholesale gate-keepers run (green, on the fixed
`cd`), the commit gate (prebuilt parity, live), the full local
battery (green). The supply-chain lane stays where the policy put
it: `cargo audit --deny warnings` runs in CI on every manifest push
and weekly — a local run here would scan a stale advisory DB, so
the honest verdict is CI-owned (peak by delegation). The VM battery
has live-measured pin OWNERSHIP since night-dinner-5; the exact
mode bits remain the root-host one-liner below.

**Finding: PASS.**

## The fix accounting (second pass)

- **night-total-lts-3**: `src/commands/sound/mod.rs` (the
  `classify_pin_stat` law + the enum's Reachable contract), 
  `src/commands/sound/status.rs` (the third arm), 
  `test/commands/sound_tests.rs` (the law pin), the CHANGELOG
  Fixed entry. Verified: fmt green, full local suite 224/224
  (1.2 s native — the classification is a decision surface, the
  wiring mirrors shapes the pins already hold), clippy `-D
  warnings` green.
- **night-total-lts-4**: `scripts/gate-keepers.sh` (fail-fast cd),
  `scripts/gates/check-actions-pins.sh` (dead variable removed),
  `scripts/gates/check-prebuilt-parity.sh` (directive + reason),
  this report, the CHANGELOG Changed entry. Verified: shellcheck
  clean on all three, tab bytes unchanged in the tab-formatted
  gate (109/109), wholesale gate-keepers green live.
- **Benchmark, skipped with the why:** zero engine/render-path
  bytes changed across both commits; the changed runtime code
  executes only on error paths (the stat classification), and the
  native suite's wall time is unchanged at 1.2 s.
- **Version:** untouched, owner-decided, as always.

## The carried residuals (unchanged + one note)

1. The bpffs pin-MODE bits (0600) on a real root host — pin
   OWNERSHIP is now live-measured by the VM battery on every
   Supermassive run (night-dinner-5), narrowing this residual to
   the mode render only.
2. The loaded-program hash confirmation (needs root + bpftool).
3. The six Zactrix proof wrappers on an audio-capable host — the
   owner's release ritual; the fmt/clippy/test half of this
   residual is now closed locally (Dimension 1).
