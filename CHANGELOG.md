# Changelog

All notable changes to zylaxion.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file carries ONLY the entries newer than the v10.2.0 release —
the fresh record for the v11 line. night-improve-5 (2026-10-11)
trimmed the legacy bulk out so this file starts clean for the
upcoming version instead of dragging the archive along: the v10-era
history (the v10.0.0, v10.1.0 and v10.2.0 entries plus the v6.0.1
summary stub, as they stood at the end of the v10 line) is archived
in
[docs/archive/CHANGELOG-V10.0.0-ERA.md](docs/archive/CHANGELOG-V10.0.0-ERA.md),
and the pre-v10 release notes live next to it in
[docs/archive/CHANGELOG-v6.0.0.md](docs/archive/CHANGELOG-v6.0.0.md).

## [Released]

v80.0.0-alpha.1

## [Unreleased]

### Frozen

- **night-private-research-2xblade-4** — the freeze. Declared
  2026-10-14 (UTC, read from the system clock at declaration time —
  not hardcoded): the Zactrix Engine is LTS-frozen as of the
  certified commit c028ae6f907055807125c9a20c156cb16a3a2038, the
  head of night-private-research-2xblade-3.6X, the pass that closed
  the security audit. From this commit forward, a change to the
  engine requires one of three admission paths, verbatim:
  (a) a security fix — a CVE in a dependency, or a vulnerability
  reported through SECURITY.md, that reaches the engine;
  (b) a compatibility fix — a kernel, glibc, cpal, PipeWire, or
  sample-rate change that the engine must adapt to in order to keep
  running; (c) a measurable improvement proven by the same audit
  harness that certified the engine (scripts/audit/zactrix-*.sh), on
  a published before/after matrix, with no tier regression in any
  preset. A change that cannot name which of the three it belongs to
  does not land. Cosmetic changes do not land. Tuning by taste does
  not land. The Zactrix Engine is frozen as of
  c028ae6f907055807125c9a20c156cb16a3a2038. It was built, sharpened,
  measured, and audited across seven passes; every proof and every
  audit finding is recorded in docs/ZACTRIX_LTS.md and
  docs/ZACTRIX_SECURITY.md. From this commit forward, changes
  require a why that passes the maintenance bar: a security fix, a
  compatibility fix, or a measured improvement proven by the same
  audit harness. The engine is whole.

### Security

