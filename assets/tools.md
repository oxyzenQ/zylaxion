<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# local tools

The owner's local image/demo toolkit, carried from the sibling
zejtron estate's assets/tools.md. These are the commands that
produce the assets this repo ships - the rounded-corner logo
(`assets/zylaxion-original-logo-2026.png`, 1254x1254 with the 68px
corner radius) and the terminal demo captures (`assets/zylaxion-*.png`,
the 120x40 grid every demo screenshot uses). Nothing here builds or
tests the binary; the tools page exists so the asset shapes
reproduce exactly instead of drifting by hand.

### usually for rounder corner image

```sh
magick input.png \( -size 1254x1254 xc:none -fill white -draw 'roundrectangle 0,0 1253,1253 68,68' \) -alpha off -compose CopyOpacity -composite output.png
```

### for demo on terminal use this size

```sh
alacritty -o "window.dimensions.columns=120" -o "window.dimensions.lines=40"
```

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
