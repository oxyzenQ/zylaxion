<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion FAQ — desktop, server, and ultra-scale usage

The deployment question bank: which shape fits which machine, and the
answers the doctor will confirm. Every answer names the mechanism
that verifies it — the same claims discipline the rest of the tree
carries. The source code is the single source of truth; when a
release's `zylaxion doctor` disagrees with a row below, trust the
doctor and open a PR.

## Desktop (the everyday lane)

### What is the best daily usage on a desktop?

Attach once per boot, then play as your own user:

```bash
sudo zylaxion attach      # the one privileged verb, once per boot
zylaxion on               # spawn the stateless player, sound starts
zylaxion preset --switch mechanical
zylaxion off              # the player self-exits, zero processes
```

`attach` pins the eBPF tap and hands the pins to your user — every
later verb (`on`, `off`, `preset`, `volume`, `status`) runs
unprivileged (the doctor's `unpriv bpf` row confirms it). `zylaxion
on` spawns the internal `_play` verb as a stateless process: it reads
every byte of state from the pinned kernel maps and stops itself when
`off` clears the enabled flag. No daemon, no config files, no stray
processes.

### How does the sound survive a reboot?

The one-command installer lands a systemd boot oneshot plus login
hand-over units beside the binary: the oneshot replants the tap early
in the boot (kernel pins do not survive a reboot), and the hand-over
delivers the pins to your session at the first login. After
`./scripts/package/install-on-my-machine.sh`, the desktop contract is
automatic — log in, type, hear the keys.

### Which preset wins when nothing is passed?

The resolution chain is fixed: the `--preset` flag first, then the
`ZYLAXION_PRESET` environment variable, then the compiled default.
There is no config file to drift (the no-config law) — the registry
is compile-time data, listed by `zylaxion list-presets`.

### How loud can it go?

`volume` takes a percent of the compiled default. The session gate
refuses anything above 200% (values that loud can damage speakers)
unless `--force-this` rides along, which lifts the ceiling to 10000;
anything past that is refused outright. The cap is a hardware-kill
feature, not a bug (night-improve-27).

## Server and headless machines

### Which build shape should a server run?

The glibc lane the release matrix ships: `cargo pro-linux-amd64-v3-gnu`
/ `pro-linux-amd64-v4-gnu` (the x86-64-v3/v4 tiers) or `cargo
pro-native-gnu` (host-CPU tuned). The eBPF tap is embedded inside the
binary, so deployment is still a copy: `scp` it to the box, run the
attach oneshot, done — the only host expectations are glibc and the
ALSA runtime (`alsa-lib`), which a normal distro audio stack already
carries. There is no musl lane: a musl cross-build from a glibc host
welds the host's libasound onto a musl CRT — a binary that links but
cannot run — and the static story was partial anyway (libasound still
dlopens its PCM plugin tree at runtime), so the lane was removed
rather than left as a trap.

### Does a headless server need a sound card?

The tap and the telemetry run without one; the voice needs one ALSA
PCM device. A USB DAC plugged into the rack counts — `zylaxion
list-backends` shows what the host answers, and `zylaxion doctor`
renders the audio verdict honestly (a cardless host is a warning, not
a crash). The glibc binaries expect the host to carry the ALSA
runtime (`alsa-lib`) plus the plugin tree (`alsa-plugins`): libasound
dlopens its PCM plugins from `/usr/lib/alsa-lib` at runtime.

### The doctor mentions XDG_RUNTIME_DIR — does a server care?

Some ALSA configurations route through user-session sockets that live
under `XDG_RUNTIME_DIR`; headless shells often leave it unset (the
doctor's XDG row says so). Pointing the variable at a user-owned
runtime directory (systemd sessions create one at
`/run/user/<uid>`) settles it. Raw `hw:` devices do not need it.

### What is the kernel floor on a server fleet?

Linux 5.15 or newer, with `/sys/fs/bpf` mounted (the pinned state
store's home). At attach time the chain probes the kernel's surfaces:
`fentry` when the kernel carries BTF for `input_event`, `kprobe`
otherwise — both lanes ship, no flag decides it. Below the floor the
attach refuses honestly and says so (KERNEL_COMPATIBILITY.md carries
the full matrix). Unprivileged BPF, where the distro allows it, keeps
the control-plane verbs user-owned; where it does not, `sudo
zylaxion attach` still plants everything and hands the pins over.

## Ultra scale (fleets, kiosks, CI, many machines)

### Why does zylaxion scale to many machines without config management?

The binary is the whole state: zero daemons, zero config files, zero
sample assets, one embedded eBPF object. N machines run the identical
bytes, so the fleet's drift surface is the package version alone —
`zylaxion -V` prints the exact lane (Build:), commit (Build-time and
the sha chain), and version on every box, and the doctor renders the
same block everywhere. Configuration that would otherwise be fleet
state is either compiled in (the preset registry) or per-invocation
(the flags).

### Can scripts select the preset without flags?

Yes — `ZYLAXION_PRESET` resolves in CI, cron, kiosk, and container
contexts where argv plumbing is awkward. It sits second in the
resolution chain, so an explicit flag still overrides it where both
exist.

### What does the kernel-side telemetry cost at fleet scale?

Nothing in userspace until someone reads it. The tap accumulates
per-key counters in pinned kernel maps while typing happens;
`zylaxion status --verbose` reads them on demand. There is no metrics
daemon to run, no exporter to feed — the observability surface is the
same binary, which is what keeps a fleet of them boring to operate.

### Is the passive tap safe alongside automation and heavy input?

The tap is a reader: it observes `input_event` streams through its
fentry/kprobe chain and never injects, filters, or delays them. It
has no network surface in the audio path, persists no key content
(the input-data discipline, SECURITY.md), and the one network verb in
the binary (`--check-update`) refuses to run under root. Keyboard
automation, remappers, and high-rate typists sit upstream untouched.

### Which build reports what on `-V`?

The label grammar (night-dragon-maintenance-T1): a downloaded release
reads `Build: pro-<shape>`, a local alias build reads `Build:
local-pro-<shape>`, a plain local `cargo build` reads `Build:
local-<os>-<arch>-<libc>`, and the parenthesized sha names the exact
commit — recovered even from a crates.io install via
`.cargo_vcs_info.json`. A fleet audit is therefore one
`zylaxion -V` per box, greppable (`zylaxion -V | grep Build:`).

## Objections from the wild

### Why eBPF for keyboard sound? eBPF is for servers, networks, security.

eBPF is mostly used there — that is true. zylaxion is the exception.
A passive probe on `input_event()` is a desktop-interaction application
of eBPF that is almost unexplored, not because it is impossible but
because few have tried. zylaxion uses eBPF for a specific reason: it
is the only way to achieve zero daemon, zero device grab, zero config
file. A userspace daemon can do keyboard sound. It cannot do it
without a resident process, a device grab (or `input` group
membership), and a mapping file. eBPF can. That difference is the
whole product. See
[EBPF_ONLY_VS_DAEMON.md](EBPF_ONLY_VS_DAEMON.md) for the honest
pros-and-cons ledger.

### Isn't this just a keyboard sound tool? Why is it priced like architecture?

The keyboard sound is the proof of concept. The architecture is the
asset. See "What You're Actually Licensing" in the README (five
patterns: eBPF input tap, daemonless control plane, Zactrix Engine
LTS-frozen with six proofs, zero-sample DSP discipline,
micro-randomization law) and "Use Cases Beyond Keyboard Sound"
(accessibility, VDI, embedded devices, game engines, security
tooling, hardware keyboards). The Individual tier ($99) licenses the
keyboard sound. The Company tier ($29,999) licenses the architecture.
Different products, different prices.

### How was this built with AI?

With AI assistance, like every modern project. The boundary is
documented: AI writes, the owner decides. Every diff is read manually,
every verifier rejection is debugged manually, every test is run on
the owner's machine before push, every commit is signed. The owner is
responsible for every line — not for having typed every line. See
[AI_AGENTS_READ_THIS.md](../AI_AGENTS_READ_THIS.md) for the boundary
that applies to outside agents.

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
