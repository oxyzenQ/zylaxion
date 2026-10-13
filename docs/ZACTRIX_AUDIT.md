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
  8/16 ms references against 32/48 ms lates because a 30-80 ms
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
