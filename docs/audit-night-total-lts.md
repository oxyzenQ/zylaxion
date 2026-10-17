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

---

# The third pass — night-last-total-lts-5 (the skill) and -6 (the
# infra), 2026-10-17

The same pair of surfaces a third time, one day of fresh tree behind
them: `56a149e..5eac83c` (night-dinner-7 through -21 plus the
v80.0.0-rc.1/rc.2 release events; 23 commits, 37 files, ~2.4k
insertions) had never been under an audit pass. The milestone this
pass opened with is the host ledger's first fully green run — the
owner's fourth paste scored **PASS 23 / FAIL 0 / INCONCLUSIVE 0**
at `5eac83c` — recorded in the security doc's night-dinner-21
addendum; this report is the audit that walked the tree the ledger
measured.

**The sandbox reset, honestly.** This session's sandbox was wiped
between passes: no toolchain, no staged ALSA, no shellcheck. Every
instrument was rebuilt from the repo's own recipes before any
verdict was written — the pinned stable (`rust-toolchain.toml`'s
1.98.1, minimal + rustfmt + clippy), the staged ALSA userspace
(the Debian pool refresh: the exact `1.2.14-1` filenames the second
pass recorded are gone, the `1.2.14-1+deb13u1` security update
replaces them; pkg-config answers 1.2.14 either way), shellcheck
0.11.0, and — new this pass — the dated eBPF source-build lane
(`nightly-2026-09-18` + rust-src + the sha256-checksummed
bpf-linker via `scripts/dev/bootstrap-ebpf.sh`). The last one
closed a reading gap the earlier passes' wording glossed over: on a
git checkout the `ebpf` feature ALWAYS drives the nested nightly
build (build.rs refuses without the pin); the prebuilt lane is
registry-tarball-only. A clippy that never type-checked the loader
would have been a dishonest instrument for a pass that found its
first skill bug inside `cfg(feature = "ebpf")` code.

## Dimension 1 — Stability & Crash (third pass)

**Surface.** The fresh skill diff: the cpal 0.18 migration
(`src/audio/mod.rs`, the three `src/commands/info/` readers), the
night-dinner-18 stored-posture lane (`src/commands/sound/lifecycle.rs`,
`src/session/preset.rs`), the night-dinner-19 hand-over
(`src/ebpf/loader/chain.rs`, `orchestrate.rs`), the status third arm
(`src/commands/sound/{mod,status}.rs`), the test pins.

**Evidence.**

- **The engine freeze holds across the entire fresh range:**
  `git diff 3fe6648..5eac83c -- src/engine/ src/profiles/ ebpf/src/`
  is EMPTY — byte-identical. The six proofs' measurand never moved
  through rc.1, the dependency lane, and twenty-one night rounds.
- **The cpal 0.18 ride is mechanical, zero logic change:** the
  `SampleRate` tuple constructor → u32 type alias, `name()` →
  `description().name()` (same bare-name render, same fallback), the
  error callback's `StreamError` → `Error`, `&stream_config` → by-value
  — every hunk is the same decision with the new spelling.
- **The fresh diff adds zero `unsafe` bytes** (the one grep hit is
  this file's own second-pass prose) and zero stale markers.
- **The host ledger at this tree: 23/0/0** — R8's 25-press delta,
  R10's full effect-verified lifecycle (program 163 → 297, soak 48
  presses/10 s, posture restored), claim B held throughout.

**FINDING, fixed in night-last-total-lts-5 (this commit): the
zero-mute clobber.** The night-dinner-18 restore lane folded
`Ok(0)` into the default volume lane on the comment's premise that
"a zero volume slot reads as the fresh-attach shape (never a user
mute)". The premise was false twice over: the plant SEEDS the control
plane at attach (loader/chain.rs's init writes the default preset id
and volume 100 — a fresh attach never leaves a zero volume slot), so
the only way a stored zero exists is `zylaxion volume 0` — a
deliberate mute, which `on` then silently restored as the compiled
default, breaking the exact "takes effect on `zylaxion on`" promise
the lane exists to keep. The fix is the `stored_volume_or_fallback`
law in `src/commands/sound/mod.rs`: a successful read is always
user-meaningful and is honored verbatim (a mute is a mute); only a
FAILED read defers to the resolved default, and the posture writes
that follow fail loudly if the map stays dead, so the fallback can
never silently lie. The false comments in lifecycle.rs and the
preset module header are corrected in the same commit — a doc lie
that blessed the behavior was half the bug. Pinned by
`stored_volume_law_honors_the_mute`.

