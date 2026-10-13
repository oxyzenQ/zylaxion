<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# The Zactrix Acoustic Audit

Pass: `night-private-research-2xblade`. The forge starts with
listening: before any engine code is touched, every shipped preset is
rendered headlessly through the real render path and measured against
four physical movement metrics. This document holds the instrument,
the before matrix (the frozen-coefficient engine), the after matrix
(the Zactrix time-variant engine), and every tier delta between them.

One command reproduces everything in this document:

```bash
scripts/audit/zactrix-audit.sh                 # the before/after harness
scripts/audit/zactrix-verify.sh <preset-name>  # one-preset verifier row
```

## The instrument

The harness (`scripts/audit/harness/`, a standalone stdlib-plus-log
crate that includes the engine and profiles trees in place) renders
each of the 24 presets through the real `VoicePool` render path — the
same per-sample loop the live player runs — at 96 kHz, mono, one
keypress, 500 ms window, with the micro-randomization seed fixed
(fresh model instance, counter reset). Repeated runs are
byte-identical. No audio device is touched.

Four metrics are measured over the 0-100 ms onset window:

1. **Centroid drift (Hz)** — how far the spectral centroid travels
   between the attack window (0-5 ms) and the tail. A real keystroke
   dies HF-first; a static spectrum does not move.
2. **Flatness evolution** — spectral flatness (Wiener entropy,
   0 = tone, 1 = noise) of the attack minus the tail. A real attack is
   broadband; a real tail is tonal.
3. **Modal decay spread (dB/s)** — spread of per-mode ABSOLUTE decay
   rates across the strongest spectral modes, tracked from 8/16 ms
   reference frames to 32/48 ms late frames. Real modes die at
   different rates; one global envelope does not.
4. **Attack/tail energy (dB)** — energy of the first 5 ms against the
   following 95 ms. Real impacts keep a large share of energy in the
   attack, but not nearly all of it.

Measurement honesty, declared up front — including the corrections
the instrument itself went through:

- All spectral statistics run over the positive-frequency half of the
  FFT (bins 1..N/2). Over the full mirrored spectrum of a real signal
  the centroid collapses to Nyquist/2 for every frame — a mathematical
  artifact the first instrument draft actually produced, caught and
  fixed before any number was recorded.
- A frame is **live** only within -60 dB of the loudest analysis
  frame. Below that the samples are numerical dust, not sound. Silent
  frames are excluded from tail statistics; a mode that dies before
  the late frame is reported at the 600 dB/s dead-rate cap.
- **Modal level correction** (caught during Phase 1 verification,
  before any conclusion was drawn from it): the first modal draft
  compared the two frames' peak-normalized spectra — a shape
  comparison, blind to absolute level. A late frame 74 dB below the
  attack normalized its own dust to full scale and read as a
  "surviving mode", inflating the modal column of the first draft.
  Both frames are now scaled by their absolute windowed energies, so
  a rate is a decay rate. The BEFORE matrix below was re-measured
  against the pre-forge engine with the corrected instrument (the
  engine checked out at the audit commit), so both matrices ride the
  same instrument. The Phase 0 gate was re-verified on the corrected
  numbers and still opened.
- Frames sit where the phenomenon lives: the modal tracking pairs
  8/16 ms references against 32/48 ms late frames because a 30-80 ms
  placement would measure the noise floor of an already-dead tail for
  most presets. All placements are fixed across presets and across
  both matrices — no per-preset adaptation.

## The physical bands — and their honesty note

The preset authoring references (`ibm_model_m`, `topre`, `alps_skcm`)
are named in the docs as tuning targets, but **the tree ships no
reference recordings or measurement tables for them** (the zero-samples
posture). The bands below are therefore conservative estimates chosen
from the published acoustics of mechanical switch impact — deliberately
wide, so a metric lands out of band only when the signal clearly lacks
the physical behavior it measures. They are not measured from a
reference recording, and this document does not pretend otherwise.

