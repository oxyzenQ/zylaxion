<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# The eBPF-Only Bet — an Honest Ledger

zylaxion crossed an architecture line that most tools never cross on
purpose: the userspace daemon era was deleted entirely, and the input
path became a pure-Rust eBPF tap with pinned kernel maps as the only
state. This document is the honest cost accounting of that decision —
what the eBPF-only shape wins, what it genuinely costs, and which
user the trade serves. It is written to be quotable against the
project: every "win" row names the mechanism that verifies it, and
every "cost" row keeps its irreducible remainder instead of hiding
behind a mitigation.

## 1. The Two Architectures, Stated Plainly

The daemon-era zylaxion (through the v10 line) was a conventional
Linux sound board: a resident userspace daemon captured the keyboard
through libinput/evdev, held a `config.toml` parsed by toml/serde,
watched that file for edits with `notify`, talked to client verbs
over an IPC surface, and dragged the xdg/directories and
dbus/zbus-shaped families along for session plumbing. The state of
the sound lived in a process; the user's wishes lived in a file; the
two met through a protocol that had to be parsed, versioned, and
defended.

The eBPF-only zylaxion (night-improve-23 onward) is a different
shape. A small kernel-side tap — one fentry (or kprobe fallback) on
the exported `input_event()` — watches key events passively and
publishes them into a pinned ring buffer. The control state (sound
on/off, preset index, volume, player pid) lives in pinned BPF maps on
bpffs, not in a process. `zylaxion on` spawns a stateless player that
reads every byte of its state from those maps and self-exits on
`off`; nothing residents between sessions except the kernel-side tap
and maps. There is no config file: the preset registry is
compile-time, type-checked Rust data (night-improve-22 deleted the
TOML parser, watcher, and validator together).

## 2. What the eBPF-Only Shape Wins

Each row carries its verifier, because a claim without a mechanism is
marketing.

| Win | Why it is real | The verifier |
|---|---|---|
| Zero daemons | No PID file, no Unix socket, no flock, no IPC protocol, no supervision loop; the state store is pinned BPF maps | The claims battery sweeps the binary for daemon-era strings and the lockfile for daemon-era crates — zero hits, guarded as a regression test |
| Passive input | No evdev grab, no `/dev/input` node access, no udev rule, no `input` group membership; the tap observes `input_event()` kernel-side and gates every event against pinned maps | The attach path plants no device nodes; the privilege doc table lists exactly one privileged verb |
| Smaller supply chain | libinput/udev capture stack, toml/serde config stack, notify watcher, and the xdg/dbus session families are all gone from the tree | `Cargo.lock` swept transitively per family, zero hits; the CI cargo-audit lane (`--deny warnings`) guards what remains |
| Unprivileged control plane | After one `sudo zylaxion attach` per boot, every verb rides the pinned maps — no privileged process ever residents | The E2E battery runs the full zero-to-hero chain as a plain user |
| Zero idle cost | Interrupt-driven: the player's epoll loop wakes only for mapped keys while sound is enabled; between keystrokes it costs nothing | The ring buffer bridge renders under 3 ms typical latency |
| Kernel-side telemetry for free | The tap accumulates per-key counters in the kernel at zero userspace cost | `zylaxion status --verbose` renders them; no userspace sound tool offers this for free |
| No config to drift | No parse errors, no schema versioning, no file watching, no per-boot divergence between what you typed and what runs | The registry is compile-time data proven in-range by the registry test tree |
| Honest failure surfaces | `doctor` probes what it can actually probe (kernel floor, BTF, bpffs, lockdown, unprivileged-bpf posture, audio); refusals are classified, not generic | The E2E probes pin the refusal contract per kernel posture |

## 3. What the eBPF-Only Shape Costs

This is the half most architecture documents leave out. Every row
here is a real cost paid in full; the mitigation column names what
the project built to blunt it, and the remainder names what stays
blunted anyway.

