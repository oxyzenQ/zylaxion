<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Contributing to zylaxion

Thank you for your interest in contributing to zylaxion! This document
covers the build process, project structure, and the discipline every
commit rides through.

## Contributor License Agreement

zylaxion is dual-licensed (GPL-3.0-only + Commercial), so contributions
ride on the [Contributor License Agreement](CLA.md): it grants the
Owner the right to relicense your Contribution — including into the
commercial lane — while you keep full copyright of your own work.
There is no separate document to sign: the DCO-style `Signed-off-by:`
line is the acceptance. Sign every commit with `git commit -s` (the
flag appends the line from your configured git identity), or comment
`I have read the CLA and I agree` on the pull request. The full terms
and the acceptance options live in [CLA.md](CLA.md).

## Prerequisites

- Rust 1.98.1 (pinned in `rust-toolchain.toml` — rustup picks it up on
  the first command inside the checkout; no manual install).
- Linux build headers for the hardware lanes, from your distro:
  `libasound2-dev` (ALSA), `libudev-dev`, `libinput-dev` (plus its
  runtime libraries). A container-less Debian/Ubuntu one-liner:
  `sudo apt install libasound2-dev libudev-dev libinput-dev`.
- A Linux desktop session with an audio server (PipeWire or ALSA) for
  actually running the binary — building needs none of that, and the
  pure-computation test surface runs headless.

## Build

The build commands (plain release and the pro-native host alias) live
once in the README — see [Install from source](README.md). The same
commands apply for contributors. `./scripts/build.sh check-all -q` is
the one-command local gate (format, clippy, tests — the exact lane CI
runs).

## Project Structure

```
crates/
  zylaxion/            — the binary: CLI, config system, daemon, commands
  zylaxion-core/       — the orchestrator: wires input events to synthesis
  zylaxion-input/      — evdev/libinput capture lane
  zylaxion-output/     — audio backend lane (cpal / PipeWire / ALSA)
  zactrix-engine/      — the real-time synthesis engine (voice pool)
  zactrix-profiles/    — the acoustic preset tables and tuning model
docs/                  — architecture, branding, verification, audits
scripts/
  build.sh             — the local gate entry point
  gates/               — the individual gate checks (LOC, permissions,
                         language, pins, headers)
  dev/                 — maintainer tooling (version-to.sh)
  package/             — install/uninstall scripts
.github/workflows/     — the CI estate (Dragon Guard family)
```

## The Discipline (what the gate-keepers enforce)

These are not suggestions — every commit in the history rides them,
and the `Dragon Guard - Gate-keepers` CI workflow re-checks every
push:

1. **English-only tree.** Comments, docs, script text, and string
   literals are pure English (no emoji, no non-Latin scripts in
   committed artifacts; mixed-language chat with the Owner is fine —
   the tree never is).
2. **The 600-line law.** Every `.rs` file stays under 600 lines.
   `scripts/` files carry a 1000-line cap with the `# LOC_EXEMPT:`
   self-declaration mechanism for tracked migration debt. Over the
   Rust cap means split the file — a pure move, gates green in
   between.
3. **Permissions freeze.** Tracked files carry mode 644, executables
   and directories 755. `scripts/gates/check-permissions.sh --fix`
   repairs a drifted worktree.
4. **Header parity.** Every source file opens with the copyright line
   and the `SPDX-License-Identifier: GPL-3.0-only` tag.
5. **No version drift.** Version fields, changelog release dates, and
   tags move only by the Owner's hand (`scripts/dev/version-to.sh` is
   the single sanctioned tool, run by the Owner).

## Commit Discipline

- Micro commits: one purpose per commit, pushed as you go.
- Commit messages: pure English, no emoji, imperative subject line,
  body explains the why and the honesty notes (what was verified, what
  was deliberately not).
- Every commit carries the `Signed-off-by:` line (DCO + CLA
  acceptance in one).
- Never amend or force-push `main`; the history is the audit trail.

## Reporting

- Bugs and feature requests: GitHub issues.
- Security: the private channel in [SECURITY.md](SECURITY.md) — never
  a public issue.
- Licensing: [COMMERCIAL_LICENSE.md](COMMERCIAL_LICENSE.md).
- Trademark and naming: [TRADEMARK.md](TRADEMARK.md).