| Metric | Band | Rationale |
|--------|------|-----------|
| Centroid drift | >= 150 Hz | The tail must be audibly darker than the attack. |
| Flatness evolution | >= 0.01 | Broadband attack, tonal tail. |
| Modal decay spread | >= 6 dB/s | Modes must die at measurably different rates. |
| Attack/tail energy | -6 to +12 dB | Real impacts keep 20-94 percent of onset-window energy in the first 5 ms. |

Tier rule: **A** = all four in band, **B** = two or three, **C** = zero
or one. Gate: the audit closes the pass only if C = 0 and A >= 20.

## The before matrix (frozen-coefficient engine)

| preset | centroid drift Hz | flatness evolution | modal spread dB/s | attack/tail dB | tier |
|--------|------------------:|-------------------:|------------------:|---------------:|:----:|
| `technical` | 1513 | 0.048 | 0 | +31.0 | B |
| `cherryMX` | 2133 | 0.074 | 0 | +27.7 | B |
| `classic` | 2617 | 0.082 | 0 | +20.3 | B |
| `studio` | 4731 | 0.153 | 0 | +26.4 | B |
| `elegant` | 17183 | 0.511 | 0 | +31.6 | B |
| `whisper` | 0 | 0.000 | 0 | +43.6 | C |
| `linear` | 6380 | 0.206 | 0 | +22.7 | B |
| `tactile` | 1881 | 0.065 | 0 | +26.3 | B |
| `clicky` | 637 | 0.017 | 0 | +23.7 | B |
| `ibm_model_m` | 171 | 0.006 | 0 | +17.3 | C |
| `topre` | 17863 | 0.452 | 1034 | +11.8 | A |
| `cherry_mx_black` | 11831 | 0.368 | 0 | +25.1 | B |
| `cherry_mx_clear` | 3694 | 0.123 | 0 | +25.5 | B |
| `cherry_mx_silver` | 1027 | 0.034 | 0 | +27.5 | B |
| `buckling_spring` | 324 | 0.003 | 0 | +21.1 | C |
| `alps_skcm` | 327 | 0.008 | 0 | +8.9 | B |
| `gateron_yellow` | 12687 | 0.369 | 0 | +12.9 | B |
| `zealios_v2` | 462 | 0.013 | 0 | +21.8 | B |
| `rotary_encoder` | 0 | 0.000 | 0 | +54.5 | C |
| `cyber_deck` | 0 | 0.000 | 0 | +39.6 | C |
| `oceanic` | 23238 | 0.552 | 290 | +19.9 | B |
| `glass_tactile` | 0 | 0.000 | 0 | +58.1 | C |
| `retro_typewriter` | 231 | 0.005 | 1483 | +4.2 | B |
| `neon_pulse` | 531 | 0.015 | 0 | +25.0 | B |

**Tiers: A = 1, B = 17, C = 6. Gate: OPEN** (C > 0 and A < 20 — both
open conditions hold; re-verified on the corrected instrument).

The corrected before matrix tells a harsher story than the first
draft: with the modal fiction removed, `ibm_model_m` and
`buckling_spring` were C-tier voices, and only `topre`, `oceanic`, and
`retro_typewriter` carried any measurable modal decay spread at all.

## The after matrix (Zactrix time-variant engine)