**Finding: PASS** with the finding closed in the same audit.

## Dimension 2 — Code Hygiene (third pass, the infra)

**Surface.** The fresh infra diff: `scripts/audit/zactrix-host.sh`
(nd-14..21's effect-verification and the R5 systemd rewrite),
`scripts/dev/{version-to,regenerate-ebpf-prebuilt}.sh` (the
self-refreshing prebuilt lane), `scripts/package/` (the hand-over
units' install/uninstall lanes), the gates (the already-certified
lts-4 fixes), README's download lanes (nd-7).

**Evidence.**

- **Shellcheck 0.11.0, whole repo, severity >= warning: exactly one
  hit, fixed in night-last-total-lts-6** — `zactrix-host.sh`'s bare
  `cd` (SC2164), the same class night-total-lts-4 killed in the
  wholesale gates: a failed cd would leave the audit measuring
  whichever directory it happened to be in. Now fail-fast with the
  why in a comment. Every other repo script is clean.
- **The version-to.sh regex fix (nd-7) verified by reading:** the
  prerelease group `-((alpha|beta|rc)\.[0-9]+)?-` now strips only
  KNOWN prerelease suffixes — the old shape ate the first `-token-`
  after the semver, which is how `-linux-` vanished from the rc.1
  README URLs. The failure mode and the fix are both documented
  in-tree.
- **The prebuilt lane's auto-refresh discipline (nd-4/nd-8) reads
  correct end to end:** one writer (regenerate), one enforcer
  (parity gate), the bump lane fail-closed — a stale lane refuses
  the commit and names the refresh; a WIP lane stale before the bump
  keeps the honest manual verdict.
- **Observed and accepted, with the why:** the `--check` block of
  `regenerate-ebpf-prebuilt.sh` moved tabs → spaces in the fresh
  range. No gate governs script indentation style (the "tab gate"
  note in the second pass was byte-preservation during an edit, not
  a standing law); rewriting whitespace for symmetry would churn a
  working file for zero behavior — the no-over-engineering rule
  keeps it.

**Finding: PASS** — one dragon found (the bare cd), killed; zero
stale markers, zero new unsafe, zero orphans in the fresh surface.

## Dimension 3 — Optimization (third pass)

**Finding: PEAK — skipped.** The engine is frozen and byte-identical
across the whole range — no profile names a deficit, and the freeze
bar demands a measured improvement before any diff. The fresh runtime
costs are startup-shaped, not hot-path: the nd-18 restore adds one
map read to `on` (a user-initiated verb that already spawns a player
or writes six slots), and the nd-19 desktop-user scan reads a
`/run/user` directory that holds a handful of entries, once per boot,
inside the attach one-shot. The infra scripts run seconds and are not
hot. Micro-optimizing any of these would be the over-engineering the
tree itself forbids.

## Dimension 4 — Security Hardening (third pass)

**Surface.** The privilege and lifecycle surfaces in the fresh range:
the hand-over recipient resolution, the root refusal, the audit
script's mutating rows, the bump/download lanes, the unit files.

**Evidence.**

- **The nd-19 hand-over, read end to end:** the recipient resolves
  in two lanes — `$SUDO_USER` (the interactive lane) first, then the
  first `/run/user` entry with uid >= 1000 (the boot lane). A set
  but unresolvable `$SUDO_USER` does NOT fall through to the scan:
  the invoking identity was named and lost, and hiding that behind a
  scan would be a lie. The scan skips non-numeric names and the
  system uid range; multi-seat determinism (lowest uid wins) is
  documented at the code. The o+x relax remains the hand-over's last
  step, one bit, never other-read.
- **The nd-17 root refusal re-read and holds:** `on_root_refusal`
  is the night-improve-26 session_refusal shape at the `on` verb —
  the parent owns the door, the child owns the audio, and the guard
  fires before any map write or spawn.
- **The mutating audit rows stay honest:** night-dinner-15..21's
  effect-verification (off retires the pid, detach empties the
  kernel answer, attach replants a monotonic id) survived the
  sandbox rig's ten-scenario battery at nd-21, and this pass adds
  the fail-fast cd to the script's own hygiene.
- **The supply-chain lanes hold:** the bump's README regex now
  strips only known prerelease suffixes; the bpf-linker bootstrap
  still dies at its sha256 gate (re-verified by this pass's rebuild:
  the pinned hash matched the downloaded binary); CI stays
  SHA-pinned and least-privilege (no workflow changes in the range).

**FINDING, fixed in night-last-total-lts-6 (this commit): the
hand-over watch fires on greeter uids.** The path unit's trigger and
the attach's recipient filter spoke different laws:
`PathExistsGlob=/run/user/*` matches ANY runtime directory, while
`first_desktop_runtime_user` accepts uid >= 1000 only. On a host
whose login manager creates its own runtime directory (GDM's greeter
user, the classic `/run/user/120`), the watch fires at the greeter
login, the attach finds no desktop recipient, and — because
`RemainAfterExit=yes` keeps the service active — the REAL login's
trigger becomes a no-op start job: the pins stay root-owned and the
zero-touch promise silently degrades to the doctor's FAIL row and a
manual `sudo zylaxion attach`. The fix is one glob:
`/run/user/[1-9][0-9][0-9][0-9]*` — four-plus digits starting 1-9
never match the system uid range (0-999) and always match the
desktop range, so the trigger speaks the same law as the filter.
Host note, named: installed units predate the fix; the install lane
refreshes them, and the audit's parse-hygiene row will name the
mismatch byte-for-byte until it runs — the honest cost of a unit
file fix.

**Finding: PASS** with the finding closed in the same audit.

## Dimension 5 — LTS Stability (third pass)

**Evidence.** The strongest LTS evidence this pass is negative
space: the sandbox reset happened, and the repo's own recipes
rebuilt every instrument from zero — the toolchain pin, the ALSA
staging, the checksummed bootstrap, the documented battery — with
one pool-filename drift (the deb13u1 update) named where the second
pass's exact strings no longer resolve. That is the tree's
self-enforcement chain doing its job under the worst conditions it
will see. The docs-law dimension found one rot instance and fixed
it beside the behavior fix: the nd-18 comment's "fresh attach
leaves both slots at zero" was false in the tree (the plant seeds
the defaults), the exact claim-blesses-bug shape this dimension
hunts. The CHANGELOG carries both fixes and the security record
carries the 23/0/0 milestone with the residuals that stay named:
the 25-second attach sleep (unreproduced, two clean boots since),
the soak-silence split proposal (not implemented), the
loaded-vs-built byte hash (root-host one-liner), and the unit
refresh on already-installed hosts.

