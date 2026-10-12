<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion Glossary

The vocabulary the docs, the CLI, and the commit history share. Every
term is defined once and used the same way everywhere — a term that
drifts between docs is a bug.

## The project

- **zylaxion** — the binary and the project: a real-time mechanical
  keyboard acoustic synthesizer for Linux. Pure procedural synthesis,
  zero audio samples.
- **zelynic** — the owner's flagship reference project (the network
  rate limiter). zylaxion's infra estate — the gates, the release
  pipeline, the version tooling, the CLI conventions — ports
  zelynic's mature shapes, adapted and documented with honest deltas.

## The build

- **Dragon Guard** — the CI workflow family name (ci, maintenance,
  supermassive, release, miri). One family, one grep surface.
- **gate-keepers** — `scripts/gate-keepers.sh`, the orchestrator of
  the hard gates (permissions, headers, LOC caps, language, emoji,
  pins health, release parity). Every warning is a failure.
- **the 600-line law** — every tracked `.rs` file stays under 600
  lines (`scripts/gates/check-loc.sh`). Over the cap means split the
  file — a pure move into modules, gates green in between.
- **LOC_EXEMPT marker (retired)** — the self-declared, in-file
  migration debt a pre-gate file once carried while it waited for its
  Owner-sequenced split. night-improve-18 paid the debt (all six
  files split) and retired the lane: the Rust gate honors no marker —
  over the cap means split, the same hour. The scripts cap (1000)
  keeps the marker for its own debt lane.
- **frozen record** — a file whose history is never rewritten
  (CHANGELOG.md, the v6.0.0-era audit docs). The gates exclude them
  by name; new facts get new entries.
- **Hinnant clock** — Howard Hinnant's `civil_from_days` algorithm
  converting days-since-UNIX-epoch to a calendar date, std-only. The
  project's answer to every wall-clock need: the `Build-time:` stamp
  (build.rs) and the daemon log timestamps (`clock.rs`). No time
  crate exists in the dependency tree.

## The release

- **platform id** — the canonical build label:
  `linux-amd64-<baseline>-<libc>` (e.g. `linux-amd64-v3-gnu`). Stamped
  into the binary via `ZYLAXION_BUILD`, printed on the `Build:` line,
  and carried by the tarball name — one field, one truth.
- **x86-64-v3 / x86-64-v4** — the AMD/Intel baseline microarch
  levels: v3 = AVX2/BMI2/FMA era (Haswell 2013+), v4 = AVX-512 era.
  The release matrix ships both glibc legs; v3 is the safe default
  (every CPU that runs v4 runs v3).
- **musl dormant lane** — the documented non-shipping musl legs:
  historically blocked by the C input stack (libinput, libudev) with
  no static musl answer. night-improve-23 deleted that stack (the eBPF
  tap replaced it), leaving alsa-lib — the leg already proven static —
  so the lane is dependency-unblocked; re-enabling the matrix legs
  stays the owner's call. The evidence lives in the release workflow's
  matrix header.
- **checksum family** — the three digests per release tarball:
  SHA-512 (`*.sha512sum`), BLAKE2b-512 (`*.b2sum`), and SHAKE256
  (`*.shake256`, quantum-resistant). Defense in depth across three
  independent hash families.
- **flat archive invariant** — a release tarball carries exactly
  `zylaxion`, `LICENSE`, `README.md` at archive root — nothing else,
  no nesting. The workflow's tripwire asserts it byte-for-byte.

## The runtime

- **preset** — a named acoustic tuning (materials, pitch, decay,
  volume curve) compiled into the binary in the registry
  (`src/profiles/registry.rs`), selected with `--preset` or
  `ZYLAXION_PRESET` and played by the engine when a key event
  arrives. The profiles module (`src/profiles/`) owns the tables.
- **voice pool** — the engine's fixed-capacity synthesizer voice
  set (`src/engine/`); keystrokes claim and release voices; the
  render path is allocation-free by contract.
- **orchestrator** — the core module's event loop (`src/core/`): input
  events in,
  voice triggers out, audio batches to the sink; the daemon's core.
- **IPC socket** — `$XDG_RUNTIME_DIR/zylaxion.sock`, mode `0o600`,
  length-prefixed, speaking only the documented verbs (`stop`,
  `status`).
- **input-data discipline** — the product's core security promise:
  keystroke events drive synthesis in memory only; nothing persists,
  nothing transmits. See SECURITY.md.

## The CLI

- **version report** — the `-V`/`--version` output: brand header
  (name + version + tagline), then `Build:` / `Build-time:` /
  `Copyright:` / `License:` / `Source:`. Two surfaces (the first-arg
  intercept and clap's own path) share one body.
- **word verdicts** — the doctor's PASS:/FAIL: lines through the
  green/red lanes. No glyph decorations anywhere in the tree.
- **color gate** — color renders only on a TTY with `NO_COLOR`
  unset and `CLICOLOR != 0`; stdout and stderr are gated
  separately. Piped output is byte-identical plain text.