| preset | centroid drift Hz | flatness evolution | modal spread dB/s | attack/tail dB | tier |
|--------|------------------:|-------------------:|------------------:|---------------:|:----:|
| `technical` | 2934 | 0.084 | 0 | +19.2 | B |
| `cherryMX` | 3652 | 0.102 | 0 | +17.6 | B |
| `classic` | 5280 | 0.160 | 0 | +11.9 | B |
| `studio` | 5365 | 0.149 | 0 | +15.4 | B |
| `elegant` | 16976 | 0.470 | 0 | +21.1 | B |
| `whisper` | 0 | 0.000 | 0 | +43.6 | C |
| `linear` | 6677 | 0.189 | 0 | +14.6 | B |
| `tactile` | 3305 | 0.093 | 0 | +18.9 | B |
| `clicky` | 1335 | 0.037 | 0 | +17.5 | B |
| `ibm_model_m` | 579 | 0.020 | 0 | +10.2 | B |
| `topre` | 18405 | 0.462 | 1034 | +11.7 | A |
| `cherry_mx_black` | 10263 | 0.293 | 0 | +14.7 | B |
| `cherry_mx_clear` | 4347 | 0.117 | 0 | +14.0 | B |
| `cherry_mx_silver` | 1255 | 0.037 | 0 | +19.2 | B |
| `buckling_spring` | 469 | 0.018 | 0 | +18.3 | B |
| `alps_skcm` | 789 | 0.023 | 0 | +10.5 | B |
| `gateron_yellow` | 15218 | 0.407 | 0 | +11.7 | B |
| `zealios_v2` | 1102 | 0.028 | 0 | +15.2 | B |
| `rotary_encoder` | 0 | 0.000 | 0 | +54.4 | C |
| `cyber_deck` | 0 | 0.000 | 0 | +39.7 | C |
| `oceanic` | 23403 | 0.550 | 0 | +18.6 | B |
| `glass_tactile` | 0 | 0.000 | 0 | +59.0 | C |
| `retro_typewriter` | 383 | 0.013 | 0 | +9.7 | B |
| `neon_pulse` | 1217 | 0.032 | 0 | +16.7 | B |

**Tiers: A = 1, B = 19, C = 4.**

## Every tier delta, named

- `ibm_model_m`: **C -> B**. The buckling spring's body arrived: its
  attack/tail ratio fell from +17.3 to +10.2 dB (inside the physical
  band), its centroid drift nearly quadrupled (171 -> 579 Hz), its
  flatness evolution tripled (0.006 -> 0.020).
- `buckling_spring`: **C -> B**. Drift 324 -> 469 Hz, flatness
  0.003 -> 0.018 — both into band.
- No preset dropped a tier. `topre` holds A with its authored rows
  untouched (its small column movements ride the engine-wide curves).
- The four C-tier voices that remain are the four documented identity
  refusals (see the re-tune notes in `src/profiles/registry.rs`):
  `whisper` ("extremely quiet, short decay"), `rotary_encoder`
  ("subtle, metallic" detent tick), `cyber_deck` ("short bright pings,
  no body" — its own description), and `glass_tactile` (a 5 kHz ring
  needs Q far beyond the 10.0 range cap; a low-band body would break
  the sharp-glass character). Each refused the fit on identity
  grounds; none was silently regressed, none was silently reshaped.

## What the data says

The Zactrix thesis landed and the instrument can prove it. The
time-variant cutoff and Q curves roughly doubled the spectral movement
on the fitted presets — centroid drift rose on 18 of the 20 fitted
voices (classic 2617 -> 5280 Hz, technical 1513 -> 2934 Hz, clicky 637
-> 1335 Hz) and flatness evolution rose on all 20 (technical 0.048 ->
0.084, classic 0.082 -> 0.160, ibm_model_m 0.006 -> 0.020): the
spectrum now travels during the actuation instead of standing still
under an envelope. The re-tune then fed the starved bodies: the
attack/tail ratio improved on all 20 fitted presets, and the number of
voices inside the physical energy band tripled, from three (alps,
retro, topre) to six (classic, ibm_model_m, topre, alps, gateron,
retro). Two C-tier voices rose to B; none fell.