| Cost | Why it is real | Mitigation | The irreducible remainder |
|---|---|---|---|
| A kernel floor of 5.15 | The tap needs fentry-with-BTF or kprobe-over-eBPF; the daemon era ran on any kernel with evdev | The kprobe fallback covers BTF-less kernels; `doctor` names the verdict | Below 5.15 there is no zylaxion at all, by design |
| One privileged step per boot | `attach` needs root (or an unprivileged-bpf-permissive boot); the daemon only ever needed `input` group membership | The attach is once per boot, logged per step, and every later verb is unprivileged | On distros that build `CONFIG_BPF_UNPRIV_DEFAULT_OFF=y` (Ubuntu does), plain-user verbs are locked out for the whole boot — the verbs refuse classified, and root remains the only key |
| A kernel-adjacent bug class | The tap runs in kernel context; the daemon's bugs crashed a userspace process instead | The verifier bounds everything the tap can do; the object is one small program with two map families | If the tap misbehaves, the debugging lives in verifier logs and pinned state, not in a Rust backtrace |
| Harder field diagnosis | A silent tap can mean locked lockdown, missing bpffs, an unprivileged-bpf gate, pin ownership, or a verifier shape mismatch | The `doctor` report, the classified-refusal contract, and the per-step attach log exist precisely for this | The stack is genuinely deeper than "is the process alive" — the daemon's one question was easier to answer |
| No runtime sound customization | Presets are compile-time; a new sound means a new release, not a new file | The registry is type-checked at build time; the release cadence is the owner's | The daemon-era user who tuned `config.toml` by hand is served by nothing — that flexibility was traded away on purpose, not lost by accident |
| Build complexity at the source | Building from a git checkout needs the dated nightly pin and bpf-linker for the nested tap build | `ebpf-prebuilt/` and the crates.io tarball embed the maintainer-built object — installers need no nightly | Source builders carry the toolchain requirement forever |
| State that outlives the process | Pins survive a crashed player (that is the idempotency feature) — and a crashed attach leaves a partial pin set | `attach` replants idempotently; `status` and the overview name a PARTIAL pin set honestly | A partial pin set is kernel state a daemon era never had to clean up |
| Linux-and-lower-bounded | Both eras were Linux-only by input stack (libinput was Linux-only too); eBPF-only additionally narrows WITHIN Linux — version floor, bpffs mount, BTF/lockdown posture | `docs/KERNEL_COMPATIBILITY.md` documents the matrix honestly | Portability beyond the tested kernels is out of scope by design, and no future feature changes that |

## 4. The Dependency Ledger, In Numbers

The honest arithmetic is not "fewer crates is better" — it is which
crates the binary needs to do its one job.

- The current tree: 11 direct dependencies, 150 lockfile crates, and
  every feature gate mapped to a live call site (night-security-1
  audited them gate by gate). The Linux-resolved graph is 52 crates;
  the rest of the lock is cpal's target-gated backends for platforms
  zylaxion never runs on.
- The daemon era carried, among others: libinput + udev (capture),
  toml + serde (config), notify (watcher), daemonize, and the
  xdg/directories and dbus/zbus families (session plumbing) — every
  one of them swept and confirmed absent from today's lockfile.
- The eBPF loader added its own weight the daemon never carried:
  aya, aya-obj, object, and their supporting cast. That is the
  honest debit side of the tap; it is also bounded, audited, and
  far smaller than what the daemon stack weighed in.

## 5. The Verdict

The trade is correct for what zylaxion claims to be. The product's
promise is the boring and stealth: predictable sound, zero
keystroke leakage, no resident surface, no file to parse, nothing to
audit beyond one small kernel program and one binary. Every row in
the win table serves that promise directly, and every row in the
cost table is a boundary the project names in its own doctor, its
own privilege table, and this document — rather than pretending the
costs away.

The trade would be wrong for a different product. A tool whose users
hand-tune sound definitions at runtime, a tool that must run on
long-support kernels below 5.15, a tool that cannot ask for one
privileged attach step, or a tool that must never touch the kernel's
BPF machinery — any of those should keep the daemon architecture,
and there is no shame in that: it was the right shape for its era
too. The honest difference is that the era's shape optimized for
compatibility first and let the surface grow with it; this shape
optimizes for a minimal, verifiable surface and accepts the kernel
boundary as the price. The project knows exactly what it sold, and
this ledger is the receipt.

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
