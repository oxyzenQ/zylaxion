<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion QA Record

Owner QA rounds and ask-mode sessions: the owner asks, the answer is
researched against the source tree before it is written. Every claim
below was verified against the code at the time of writing (the
night-improve-13 session, commit 0fd7cc4), and each answer names the
file it came from — this record is evidence, not opinion. Questions
arrive in the owner's own words (translated; the repo is
English-only) and are kept focused: one question, one honest answer,
no padding.

---

## Q1 — How big is the dependency tree, and what is the no-bloat policy?

The package resolves to **67 unique packages** (`cargo tree
--workspace` count at commit 39586b4, night-improve-21: the six-crate
workspace became one package and six path deps left the lock), and
the standing contract is
that every one of them earns its place:

- **No time crates.** Any wall-clock need is answered by the std clock
  plus Howard Hinnant's `civil_from_days` algorithm — the runtime twin
  lives in `src/info/clock.rs` (log stamps) and the
  compile-time twin in `build.rs` (the `Build-time:`
  stamp). env_logger runs with `default-features = false` because its
  default timestamp feature transitively pulled the jiff tree
  (`Cargo.toml`, night-improve-13).
- **No regex crates in the build graph.** env_filter's regex feature
  went with the same change; RUST_LOG keeps the standard
  comma-separated directive syntax, which was always the documented
  surface.
- **No mock, no async runtime, no HTTP framework.** The player is
  threads + a Unix socket; the update check is the one deliberate
  network surface.

When a dependency cannot avoid being pulled (a transitive requirement
of a hardware lane), the honest move is to shrink the feature set
that activates it, not to add a second crate doing the same job.

## Q2 — What exactly does `-V` / `--version` print, and why that shape?

The zelynic reference contract
(`src/cli/`, night-improve-13):

```
zylaxion: v<version>
Real-time mechanical keyboard acoustic synthesizer for Linux
Build: linux-amd64-gnu (<short-sha>)
Build-time: M/D/YYYY HH:MM (UTC)
Copyright: (c) 2026 rezky_nightky (oxyzenQ)
License: GPL-3.0-only
Source: https://github.com/oxyzenQ/zylaxion
```

- The `Build:` label comes from the canonical label chain
  (`src/info/build.rs`): the `ZYLAXION_BUILD` env
  stamp when a release lane exports one, else the compile-time
  arch+libc detection.
- The `Build-time:` stamp is computed in `build.rs` by the Hinnant
  algorithm — UTC-only, no timezone database in the binary, identical
  across build hosts.
- Two surfaces, one body: the first-arg `-V`/`--version` intercept
  renders the brand header in purple on a TTY
  (`src/main.rs`), and clap's own version path shares
  the same `version_body()` — the tests in `cli.rs` pin the line set
  and the ordering so the surfaces cannot drift.

## Q3 — What does `doctor` check, and can it fail a script?

`src/commands/info.rs`, four sections:

1. **Build facts** — version, commit, variant, build time, rustc,
   profile (compile-time truth, never counts against the verdict).
2. **Input group membership** — the user must be in the `input`
   group to read evdev devices; failure prints the exact `usermod`
   fix.
3. **`XDG_RUNTIME_DIR`** — the IPC socket and lock file land here;
   missing or unwritable is a failure with the export fix.
4. **Audio server** — best-effort probe of the default output device
   through cpal.

The exit code reflects the hard checks (sections 2 and 3, plus audio
degradation), so `zylaxion doctor` is scriptable as a health probe.
The doctor speaks in word verdicts (green/red lanes through
`src/output/mod.rs`), not glyph decorations.

## Q4 — Where does the 600-line law stand today?

`scripts/gates/check-loc.sh` enforces the cap tree-wide, and
`scripts/gates/check-scripts-loc.sh` enforces 1000 lines for every
script under `scripts/`. The no-mercy era (night-improve-18) closed
the debt ledger: the six pre-gate files that carried the
`// LOC_EXEMPT:` marker are split, the marker is retired, and the
gate honors no exemption — over the cap means SPLIT, the same hour,
a pure move with gates green in between. The same gate enforces the
layout law beside the cap: the package's src/ root holds only its
entry file (main.rs / lib.rs), and every *.rs test file lives under
the package's test/ tree (the Pattern C #[path] wiring), counted
under the same cap.

## Q5 — How do I verify a downloaded release binary?

[docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md) is the walkthrough:
sha256/sha512 checksum first, then the GPG signature when the release
carries one, then the smoke checks (`zylaxion -V` should print the
Build facts that match the release page's declared platform label).
The release lane builds `x86_64` gnu binaries at the x86-64-v3 and v4
optimization baselines — pick the artifact whose label matches your
CPU (v3: AVX2-era; v4: AVX-512-era); when in doubt, v3 runs
everywhere v4 does. The musl lane is documented dormant: the C audio
stack (alsa-lib) has no static musl answer yet — alsa-lib alone was
proven solvable (the static musl .a builds), and the unlock paths
live in the release workflow's matrix header (`release.yml`, the
night-improve-12c evidence block). night-improve-23 removed the
libinput/libudev half of that blocker (the eBPF tap needs no input
libraries); alsa remains the gate. Every artifact self-reports its
platform id through the `ZYLAXION_BUILD` stamp, and the local alias
parity is gate-enforced (`scripts/gates/check-release-parity.sh`).
