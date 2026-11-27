<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
# The night-cafe-mclaren-2 engine and architecture depth audit (2026-11-27)

One task, one question: before the car ships, audit the McLaren the
way a McLaren is audited — design, engine, and architecture, at depth.
Is the render path smoothbore (no lag, no bottleneck, no hidden stall),
is the math provable and deterministic bit-for-bit, is the safety real
or decorative, and is every claim the docs make actually true? This
audit answers for the whole engine surface at commit `29a7df2`
(v80.5.7): `src/engine/` (pool, voice, detmath, denormals, gain), the
real-time `src/audio/` sink, the `src/core/` orchestrator render loop,
the control-plane lanes that can reach the engine at runtime, and the
formal proof estate. Honest verdicts only; every finding below was
either reproduced by exact bit-level simulation (`scripts/audit/
mclaren2_math.py`, committed with this audit — one command replays
every number) or traced through every call site by hand.

## Method

The audit read the full hot path top to bottom — audio callback,
SPSC ring, voice pool, init/render curves, detmath, denormal pinning,
orchestrator regimes — then traced every input that can reach a
transcendental or a panic at runtime (profile constants, drift
curves, the control map, preset indexes), then verified each safety
claim against its actual enforcement site (not its documentation),
then computed the numeric proofs in IEEE-754-exact Python (the
generator-grade `f32` rounding simulation, the same op order the Rust
runs).

## The findings (all three fixed on this cycle)

1. **M1 — the prewarp ceiling leaked the certificate by one ULP**
   (fixed in `184340e`). The TPT bilinear prewarp argument is
   `PI * fc / sr` computed in f32; `det_tan`'s Taylor certificate ends
   at `0.45 * PI`; the clamp ceiling was `0.45 * sr`. At the ceiling,
   the f32 argument landed one ULP PAST the bound at the 48k / 96k /
   192k device rates — the exact rates the sink's own negotiation
   ladder prefers (simulated margin: +9.9e-8 at 48000). And the init
   side's broadband ceiling `min(click_freq * 2.5, 0.45 * sr)` can hand
   the render loop exactly that fc, so detmath's "a domain exit is
   UNREACHABLE by construction" was false at the ceiling: a future
   preset with a click band above ~7.9 kHz at 44.1 kHz (or ~8.6 kHz at
   48 kHz) would have panicked the render loop on its first sample.
   The fix rides one constant, `FC_CERTIFIED_FRAC = 0.44`, at all
   three call sites: the argument now sits 0.0314 radians inside the
   certificate (~260000 f32 ULPs) at every device rate, while the
   shipped presets' broadband start (13750 Hz worst case) is under the
   old and new ceiling alike — every golden coefficient and every
   hash row reads exactly what it read before (sections G and H).
2. **M2 — the bounds docs claimed a runtime enforcement that does not
   exist** (fixed in `6bcfde8`). `validate_and_clamp` documented
   itself as enforced "when loading user-supplied TOML profiles", and
   the pool doc cited its "guardrails" as a live safety layer — but
   the TOML loader died with night-improve-22 and the function has
   zero production call sites. The real law is better than the lie:
   every profile the engine renders is a registry-authored constant,
   CI-proven in-range and bit-stable by
   `every_profile_survives_clamp_without_drift` (all 24 presets,
   clamp no-op demanded bit-for-bit), with the detmath domain asserts
   as loud tripwires. The docs now name that law and mark the
   function as the contract any future loader must call.
3. **M3 — the rate negotiation could invent a combo the device never
   offered** (fixed in `4e1e363`). The 48000/96000/192000 ladder took
   the first preferred rate ANY supported range listed and applied it
   to the default config — but a range belongs to one (channels,
   sample_format) family, and a foreign-family rate names a combo the
   device never advertised. The mismatch fails closed at
   `build_output_stream`, but fail-closed is not
   correct-by-construction: an exotic device paid a startup failure
   for nothing. The ladder now matches the default config's own
   family before narrowing.

## Verified clean (the honest no-finding list, with the proof each rests on)

- **The audio callback is genuinely RT-safe.** One SPSC ring is the
  only shared structure; the callback runs zero allocations, zero
  locks, zero syscalls; it reads one relaxed atomic (the paused flag)
  and pops frames; NaN/Infinity are replaced and every channel is
  clamped before the hardware sees it; channels 2..N are zeroed, never
  stale. The producer side drops only on a documented-full ring and
  the orchestrator paces against vacancy so the drop lane stays
  theoretical.