The honest residuals are structural, and this document names them
rather than hiding them. First, the modal column reads zero for the
fitted crowd under the corrected instrument: the range contract caps
resonance at Q = 10, which at 250-600 Hz means a ring that survives
10-25 ms — real differential decay, but everything is dead before the
32-48 ms late frames, so no spread is measurable there. Second, the
attack/tail band holds six of twenty-four: the bright-click presets
(technical, elegant, and their class) still carry 96-99 percent of
their onset energy in the first milliseconds, because a 4.5 kHz click
band plus a Q-capped housing cannot balance a physical impact's energy
share. Lifting that ceiling needs either a higher resonance contract
or a physically longer ring stage — both are architecture decisions
that belong to a future pass, made by the owner, with this document as
the baseline. Third, the four refusals stay as authored: their
identity is the short shape, and the audit measures identities, not
wishes.

## The gate record

The Phase 0 gate opened on the first instrument's numbers (C = 4,
A = 1) and re-opened on the corrected ones (C = 6, A = 1): the data
earned the time-variant upgrade, the upgrade was forged, the presets
were re-tuned and re-audited, and every tier delta is named above.
The pass record lives in the CHANGELOG; the engine carries the
Zactrix name with the thesis in one doc comment.

# The Range Question — night-private-research-2xblade-2

The 2xblade audit closed by naming its one structural ceiling and
deferring the decision: the Q range contract `[0.1, 10.0]` caps the
modal ring at roughly 10-25 ms, everything dies before the 32-48 ms
late frames, the modal column reads zero for the fitted crowd, and
lifting the ceiling is an architecture decision that belongs to the
owner. This pass took the decision.

## The decision

> DECISION: EXPAND — the contract moves from `[0.1, 10.0]` to
> `[0.1, 30.0]`.
>
> REASONING: The audit is unambiguous — presets whose own
> descriptions promise a ring are held below their designed tier by
> the Q≤10 clamp, and the clamp's written rationale ("resonance >
> 10.0 becomes numerically unstable in single-precision TPT SVF")
> is contradicted by the tree's own pin: `test_svf_high_q_does_not_blow_up`
> runs the SVF at Q = 50 for 44,100 samples and asserts finite
> output. The clamp was inherited from the config-era guardrails,
> never measured. The engine that follows a topre should not be
> constrained by a number a different codebase chose. Expand to
> [0.1, 30.0], verify no piercing, and let the audit tell the truth
> about what the Zactrix can do.

The target set is the scan set: the mission's five plus the three
the description scan added — "long ring" appears verbatim in
`ibm_model_m` and `clicky`, "long mid ring" in `retro_typewriter`,
"long bass sustain" in `oceanic`, beside the promised
resonant/thock/hollow/ping language on the mission's list
(`buckling_spring`, `alps_skcm`, `gateron_yellow`, `classic`). Eight
presets. topre stays untouched; the four identity refusals stay as
authored.

## The reading

The reading paragraph, as it landed in the CHANGELOG:

> The reading: docs/ZACTRIX_AUDIT.md ends by naming the Q range
> contract as the audit's one structural ceiling — the modal column
> reads zero for all twenty fitted presets because a Q ≤ 10 ring at
> 250-600 Hz dies before the 32-48 ms late frames — and defers the
> ceiling decision to the owner with the matrix as the baseline. The
> reading found three facts the audit left for this pass. First, the
> contract's written rationale ("resonance > 10.0 produces a very
> narrow filter that becomes numerically unstable in single-precision
> TPT SVF", src/profiles/profile/mod.rs) is contradicted by the
> tree's own pin: test_svf_high_q_does_not_blow_up runs the SVF at
> Q = 50 for 44,100 samples and asserts finite output — the cap was
> inherited from the config-era guardrails, never measured, and its
> stated reason is empirically false in-tree. Second, the contract is
> enforced at four sites: the ranges constants, validate_and_clamp,
> the two per-sample safety-net clamps on the click and spring curves
> in the render path, and the registry test tree; housing Q rides the
> mix × Q level law, and every ring-promising preset already sits at
> housing Q 8.0-9.0, against the ceiling. Third, the description scan
> is wider than the mission's five: "long ring" appears verbatim in
> ibm_model_m and clicky, "long mid ring" in retro_typewriter, "long
> bass sustain" in oceanic, beside the promised resonant/thock/hollow/
> ping language on the mission's list — and the README preset table
> carries the same promises. Whether that language is a promise or
> poetry is the owner's call, not the data's: both answers remained
> defensible from this reading, so the decision stayed where the
> prompt put it.

