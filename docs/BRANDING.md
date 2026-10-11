<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion™ — Branding

**Project:** zylaxion
**Author:** rezky_nightky (oxyzenQ)
**Repository:** https://github.com/oxyzenQ/zylaxion
**Contact:** with dot rezky at gmail dot com

---

## Name Usage

The project name is **zylaxion** (all lowercase) in every context —
prose, titles, headings, code, CLI output, comments, commit subjects,
and file paths. There is no context where a capital first letter or
an internal capital is correct, including the start of a sentence.

| Context | Format |
|---|---|
| Running text / prose | zylaxion |
| Titles / headings | zylaxion |
| Code / CLI | `zylaxion` (lowercase) |

The one uppercase family that survives is identifiers, never the
name itself: the `ZYLAXION_*` environment variables and build stamps
(`ZYLAXION_BUILD`, `ZYLAXION_BUILD_TIME`, ...), plus the all-caps
banner comment titles heading the scripts/ gate files — shell and
identifier conventions, not brand displays.

Incorrect forms (the name-case gate rejects every one of them):

- a capitalized first letter, or any internal capital, in any
  context — even at the start of a sentence;
- all-caps prose uses outside the identifier family.

Since night-improve-20 the rule is machine-enforced:
`scripts/gates/check-name-case.py` (gate-keepers section 9) scans
every tracked file and path — nothing excluded — and fails the build
on any casing outside the two families above.

## Identity

zylaxion is a premium mechanical keyboard sound daemon for Linux.
The brand conveys precision, acoustics, craftsmanship, and zero-leakage privacy.

## Logo

The zylaxion logo lives in `assets/`. Width: **260px**.

## Color Palette

The zelynic family color contract (night-improve-6). Purple is the branding
color; the semantic tiers below are shared across the toolchain so the clap
surface (`crates/zylaxion/src/cli/styles.rs`) and the application-rendered
output (`crates/zylaxion/src/output/mod.rs`) read as one tool:

| Tier | Value | Used for |
|---|---|---|
| Brand purple | `#A855F7` (bold) | Section headers (`Usage:`, doctor sections) |
| Error red | `#FF5A5A` | The labeled error lane (`zylaxion: error:`) |
| Warning yellow | `#FFEB3C` | The labeled warning lane (`zylaxion: warning:`) |
| Calm grey | `#8B8B8B` | Grammar tier: passive suggestions (`tip:`, `hint:`, `fix:`) |
| Status green | `#50FA7B` | Affirmative verdicts and runnable tips |

Color renders only on a terminal (`NO_COLOR` and `CLICOLOR=0` strip it);
pipes and captures get plain text, byte-identical to the historical
uncolored prefixes so `grep 'zylaxion: error:'` keeps working.

## Badges

Only the Ko-fi badge is permitted on the README:

```
[![Ko-fi](https://img.shields.io/badge/Ko--fi-support-7C3AED?style=flat-square&logo=kofi&logoColor=white&labelColor=111827)](https://ko-fi.com/rezky)
```

No other shields, stats, or tracking badges.

## Copyright Notice

All files must carry:

```
Copyright (C) 2026 rezky_nightky
SPDX-License-Identifier: GPL-3.0-only
```

## Derivative Works

Forks and GPL-compliant derivatives must adopt distinct branding
(name, logo, identity) to avoid confusion with the official project.

---

© 2026 rezky_nightky (oxyzenQ). All rights reserved.
