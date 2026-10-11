<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion Design Philosophy

> zylaxion is "the boring and stealth" — a real-time mechanical keyboard
> acoustic engine that plays the same predictable click on every
> keystroke, forever, and never lets a single keystroke leave the
> process. The sound is the product; everything else is silent.

This document consolidates the design philosophy governing zylaxion's
decisions. It is the canonical reference for why certain designs were
chosen, why entire classes of features are rejected, and what a
contributor should feel in their gut before proposing a change to the
core. Where a rule here overlaps a hard gate in RULES.md, the gate
wins; this file explains the reasoning that produced the gate.

## 1. Boring: One Sound, Done Perfectly

The daemon has exactly one job — turn key events into keystroke
sounds — and it does that job the same way on every boot. No skins,
no visualizers, no companion app, no cloud profiles. Every acoustic
parameter lives in one `config.toml`, the daemon hot-reloads it
within one second of a save, and `zylaxion testconf` catches TOML
typos and out-of-bounds DSP values before they can touch a running
session. When a preset name does not resolve, the program prints the
available presets and exits — there is no silent fallback, because a
tool that quietly guesses is a tool the owner cannot trust. Boring
means the behavior is fully predictable from the files on disk.

## 2. Stealth: The Keystroke Never Leaves the Process

A keyboard listener is one debug log away from being a keylogger, so
zylaxion refuses to be one by construction. It reads hardware
scancodes — physical key locations, never characters — feeds them
straight into the DSP engine, and forgets them. Nothing is logged at
any level, nothing is written to disk beyond the PID file and the
IPC socket, nothing is transmitted — the only outbound network call
in the entire codebase is the explicit `zylaxion --check-update`,
which the user must invoke by hand. The IPC socket is mode `0o600`,
so other users on the machine cannot even talk to the daemon. The
zero-trust logging policy is the whole point: there is nothing in
the logs to reconstruct, and the README shows the grep commands to
verify that yourself.

## 3. Linux-First, evdev-Only

zylaxion speaks to the Linux kernel's `evdev` interface through
libinput and nothing else. There is no X11 hook, no Wayland
compositor protocol, no desktop-environment plugin — which is
exactly why the input capture works everywhere the kernel does:
Wayland, X11, and the raw TTY. The display server is irrelevant to
a keyboard; treating it as irrelevant is what makes the input path
one small, auditable module. Portability to other operating systems
is explicitly out of scope; the project would rather be perfect on
one kernel than average on three.

## 4. Pure Procedural Synthesis, Zero Assets

Every sound is computed, not replayed. Three TPT state-variable
filter layers model the click transient, the spring resonance, and
the housing "thock"; micro-randomization (unique noise seed, pitch
and amplitude drift, pan jitter, timing variation) guarantees the
same key never sounds identical twice. The binary ships no
wavetables, no sample libraries, no asset files — which means
nothing to load, nothing to corrupt, nothing to re-license, and a
single self-contained artifact. If a proposed feature requires
shipping audio data, it is a different product.

## 5. Lean Dependencies, Lean Surface

The dependency set is kept to what has a live call site: cpal for
audio, libinput for capture, serde/toml for config, clap for the
CLI. The `notify` crate was rejected for the config watcher because
a 1 Hz mtime poll costs single-digit microseconds — invisible next
to a 2 ms audio latency budget — and a dependency is supply-chain
surface that must be audited forever after. The same instinct caps
file size (600 LOC, the no-mercy cap), keeps `main.rs` a thin
bootstrap, and keeps the tree at one package with a fixed data flow:
input → core → engine → audio.

## 6. Honesty Over Impression

The overview screen prints what was verified — daemon state, preset,
audio device, input group — and the `doctor` subcommand diagnoses
what it can actually probe. Version output names the build target,
git hash, and license. When something is not supported, the answer
is a clear refusal, not a degraded attempt. A tool that reports
less than it knows wastes the owner's time; a tool that reports
more than it knows wastes the owner's trust.

## 7. The Owner Decides

zylaxion is the exclusive work of rezky_nightky (oxyzenQ). Versions
are cut by the owner, the name and marks are governed by
TRADEMARK.md, the license is GPL-3.0-only everywhere, and no
contribution may rebrand, relicense, or soften the privacy posture.
This philosophy is descriptive, not negotiable: code that contradicts
it does not merge, no matter how clever the diff looks.
