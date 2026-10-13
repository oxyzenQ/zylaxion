<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# The Zactrix Phase 0 Acoustic Audit

Pass: `night-private-research-2xblade`, Phase 0. The forge starts with
listening: before any engine code is touched, every shipped preset is
rendered headlessly through the real render path and measured against
four physical movement metrics. The question the audit answers is
whether the DSP architecture has a ceiling that pins every preset into
the same tier — and the data below decides whether the time-variant
upgrade is earned.

One command reproduces everything in this document:

```bash
scripts/audit/zactrix-audit.sh
```

## The instrument

The harness (`scripts/audit/harness/`, a standalone stdlib-plus-log
crate that includes the engine and profiles trees in place) renders
each of the 24 presets through the real `VoicePool`/`MechanicalClick`
render path — the same per-sample loop the live player runs — at
96 kHz, mono, one keypress, 500 ms window, with the micro-randomization
seed fixed (fresh model instance, counter reset). Repeated runs are
byte-identical. No audio device is touched.

Four metrics are measured over the 0-100 ms onset window:

1. **Centroid drift (Hz)** — how far the spectral centroid travels
   between the attack window (0-5 ms) and the tail. A real keystroke
   dies HF-first; a static spectrum does not move.
2. **Flatness evolution** — spectral flatness (Wiener entropy,
   0 = tone, 1 = noise) of the attack minus the tail. A real attack is
   broadband; a real tail is tonal.
3. **Modal decay spread (dB/s)** — spread of per-mode decay rates
   across the strongest spectral modes, tracked from 8/16 ms reference
   frames to 32/48 ms late frames. Real modes die at different rates;
   one global envelope does not.
4. **Attack/tail energy (dB)** — energy of the first 5 ms against the
   following 95 ms. Real impacts keep a large share of energy in the
   attack, but not nearly all of it.

Measurement honesty, declared up front:

- All spectral statistics run over the positive-frequency half of the
  FFT (bins 1..N/2). Over the full mirrored spectrum of a real signal
  the centroid collapses to Nyquist/2 for every frame — a mathematical
  artifact the first instrument draft actually produced, caught and
  fixed before any number was recorded.
- A frame is **live** only within -60 dB of the loudest analysis
  frame. Below that the samples are numerical dust, not sound. Silent
  frames are excluded from tail statistics; a mode that dies before
  the late frame is reported at the 600 dB/s dead-rate cap.
- Frames sit where the phenomenon lives: the modal tracking pairs
  8/16 ms references against 32/48 ms lates because a 30-80 ms
  placement would measure the noise floor of an already-dead tail for
  most presets. All placements are fixed across presets — no
  per-preset adaptation.

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

## The 24x4 matrix

| preset | centroid drift Hz | flatness evolution | modal spread dB/s | attack/tail dB | tier |
|--------|------------------:|-------------------:|------------------:|---------------:|:----:|
| `technical` | 1513 | 0.048 | 0 | +31.0 | B |
| `cherryMX` | 2133 | 0.074 | 599 | +27.7 | B |
| `classic` | 2617 | 0.082 | 585 | +20.3 | B |
| `studio` | 4731 | 0.153 | 576 | +26.4 | B |
| `elegant` | 17183 | 0.511 | 1128 | +31.6 | B |
| `whisper` | 0 | 0.000 | 0 | +43.6 | C |
| `linear` | 6380 | 0.206 | 1306 | +22.7 | B |
| `tactile` | 1881 | 0.065 | 944 | +26.3 | B |
| `clicky` | 637 | 0.017 | 0 | +23.7 | B |
| `ibm_model_m` | 171 | 0.006 | 683 | +17.3 | B |
| `topre` | 17863 | 0.452 | 630 | +11.8 | A |
| `cherry_mx_black` | 11831 | 0.368 | 1237 | +25.1 | B |
| `cherry_mx_clear` | 3694 | 0.123 | 576 | +25.5 | B |
| `cherry_mx_silver` | 1027 | 0.034 | 600 | +27.5 | B |
| `buckling_spring` | 324 | 0.003 | 775 | +21.1 | B |
| `alps_skcm` | 327 | 0.008 | 674 | +8.9 | B |
| `gateron_yellow` | 12687 | 0.369 | 575 | +12.9 | B |
| `zealios_v2` | 462 | 0.013 | 831 | +21.8 | B |
| `rotary_encoder` | 0 | 0.000 | 0 | +54.5 | C |
| `cyber_deck` | 0 | 0.000 | 0 | +39.6 | C |
| `oceanic` | 23238 | 0.552 | 290 | +19.9 | B |
| `glass_tactile` | 0 | 0.000 | 0 | +58.1 | C |
| `retro_typewriter` | 231 | 0.005 | 593 | +4.2 | B |
| `neon_pulse` | 531 | 0.015 | 598 | +25.0 | B |

**Tiers: A = 1, B = 19, C = 4 (of 24). Gate: OPEN** (C > 0 and
A < 20 — both open conditions hold).

## What the data says

The engine's ceiling is real, measurable, and it is not the one the
ear drafts first. The three-layer architecture does move the macro
spectrum — 20 of 24 presets show honest centroid travel, several by
kilohertz — so the failure is subtler than a frozen spectrum. What the
layers cannot do is hold energy past the attack: 21 of 24 presets sit
far above the +12 dB attack/tail ceiling (up to +58 dB), meaning the
first 5 ms carries 99 percent or more of the onset energy where a real
actuation keeps 20-94 percent there and spends the rest ringing. The
cause is structural: each bandpass's ring dies at its own bandwidth
rate (tens of Hz of bandwidth = a few milliseconds of ring) while the
slow global envelope scales an already-dead filter state — the
envelope believes in a tail the filters never deliver. Four presets
(`whisper`, `rotary_encoder`, `cyber_deck`, `glass_tactile`) are the
limit case: a click with no body at all, everything dead before the
first 12 ms frame. Flatness barely evolves for eight presets
(0.003-0.009 against the 0.01 floor) — their attack is nearly as tonal
as their tail. The one A-tier preset is `topre`, the smooth
low-resonance voice whose +11.8 dB ratio and 0.452 flatness travel are
the only physically proportioned shape in the catalog; the data's
one-line instruction to Phase 1 is to build every preset the balance
topre already has, by giving the stages' cutoff and Q the time-varying
curves the actuation physics dictates. Nineteen presets slumped in the
same B tier is the ceiling the thesis predicted, measured.

## The gate decision

The Phase 0 gate is **open**: C = 4 > 0 and A = 1 < 20. The data
earned the time-variant upgrade. Phase 1 (the Zactrix time-variant
envelope: physics-derived cutoff and Q curves on the three TPT SVF
stages, verifier-first re-tuning of all 24 presets) is authorized by
this document, and its before/after record lands in the section below
when the forge cools.