- **night-private-research-2xblade-3.6X** — the security hardening
  pass, the follow-up the 3.6 record pre-named. The security audit
  examined six dimensions and found 0 finding(s) and 1 INCONCLUSIVE
  dimension(s): privilege boundaries. The inconclusive dimensions
  could not be measured in the sandbox and are named with what would
  be needed to conclude. The remaining dimensions pass.
  Evidence in docs/ZACTRIX_SECURITY.md. The two LOW residuals the
  3.6 record pre-named are fixed in this pass; both live-verified in
  the sandbox, both inside the audit-pass fix budget (77 LOC across
  the 2 authorized files; the proof harness is a third file because
  the layout law requires tests in the test/ tree, never inline):
    - The coredump carrier (Dimension 1's residual) — the player now
      forces `RLIMIT_CORE = 0` (soft and hard) at start, before any
      scancode-capable buffer exists, so a file-backed core dump of
      the player is no longer producible by the player itself; the
      hard-limit drop also bars quiet re-raising without
      CAP_SYS_RESOURCE. Best-effort by contract; piped `core_pattern`
      stays the operator's policy, named in the record. Two new test
      pins hold the 0/0 fact and the idempotence
      (test/commands/player_tests.rs).
    - The unsigned bpf-linker download (Dimension 5's LOW) —
      bootstrap-ebpf.sh now pins the measured sha256 of both musl
      release assets beside the version constant and dies at the
      checksum gate (both hashes printed) before extraction or
      execution. Verified live on both paths: a tampered pin is
      refused with nothing installed or executed; the honest archive
      passes and installs. The pin's root of trust is the repo's own
      commit process — the same epistemic class as Cargo.lock.
  - Verdicts this pass: keystroke confidentiality LOW, narrowed (the
    coredump carrier closed; same-uid memory reads and operator-piped
    dump policy are the named remainder); BPF program safety NONE (no
    byte of ebpf/ moved; the loaded-hash confirmation stays the named
    inconclusive measurement inside the passing dimension);
    privilege boundaries INCONCLUSIVE (carried, measurement named);
    network surface NONE; supply chain NONE (the 3.6 LOW closed;
    cargo-audit 0.22.2 ran this pass with --deny warnings: exit 0 on
    all three lockfiles — 150 + 31 + 119 packages, 1314 advisories;
    the ebpf-prebuilt manifest enforcement stays the freeze pass's);
    update path and version honesty NONE (the -V build stamp
    re-measured matching git rev-parse at the same tree).
  - Full battery green at this tree: fmt, clippy -D warnings, 340
    tests, rustdoc -D warnings, cargo audit x3, build.sh --check-all,
    gate-keepers all sections. No DSP math, no preset registry, no
    CLI surface, no attach chain touched; the version stays
    owner-decided.

- **night-private-research-2xblade-3.6** — the security audit. The
  security audit examined six dimensions and found 2 finding(s) and
  1 INCONCLUSIVE dimension(s): privilege boundaries. The inconclusive
  dimensions could not be measured in the sandbox and are named with
  what would be needed to conclude. The remaining dimensions pass.
  Evidence in docs/ZACTRIX_SECURITY.md.
  - Verdicts: keystroke confidentiality LOW (no counterexample to the
    zero-leakage claim; the named residency class is recent scancodes
    recoverable from process-memory artifacts — ring pages, the event
    channel, active voices — and the kernel-resident aggregate
    counters; the coredump `RLIMIT_CORE` hardening is pre-named as a
    residual for a hardening pass, not fixed here); BPF program
    safety NONE (every map access bounds-checked, the ring record
    fully initialized before submit, no loops, the `dev` pointer
    never dereferenced; the loaded-hash confirmation is the one
    inconclusive measurement inside the passing dimension);
    privilege boundaries INCONCLUSIVE (the pinned-file permission
    modes after attach need a real bpffs host: `stat
    /sys/fs/bpf/zylaxion/*` plus a second-user dump probe — the
    source-level analysis attaches: root-only attach, the sudo-user
    chown hand-off, and the kernel's own cross-uid `kill()` EPERM
    boundary); network surface NONE (zero socket call sites, the
    only outbound is the constant-URL curl in `--check-update`,
    root-refused before any I/O, the response parsed for one field
    and never executed); supply chain LOW (bootstrap-ebpf.sh
    authenticates the HTTPS channel but not the bpf-linker artifact —
    no pinned checksum, no signature; named residual, the fix edits
    the dependency policy this pass may not touch); update path and
    version honesty NONE (`-V` rides the build.rs-computed git hash,
    the release pipeline emits exactly the three checksums plus the
    GPG lane docs/VERIFY_RELEASE.md documents).
  - Verified against the committed tree: the ebpf-prebuilt object
    hash matches its manifest byte for byte, the lockfile carries no
    time crate, no serde, no toml (the lock's `tokio` entry is the
    wasm32-gated cpal transitive and never compiles for the Linux
    production target), and the production-path scancode grep names
    only the disclosed aggregate telemetry render.
  - Three stale claims corrected, text only, no code behavior:
    SECURITY.md's supported-versions row now matches the manifest
    version family (80.x, was 11.x), README no longer describes a
    nonexistent `--dump-scancodes` examples lane (the examples
    synthesize fixed scancodes for audio sanity runs), and the
    daemon-era drop-order comments in `src/session/signals.rs` and
    `src/main.rs` now describe the actual teardown set. No DSP math,
    no preset registry, no CLI surface, no attach chain, no player
    lifecycle change; the version stays owner-decided.

### Added

- **night-private-research-2xblade-3.5** — the resource footprint:
  the sixth proof, the one pass the LTS certification was missing.
  The instrument (`scripts/audit/zactrix-footprint.sh`, harness
  modes `--footprint-state-b/-c/-cold`) reproduces the player's
  exact process topology — the production `Orchestrator::run` loop,
  the 16384-frame stereo ring, the `VoicePool` at the default
  posture, a wall-clock event source on the bridge's 100 ms cadence,
  the control watcher, and a device-clock thread — and samples its
  own `/proc` footprint at the anchors, cross-checked by the
  wrapper's external reads of the same files (the two instruments
  agreed tick-for-tick at every shared anchor). The instrument's own
  artifact is part of the record: the first run drove every state
  through cpal against the ALSA null PCM and its idle state burned a
  full core — per-thread `/proc` attribution named `cpal_alsa_out`
  (the null device is POLLOUT-always-ready, so the callback thread
  spins writing silence), not the player, whose orchestrator loop
  cost 8 ticks per 10 s; the 2xblade-2 lesson held again, and states
  B/C moved to `PacedNullSink` — the same ring and back-pressure
  with the device thread replaced by a wall-clock drain at the
  desktop's 1024-frame quantum — before any engine claim was made.
  Measured (Xeon sandbox, 2 vCPU, CLK_TCK 100; the locked CPU
  formula Δ(utime+stime) ÷ wall x ncores, the per-single-core
  figure printed beside it): STATE A — 60/60 samples with zero
  `zylaxion` processes (idle CPU 0% because no process exists); the
  kernel-tap memory half is INCONCLUSIVE, named — no root, no
  `CAP_BPF`, `/sys/fs/bpf` unwritable, `bpftool` absent; the
  compiled map definitions (about 76 KiB of map data) are recorded
  as definition, not measurement. STATE B (5 keys/s, 60 s) — RSS
  3712 KiB flat at t = 5/30/60 (growth 0 KiB in-window; the +96 KiB
  the final external sample caught is the teardown boundary), CPU
  1.075% (2.15% of one core), 299 presses + 299 releases at
  48000 Hz. STATE C (zero keystrokes, 60 s) — RSS 1580 KiB at both
  anchors (zero in-window growth), CPU 0.642% (1.28% of one core;
  the input bridge, the watcher, and the device clock all measured
  0 ticks — the remainder is the orchestrator's 1 ms
  `recv_timeout` cadence, ~10 us per wakeup on this Xeon). The
  README's "the player's epoll loop costs nothing between
  keystrokes" names the bridge lane and is confirmed as scoped; no
  claim was weakened and none needed correcting. COLD-START —
  device open 0.438 ms, first output sample pushed 0.439 ms, first
  rendered sample pushed 0.475 ms (the real null-PCM open path; the
  null device's fast open is named). No engine file changed, no
  preset changed, no CLI surface changed; the audit harness gained
  the three crates the main package already pins (`cpal` 0.15.3,
  `ringbuf` 0.5.3, `crossbeam-channel` 0.5.17) so the driver could
  run the real production audio stack. The required sentence,
  adapted to the one honest delta (the tap-memory clause states the
  INCONCLUSIVE fact instead of a number, because no number was
  measured):

  > The sixth proof — resource footprint — PASSed on every
  > measurable dimension: idle CPU 0% (60/60 process-absence
  > samples), player RSS bounded and flat in both the active and
  > inter-keystroke states, player CPU bounded in both states,
  > cold-start latency bounded. The kernel-tap memory dimension is
  > INCONCLUSIVE in this sandbox — the tap cannot be attached
  > without privileges — and is recorded as definition, not
  > measurement. docs/ZACTRIX_LTS.md now carries six verdicts; the
  > LTS certification stands.

- **night-private-research-2xblade-3** — the LTS certification: the
  pass that tests the sword, not forges it. The reading (the audit
  end to end, the render path from the cpal callback to the
  per-sample SVF stages, the volume law, the pool) found the
  architecture the proofs measure: the DSP render is NOT inside the
  cpal callback — the callback is a lock-free ring-buffer drain
  (`try_pop`, `is_finite`, clamp, one relaxed atomic load), and the
  render runs on the producer side, where `VoicePool::process_sample`
  iterates a fixed 16-voice array, loads the master volume from one
  relaxed atomic f32-bits cell, and hard-clamps the mix to +/-1.0 —
  that clamp IS the master limiter, and the NaN/Inf-to-silence branch
  IS the safety net every raw-path proof must bypass. The volume law
  is percent x 5.5 into a pool-side clamp of [-100, 100] linear gain
  (the 200% CLI cap is gain 11.0; `--force-this` at the 10000% force
  ceiling lands on the internal ceiling 100.0). The pool never
  allocates: trigger steals the oldest slot, release ramps over
  ~2 ms, and a voice dies only when its envelope crosses the
  threshold. The engine exposes no attack-time axis (the attack shape
  is the excitation burst plus the per-preset decay coefficient), so
  the attack-max/attack-zero axis the contract named is not
  applicable and is documented as such. One sandbox fact shaped the
  instrument: at ~2 tan() evaluations per active voice per sample,
  the render path costs ~0.4-1.6M pool samples/s here, so the
  long-run soak renders its one million keystrokes on a max-pressure
  schedule (the pool saturated, voice stealing continuous) instead of
  natural pacing — the compression is named in
  docs/ZACTRIX_LTS.md, and the natural-lifetime dimension is covered
  by the hot-reload session (one hour of audio at 5 keystrokes/s with
  releases) and the million-sample raw battery. The proofs, in
  execution order: PROOF 4 (numerical stability) — 144 combinations
  (24 presets x 44.1/96 kHz x three volume axes derived through the
  shipped laws), zero NaN, zero Inf/overflow, post-limiter peak
  1.0000 everywhere, the forced-Q battery above it byte-identical to
  the 2xblade-2 record; PROOF 2 (sample-rate span) — the voice
  lifetime, the c^(R/S) law's exact measurand, spreads 0.0068-0.0628%
  across 44.1/48/88.2/96/192 kHz on all 24 presets (target 0.5%),
  every per-rate hash byte-identical across independent runs, the
  -60 dB tail endpoint recorded as the excitation statistics the
  2xblade-2 control experiment already named; PROOF 3 (RT-safety) —
  the written no-alloc/no-lock/no-syscall audit of the callback and
  the per-sample path, plus the jitter measurement (natural pass
  max/p50 2.60x, saturated pass 3.58x, worst block 2.23 ms against
  the 5 ms target, on the named Xeon sandbox); PROOF 5 (hot-reload
  safety) — one hour of audio time, 120 ArcSwap preset stores, 360
  volume_bits stores, 18,000 keystrokes: zero artifacts above the 10x
  baseline, zero net-concealed blow-up signatures, pool fully
  returned after the drain; PROOF 1 (long-run soak) — one million
  keystrokes round-robin over all 24 presets, two independent runs:
  final-1000-sample hashes byte-identical
  (`0abf267b6833e0c7`), RSS growth 0 KiB, per-1000-keystroke CPU
  drift +0.93%, 19 raw-path spot checks clean, pool end state zero
  active voices. New instruments: `scripts/audit/zactrix-soak.sh`,
  `zactrix-rates.sh`, `zactrix-rt.sh`, `zactrix-reload.sh`, and the
  volume-axes extension of `zactrix-stability.sh`; the audit harness
  gained one justified dev-dependency (`arc-swap`, the crate the main
  package already pins, for the driver's faithful watcher mutation).
  The required sentence, verbatim:

  > The Zactrix Engine is certified LTS as of commit
  > `3dedb8ab26a6267066499702429d3b891f60d64f`: five independent
  > proofs — long-run soak, sample-rate span, RT-safety, numerical
  > stability, hot-reload safety — all passed on the instrumented
  > path, and docs/ZACTRIX_LTS.md carries every raw measurement. The
  > engine is frozen. Future changes require a why that passes the
  > maintenance bar: a security fix, a compatibility fix, or a
  > measured improvement proven by the same audit harness.

### Changed

- **night-doc-1** — the honest surface. The musl-dormant phrasing
  and the design-bounded philosophy are now stated in plain language
  where a reader will find them: the dormant lane's reason is
  technical (cpal → ALSA → no static libasound.a for musl; glibc
  static-shape covers the 99%), and the freeze is framed as a
  decision the engine earned, not a shortage of ideas. No code,
  tests, or frozen surfaces touched. Two paragraphs the engine
  earned: the musl lane is dormant for a reason, not a promise, and
  the freeze is a decision, not a shortage. Both are now written
  where a reader will find them.

- **night-private-research-2xblade-2** — the Range Question: one
  decision, taken from the audit data, executed in one direction.
  The reading (the audit end to end, the engine's clamp sites, the
  registry's descriptions, the range pins) found three facts: the
  contract's written rationale ("resonance > 10.0 becomes
  numerically unstable in single-precision TPT SVF") is contradicted
  by the tree's own pin — `test_svf_high_q_does_not_blow_up` runs
  the SVF at Q = 50 for 44,100 samples and asserts finite output —
  so the ceiling was inherited, never measured; the contract is
  enforced at four sites (the ranges constants, `validate_and_clamp`,
  the two per-sample safety-net clamps on the click and spring
  curves, the registry test tree); and the description scan is wider
  than the mission's five — "long ring" is verbatim in
  `ibm_model_m` and `clicky`, "long mid ring" in `retro_typewriter`,
  "long bass sustain" in `oceanic`, beside the promised
  resonant/thock/hollow/ping language on `buckling_spring`,
  `alps_skcm`, `gateron_yellow` and `classic`. The owner's decision:
  EXPAND — the Q range contract moves from `[0.1, 10.0]` to
  `[0.1, 30.0]`. The Q range contract expanded because the audit
  named it as the ceiling and the reading confirmed five presets
  whose own text promises a ring the old contract could not deliver.
  The engine's architecture is now the shape its descriptions
  promise. No preset regressed; every tier change is named in
  docs/ZACTRIX_AUDIT.md. Path A ran in order: the numerical
  stability battery (`scripts/audit/zactrix-stability.sh`, a new
  harness mode rendering each scan-set preset with every stage
  forced to the 30.0 ceiling, one million samples per rate, through
  the raw model path with no VoicePool safety net) came first and
  came back clean — zero NaN, zero Inf, zero output denormals, every
  TPT integrator state bounded, and the c^(R/S) retention law's
  direct measurand (voice lifetime) agreeing across 44.1/48/96 kHz
  within 0.02%; the two honest residues it found (denormal
  integrator dust at voice death, and a rate-dependent -60 dB tail
  endpoint) are control-proven pre-existing properties of
  noise-driven synthesis, recorded in the audit doc. Then the
  re-tune, fit-surface-first (`--fit`, `--modal-debug`): eight scan
  presets re-tuned, five rose B -> A (ibm_model_m, classic,
  buckling_spring, alps_skcm, retro_typewriter — the modal column
  went from one non-zero row to six), three hold B with zero
  regressions and the reason named (clicky: its 1 ms spring cannot
  reach the modal picker without breaking the flatness and
  attack/tail bands; gateron_yellow: a lubed linear's spring is
  deliberately ringless — the thock is the identity; oceanic: its
  100 Hz mode sits on FFT bin 1, below the tracker's bin-2 pickup
  floor — the ring is delivered and now measured inside the energy
  band, +18.6 -> +11.6). topre holds A untouched; the four identity
  refusals hold C untouched; the sixteen untouched presets reproduce
  the 2xblade after matrix byte-for-byte. The audit instrument gained
  the stability mode and the fit surface; the metric definitions
  did not change, and `zactrix-audit.sh` stays byte-reproducible
  (two consecutive runs diff-clean). Full battery green: fmt, strict
  clippy all-features, 209+38+38+53 workspace tests + 55 harness
  tests, rustdoc -D warnings, build.sh --check-all (cargo-audit
  skipped: not installed in the sandbox), gate-keepers all sections.

- **night-private-research-2xblade** — the Zactrix Engine: the
  acoustic engine's spectrum learns to move. The pass started as a
  measurement, not a feature: a headless audit harness
  (`scripts/audit/zactrix-audit.sh`) rendered all 24 presets through
  the real render path at 96 kHz and scored four physical movement
  metrics per preset against conservative physical bands
  (`docs/ZACTRIX_AUDIT.md`). The frozen-coefficient engine measured
  A = 1, B = 17, C = 6 — 21 of 24 presets carried 99+ percent of
  onset energy in the first 5 ms — and the Phase 0 gate opened. The
  Zactrix Engine makes the spectrum evolve with the actuation it is
  voicing. The change is architectural: the three TPT SVF stages
  gain time-varying cutoff and Q, driven by physics-derived curves.
  Every preset was re-audited; every tier change is named in
  docs/ZACTRIX_AUDIT.md. No preset regressed. Stage A (click)
  collapses from broadband to the click band over ~3 ms, stage B
  (spring) travels from the click band into the spring band over
  ~5 ms while Q wakes, stage C (housing) keeps the constant whose
  long ring stays the existing envelope's job; the curves are
  authored in seconds and Hz and are sample-rate invariant by
  construction. The engine struct carries the Zactrix name (the
  historical `MechanicalClick` remains as a documented alias). The
  re-tune raised the housing resonance on 20 presets under the
  level-preserving `mix x Q` law, with click/spring/decay/ambient
  rows untouched; four presets refused the fit on identity grounds
  and are recorded as exceptions. Same-instrument before/after:
  centroid drift and flatness evolution roughly doubled on the
  fitted crowd, six voices now sit inside the physical attack/tail
  band (three before), `ibm_model_m` and `buckling_spring` rose
  C -> B, `topre` holds A untouched. The honest residuals — the
  Q <= 10 range ceiling on ring survival, the modal column's
  32-48 ms late frames, and the four refusals — are named in the
  audit doc, not hidden.

- **night-improve-28** — the honest architecture ledger,
  `docs/EBPF_ONLY_VS_DAEMON.md`: the eBPF-only bet costed against
  the deleted daemon era, with the win rows carrying their
  verifiers and the cost rows keeping their irreducible remainders
  (the 5.15 kernel floor, the one privileged attach step, the
  Ubuntu unprivileged-bpf lockout, the kernel-adjacent bug class,
  the harder field diagnosis, the lost runtime sound customization,
  the nested nightly build, the partial-pin crash artifact, the
  aya-side debit in the dependency ledger). The stale daemon-era
  prose in `docs/PHILOSOPHY.md` is dead with it: the daemon-era
  "one job" opener, the PID file and IPC socket claims, the
  "evdev through libinput" input chapter (now the eBPF-only input
  chapter, linking the ledger), the libinput/serde/toml dependency
  chapter (now the real set), and the daemon-state overview facts
  (now the real ones).

- **night-improve-29** — the masterclass CLI/UX tidy of the discovery
  and probe surfaces. `list-presets` renders ONE line per preset — the
  name in a fixed-width column, the description beside it, the
  session-active row marked at the end — and the how-to footer
  (`Active preset:` restatement, the `Switch preset:` block, the
  volume tip) is gone: a list lists, the help and the overview teach.
  An env override that names no registered preset is now named in an
  inline note instead of silently falling back to the marked default.
  `list-backends` and the overview's audio line, and the doctor's
  audio section, all render the same compact one-device shape; the
  doctor's audio probe now OPENS the device (a name without a working
  config was a false PASS on hosts whose `default` PCM cannot open)
  and reports "present but not openable" honestly. The raw libasound
  C-level stderr spew (up to eight `ALSA lib ...` lines on a cardless
  host) is silenced for the probe surfaces via a quiet-ALSA guard —
  alsa-sys joins as a target-gated dependency at cpal's own pin
  (0.3.1, zero lockfile delta), restoring the library default on drop
  so the player path keeps its full diagnosis. Stale-claim kills in
  the help and the crate doc: "kernel evdev in, low-latency PipeWire
  audio out" → "the eBPF tap in, low-latency ALSA audio out"; the
  `ZYLAXION_PRESET` env row no longer references the dead `start`
  verb. The supermassive preset-catalog probe re-pinned to the new
  one-line shape (the scanner carries the surface it scans).

- **night-security-1** — supply-chain dependency audit and tightening.
  Every feature gate in the manifest was walked against the audited
  API surface (crate-level: all 11 direct dependencies are live, zero
  unused). Two trims landed: `signal-hook` moves to
  `default-features = false` — the default `channel` and `iterator`
  features compile the signal-delivery channel and the `Signals`
  iterator module trees this crate never touches (only
  `flag::register` and the `consts` are used, both ungated); `nix`
  drops the explicit `process` key — `signal = ["process"]` in nix's
  own feature map, so the gate was a redundant key (the audited
  surface: `sys::signal::kill`, `poll`, `unistd::{getpid, geteuid,
  Pid, User::from_name}`, `errno::Errno`). Audit findings recorded
  with no change required: the GitHub Actions estate is already
  SHA-pinned byte-identical to the zelynic reference; every direct
  dependency is current within its semver requirement; the tokio /
  regex / jni / windows lockfile entries are cpal's target-gated
  backends that never compile on Linux (the Linux-resolved graph is
  52 crates); aya's upstream `assert_matches` runtime dependency is
  an upstream choice, not addressable here. README's stale lockfile
  counts (135) synced to the real 150.

### Fixed

- **night-hunter-4 (the lane law and the completed palette)** — the
  stdout verdict helpers (`ok_line`, `fail_line`, `section`,
  `tip_line`, `ok_marker`) gated on the STDERR capability probe while
  decorating STDOUT report lines: on an interactive shell with a piped
  stdout (`zylaxion list-presets | grep topre`) the ANSI bytes leaked
  into the pipe. One gate per stream now — the stdout helpers probe
  stdout, the stderr lane (error/warning) keeps its own probe. On top
  of the lane law the semantic tiers complete the report surfaces the
  palette contract promised: `status` and the no-subcommand overview
  color their verdicts (green supported/on/running/attached, grey
  off/not running/idle, yellow the stale pid and unknown states, red
  failed and verified-unsupported), the introspection footer and the
  not-attached hint ride the calm-grey tier, `list-presets` carries a
  brand-purple header, the overview's runnable commands ride the
  status-green tier with grey comments, and the doctor's BTF line and
  last-attach note join their tiers. Plain mode stays byte-identical
  everywhere (the pin holds every helper plain under the capture
  harness).

- **night-hunter-2 (the stale `<- active` marker)** — `list-presets`
  marked the active preset from the environment resolution alone
  (`ZYLAXION_PRESET`, then the compiled default), so a live session
  switched with `zylaxion preset --switch <name>` kept pointing at a
  row the player had left behind — the map slot IS what is sounding.
  The marker now reads the pinned control plane first: a live player's
  preset slot outranks the would-start resolution, a dead session's
  leftover slot does not (the next `on` re-resolves it, and the
  fallback answers what the next start would pick). The no-subcommand
  overview shares the same posture law — and its player line stops
  parroting a stale pid as if it were running (`stale pid N — the
  next \`on\` re-resolves`), with the preset and volume rows rendered
  from the same live-or-next-start rule. The overview also reads the
  control plane through the crate-visible slot constants now instead
  of raw numbers that could drift from the verbs' own mirror.

- **night-hunter-2 (the description drift)** — the version tagline and
  the help banner carried a hand-written second description
  ("Real-time mechanical keyboard acoustic synthesizer for Linux" /
  "Turn every keystroke into a spatially-accurate click sound …") that
  had drifted from the registry-facing Cargo.toml `description`. The
  zelynic family contract wins: `about = env!("CARGO_PKG_DESCRIPTION")`
  — one string, pulled at compile time, wrapping itself into the help
  banner at render time, so no second copy of the prose exists anywhere
  in the tree to drift. A test pin holds the flattened reference
  against the crate description verbatim.

- **night-hunter-2 (the Pro mode estate's missing flag)** — the
  `--force-this` escape was taught only inside the Run section's
  volume prose; the Pro mode section (the estate that documents every
  hidden and advanced spelling COMPLETELY) had no row for it. The
  Advanced flags group adds the spelling, the computed description
  column keeps its geometry, and a runnable example
  (`zylaxion volume -p 250 --force-this`) gives the copy path. The
  integration pins hold the new spelling, the group heading, and the
  example.

- **night-hunter-1 (the stale-reference sweep)** — the panic hook's
  comment taught the reader to look for the panic record in
  `journalctl --user -u zylaxion` (a unit that does not exist in the
  eBPF-only architecture; the record lands on the stderr the `on`
  invocation leaves behind) and spoke of "the daemon" and
  "foreground mode" — all reworded to the player's actual stderr
  story. `docs/RULES.md`'s zero-leakage rationale named the same dead
  unit as the side-channel surface; it names the honest surfaces now
  (the spawning terminal, shell scrollback, any log capture).

- **night-hunter-1 (the two half-blind scans)** — the telemetry
  render's per-key sweep covered `0..255` while the tap counts and
  the loader maps the full `0..=255` domain (the total rides slot
  256): keycode 255's presses landed in the total but could never
  reach the top-keys table. And `version-to.sh`'s member-name
  extractor (the dormant `crates/` lane of the lock verification)
  opened a sed group it never closed — the day the lane wakes, the
  extraction errors per member, the names come out empty, and every
  verification line false-warns; the group is closed and the pattern
  proven against a live manifest line.

- **night-hunter-1 (the control plane's one probe and one clamp)** —
  three consistency finds in the player/verb plumbing. The volume
  cell had two writers with two laws: the startup lane clamped
  through `with_volume` (±100 gain) while the watcher's live retune
  stored raw f32 bits — the same map value meant two loudnesses
  depending on when it was written; the watcher now rides the pool's
  own `volume_bits`. The player pid probe existed in three copies,
  two of them the zombie-blind kill(0) form: a ZOMBIE player read as
  alive, so `_play` refused "already playing" on a dead player, `on`
  claimed "sound already on" for one, and `off` stalled its full
  stop budget against one before advising a `kill -TERM` that cannot
  fix a zombie; both copies are deleted — every surface shares the
  orphan module's tested zombie-aware probe. And the `volume` verb's
  success print said "cap 200%" even when `--force-this` had just
  written 201..=10000: the note now names the lane that carried the
  write.

- **night-hunter-1 (the signed-noise range law)** — `u32_to_signed_f32`
  mapped the u32 domain to [-1.0, +3.0], not the documented symmetric
  [-1.0, +1.0]: the divisor rode 2²³ − 1 under a `* 2.0 - 1.0` tail
  (half the needed 2²⁴ − 1), so every consumer carried a +1.0 DC
  bias — the noise excitation feeding the waveshaper, the pitch and
  amplitude drift (up to 3× their designed extremes, asymmetrically),
  and the pan jitter (stereo pulled right). The divisor is the
  domain's own max now, both endpoints exact, and the range law is
  pinned by a sweep test.

- **night-badas-4 (round 5)** — the first full E2E probe run inside
  the micro-VMs exposed four truths, three environmental and one
  structural. The rootfs was assembled by a non-root tar extraction,
  which defaults to `--no-same-owner`: the whole guest tree landed
  owned by the runner's uid and the guest's sudo refused to run
  (`/etc/sudo.conf is owned by uid 1001`) — the real-user lane died at
  the sudo door before zylaxion was reached; the assembly now chowns
  the tree to root. The uinput built-in probe checked
  `/sys/module/uinput`, which a built-in module without parameters
  never populates (both reference kernels, verified empirically while
  `/dev/uinput` existed) — the probe now accepts the driver's device
  surfaces. The re-attach probe still expected the pre-night-improve-26
  refusal shape; it now pins the shipped idempotent-success contract.
  And the structural one: both reference kernels build
  `CONFIG_BPF_UNPRIV_DEFAULT_OFF=y` in, so the guest boots with
  `kernel.unprivileged_bpf_disabled=2` — locked for the whole boot,
  root cannot re-enable it, and every plain-user map op is refused.
  The product now probes that posture, the control-plane verbs refuse
  classified (naming the sysctl) instead of an EPERM masquerading as
  "not attached", the doctor carries the posture line, and the E2E's
  real-user lane tests what the kernel honestly allows: the full
  zero-to-hero chain where unprivileged bpf is permitted, the
  classified refusal where it is not.

- **night-badas-4 (round 3)** — the E2E rootfs lane hit the next
  era-drift: ubuntu:24.04 (noble) renamed the ALSA runtime package in
  the time_t transition, so `libasound2` has "no installation
  candidate" inside the micro-VM rootfs and both legs died before
  qemu. The rootfs step now detects the name the archive actually
  serves (`libasound2`, else `libasound2t64`) and says which one it
  picked — the payload's soname need is `libasound.so.2` either way.

- **night-badas-4 (round 4)** — the round-3 name probe was itself
  era-blind: noble keeps `libasound2` alive as a purely-virtual name
  (Provided by `libasound2t64`), and `apt-cache show` exits 0 on a
  purely-virtual name — the probe picked the dead name and the install
  still died with "no installation candidate". The honest probe is the
  resolver itself: an `apt-get install -s` simulation of each name,
  and the first that resolves wins.

- **night-badas-4 (round 2)** — the first runs to reach the deep lanes
  found three more facts, each verified empirically before the fix.
  (1) The eBPF validator's own diagnostic caught its second bug: plain
  `readelf -S` truncates section names to 17 columns
  (`fentry/input[...]`), so a full-name grep can never match — the
  section list now comes from `readelf -SW`, and the fresh validator's
  named-failure shape is what surfaced it on the first red run.
  (2) The supermassive E2E kernel resolver died per-leg: noble's
  7.0.0-38 carries the whole 166 MB module set in `linux-modules`
  alone and the archive refuses the `linux-modules-extra-7.0.0-38`
  name (apt exit 100), while jammy's 5.15 ships the 64 MB extra deb —
  the extra download is now best-effort by design. (3) The module
  chain's uinput requirement was written blind and both reference
  kernels disprove it: jammy's 5.15.0-198 and noble's 7.0.0-38 build
  uinput INTO vmlinuz (`modules.builtin` carries
  kernel/drivers/input/misc/uinput.ko in both; no .ko ships in any
  deb — verified by downloading and listing all five module debs).
  The workflow now requires only the sound chain (soundcore, snd,
  snd-timer, snd-pcm, snd-dummy — staged from the same kernel's debs),
  stages a uinput.ko when the archive ships one, otherwise verifies
  the built-in and says so; the guest init accepts the built-in via
  the honest probe (`/sys/module/uinput`) instead of failing the
  keystroke injector's kernel half.

- **night-improve-27** — the old era is eliminated and the master
  volume has a ceiling. The volume lane carried no upper bound at all:
  `zylaxion volume -p 1000` answered "volume set to 1000%" and wrote
  the map straight through (the help text already promised `<0-100>` —
  a promise the parser never kept). Both write entries now enforce the
  owner's 200% safety cap: the CLI parser refuses anything above 200
  with the fatal-input verdict (exit 2, before any map write), and the
  `ZYLAXION_VOLUME` env lane refuses a gain mapping above the same
  ceiling with a hard error naming it — never a clamp, so the control
  map can only ever carry a value the user asked for in writing. The
  help surface and its pins now document `<0-200>`. The same sweep
  killed the daemon-era residue that survived in the runtime wording:
  the root refusal for `on`/`_play` told a story about "the daemon"
  planting "root-owned session state (lock, socket, PID)" — state no
  process in this architecture has written since night-improve-23;
  the honest harm today is a root audio session no user desktop
  shares (the privilege audit table now lists the real surfaces:
  `on`/`_play` refused, `off`/`status` allowed, and no `start`/
  `daemon`/`stop` verbs that do not exist), the signals module's
  SIGHUP rationale named `cmd_start`/`cmd_daemon --foreground`
  (deleted verbs) and now explains the real lane (`_play` in a
  foreground terminal vs the detached spawn), and `on`'s second run
  says what actually happened: "sound already on: preset ..., volume
  ...% (player pid N kept — preset updated live)" versus the fresh
  spawn's "new player pid N". The version manager
  (scripts/dev/version-to.sh) learned the lesson of the
  v80.0.0-alpha.1 rehearsal: a sibling manifest that carries its own
  version line can also carry its own DETACHED workspace lock, and a
  bump that leaves that lock stale kills every later --locked build of
  the lane ("cannot update the lock file ... because --locked was
  passed"). The script now syncs each touched sibling's lock with
  `cargo update --workspace --offline` in its own directory, verifies
  the entry, and the staging hint (and the --commit path) name every
  file the run actually touched — never a partial `git add` again.

  The live test then surfaced the lifecycle seam, and the second
  half of the night closed it. The player could outlive the map it
  was watching: detach unpinned the control map, but the old player
  held the old inode and kept running against a flag only it could
  read. Two players, two maps, two worlds, both producing sound.
  The wire was made whole in night-improve-26; night-improve-27
  makes the lifecycle whole. `detach` now reaps the running player
  BEFORE a single pin unlinks — the enabled gate clears in the map
  the player is watching (its own 100 ms watcher exits on it), and
  the 0 → SIGTERM → SIGKILL escalation ladder drives until the pid
  is gone, every step logged. `attach` sweeps /proc for a process
  whose cmdline is EXACTLY this binary plus the `_play` argument and
  reaps it before any kernel state is touched (exact match only —
  unrelated processes are untouchable; a survivor refuses attach
  with the pid and the fix). `on` runs the same sweep before
  spawning, idempotent from any state, and its already-running check
  keeps its place ahead of the sweep so a live registered player is
  kept, never killed-and-respawned. The liveness probe reads
  /proc/<pid>/stat's state byte because signal 0 alone cannot tell a
  zombie from a live process. The volume lane completed its design
  in the same night: the gate moved from the clap parser (a generic
  range error) into the session layer, where `volume 1000%` refuses
  with the exact boundary message and the `--force-this` escape
  (201..=10000; past the force cap nothing passes), the success
  print names the cap ("volume set to N% (100 = the compiled
  default, cap 200%)"), and the `ZYLAXION_VOLUME` env lane gained
  its absolute-gain ceiling (10.0). The 300s idle line was reframed:
  it teaches the two actions ("type to hear the sound, or
  `zylaxion off` to stop") and hands the tap-liveness fork to
  `zylaxion doctor` — the player's seat cannot prove tap liveness
  (the held ring-buffer fd outlives an unpin, the same inode
  lesson), so the line does not claim it. The sweep test now pins
  the dead service command as absent from the whole living tree.

- **night-badas-4** — the Dragon Guard estate goes green: five red
  checks, four root causes, all fixed. (1) The eBPF tap build failed
  at the structural validation on every run since the job was
  introduced: the validator grepped the ELF section list for
  `tracepoint` — a tap shape this crate never shipped — while the
  night-improve-25 chain plants `fentry/input_event` and
  `kprobe/input_event` sections (the validator's own build step
  succeeded every time; the ~0.1 s "failure" was readelf finding no
  tracepoint section). The validation now demands the real contract:
  the maps and license sections plus a program section on
  `input_event`, each with a named failure message and the section
  dump. (2) The two supermassive E2E legs (the 5.15 floor and the
  latest kernel) died in the payload build before the micro-VM ever
  booted: the job installed only qemu + cpio, and cpal's alsa-sys
  needs `alsa.pc` via pkg-config — `libasound2-dev pkg-config` now
  rides the same apt step. (3) The workflow lint failed on
  actionlint's shellcheck SC2012 in the kernel resolver's module
  listing (`ls` piped into `tr`); the listing now comes from `find`,
  and the two yamllint comment-spacing warnings in crates-io.yml are
  cleaned with it. (4) The world-first claims probe "doctor prints
  the kernel verdict" modeled the doctor's exit code as the KERNEL
  verdict's polarity and failed on every runner: the doctor's actual
  contract is the WHOLE report's health (rc 0 ⇔ "All checks passed",
  rc 1 ⇔ "Some checks failed") — on a runner with no audio device the
  kernel can honestly be "verified supported" while the report exits
  1. The probe now pins the real contract: the summary line matches
  the rc AND the kernel verdict line is present with its PASS/FAIL
  marker agreeing with the verdict string.

- **night-master-4** — the presets stopped being sample-rate-
  dependent. Every decay coefficient in the registry (the main
  envelope, the two-stage fast lane, the ambient rattle envelope) was
  applied raw per sample, so the same preset rang a different length
  per audio device: retention per second is coeff^sr, which made a
  96 kHz device decay voices twice as fast as a 44.1 kHz one. The
  headless render audit of all 24 presets at 44.1/48/96 kHz measured
  the damage — tail spread 54.1% on every single preset
  (technical 426 → 196 ms, ibm_model_m 1237 → 568 ms,
  glass_tactile 2113 → 971 ms, oceanic 1516 → 696 ms): a studio-rate
  user was hearing half of every preset's tail. `init_state` now
  renormalizes each coefficient to the device rate with the
  `c_S = c_R^(R/S)` law — the same law the release ramp (v10.2.0 N2)
  already followed — anchored at the crate's canonical 44.1 kHz
  reference (params::SAMPLE_RATE, the rate every decay sentence in
  the docs is written at). Re-measured after the fix: the spread is
  0.0% on all 24 presets (tails agree within float noise), and at
  44.1 kHz the factor is powf(1.0) = 1.0 — bit-identical to before,
  so the owner's tuning and the whole test tree are untouched. The
  audit also verified, with numbers, what needed no change: zero NaN
  across 24×3 renders, DC offset ≤ 5e-3 (-46 dBFS), the authored
  preset descriptions match their measured tails (whisper 75 ms
  "short", oceanic 1516 ms "long bass sustain", glass_tactile 2113 ms
  "almost musical"), and the 8 loudest presets riding the documented
  unity ceiling engage the designed hard-clamp compressor exactly as
  documented — no gain-structure change, the peak is the design. The
  decay-parameter doc contracts now state the 44.1 kHz authoring
  reference and the normalization law so future presets stay
  rate-honest.

- **night-hunt-1** — the 300-second idle warning spoke for a dead
  architecture. The heartbeat that fires after five minutes without
  key events read "input thread may be stuck — restart it
  (`systemctl --user restart zylaxion`)": every noun in that sentence
  is a fossil. The libinput input thread was deleted with the daemon
  (night-improve-23) — input now arrives from the kernel-side tap
  through the pinned ring buffer, and a wedged input lane cannot hide
  inside a running player (the bridge thread dies loudly: the channel
  disconnects and the render loop exits; its poll errors log their
  own warn). The advice named a systemd user unit the installer no
  longer ships, and the message contradicted its own 60-second debug
  sibling, which already admits idle is normal — a user reading or
  watching for five minutes got a restart order for a healthy player.
  The rewritten messages claim only what this seat can honestly know
  (no keys arrived — idle, or the kernel tap is not delivering) and
  route the real failure to the lanes that can distinguish the two:
  `zylaxion doctor` for the environment verdicts and
  `sudo zylaxion attach` to re-plant a detached tap. The same hunt
  caught the last stale restart instruction in the preset-resolution
  module doc, which described the deleted daemon and a user unit —
  and denied the live preset swap the control-map watcher has applied
  since the player landed. The stale-text sweep found no other
  copies: README, QA, docs and the archive changelog never quoted
  the old wording.

- **night-improve-26** — the parent side of the player wire speaks
  honestly and owns its door. The night's audit found the clap side
  already soldered (the explicit `name = "_play"` pin and its parse
  tests landed with night-improve-25), but two parent-side gaps
  remained. First, both startup-failure strings led with
  `zylaxion _play` — pointing a stranded user at a hidden internal
  verb as the primary diagnostic; they now lead with `zylaxion
  doctor` (the environment verdicts) and mention the internal verb
  only as a secondary, honest pointer to something that exists.
  Second, `zylaxion on` had no root door: root sailed past the
  parent into the player's refusal and met a spawn error instead of
  the session wording. `cmd_on` now refuses root BEFORE any path
  resolution, map open, or spawn, through the pure decision surface
  `on_root_refusal` — the exact mirror of the player-side guard,
  one wording, two doors. The wire is pinned against drift besides:
  the spawn literal becomes the `PLAYER_SPAWN_VERB` constant the
  spawn consumes, and test/commands/sound_tests.rs holds the three
  decision-surface pins (the underscore spelling, the root refusal
  carrying the do-not-use-sudo note, the non-root pass).

- **night-improve-26** — the attach cleanup owns the directory. The
  old cleanup unpinned files but never removed the pin directory:
  an attach that failed after `mkdir` left /sys/fs/bpf/zylaxion
  behind, and the next first-ever attach died with EEXIST at that
  path — on CachyOS (nested bpffs) it surfaced on the owner's live
  host (docs/KERNEL_COMPATIBILITY.md carries the entry). The one
  rmdir that did exist was a silent `let _ =`: every outcome —
  foreign entries and real failures included — vanished without a
  word, and the attach-side self-heal fired only when a pin (not
  the bare directory) was present, so a stale-EMPTY directory
  survived into the next attach. The cleanup is now classified end
  to end: `PinDirRemoval` splits the outcomes (Removed,
  AlreadyAbsent, LeftForeignEntries — ENOTEMPTY reads as
  handled-not-fatal because foreign entries are not ours to remove
  — and Unexpected, which surfaces), attach pre-clean removes the
  stale-empty directory before mkdir and refuses honestly on
  foreign entries, detach reports every outcome instead of
  swallowing it, and the failure marker the doctor reads survives
  the cleanup lanes — read, logged, then deliberately superseded.
  The detach half lives in the new teardown module (loader.rs hit
  the 600-line no-mercy cap and split the theme out); the
  classified core is pinned by four fixture tests in
  test/ebpf/pin_tests.rs.

- **night-improve-25** — the hidden player verb parses again: clap
  derives `play` from the bare `Commands::Play` variant, but every
  call site spells the verb with the underscore (sound.rs spawns
  `.arg("_play")`, both startup-error strings teach `zylaxion _play`,
  player.rs's module doc names it, README's architecture diagram shows
  it — eight references to `_play`, zero to `play`). Before the
  explicit `name = "_play"` pin the spawn argument was a verb clap
  rejects, so `zylaxion on` could never start the player — masked
  until now because the broken attach path (the tracepoint fiction,
  fixed earlier this night) sat upstream of the spawn and the
  failure never got reached. The parse pin in test/cli/ux_tests.rs
  holds both directions: the spawned spelling parses and dispatches,
  and the derived bare spelling stays an unknown subcommand.

- **night-improve-25** — the input tap returns to the plan the
  night-private research doc already approved: a passive kernel-side
  probe on the exported `input_event()` function. The
  night-improve-23 attach path targeted a tracepoint that does not
  exist in mainline Linux; the research doc's fallback claim was
  never verified against kernel source. The primary plan (fentry /
  kprobe on the exported input_event function) was correct and is
  what this revision restores. The evidence chain, for the record:
  there is no `include/trace/events/input.h` at any tag from v6.1
  through master, `TRACE_SYSTEM(input)` matches nothing in the
  tree, `drivers/input/input.c` and `evdev.c` carry zero trace
  calls, and the one patch that ever proposed the tracepoint (the
  WangYuli `input-event` series) never merged. The discovery vector
  was the night-badas-1 polarity fix: once `sudo zylaxion attach`
  could actually reach its lane, every attach on every kernel died
  at the tracepoint-id read — the loader's own error text naming a
  hook the code had invented. The four teaching surfaces that carried the
  fiction (README feature bullet + privacy claim + architecture
  diagram, SECURITY.md, the boot oneshot comment, the doctor's tap
  line) are swept with it; `grep -rn 'input_event tracepoint'`
  now returns zero production hits, and the one place the string
  may appear is the negative help pin that fails the suite if it
  ever teaches the removed wrong path again. One honest correction
  to the record: the research doc's "(5.2+) fallback" claim was
  never verified against mainline kernel source and is corrected
  here — the primary plan was always right.
- **night-badas-1** — the estate green on push and schedule, and the
  one privileged surface actually usable:
  - the `Workflow lint + codespell` lane: the release GPG step's
    `mkdir -p -m 700` split into `mkdir -p` + `chmod 700` (SC2174 —
    with `-p`, `-m` only applies to the deepest directory;
    byte-identical outcome, shellcheck-clean).
  - the `Miri` lane: `tee target/miri-log.txt` opened its output
    before cargo materialized `target/` on a cold cache, so the job
    went red with zero failing tests (143/143 interpreted clean) and
    the failure artifact never existed. A `mkdir -p target` ahead of
    the pipeline returns the exit code to cargo and makes the
    artifact contract honest. The bug was masked until
    night-improve-24 landed the eBPF bootstrap the same job needed.
  - `sudo zylaxion attach` refused root: the privilege gate was
    wired with the negated euid fact — root got "attach must run as
    root" while non-root sailed through into the loader's failure.
    The polarity is corrected to match the sibling gates, the doc
    that described the negated call as the contract is fixed, and
    `attach_requirement` gains its missing test pins (the non-root
    refusal wording, and the root-opens-the-gate polarity).
  - the Supermassive and Dependency Maintenance lanes: both were fed
    the eBPF source-build prerequisites (the canonical bootstrap
    behind the shared `ebpf/target` cache), and the daemon-era
    `libinput-dev` / `libudev-dev` packages leave with them.
  - the Release lane latent gap: the build job never had the eBPF
    prerequisites either — the first tag push after night-improve-23
    would have died in the build script's preflight (the last green
    release, v11.0.0, predates the turn by 70 commits). The same
    bootstrap pair rides the matrix now.
  - `scripts/dev/bootstrap-ebpf.sh` header comment: it still claimed
    the default stable build never needs the prerequisites; since
    night-improve-23 every git-checkout build hard-requires them, and
    the comment now says so.
- **night-improve-24** — the Dragon Guard estate is green again. All
  four red lanes diagnosed on real ground and repaired:
  - the `check` and `Miri` lanes died inside the build script's eBPF
    preflight: the ebpf feature went default in night-improve-23, so
    every git-checkout build source-builds the kernel object and
    hard-requires the dated nightly pin + bpf-linker — neither lane
    had them. Both now run `scripts/dev/bootstrap-ebpf.sh` (the
    canonical installer) behind a shared `ebpf/target` cache keyed on
    the toolchain pin, and drop the daemon-era `libinput-dev` /
    `libudev-dev` packages (only the cpal ALSA headers remain a build
    prerequisite).
  - the `Workflow lint + codespell` lane: eight codespell dictionary
    hits repaired or allow-listed (`retuned` is a false positive on
    the deliberate past tense of retune), yamllint comment-spacing
    compliance across all five workflows, and three release.yml
    shell lines back under the 120-character cap with byte-identical
    behavior.
  - the `Supply-chain CVE scan` lane: RUSTSEC-2026-0293 (ringbuf
    0.4.8 double free / use-after-free in `Consumer::skip`/`clear` on
    a panicking `Drop`). The audio bridge never calls either
    function, so the exposure was the advisory, not an exploitable
    path; ringbuf is bumped to 0.5.3 (>= the 0.5.2 fix), an
    API-compatible move for every call site.
  - `scripts/dev/bootstrap-ebpf.sh`: the python zstandard fallback
    installed the raw tar as the linker binary (the release asset is
    a zstd stream wrapping a POSIX tar; only the `tar --zstd` branch
    unpacked both layers). Reproduced on a zstd-less host; the
    fallback now untars and refuses to stage anything without ELF
    magic.
  - the miri-interpreted suite, once it finally ran, found five real
    test bugs plus one interpreter-physics mismatch, all fixed with
    the evidence in the commit: a tracepoint-id test that read /sys
    (split into the pure path-order + parser contract and the root-run
    IO shell), a pin test statx-ing live bpffs paths (fenced from the
    interpreter lane), an alignment fixture that assumed stack
    alignment by luck (bare `[u8; N]` locals carry alignment 1 — now
    `#[repr(align(8))]`, the exact law the file pins), a wall-clock
    probe (fenced), and a batch-vs-per-sample tolerance that ignored
    the interpreter's non-deterministic libm transcendentals
    (1e-10 -> 1e-4, four orders of magnitude below any real
    divergence, with the tan-on-identical-bits experiment recorded).
- **night-improve-24** — the changes job's detector now matches the
  `.github/` tree too: workflow edits are build surface (the check
  lane carries the eBPF prerequisites) and used to ship unvalidated.

### Changed

- **night-badas-6** — ci.yml's trigger is the native `paths:` filter
  and the detector job is dead. The former `changes` job spent a full
  `fetch-depth: 0` clone on every single push re-implementing — with a
  git-diff regex — what GitHub's native path filter already knows
  (the zelynic NIGHT-improve-13 deletion, landed here). The push and
  PR triggers now carry the estate-wide boost-10 spelling directly:
  `**/*.rs`, `**/*.toml`, `**/Cargo.lock`, `.cargo/**`, `ebpf/**`,
  `scripts/**`, plus the workflow's own file (the hunt-34 law); the
  check and eBPF jobs run ungated (the filter decides), the header
  carries the coverage facts, and the workflow lost the two seats that
  moved to their proper homes the same push (the lint quad to
  gate-keepers.yml — under the path-filtered shape a
  workflow-file-only change matched no glob and skipped the very
  gates that could catch it — and the CVE scan to audit.yml, whose
  weekly schedule claim is now true). A docs-only push now buys zero
  build minutes across the whole estate, with the wholesale gates the
  only lane that runs.

- **night-badas-3** — the README is a total refactor to the zelynic
  masterclass shape, and the stale claims inside it are gone with it.
  The old intro still sold the dead architecture ("transforms every
  keystroke ... using Linux's `evdev` interface" — the evdev capture
  died with the daemon in night-improve-23) and its preset demo block
  taught `zylaxion start`, a verb the CLI has never had; both now
  state the eBPF tap truth and spell `on`. The new structure follows
  the zelynic reference section for section: the "Why zylaxion?" hook
  with a claims table where every edge maps to the mechanism that
  verifies it (the claims battery's probes, the doctor's verdicts,
  the render audit's numbers), the dependency-policy and vs-
  traditional-tools tables (the class rows honestly labeled as the
  published shape of those tools, not benchmarks), the reordered
  install lanes (release first, then crates.io with the ebpf-prebuilt
  story, the one-command machine installer, then manual source), an
  honest Limitations section (root-once-per-boot, the dormant musl
  lane, one engine for every keyboard, CI has no ears — plus the
  nightly-toolchain quarantine story), the IS / will-NEVER-be
  philosophy lists, the branches table, Contributing (CLA + DCO),
  the private security-reporting pointer, the documentation shelf
  under the master index, the Test Results harness table with the
  world-first claims battery spelled out, and the ZYLAXION-DISCLAIMER
  documentation-disclaimer block. The trademark section was rewritten
  to say what TRADEMARK.md actually says — every fork must rename and
  rebrand (the zelynic two-category fork policy does not exist here;
  the draft that claimed it was corrected against the real §4 before
  landing). Verified: all 20 linked files exist, codespell clean,
  gate-keepers fully green (the name-case rule included).

- **night-badas-2** — the supermassive deep battery joined the push
  estate and its weekly clock is retired (the owner's call, the
  zelynic NIGHT-reject-1 lineage). The workflow that deliberately sat
  off-push ("the push estate is ci.yml's job") now triggers on every
  push to main that touches the surfaces the battery can prove or
  break, spelled the zelynic boost-10 way: the shared surface globs
  (`**/*.rs`, `**/*.toml`, `**/Cargo.lock`, `.cargo/**`, `ebpf/**`,
  `scripts/**`) plus the workflow's own file — a workflow that can
  change its own contract owes itself a run. `ebpf/**` rides the
  shared spelling because the tap source is this repo's core kernel
  surface and every battery leg source-builds the object; a
  docs-only commit matches nothing, so the expensive KVM legs never
  buy a typo in the README. `workflow_dispatch` stays for the
  on-demand deep run. The concurrency group moves to the zelynic
  spelling (`${{ github.workflow }}-${{ github.ref }}`), so the
  group name rides the Dragon Guard family name instead of a private
  prefix. Estate-wide name audit against the zelynic reference: all
  six zylaxion workflow titles were already byte-identical to their
  zelynic counterparts (`Dragon Guard - CI / Supermassive / Miri /
  Release / Dependency Maintenance / crates.io publish`) — the one
  stale doc was docs/RULES.md, which still mandated the pre-lineage
  `"Maintenance deps weekly"` title and a prose CI filter; both now
  state the Dragon Guard family law and the shared glob spelling.

- **night-improve-25** — the attach chain is fentry -> kprobe ->
  kprobe-alt, three steps, no tracepoint lane at all. The tap runs
  as two programs over one body in the object (`fentry/input_event`,
  `kprobe/input_event`; the alternate symbol is an attach-time
  parameter on the same kprobe program), the argument extraction
  lives in one shim both crates compile (`src/ebpf/args.rs`, the
  userspace crate owns the file, the ebpf crate includes it via
  `#[path]` — one source of truth, tested where tests can run).
  The loader walks the chain and records the winning step in the
  control map's new TAP_KIND slot; `status` and the doctor report
  it ("tap: attached via fentry:input_event"), and a failed attach
  is a classified error that cleans every pin before returning —
  never a half-attached state. Attach on a complete pin set is now
  the success shape "already attached — nothing to do" (the
  double-attach idempotency contract). The capability probe
  pre-verifies the same chain from /sys facts (BTF token search,
  kprobe blacklist) and renders one of four canonical verdict
  strings the doctor prints verbatim; the doctor's kernel section
  also lost its [2/5]-among-[x/4] numbering bug.
- **night-improve-24** — the CLI surface revision, three owner
  verdicts on the flag grammar:
  - `--live` dies totally. The 2-second alternate-screen refresh
    loop was the wrong shape for a status surface — a signal
    handler, a cursor hider, and a terminal restore protocol
    attached to what should be a static read. Six surfaces carried
    the flag (the Cli field, the dispatch branch, the 52-line live
    loop, the top-level rescue table, the help reference, the
    README usage line); all six are cleaned, and `zylaxion --live`
    now answers with the honest unknown-argument error, exit 2.
  - `stats` and `status` are one verb: `zylaxion status
    [--verbose]`. The verbose flag appends the kernel-side typing
    telemetry (the merged stats surface, the zelynic `-v`
    vocabulary) to the one-screen truth — a static print-once deep
    read, never a live refresh. The telemetry renderer keeps its own
    module per the 600-LOC cap, and a section of a delivered report
    warns where the standalone verb hard-exited (the map was the
    verb's whole purpose then; as a section it is a gap to name).
    The stale-teaching pin is new law: the reference must not teach
    a dead verb or a dead flag, and the typo-rescue test token
    moved from the merged `startt` (a stats typo, now singular-tip)
    to `presets` (rescues both `preset` and `list-presets`, keeping
    the plural-engine contract).
  - `preset --name <NAME>` becomes `preset --switch <NAME>` (short
    `-n` becomes `-s`): the verb switches, the flag names the
    action, not the value's type. The rename hunt also found the
    curated help teaching a dead example — `tpt` was never in the
    registry (the author had reached for the TPT waveshaping
    module's name); every teaching surface now says `--switch
    topre`, which smoke-runs through the registry into the honest
    tap-not-attached verdict instead of a not-found error.

### Added

- **night-badas-5** — the Dragon Guard estate gains the two zelynic
  reference lanes it was missing, and the E2E legs take the reference
  naming. `Dragon Guard - Gate-keepers` (gate-keepers.yml) runs the
  wholesale non-code gate on EVERY push and PR, unfiltered: the nine
  gate-keepers.sh sections (permissions, SPDX headers, the Rust and
  scripts LOC caps, language discipline, the emoji sweep, the actions
  pins health, release parity, the name-case rule) plus the workflow
  lint quad (actionlint 1.7.7, yamllint 1.38.0, codespell 2.4.3,
  pinned). The doctrine is the zelynic NIGHT-improve-13 law: a
  docs-only change matches nothing in the path-filtered build estate,
  and a docs-only change is exactly the class of commit that can break
  the gates that police docs — so the wholesale lane never sleeps, at
  zero build minutes (bash, git, python3 and four pinned lint tools;
  no Rust toolchain, no compile). `Dragon Guard - Security Audit`
  (audit.yml) takes the cargo-audit scan out of ci.yml into its own
  scanner workflow under the zelynic NIGHT-improve-11 scanner rule —
  a scanner carries the surface it scans (`**/*.toml`,
  `**/Cargo.lock`, its own file) plus the weekly Monday 07:00 UTC
  advisory-DB clock; the scan is now actually weekly, which the old
  ci.yml job only claimed to be (its workflow had no schedule
  trigger). The supermassive E2E legs ride the zelynic low/best specs
  naming: `supermassive test - low specs (the 5.15 documented floor)`
  and `supermassive test - best specs (the archive's head)` — no musl
  twins, the musl lane being the documented dormant lane.

- **night-master-1** — the world-first claims battery: the
  extraordinary claims get their own independent, statically-verifiable
  proof lane (`scripts/supermassive/sm_static.py`), wired into both
  supermassive lanes and given its own rootless CI job. The battery
  proves, with numbers and bytes rather than adjectives:
  - **pure math** — beyond the existing RIFF/WAVE scan, the binary's
    ELF64 DT_NEEDED table is parsed by a hand-rolled stdlib parser (no
    readelf/ldd dependency — the micro-VM guest has neither) and must
    link zero audio decoders: without sndfile/avcodec/mpg123/vorbis/
    FLAC/opus and their kin in the process image, a sample physically
    cannot be played — synthesis is the only path the binary owns. The
    positive control cross-checks `list-presets` against the compiled
    registry source: the binary declares exactly the 24 authored
    presets, byte-for-byte, no drift.
  - **eBPF, embedded** — the binary is scanned for ELF magic with
    e_machine 247: a real BPF object ships inside the image, and the
    `input_event` target string rides with it (the fentry/kprobe chain
    the tap attaches through); `ebpf-prebuilt/`'s staged object is
    verified to be the same species and to carry zero audio magic.
    Against a dormant binary the probe reports an honest SKIP; the
    full lane (which always holds the release shape) makes absence
    fatal.
  - **no daemon, no bloat config** — the binary carries zero
    daemon-era strings: no config.toml, no pidfile, no socket, no
    libinput, and — the night-hunt-1 regression guard — no stale
    `systemctl --user restart` advice; Cargo.lock is swept
    transitively for the whole deleted stack (toml, notify, daemonize,
    dbus/zbus, libinput, the xdg/directories config families): 135
    locked crates, zero hits. The pin surface
    (`/sys/fs/bpf/zylaxion`) remains the only state the architecture
    owns.
  The engine's `--self-test` lane and its docstring now name the
  claims battery, and `.github/workflows/supermassive.yml` grows the
  `claims` job: the exact release shape built with the same cache +
  bootstrap pair as the release-shape job, the rootless battery run
  against it, the MASS-RESULT ledger relayed into the step summary.
  The battery is pure stdlib on purpose — the same code runs rootless
  on a runner, inside the KVM guest, and on the owner's bench.

- **night-master-3** — the supermassive E2E: the whole zero-to-hero
  chain on the KVM micro-VM floor, the zelynic lineage's crown job
  ported to zylaxion's claims. The engine
  (`scripts/supermassive/supermassive-test.py` plus the sm_* module
  family and the bash wrapper) drives two lanes: `--self-test`, the
  rootless lane (the CLI/UX battery — identity, discovery, the fatal
  input matrix with exit-code pins, the tap-less doors, the root-door
  polarity — plus the pure-math proof: zero RIFF/WAVE bytes in the
  binary, zero audio assets in the tree; and the zero-config proof:
  a pristine HOME/XDG run writes nothing anywhere), and `--full`, the
  root lane the guest init calls: environment probes, the tap
  lifecycle (attach with the pin handover to a real user, the
  complete pin layout, the live re-attach refusal, the
  foreign-entry-keeping detach, the idempotent re-detach, five rapid
  attach/detach cycles, the zero-residue proof), and the crown — the
  real-usage chain: a user with no input group and no /dev/input
  access turns the sound on, a uinput virtual keyboard types through
  the kernel's own input path, `status --verbose` counts the presses
  the tap accumulated kernel-side, the PCM state under /proc/asound
  witnesses the synthesizer rendering real frames, live preset and
  volume switches ride the pinned maps as the unprivileged user, `off`
  self-exits the player to a zero-process state, and the detached
  `on` closes the loop at the tap-less door. Every probe lands on one
  ledger that prints a human row and the MASS-RESULT sentinel, one
  final MASS-VERDICT line is the CI relay. `scripts/ci/supermassive-init.sh`
  is the guest's PID 1 (the zelynic kernel-floor contract: the mounts,
  the staged module chain from the guest kernel's own debs —
  snd-dummy is the PCM witness's card, uinput the injector's kernel
  half — the install step landing the payload at /usr/bin the way
  install-on-my-machine.sh does, the engine, the isa-debug-exit).
  `.github/workflows/supermassive.yml` grows the e2e-vm job: the
  release-shape payload built on the runner, the guest kernel
  resolved per leg (the documented 5.15 floor from jammy's pockets,
  the newest kernel the runner's own distro serves), an
  ubuntu:24.04 initramfs rootfs, and a dynamic envelope (every core,
  three quarters of the RAM) — two legs, one battery, the verdict
  grepped off the serial console. The self-test lane ran green in
  the build sandbox (26 probes, 1 honest skip); the VM legs ride CI.

- **night-revert-1** — the crates.io publish lane, mature CI and all.
  The owner's verdict on night-simple-1 landed plainly: removing the
  registry lane was a mistake — crates.io is a fully supported
  distribution channel, and the zelynic family already runs the
  mature reference CI for it. The reversal restores everything the
  removal touched (the `documentation`, `keywords` and `categories`
  manifest fields, the `registry-prebuilt` lane naming in
  build/ebpf.rs with its `cargo install zylaxion` panic remedy, the
  registry-tarball narrative in build.rs, loader.rs and
  bootstrap-ebpf.sh) and then grows what the tree never had: a
  `v*`-tag-triggered `Dragon Guard - crates.io publish` workflow —
  the commit+tag collision gate (scripts/release/wait-for-ci.sh,
  ported from the zelynic reference: it serializes the publish behind
  the ci.yml push-run on the exact tagged SHA, with the infra-failure
  carve-out that buys automatic re-runs for GitHub-side faults and
  never mistakes them for a code verdict), the tag/version match
  check, the CRATES_IO_TOKEN presence check, the already-published
  idempotency probe (a re-pushed tag skips instead of failing), and
  the `--locked` upload whose verification build exercises exactly
  the registry-prebuilt lane a `cargo install zylaxion` consumer
  rides (the packaged tree carries no ebpf/ workspace, so build.rs
  stages the maintainer-built objects from ebpf-prebuilt/ — no
  nightly anywhere near the user).

- **night-improve-26** — the kernel compatibility record:
  docs/KERNEL_COMPATIBILITY.md. The kernel floor the tap requires
  (5.15, with the honest doctor verdict either way) plus one entry
  per distro layout the tap actually meets, each carrying its
  evidence — opened with the CachyOS nested-bpffs case: the layout
  (`findmnt -t bpf` shows bpf-on-bpf), the symptom (the first-ever
  attach's EEXIST at /sys/fs/bpf/zylaxion), the classified-cleanup
  fix, and the provenance chain from the live 6.18.50 host (the
  EEXIST, the detach clearing the leftover, the clean attach,
  `bpftool prog show` reporting `prog_type tracing` on the BTF id
  of `input_event`). A resolved entry is RESOLVED — not worked
  around: the fix lives in the code, so nobody re-litigates it as
  a zylaxion bug later. Indexed from docs/README.md.

- **night-master-2** — the one-command machine installer,
  `install-on-my-machine.sh` at the repo root. Linux only, no flags:
  it reads the CPU's x86-64 microarchitecture level from /proc/cpuinfo
  (x86-64-v4 when the full AVX-512 set is present, x86-64-v3 for the
  AVX2/FMA/BMI class, baseline otherwise), builds the matching shape
  through the parity-checked cargo aliases (`pro-linux-amd64-v4-gnu`,
  `pro-linux-amd64-v3-gnu`, plain `--release` fallback for baseline or
  non-amd64 hosts), then installs the binary to `/usr/bin/zylaxion`
  and the boot oneshot to `/etc/systemd/system/zylaxion-attach.service`.
  The same commit collapses `scripts/package/install.sh` to the
  system-only posture — before: `--user` (default, `~/.local/bin`, no
  unit) versus `--system` (`/usr/bin` plus the unit); after: one
  posture, `/usr/bin` plus the unit, no flags. Both installers clean a
  stale `~/.local/bin/zylaxion` first so nothing shadows the system
  binary. `scripts/package/uninstall.sh` keeps its user-path sweep as
  legacy cleanup for installs made before this change, and README plus
  docs/RULES.md describe the new posture (the release-binary download
  example now lands `/usr/bin` too).

- **night-improve-25** — the Pro mode section on the one help
  reference (the zelynic NIGHT-improve-45 lineage): every hidden and
  advanced spelling documented COMPLETELY, grouped by the lane that
  owns it — the three short aliases (`-s, --switch <NAME>`,
  `-p, --percent <0-100>`, `-v, --verbose`) with their runnable
  examples (the short spellings are new to the surface; the Run
  section keeps the long ones, the no-duplicate law), the four
  environment spellings of the no-config law (`ZYLAXION_PRESET`,
  `ZYLAXION_VOLUME` with its linear-scale warning — 5.5 laptop
  speakers, 1.5 headphones, a different unit than the percent lane —
  `NO_COLOR=1`, `RUST_LOG` with the log lane's split defaults), and
  the one hidden verb (`zylaxion _play`, the internal stateless
  player, with the foreground-debug teaching its own error strings
  already hand users). One computed description column across every
  group and its continuations (the hunt-35 geometry law, adapted:
  the column derives from the estate's longest spelling and cannot
  drift per row); tier law holds — flags and the verb synopsis grey,
  environment rows default prose (an environment variable is not a
  flag, the hunt-39 boundary), examples green. The drift pin in
  test/commands/help_tests.rs fails the suite if any spelling silently
  thins back to hidden, and its negative arm fails it if the bare
  `play` spelling is ever taught as a verb. README's usage section
  gains the pointer paragraph; the help demo screenshot refresh
  follows at the next owner-host capture session.
- **night-improve-25** — the classified attach failure surface:
  the `AttachError` taxonomy (nine locked variants —
  FentryUnavailable, KprobeBlacklisted, SymbolMissing,
  VerifierRejected, PermissionDenied, BpffsUnavailable, PinFailed,
  LockdownActive, Unknown), pure errno classifiers pinned by the
  test matrix; the failure marker, a one-entry array map planted
  on bpffs by a failed attach carrying the variant's stable code,
  which the doctor reads back to name the last failure after the
  process is gone; the negative help pin against the tracepoint
  fiction's teaching string (the drift gate the Fixed entry
  names); and the probe's four
  canonical input-tap verdict strings. The prebuilt-object
  manifest's tree-hash recipe is documented and automated
  (`scripts/dev/regenerate-ebpf-prebuilt.sh`) — the value
  committed by night-improve-23 was generated by an unrecorded
  command and could not be reproduced from history.
- **night-improve-24** — the kernel-side CI lane: `eBPF tap build`
  proves the detached ebpf/ workspace standalone on its own dated
  nightly — `cargo fmt --check` inside ebpf/, the locked
  `bpfel-unknown-none` release build with `-Z build-std=core` through
  bpf-linker, and ELF structural validation of the object (real ELF
  magic; the tracepoint, maps and license sections aya's loader
  demands). No system packages: aya-ebpf is pure Rust. The check lane
  remains the embedding half's proof; this lane is the kernel half's.

### Removed

- **night-improve-23** — the daemon dies. The entire POSIX daemonization
  stack is deleted: fork/setsid with the init-sync pipe, the PID file
  (with its /proc/exe identity forensics), the Unix-domain-socket IPC
  layer (the JSON protocol, the poll-thread hardening, the umask dance),
  the flock instance lock, and the libinput seat capture (the
  `input` crate with it, plus the `input` group requirement). The two
  libinput-era examples (listen_keys, zylaxion_live) retire with their
  subject. serde and serde_json leave the dependency table — there is
  no protocol left to serialize. Eight of the eleven v10.2.0 audit
  findings (B4, B6, B7, B9, B10, B17, S1, S2, P7) were daemon-induced;
  those bug classes are structurally impossible now.

### Added

- **night-improve-23** — the daemonless eBPF control plane, the world
  first: a keyboard-sound tool whose input tap lives in the kernel, whose
  state store is pinned BPF maps, and whose player is a stateless,
  own-nothing process.
  - `ebpf/` — the pure-Rust BPF crate (aya-ebpf 0.2.1, detached
    workspace, dated nightly pin, repo-local bpfel target spec with
    atomic-cas): one tracepoint program (`zylaxion-tap`) on
    `input:input_event` that gates against pinned maps and publishes
    press/release records to a ring buffer. Autorepeat and mouse
    buttons filter in the kernel (documented deltas).
  - `src/ebpf/` — the userspace loader: `attach` (load, raw-syscall
    link pin, control-plane init, pin everything, chown to the sudo
    user), `detach` (unpin everything — afterwards NOTHING of zylaxion
    exists in the kernel), and the aligned embedding (the zelynic
    NIGHT-hunt-30 AlignedElf lesson).
  - build.rs — three object lanes: source-built (nested nightly
    cross-compile with the prerequisite preflight, RUSTFLAGS poison
    strip, v3 force, structural ELF validation, self-heal),
    registry-prebuilt (ebpf-prebuilt/ + provenance manifest), and
    dormant (`--no-default-features` builds a binary whose eBPF
    surfaces answer with the honest refusal).
  - `src/commands/sound.rs` — the CLI verbs: `on` (write the control
    plane, spawn the player), `off` (clear the gate — the player
    self-exits within 100 ms, zero processes remain), `preset` /
    `volume` (live map writes), `stats` (kernel-side per-key telemetry
    accumulated as BPF_ATOMIC fetch-adds at zero userspace cost),
    `status` (kernel verdict + tap residency + player state).
  - `src/commands/player.rs` — the hidden `_play` internal verb: the
    stateless player that reads every byte of state from the pinned
    maps, registers its pid in the control map (the pidfile lives in
    the kernel now), drains the ring buffer through an epoll bridge,
    and applies preset/volume changes live (the ArcSwap model swap and
    the new atomic volume cell in the voice pool).
  - `src/session/kernel.rs` — the kernel capability probe: release
    parsing, bpffs mount check, BTF presence, tracepoint visibility,
    lockdown mode, and the canonical verdicts against the 5.15 owner
    minimum ("verified supported" / "verified unsupported") wired into
    the doctor and the attach gate.
  - assets/zylaxion-attach.service — the boot ONESHOT (Type=oneshot,
    not a daemon) that plants the tap once per boot; bpffs pins do not
    survive a reboot, the oneshot replants them.
  - scripts/dev/bootstrap-ebpf.sh — the host bootstrap for developing
    the tap (dated nightly + the pinned bpf-linker prebuilt).
  - packaging — install.sh/uninstall.sh teach the oneshot posture and
    sweep the kernel side (best-effort `detach`) on removal.

### Changed

- **night-improve-23** — the CLI surface speaks the daemonless world:
  `start`/`daemon`/`stop` are gone; `on`/`off`/`preset`/`volume`/
  `stats`/`status` are one-map-write verbs that live under 5 ms. The
  doctor reports the kernel/eBPF verdict instead of the input group
  (the group died with the seat capture — the passive tap needs no
  device permissions). The volume retune rides a new atomic cell in
  the voice pool (`set_master_volume` takes `&self` now, wired through
  the orchestrator's `volume_factor` handle). nix trims to
  signal/process/user/poll; libc survives solely for the raw
  BPF/perf syscall lane in src/ebpf/bpf_syscall.rs (aya's typed
  TracePointLink wraps a pub(crate) inner, so its link cannot be
  pinned through the public API — the zelynic bpf_syscall precedent).

### Removed


- **night-improve-22** — the no-config law. `config.toml` (1672 lines)
  and its entire support stack are deleted: the session search-path
  module (src/session/config/), the TOML parse/validate surface, the
  mtime config-watcher thread (a 1-second filesystem poller), the
  `testconf` subcommand, the per-key `[[keys]]` override machinery
  (KeyOverride, the Override* field-patch structs, ProfileWithOverrides,
  MechanicalClick::with_overrides), the MasterParams [master]-table
  carrier, and the `toml` dependency (the lock graph sheds the
  toml/toml_edit/toml_write/serde_spanned branch; serde survives solely
  for the daemon IPC protocol). Owner's verdict: hard to maintenance,
  low gains investment, absurd for keyboard sound.

### Added

- **night-improve-22** — the masterclass replacement, the compiled
  preset registry. All 24 acoustic presets are now compile-time Rust
  data in src/profiles/registry.rs, transcribed byte-for-byte from the
  retired config.toml and pinned in-range by a new test tree (each
  value must survive validate_and_clamp with zero drift — the old
  testconf guarantee, enforced at test time on typed data). Preset
  selection is the Unix configuration surface: `--preset <name>` on
  the CLI > `ZYLAXION_PRESET` in the environment > the compiled
  default (`technical`, unchanged as the known-good fallback). The
  master output volume follows the same law via `ZYLAXION_VOLUME`
  (default 5.5; invalid values are hard errors — the no-silent-
  fallback contract survives). A preset is chosen once at process
  start; switching is a daemon restart — the honest lifecycle for a
  sound switcher. `list-presets` renders the catalog with one-line
  descriptions; unknown names keep the GNU-shaped error with the
  did-you-mean tip; the systemd unit gains a commented
  `Environment=` preset pin. install.sh ships two artifacts instead
  of three; uninstall.sh cleans legacy config-era directories.
  Net tree delta: about 900 lines deleted, 6 dependencies shed.
  Suite: 154 tests green (five consecutive runs), clippy
  --all-targets -D warnings clean, version untouched at 11.0.0.

### Changed

- **night-improve-21** (39586b4): the cosmic dragon unification — the
  six-crate workspace (zactrix-engine, zactrix-profiles, zylaxion-core,
  zylaxion-input, zylaxion-output, zylaxion) becomes ONE package at the
  repo root, the cosmic dragon engine and architecture. The former
  crate boundaries survive as module boundaries: src/engine/ (the
  voice pool), src/profiles/ (the acoustic model), src/core/ (the
  Orchestrator spine, the error contract split into src/core/error/
  under the no-mercy 600-LOC cap), src/input/ (libinput capture),
  src/audio/ (the cpal sink and the SPSC ring bridge), and the shell
  modules (cli, commands, daemon, info, output, session) unchanged.
  The Pattern-C test tree consolidated under the root test/ with every
  #[path] wiring remapped; the five vetted examples/ targets moved
  with module includes and keep their autoexamples-ON status. The
  dependency table is the exact union of the six former manifests,
  every version untouched (11.0.0 lineage, owner-decided bumps);
  cross-crate imports rewired to crate:: paths (the bare `input`
  extern carries a leading :: so the input module and the libinput
  binding never collide). check-loc.sh retires the per-crate loop
  with the workspace; the single-file policy now has one subject. Dead
  re-export bridges retired; unwired API-complete items carry scoped
  allow/expect markers with night-improve-21 reasons. Live docs
  refreshed tree-wide (README, RULES, GLOSSARY, BRANDING,
  CONTRIBUTING, QA, SECURITY, NOTICE, config.toml, release.yml); the
  frozen historical records stay untouched.

- **night-improve-20** (bb23bfb): the lowercase law enforced — the
  zelynic NIGHT-dinner-22 lineage, ported whole. New
  `scripts/gates/check-name-case.py` (gate-keepers section 9) scans
  EVERY tracked file and every tracked path (git ls-files, the .cargo/
  and .github/ hidden trees included, nothing excluded) and fails on
  any casing outside the two legal families: lowercase, and the
  identifier family (ZYLAXION_* env vars and build stamps, plus the
  all-caps banner titles heading the scripts/ gate files). The tree
  re-cased: 44 wrong-case occurrences across 23 files — README, RULES,
  BRANDING, the v6.0.0-era frozen records included (the law excludes
  nothing), the three crate descriptions, the systemd unit
  description, and the instance-lock error string (sentence-start
  lowercase is the law). docs/BRANDING.md grew the full Name Usage
  codification (correct form table, identifier-family rule, incorrect
  forms, the machine-enforcement pointer). 92 files, 671 name tokens,
  0 violations.

- **night-improve-19** (83ea734): the UX bridge — every fatal parse
  error now renders through one authority: brand-styled clap render
  through the shared capability gate (one color law), the
  case-insensitive typo rescue behind clap's own engine, exactly one
  canonical "For more information, try '--help'." footer (clap cannot
  append it — zylaxion's --help is a parsed field per the single-tier
  law), exit 2. The rescue ladder: clap's tip stands; then the
  top-level authority table (help/V/check-update/live typed at
  subcommand position tip the exact top-level spelling); then the
  case-insensitive Jaro fallback over the FAILING command's flag pool
  — argv walks to the subcommand the user is actually in, so
  `zylaxion start --PRESET` rescues `--preset` where the reference's
  root-only pool would stay silent. New `cli/ux.rs` + `cli/suggestion.rs`
  (Levenshtein value matcher, both sides case-folded for the mixed-case
  preset vocabulary — `cherymx` tips `cherryMX` — and the Jaro flag
  matcher); the two duplicated preset-not-found sites deduplicated into
  one constructor carrying the did-you-mean tip. Live-proven on all
  five surfaces; 6 bridge pins, 7 engine pins, 2 config pins.

- **night-improve-18** (4deabb2): the no-mercy era — the zelynic
  structure discipline completed. The LOC gate law rewritten: 600
  lines for every tracked .rs anywhere in the tree, NO marker (the
  `// LOC_EXEMPT:` debt lane retired), over the cap means SPLIT the
  same hour; beside the cap the gate enforces the layout law (every
  crate's src/ root holds exactly its entry file, main.rs / lib.rs,
  everything else in dir/mod.rs subsystem dirs). The Pattern C test
  tree: every *.rs test file lives under its crate's test/ directory
  (19 files, #[path]-wired, one theme per file) — module positions
  unchanged so super::* still resolves, 150 tests, the exact
  pre-move count. The six-marker debt ledger is PAID (max file 587):
  zactrix-profiles/src/lib.rs (1641) into params/, model/, profile/,
  overrides/ + a wiring-only lib.rs with crate-root re-exports;
  mechanical.rs (1337) into mechanical/{mod,render}.rs; config.rs
  (980) into config/{mod,paths}.rs; zylaxion-core/lib.rs (981) to 587;
  zactrix-engine pool.rs (918) to 297 + voice/ dir; daemon.rs (679)
  into daemon/{mod,watcher}.rs. Docs refreshed (RULES.md's stale
  800-LOC/2000-LOC claims replaced with the live law, QA.md Q4, the
  glossary marker entry retired, MAINTENANCE ledger 1 paid).

- **night-improve-17** (58331f1): the masterclass README — the zelynic
  consistency pass. Stability section: the Stable API contract (from
  v11.0.0, the CLI surface frozen — commands, flags, output format)
  plus the release-channels law (dev/nightly/alpha/beta/rc, CI rejects
  any other suffix). Support section: Ko-fi plus the crypto-donations
  lane with the three owner-verified receive addresses and the
  network-mismatch warning. Commercial Licensing section: the ladder
  at the owner's prices — Business $2,199/year (individual businesses
  and small teams), Company $9,199/year (companies and enterprise, or
  redistribution rights); Personal free, Individual holds at
  $99/year — with the rationale (a masterpiece whose value increases
  year by year; the price tracks the value; grandfathering protects
  earlier buyers) and the unauthorized-use warning in the owner's own
  wording: stealing, illegal forking, or building closed-source
  commercial products without first contacting the owner
  (rezky_nightky / oxyzenQ) and paying the license is the mark of the
  worst kind of human — a rotten one; the honest paths are exactly
  two. COMMERCIAL_LICENSE.md re-priced with the three owner-verified
  QR codes (assets/qr/, the same verified set) beside their payment
  addresses.

- **night-improve-16** (9c61e03): the privilege guard and the root-usage
  audit — the zelynic NIGHT-hunt-11 contract lands verbatim:
  `--check-update` under euid 0 refuses with the "do not use sudo" verdict
  and a re-run-without-sudo tip BEFORE any network I/O (exit 1). The total
  audit (the verdict table lives in the new module's docs): `start` /
  `daemon` refuse too, before the instance lock and before the fork —
  acquiring the lock as root IS the pollution event (root-owned
  lock/socket/PID files inside the user's runtime dir break the next
  user-mode run), and a root daemon grabs input system-wide in a session
  no user desktop shares. `doctor` stays reachable but warns its verdicts
  describe root's session. `stop` / `status` deliberately unguarded (the
  rescue path). The new `session::privilege` module (after the
  night-improve-14 folders) carries pure, unit-pinned refusal wordings
  (4 drift-pin tests) and `euid_is_root()` via nix.

- **night-improve-15** (8c87d18): the single help reference and the
  verbose verdict — the owner's suspicion audited and confirmed:
  `-v/--verbose` was a gimmick (clap field parsed and never consumed, the
  real work an argv peek setting RUST_LOG=debug through an unsafe
  set_var, and only 3 debug!/trace! sites in the whole engine), so it is
  removed, not imitated; `cmd_start` logs at info directly and RUST_LOG
  stays the power-user door. The help surface takes the zelynic
  single-tier law: clap's auto help and `help` subcommand disabled, one
  curated end-to-end reference (purple group headings, grey synopses and
  flag spellings, green annotated example pairs) dispatched from a real
  `-h/--help` field; subcommand-level `--help` is now an
  unknown-argument error (exit 2) and `start --check-update` no longer
  hijacks into an update check. Three drift-pin tests hold the reference
  to the command surface.

- **night-improve-14** (5f6e40a): the zelynic foundation shape — src/
  root holds exactly one file (`main.rs`, bootstrap + dispatch; the
  audit found ten loose files there). Every concern moved to its folder:
  `cli/` (Cli + Commands + version report + clap styles), `output/`
  (the color/lane system, the former error_format.rs), `info/` (build
  facts + the Hinnant clock), `session/` (config, instance lock,
  pathguard, privilege, signals — the session contract); `commands/` and
  `daemon/` unchanged. About 94 module-path references rewritten, the
  `include_str!` manifest depth re-pinned, and the docs (QA.md,
  BRANDING.md) re-pointed at the new homes.

- **night-improve-12** (the mature-infra depth audit, five landing
  commits): the zelynic estate ported and adapted —
  - **12a (06b0ea6)**: the dual-license estate — COMMERCIAL_LICENSE.md
    (the four-tier ladder at the owner's current prices, the
    owner-verified crypto addresses), NOTICE, SECURITY.md (the
    input-data discipline: no key content persists, no network in the
    audio path, a 0o600 IPC socket speaking only documented verbs),
    CLA.md, CONTRIBUTING.md, and the QA record seeded with
    session-verified facts.
  - **12b (c10e9c1)**: the gate-keepers — six gates under scripts/gates/
    (check-loc 600-line law with the honest LOC_EXEMPT debt ledger,
    check-scripts-loc 1000-line cap, check-permissions the exact
    644/755 contract with shebang parity, check-headers, the
    check-language English-only sweep with the NON_LATIN_FIXTURE
    marker, the inline emoji sweep) plus gate-keepers.sh, every section
    a hard gate. The cleanups the gates demanded: 16 files gained the
    license header, the doctor switched to word verdicts (PASS:/FAIL:),
    the ohm unit ASCII-ized, the OS line added to the pre-existing
    scripts. The permission gate proved itself on run one: the umask-002
    worktree drift across 70+ files caught and --fix restored.
  - **12c (fd585ef)**: the arch-baseline release lane — Dragon Guard -
    Release with the channel/version contracts, the x86-64-v3/v4 gnu
    matrix, embedded-label verification (v3 executed, v4 byte-grepped),
    the flat tarball invariant, three checksum families (SHA-512,
    BLAKE2b-512, SHAKE256 quantum-resistant), the GPG detached-
    signature lane with passphrase confinement, the dist/ inventory
    tripwire; the pro-linux-amd64-v3-gnu / v4-gnu aliases + profiles
    with live-proofed local builds, and the release-parity gate
    (section 8). The musl legs are a documented dormant lane with the
    session's empirical evidence: static musl alsa-lib proven buildable
    end to end, the real blocker libudev-sys/libinput, unlock paths
    recorded for the owner's call.
  - **12d (ee851c8)**: the miri UB lane (Dragon Guard - Miri — the
    workspace suite under the nightly interpreter, weekly Sunday plus
    path filters) and the family rename — every workflow now carries
    the Dragon Guard name.
  - **12e (1fd74f5)**: the docs structure — the map (docs/README.md),
    the glossary (docs/GLOSSARY.md), the maintainer handbook
    (docs/MAINTENANCE.md).

- **night-improve-13** (0fd7cc4): the Hinnant clock and the zelynic
  version-report contract — env_logger stripped to `auto-color` (its
  `humantime` feature has mapped to dep:jiff since 0.11.8, and the
  default regex filter chain went with it): eight crates left the lock
  outright (jiff ×3, defmt ×3, thiserror ×2) plus the regex tree, and
  zylaxion now carries no time crate and no regex crate anywhere. The
  wall-clock needs answer through Howard Hinnant's civil_from_days
  instead: the new clock.rs renders the daemon log stamps (same
  millisecond UTC shape, journal greps keep their contract), build.rs
  stamps `ZYLAXION_BUILD_TIME` plus the rustc/profile/opt-level facts,
  and the new build_info lane feeds the version report and the doctor.
  `-V`/`--version` now speaks the zelynic report shape — brand header
  (purple on a TTY via the new stdout lanes), `Build:` /
  `Build-time:` / `Copyright:` / `License:` / `Source:` — with one
  body shared between the first-arg intercept and clap's own path; the
  doctor leads with the `[1/4] Build facts` block (Version, Commit,
  Variant, Built, Rustc, Profile with the manifest drift-guard test).

- **night-improve-11**: cargo aliases land in the zelynic usage shape —
  `.cargo/config.toml` (now tracked; the local-only ignore rule retired)
  carries `cargo pro-native-gnu` (host-CPU native build via its own profile)
  and `cargo pro-linux-amd64-gnu` (the exact release workflow contract).
  Honest deltas documented in place: no musl blocks (the ALSA constraint)
  and no build-level rustflags (release already pins codegen-units).

- **night-improve-10**: masterclass package data — the workspace author row
  adopts the zelynic obfuscated-contact format, and all six crates carry
  explicit auto-discovery controls (a stray tests/, benches/ or examples/
  directory can never silently become an unvetted target; the four crates
  with real examples/ keep that discovery ON deliberately).

- **night-improve-9**: `version-to.sh` inherits the zelynic abilities — a
  changelog release ledger (a bare version line under `## [Released]`,
  newest first, probed honestly), a sibling-manifest bump lane for any
  future own-version crate, and a six-member Cargo.lock sync (cargo
  update --workspace --offline, verified per member, third-party deps
  frozen). Pre-release versions validate now.

- **night-improve-8**: the actions-pin health gate — all eight workflow
  pins moved from floating tags to release-commit SHA pins with version
  tokens (the same commits the zelynic estate runs), and
  `scripts/gates/check-actions-pins.sh` keeps them current: modes
  off|warn|strict|auto, content-hash cache with TTL, honest skips, MAJOR
  moves never auto-healed, guarded apply.

- **night-improve-7**: the Supermassive deep battery — weekly plus on
  demand, two legs on the pinned toolchain: the full test battery (all
  targets, all features) with the docs build warnings-denied, and the
  release shape proof (the night-improve-11 alias plus the binary's
  hardware-free surfaces). The header states the physics honestly: no
  audio or input device in CI, no claims about real ALSA timing or
  libinput event flow.

- **night-improve-6**: the labeled color lanes — the depth audit of every
  error/failed/unknown/typo path. `error_format` becomes the tier module
  (bold red error lane, bold yellow warning lane, calm grey passive
  suggestions, status green runnable tips, brand purple headers) behind
  one capability gate (TTY + NO_COLOR + CLICOLOR), broken-pipe-safe, with
  the mono prefix byte-identical to the grep-able historical shape. The
  stray hand-rolled error prints in daemon/mod.rs, update.rs and
  instance_lock.rs now ride the labeled lane, and BRANDING.md documents
  the palette.

- **night-improve-3**: the README grows a Demo section in the zelynic
  shape — the brand-styled `zylaxion -h` output rendered as
  `assets/zylaxion-help-demo.png` inside the round-border wrapper
  (border-radius 24px + soft shadow, purple-family branding). The
  asset is an honest capture: the image is typeset from the actual
  binary's stdout (color escape stream parsed verbatim), not a mock —
  and the help surface touches no audio or input hardware, so a
  capture host renders it byte-identical to a desktop. A demo video
  (typing sounds) requires real audio hardware the sandbox lacks; the
  README records that it follows at the next owner-host capture
  session, the same standing-disclaimer class the zelynic demo
  section uses.

- **night-improve-1**: the scripts tree now follows the zelynic
  organization — `scripts/package/install.sh`,
  `scripts/package/uninstall.sh`, and `scripts/dev/version-to.sh`
  join `scripts/build.sh` at the root, matching the zelynic layout
  (`dev/` for development tooling, `package/` for install flow). All
  internal and documentation references updated (README, RULES.md,
  and the scripts' own usage strings) and `install.sh`'s repo-root
  resolution adjusted for its new depth. `build.sh` gains the zelynic
  gate interface: the bare `check-all` spelling plus `-q`/`--quiet`
  (essentials-only output — verdicts always emit, banners do not), so
  `./scripts/build.sh check-all -q` works verbatim. File permissions
  normalized to the owner's 644/755 contract (the zelynic
  check-permissions policy): the 21 non-executable files tracked as
  100755 (sources, manifests, lock, license, gitignore) demoted to
  644, the four shebang scripts stay 755, directories 755.

- **night-improve-2**: the helper output (`-h`/`--help`, usage lines,
  and clap-printed errors) now carries the brand palette via a new
  `cli_styles` module — the zelynic clap-styling pattern. Section
  headings (`Usage:`, `Commands:`, `Options:`) render bold brand
  purple #A855F7; every flag spelling (`-v`, `--verbose`,
  `--preset <PRESET>`) and every command synopsis rides the calm grey
  grammar tier #8B8B8B (flags are typeable grammar, not content — the
  owner's eye-strain call); error labels render bold error red
  #FF5A5A, typed mistakes invalid yellow #FFEB3C, and did-you-mean
  candidates the same calm grey. Help prose tidied alongside: the
  `--live` doc comment split into short and long help (the summary
  view shows one line per flag again), the `testconf -f` doc gained
  an explicit paragraph break, and the `start` about dropped its
  trailing period for synopsis consistency. Gatekeeper green: fmt +
  clippy + 111 tests.

- **night-improve-4**: the workspace rides Rust edition 2024 with the
  MSRV pinned to 1.98.1 — `rust-version = "1.98.1"` now sits next to
  `edition = "2024"` in the shared `[workspace.package]` table and is
  inherited by all six crates, and `rust-toolchain.toml` pins the same
  1.98.1 channel in dormant mode (a future stable cannot silently
  break the build). Migration verified clean: the only 2024 hard error
  was `std::env::set_var`/`remove_var` becoming unsafe — the call
  sites gained `unsafe` blocks with SAFETY comments plus a crate-wide
  `ENV_LOCK` so env-mutating tests serialize; four nested guards
  collapsed into 2024 let-chains; `cargo fmt` re-applied the 2024
  style edition (import version-sort, closure indentation). The main
  crate manifest carries the zelynic-grade package data: extended
  description, docs.rs documentation link, readme, keywords, and
  registry categories. Gatekeeper green: fmt + clippy
  (`--all-targets --all-features -D warnings`) + 111 tests pass on
  the pinned 1.98.1 toolchain.

- **night-improve-0**: the design philosophy is now codified in
  `docs/PHILOSOPHY.md` under the owner's "the boring and stealth"
  banner — one sound done perfectly, the keystroke never leaves the
  process, Linux-first evdev-only input, pure procedural synthesis
  with zero assets, lean dependencies, honesty over impression. The
  README Security & Privacy section links to it. Docs-only change;
  no code touched.

- **night-improve-5**: restructured the changelog — the v10-era
  record moved to `docs/archive/CHANGELOG-V10.0.0-ERA.md` (frozen
  history, wrapped with an archive note), the v6.0.0 release notes
  joined it in `docs/archive/`, and this file restarted as the fresh
  record for the upcoming version. No code change; no version change
  (bumps stay the owner's call).

<!-- Entries land here as they merge; the owner promotes
     [Unreleased] to a dated version heading at release time. -->