- **The ring budget has real headroom.** 16384 frames is 371.5 ms at
  44.1 kHz, 341.3 ms at 48k, 85.3 ms at 192k — 16x a 1024-frame
  quantum at every rate (section D), far past the 30–50 ms Linux
  scheduler-jitter spikes the sizing comment names.
- **The orchestrator is evented end to end.** SILENT regime: a
  blocking select with no timer at all — the park IS the idle state,
  zero wakeups (the night-frontier-Z4 law). ACTIVE regime: 1 ms
  evented pacing where a keystroke wakes the render instantly,
  vacancy-driven chunks capped at 64 frames, one pre-allocated batch
  (no per-iteration allocation), a 500 µs sleep when the ring is
  near-full so the CPU reaches C-states instead of busy-spinning, and
  a stop lane that lands within one pacing step.
- **The voice pool is deterministic by construction.** Fixed array,
  zero allocation in the render path, oldest-first stealing keyed on a
  monotonic u64 counter, scancodes zeroed on release (the privacy
  note), and one volume law: every writer of the volume atomic rides
  `volume_bits` (the night-hunter-1 story is pinned in the gain-law
  core).
- **The math core is certified and the certificates are enforced.**
  detmath replaces every libm transcendental on the audio path with
  pure IEEE-754 f64 (Taylor/Horner, Cody-Waite exact splits, exact
  `2^n` bit assembly) — Rust never contracts mul+add into FMA, so the
  bits are identical on every target and every glibc. Domain asserts
  refuse loudly outside each certificate. The audit verified every
  reachable call site against its domain: the housing fade argument
  sits in [-3, 0] by construction (progress is guarded below the
  window length), the release path's `det_pow(0.04, 1/N)` exponent is
  ~0.01 at every device rate (section F), the decay rate ratio
  `44100/sr` lives in (0, 1] for every rate the sink can negotiate
  (section E), and the worst authored-coefficient case keeps
  `r * ln(c)` far inside det_exp's [-64, 0] (section C). The shipped
  presets' in-range law is CI-enforced bit-for-bit (finding M2's
  truth).
- **Denormals cannot tax the voice tail.** The render lane pins
  flush-to-zero once per pool construction (x86-64 FTZ / AArch64 FZ),
  compiled to a documented no-op under miri and kani.
- **The runtime control surface cannot reach the math.** The control
  map carries a volume percent (gated 0..=200, refused-not-clamped)
  and a preset index (bounds-checked with an honest fallback and a
  warning); there is no lane from the map into any DSP parameter, so
  the certified domains have no live adversary.
- **The formal estate is real.** Kani proof harnesses classify whole
  errno domains and error-code round-trips under `#[kani::proof]` and
  ride CI (`kani.yml`); the deterministic render path is the same one
  the supermassive E2E battery measures.

## What was not changed, on purpose

- No behavior change for any shipped preset: the ceiling fix binds
  only where authored curves never reach (section H), so coefficients,
  golden corpora, and rates-parity baselines are bit-identical.
- `det_tan`'s assert stays exactly as certified — the fix belongs to
  the caller's clamp law, not to the certificate.
- The parked-idle diagnostics stay parked (the loop's own comment
  names where provable diagnostics live instead: doctor, status, the
  E2E battery).
- No new abstractions: three findings, three minimal fixes, each one
  commit.

## Honest limitations

- This sandbox carries no Rust toolchain and no ebpf prerequisites:
  fmt, clippy, cargo test, kani and the bit-identity of the golden
  corpora are CI's verdicts, and every commit above names that
  honestly. The bit-identity argument for shipped presets rests on
  section H's arithmetic, not on a local render run.
- No benchmark ran for this cycle: the shipped presets' rendered
  samples are bit-identical (section H), so an A/B would measure the
  same sound twice.

## Verdict

The McLaren analogy held: the engine's bones (RT discipline,
lock-free hot path, evented pacing, certified math, formal proofs)
are genuinely race-grade, and the audit found no lag, no bottleneck,
and no lock in the render path. What it found instead were three
precision defects — a one-ULP certificate leak, a doc claiming a
guard that does not run, and a negotiation that could invent a device
state — each fixed on this cycle with the proof committed beside it.
Zero stale: every claim this document makes names where its evidence
lives.

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