**Finding: PASS.**

## The fix accounting (third pass)

- **night-last-total-lts-5 (the skill)**:
  `src/commands/sound/mod.rs` (the `stored_volume_or_fallback` law
  + contract docs), `src/commands/sound/lifecycle.rs` (the restore
  lane honors `Ok(0)`; the corrected nd-18 comment), 
  `src/session/preset.rs` (the module header's volume lane now
  states the seed and the mute semantics), 
  `test/commands/sound_tests.rs` (the
  `stored_volume_law_honors_the_mute` pin), the CHANGELOG Fixed
  entry. Verified: fmt green, clippy `-D warnings` green across
  `--all-targets --all-features` on the ebpf-feature build, the full
  suite 225/225 (the 224 the second pass ran plus the new law pin,
  1.25 s native), shellcheck untouched.
- **night-last-total-lts-6 (the infra)**:
  `assets/zylaxion-handover.path` (the desktop-uid glob + the
  failure-mode note in the header),
  `scripts/audit/zactrix-host.sh` (the fail-fast cd),
  this report, the security doc's night-dinner-21 addendum, the
  CHANGELOG Changed entry. Verified: shellcheck clean repo-wide,
  bash -n clean, the gate-keepers battery green.
- **Benchmark, skipped with the why:** zero engine bytes (the drift
  check proves the whole range), and the changed runtime code
  executes on `on` — a startup verb that gained one map read and
  one Option match; no Zactrix metric measures this surface.
- **Version:** untouched, owner-decided, as always.

## The carried residuals (the next pass's worklist)

1. The loaded-vs-built byte hash confirmation (root host + bpftool
   one-liner) — unchanged since the first pass.
2. The six Zactrix proof wrappers on an audio-capable host — the
   owner's release ritual; unchanged.
3. The 25-second attach-path sleep — named, unreproduced across the
   two boots since; the next attach timing anomaly closes or
   deepens it.
4. The R8/R10 soak-silence split (delta 0 + unreadable run_cnt →
   INCONCLUSIVE) — proposed, not implemented; take a number when it
   lands.
5. The installed-handover-unit refresh on existing hosts — the
   install lane's job before the next `--full` host run, so the
   parse-hygiene row reads byte-identical against the fixed glob.
