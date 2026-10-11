<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion™ — Branding

**Project:** zylaxion
**Author:** rezky_nightky (oxyzenQ)
**Repository:** https://github.com/oxyzenQ/zylaxion
**Contact:** with dot rezky at gmail dot com

---

## Name

The project name is **zylaxion** (all lowercase). It must never be capitalized
as `Zylaxion` in any official context.

## Identity

zylaxion is a premium mechanical keyboard sound daemon for Linux.
The brand conveys precision, acoustics, craftsmanship, and zero-leakage privacy.

## Logo

The zylaxion logo lives in `assets/`. Width: **260px**.

## Color Palette

The zelynic family color contract (night-improve-6). Purple is the branding
color; the semantic tiers below are shared across the toolchain so the clap
surface (`crates/zylaxion/src/cli_styles.rs`) and the application-rendered
output (`crates/zylaxion/src/error_format.rs`) read as one tool:

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
