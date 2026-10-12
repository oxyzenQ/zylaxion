<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Security Policy

zylaxion reads every keystroke you type and turns it into sound — it
touches the input event stream and the audio server on purpose. That
trust position makes security part of the product, not an
afterthought. This file is the security foundation: how to report,
what we support, what counts as a vulnerability, and where the
boundaries sit.

## Supported versions

| Version | Branch | Status |
|---------|--------|--------|
| 11.x (main) | `main` | **supported** — active maintenance (bug fixes, distro compatibility, hardening) |

Pre-release channels (`alpha`, `beta`, `rc`) are development
artifacts: reports against them are welcome and triaged, but fixes
land on `main` first.

## Reporting a vulnerability

**Do not open a public GitHub issue for security problems.**

Use GitHub's **private vulnerability reporting** on this repository:
*Security tab → "Report a vulnerability"*. If that button is disabled
on your view, open a **draft security advisory** instead
(*Security → Advisories → New draft security advisory*) — both routes
reach the maintainer privately.

Please include:

- the affected command/flag or code path (`zylaxion -V` output helps),
- a minimal reproduction (commands, expected vs. observed),
- impact assessment — who can trigger it and what it costs.

You will get an acknowledgment within a reasonable window; fixes ship
as patch releases on `main` with credit in the changelog unless you
prefer to stay anonymous. Coordinated disclosure: we ask for up to
90 days before public disclosure, and we will not pursue anyone who
acts in good faith.

## Scope

In scope:

- The Rust CLI and everything it executes (`src/`),
  including the update-check surface and the pinned control plane
  (`/sys/fs/bpf/zylaxion/`) — bpffs file permissions govern who can
  write your sound state, and any weakening of the attach-time
  ownership hand-off is a vulnerability.
- The input tap lane (`ebpf/src/`, `src/ebpf/`) — the kernel-side
  tracepoint program and its loader. Keyboard event data must stay in
  the synthesis path only: it is never written to disk, never sent
  anywhere, and never logged with key content. Any persistence or
  transmission of keystroke content is critical by definition.
- The audio engine lanes (`src/engine/`, `src/profiles/`,
  `src/core/`) — the
  synthesis DSP, the orchestrator, and the model swap point.
- The preset resolution surface (`src/session/preset.rs`) —
  environment-variable handling (`ZYLAXION_PRESET`,
  `ZYLAXION_VOLUME`) is in scope: injection into log lines,
  parse-path panics, and the no-silent-fallback contract. The
  config-era filesystem watcher and TOML parser were removed with
  night-improve-22 and with them their TOCTOU and path-traversal
  surface.
- The player lifecycle (`src/commands/player.rs`) — the stateless
  player, its control-map registration, and signal handling. (The
  daemon lifecycle died in night-improve-23: no daemonization, no PID
  file, no instance lock, no IPC socket — and none of their attack
  surface.)
- The release pipeline and artifacts: workflows
  (`.github/workflows/`), tarballs, checksums, signatures, install
  scripts.
- The security-relevant UX contract: the input group membership
  guard, the `XDG_RUNTIME_DIR` checks, and the doctor's verdicts.

Out of scope (by design, not neglect):

- **Anything requiring the attacker to already control your user
  session.** The operator running zylaxion owns the session; an
  attacker who can execute code as your user can already read your
  keystrokes directly from the same evdev devices zylaxion reads.
- **The sound of your typing as a side channel.** zylaxion's entire
  purpose is to make keystrokes audible; acoustic side-channel
  analysis of an intentionally audible keyboard is a research topic,
  not a vulnerability in zylaxion.
- Volume/quality complaints, feature requests, and preset tuning —
  those belong in issues, not the security lane.

## The input-data discipline (the product's core security promise)

zylaxion consumes kernel key events (the eBPF tap) in real time to
trigger
synthesis. The standing contract, enforced by review and by the
gate-keepers:

1. **No key content persists.** Event codes drive synthesizer
   parameters in memory; nothing about which key was pressed is
   written to any file, log, or IPC message.
2. **No network in the audio path.** The synthesis engine has no
   outbound connections; the only network surface is the explicit
   `--check-update` query and the `log`/`env_logger` stderr stream.
3. **The control plane has no protocol at all** (night-improve-23):
   there is no socket, no daemon, no IPC. The pinned BPF maps are the
   only control surface, and bpffs file permissions decide who can
   write them — the attach hands ownership to the invoking sudo user,
   so other users can neither steer nor stop your sound.

Reports that show a violation of any of the three above are
critical-severity by default, regardless of exploit complexity.

## Hardening surface (what the binary already does)

- The player refuses to double-run: the control map's player-pid
  slot is the single-instance marker (a live pid means already
  playing; a dead one is taken over) — no lock file to go stale.
- The tap is gated in the kernel: with the enabled slot clear, the
  tracepoint program writes nothing, wakes nobody, and counts nothing.
- The doctor surfaces the trust-relevant facts (kernel version
  verdict, bpffs mount, lockdown mode, tracepoint visibility, audio
  server reachability) so an operator can verify the environment
  instead of guessing.
- Release artifacts ship with sha256 checksums and cryptographic
  signatures — see [docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md)
  for the verification walkthrough.

## Audit trail

Security-relevant changes are recorded in the changelog under the
release that fixed them. Historical audit evidence lives under
`docs/` (the `audit-*` files); future audit rounds append there
rather than rewriting history.
