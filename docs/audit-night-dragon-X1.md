<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# The night-dragon-maintenance-X1 end-to-end audit (2026-11-02)

Four task IDs, one tree: the asset signature ledger, the serde ban's
enforcement seat, the CI rate-limit authentication, and this document
— the end-to-end real-usage audit of every CLI surface against a
freshly built release binary. Honest verdicts only: every skip names
why, every finding names its fix, and every measurement names the
sandbox that produced it.

## Method and sandbox honesty

The repo is MEDIUM weight at this tree (108 tracked `.rs` files,
~24.2k lines; `scripts/` ~7.1k shell lines; 247 tracked files), and
the whole tree was read where it mattered: the CLI enum, the command
handlers, the gate scripts, the build lanes, the workflows, and the
test tree's wiring. The sandbox: non-root uid, no `sudo`, kernel
5.10.134 (below the 5.15 eBPF minimum), no bpffs mount, no audio
device, no nightly toolchain for the eBPF source-build lane.

Where this audit goes further than its predecessor
(`audit-night-total-lts.md`, which could not compile at all for lack
of ALSA headers): the headers were solved without root —
`apt-get download libasound2-dev libasound2t64` extracted into a
local prefix and fed to the build through `PKG_CONFIG_PATH` plus a
`-L native=` RUSTFLAGS token. The release binary compiled, linked,
and ran: every surface below is a live run of
`target/release/zylaxion` at commit 1941bf0, profile release,
`local-linux-amd64-gnu`.

The eBPF object rode the registry-prebuilt lane (the same lane a
crates.io tarball build takes; the manifest's blake3 pin verified the
staged object). The source-built lane needs the dated nightly pin and
bpf-linker, which the sandbox cannot carry — that lane is CI's and
the release builder's, and the parity gate already polices it at
commit time.

## The surface matrix — live runs, exit codes as printed

| surface | verdict | exit |
|---------|---------|------|
| `-V` | version, build label, commit hash, license — all correct; the hash matches HEAD | 0 |
| `--help` | the full reference renders; the pro-mode block lists every hidden/advanced token | 0 |
| bare invocation | the quick status overview; honest kernel verdict | 0 |
| `status` | kernel "verified unsupported" with the release string; tap "not attached" | 0 |
| `status --verbose` | BUG FOUND — byte-identical to plain `status`, the flag silently no-opped (fixed, below) | 0 |
| `doctor` | the master report: build facts, kernel verdict with the 5.15 minimum named, BTF present, bpffs absent with the mount fix, XDG_RUNTIME_DIR fix, audio honest — exit 1 with fixes on screen | 1 |
| `list-presets` | 24 presets, one line each, `<- active` marker — the shape `sm_static.py` parses | 0 |
| `list-backends` | ALSA honest: "present but not openable" on a cardless host, no panic | 0 |
| `on`, `on --preset` | refuses at the kernel gate first, then the tap gate — both refusals name the next command | 1 |
| `off` | idempotent: "sound is off (the tap is not attached — nothing to stop)" | 0 |
| `preset --switch` | tap-gated refusal with the attach tip | 1 |
| `preset --switch does-not-exist` | names the unknown preset AND prints the full available list | 1 |
| `volume --percent 60` | tap-gated refusal | 1 |
| `volume --percent 201` | 200% safety-cap refusal before any kernel I/O, names `--force-this` | 2 |
| `volume --percent 10001 --force-this` | force-cap refusal — but the parenthetical recycled the 200% boundary text (fixed, below) | 2 |
| `attach` | non-root refusal names CAP_BPF/CAP_PERFMON and the sudo tip | 1 |
| `detach` | idempotent: "tap is not attached — nothing to detach" | 0 |
| `_play` (hidden) | control-map refusal, no hang, no panic, prompt exit | 1 |
| `_play --bogus` | clap usage error, names the bare verb shape | 2 |
| `--check-update` | the sandbox's egress IP was rate-limited (403): the human message printed, exit 0, no panic — the documented informational-flag contract holds | 0 |

NO_COLOR discipline verified byte-level: zero ESC bytes in the piped
output, with and without `NO_COLOR=1` (the box-drawing rules are
Unicode decoration, not SGR color, so they survive by design — the
stripped layer is the color, exactly what `help.rs` documents).

## Findings and fixes (both verified live after the fix)

1. `status --verbose` was a silent no-op whenever the tap is not
   attached. The no-tap branch printed its verdict and returned
   before the telemetry section, so the deep read was byte-identical
   to the plain surface — the flag's own promise ("appends the
   kernel-side typing telemetry") quietly voided on the exact host
   where a user runs it first. Fix: the absent-tap arm now appends
   one honest tip line — the counters exist only while the tap is
   planted — so the flag never lands without an explanation. The
   blocked/reachable arms keep their quiet: their verdict is the
   walk, not the counters, and a second line would dilute it.
2. The force-cap refusal (`volume` above 10000 with `--force-this`)
   ended with "(values above 200 can damage speakers)" — the safety
   cap's rationale recycled under the wrong boundary. Fix: the
   refusal now ends "(nothing above the force ceiling passes)", and
   the test tree's pinned phrases (`exceeds the 10000% force cap`)
   still hold — the pins never covered the parenthetical.

Both fixes are two user-facing strings and one early-return arm; the
acoustic A/B harness is not the applicable instrument (no DSP, no
engine, no audio path touched), and the volume-gate boundary tests
plus a live rebuild re-verified the surfaces.

## Verified clean — behaviors probed and passed

- The refusal ordering discipline: argument validation (unknown
  preset, volume caps) fires before kernel I/O; the kernel gate fires
  before the tap gate; every refusal names the next command.
- Exit-code contract: 0 success/idempotent, 1 runtime refusal,
  2 argument/cap refusals — consistent across every verb probed.
- `off` and `detach` are honestly idempotent instead of erroring.
- `doctor` degrades honestly on every missing host fact and prints a
  runnable fix per line, exit 1 when checks fail.
- The pipe contract: every surface renders plain bytes under a pipe
  (the help text's own promise), probed at the byte level.
- The update checker's error paths: rate-limited API, human message,
  exit 0, no panic — and the eBPF privilege guard refuses sudo
  before any network I/O per night-improve-16 (source-read
  verified; the live probe is the sandbox's non-root run).

## Privilege-gated remainder, named

`attach`, `on`, `_play`'s render loop, and the live preset/volume
writes against a planted tap need a 5.15+ kernel with bpffs mounted
and CAP_BPF — the sandbox carries none of these, and every path this
audit could reach without them was reached. The kernel-side legs are
CI's build matrix and the release lane's job; the refusal paths, the
ordering, and the diagnostics above are the unprivileged contract and
they all hold.

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