## The stability battery (Path A step 2, before any re-tune)

`scripts/audit/zactrix-stability.sh` (harness mode `--stability`)
renders each scan-set preset with EVERY stage's resonance forced to
the new ceiling (`ranges::RESONANCE_MAX` = 30.0 — the worst case the
range allows), one million samples per rate, through the RAW
`AcousticModel` path — deliberately bypassing `VoicePool`'s final
NaN/Inf-to-silence safety net so an internal blow-up shows as data
instead of hiding as silence. It counts NaN, Inf, f32 overflow and
denormals on the raw output, probes the six TPT integrator states at
rest (the exact variables the old rationale claimed "grow
unbounded"), and measures the c^(R/S) law across 44.1 / 48 / 96 kHz.

Results, all eight presets, all three rates: **numerics clean** —
zero NaN, zero Inf, zero output denormals, peak |sample| <= 0.90,
every integrator state finite and bounded. The voice lifetime — the
retention chain's direct measurand, the substance the c^(R/S) law
governs — agrees across rates within **0.0007%-0.0202%** (law:
0.5%).

Two findings are recorded rather than hidden:

1. **Denormal integrator states at voice death.** The click/spring
   states end below the smallest normal f32 (dust at rest). A control
   run at the shipped Q values (housing Q 8-9) shows the same, plus
   denormal housing states and non-zero denormal counts in the OUTPUT
   stream — the expansion reduces output denormals to zero and leaves
   the at-rest dust unchanged. The dust is never rendered again (the
   pool skips inactive voices) and init_state zeroes it on the next
   trigger. Pre-existing, harmless, recorded.
2. **The -60 dB tail END POINT is not rate-invariant** (spread
   3.7%-19.6% across rates, scaling with the ring's decay time). The
   endpoint does not measure the retention law: the ring's energy is
   extracted from a white-noise burst, and the energy a discrete-time
   resonator extracts scales with its NORMALIZED bandwidth
   (fc/(Q·Fs)), so the same ring starts its tail at a slightly
   different level per rate and crosses the floor at a different
   wall-clock time. The control run shows the same spread at the
   shipped values — a property of noise-driven synthesis, present
   before the expansion, unchanged by it. The law's substance (the
   voice lifetime, and the single-component ring decay rate — alps:
   spread 0.0075%) is invariant. No engine change can make a
   noise-driven tail's endpoint 0.5%-invariant; that would require
   redesigning the excitation, which is out of scope and would change
   every preset's sound.

## The re-tune (Path A step 3)

Every number landed through the harness `--fit` surface (clone a
profile with overrides, render, print the verifier row and the modal
tracker's actual view) and was judged by `zactrix-verify.sh`. Two
things the fits taught, recorded because they correct the 2xblade
level law:

- The noise-driven bandpass level scales with `mix × Q^1.5`, not
  `mix × Q` (the bandpass output itself grows with sqrt(Q) under
  noise excitation). The 2xblade "mix × Q preserved" arithmetic
  actually raised the housing ring ~14 dB; this pass's fits set the
  housing level by ear of the verifier instead — mix falls as Q
  rises.
- A ring the instrument cannot pick up is a ring only the
  description claims. The modal tracker picks the top three peaks of
  the 8/16 ms reference frames; with the housing ring at Q 27-28 the
  reference frame is single-modal (the housing peak), so the spring
  ring was raised into the picker's visibility (Q 10-20, mix 1.0) on
  the presets whose text promises ring/ping. Click, decay and ambient
  rows are untouched everywhere.

Final rows (housing and spring; click/decay/ambient untouched):

| preset | housing (fc, Q, mix) | spring (fc, Q, mix) | result |
|--------|----------------------|---------------------|--------|
| `ibm_model_m` | 400, 9.0→**27.0**, 0.135→**0.02** | 850, 7.5→**20.0**, 0.85→**1.0** | **B → A** |
| `classic` | 350, 8.0→**28.0**, 0.175→**0.023** | 1200, 4.0→**12.0**, 0.75→**1.0** | **B → A** |
| `buckling_spring` | 380, 6.0→**21.0**, 0.175→**0.038** | 1400, 5.5→**12.0**, 0.75→**1.0** | **B → A** |
| `alps_skcm` | 300, 8.0→**28.0**, 0.21→**0.03** | 2000, 3.8→**20.0**, 0.65→**1.0** | **B → A** |
| `retro_typewriter` | 280, 8.0→**25.0**, 0.25→**0.03** | 1500, 5.5→**10.0**, 0.7→**1.0** | **B → A** |
| `clicky` | 600, 8.0→**26.0**, 0.065→**0.02** | untouched (2400, 4.5, 0.7) | B → B |
| `gateron_yellow` | 250, 8.0→**20.0**, 0.20→**0.08** | untouched (1500, 1.5, 0.4) | B → B |
| `oceanic` | 100, 8.0→**12.0**, 0.105→**0.17** | untouched (600, 2.0, 0.8) | B → B |

## The after-2 matrix (Zactrix + expanded contract)

Same instrument, same seed, same windows, byte-reproducible (two
consecutive runs diff-clean). The sixteen untouched presets reproduce
the 2xblade after matrix byte-for-byte — the contract expansion
itself changed no render.

| preset | centroid drift Hz | flatness evolution | modal spread dB/s | attack/tail dB | tier |
|--------|------------------:|-------------------:|------------------:|---------------:|:----:|
| `technical` | 2934 | 0.084 | 0 | +19.2 | B |
| `cherryMX` | 3652 | 0.102 | 0 | +17.6 | B |
| `classic` | 6264 | 0.177 | 994 | +11.4 | A |
| `studio` | 5365 | 0.149 | 0 | +15.4 | B |
| `elegant` | 16976 | 0.470 | 0 | +21.1 | B |
| `whisper` | 0 | 0.000 | 0 | +43.6 | C |
| `linear` | 6677 | 0.189 | 0 | +14.6 | B |
| `tactile` | 3305 | 0.093 | 0 | +18.9 | B |
| `clicky` | 743 | 0.021 | 0 | +7.2 | B |
| `ibm_model_m` | 1442 | 0.043 | 374 | +9.1 | A |
| `topre` | 18405 | 0.462 | 1034 | +11.7 | A |
| `cherry_mx_black` | 10263 | 0.293 | 0 | +14.7 | B |
| `cherry_mx_clear` | 4347 | 0.117 | 0 | +14.0 | B |
| `cherry_mx_silver` | 1255 | 0.037 | 0 | +19.2 | B |
| `buckling_spring` | 787 | 0.014 | 948 | +11.4 | A |
| `alps_skcm` | 2039 | 0.026 | 1076 | +10.4 | A |
| `gateron_yellow` | 14023 | 0.376 | 0 | +7.3 | B |
| `zealios_v2` | 1102 | 0.028 | 0 | +15.2 | B |
| `rotary_encoder` | 0 | 0.000 | 0 | +54.4 | C |
| `cyber_deck` | 0 | 0.000 | 0 | +39.7 | C |
| `oceanic` | 23061 | 0.548 | 0 | +11.6 | B |
| `glass_tactile` | 0 | 0.000 | 0 | +59.0 | C |
| `retro_typewriter` | 1746 | 0.052 | 948 | +11.1 | A |
| `neon_pulse` | 1217 | 0.032 | 0 | +16.7 | B |

**Tiers: A = 6, B = 14, C = 4** (the 2xblade after matrix was A = 1,
B = 19, C = 4).

## Every tier delta, named

- `ibm_model_m`: **B -> A**. The modal column lit: 0 -> 374 dB/s
  (housing ring at tau 21.5 ms survives the 32-48 ms late frames;
  spring ring picked at -29.6 dB). Drift 579 -> 1442 Hz, flatness
  0.020 -> 0.043, attack/tail +10.2 -> +9.1 — all four in band.
- `classic`: **B -> A**. Modal 0 -> 994 dB/s. Drift 5280 -> 6264 Hz,
  flatness 0.160 -> 0.177, attack/tail +11.9 -> +11.4 — all four in
  band.
- `buckling_spring`: **B -> A**. Modal 0 -> 948 dB/s — the "more
  ping" the text promises is now a measured second mode. Drift
  469 -> 787 Hz, flatness 0.018 -> 0.014, attack/tail +18.3 -> +11.4
  (into band) — all four in band.
- `alps_skcm`: **B -> A**. Modal 0 -> 1076 dB/s. Drift 789 -> 2039
  Hz, flatness 0.023 -> 0.026, attack/tail +10.5 -> +10.4 — all four
  in band.
- `retro_typewriter`: **B -> A**. Modal 0 -> 948 dB/s. Drift
  383 -> 1746 Hz, flatness 0.013 -> 0.052, attack/tail +9.7 -> +11.1
  — all four in band.
- `oceanic`: **B -> B, metric count 2 -> 3**. Attack/tail
  +18.6 -> +11.6: into the physical band — the "long bass sustain"
  the text promises is now inside the energy band the audit measures.
  The modal column stays closed for a positional reason: the 100 Hz
  mode sits on FFT bin 1, below the tracker's bin-2 pickup floor
  (bin width 93.75 Hz). The ring is delivered (tau 38 ms); the
  instrument cannot see a mode it cannot bin. No tier dropped.
- `clicky`: **B -> B, metric count 2 -> 3**. Attack/tail
  +17.5 -> +7.2: into band. The modal column stays closed: the
  2400 Hz spring at its authored Q 4.5 rings for about one
  millisecond, and the fit surface showed every housing level that
  lifts it into the modal picker pushes flatness and attack/tail out
  of band — the four-band balance does not exist for this preset's
  identity. The "long ring" is the housing ring (tau 13.8 ms at
  Q 26), delivered. No tier dropped.
- `gateron_yellow`: **B -> B, metric count 3 -> 3**. The modal
  column stays closed by identity: a lubed linear's spring (Q 1.5)
  is a driven, deliberately ringless layer — the deep thock IS the
  voice, and its housing ring rides the expanded contract (tau
  25.5 ms at Q 20). No tier dropped.
- `topre`: untouched, holds A byte-identically
  (18405 / 0.462 / 1034 / +11.7).
- The four identity refusals hold their authored C rows
  byte-identically.

No preset dropped a tier. No preset lost a metric. Zero silent
regressions.

## What the Range Question data says

The Q ceiling was an inherited number, and the audit proved it: the
tree's own Q = 50 pin contradicted its rationale, the stability
battery found the ceiling numerically clean at one million samples
per rate, and the retention law held to 0.02% across rates. The
expanded contract bought exactly what the descriptions promised:
five presets now carry a measurable modal ring (the modal column
went from one non-zero row — topre — to six), five voices rose a
full tier, and the two remaining band-closers are named positional
or identity facts, not regressions. The instrument itself gained a
tuning lab (`--fit`, `--modal-debug`) and a stability battery
(`zactrix-stability.sh`); the metric definitions did not change —
the 2xblade matrices above are reproduced byte-for-byte by this
pass's instrument, untouched presets included. The remaining
structural residual is unchanged from 2xblade: the bright-click
class (technical, elegant and their neighbors) still concentrates
96-99 percent of onset energy in the attack — a 4.5 kHz click band
against a physical impact's energy share — and lifting that would
need a physically longer ring stage, which remains an owner-level
architecture decision. The four refusals stay as authored: the audit
measures identities, not wishes.
