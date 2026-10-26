<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion Documentation Map

Every document in this tree, with the reading path that fits your
situation. The rule that keeps this tree honest: source code is the
single source of truth — a doc that disagrees with the tree is wrong,
and a PR fixing the doc is welcome (open one; the gate-keepers keep
the tree English-only and header-complete).

## I want to...

### ...run zylaxion

The [README](../README.md) is the user manual: install, the
service setup, the command surface, the compiled preset registry,
presets. Start there; come back here when you want depth.

### ...verify a downloaded binary

[VERIFY_RELEASE.md](VERIFY_RELEASE.md) — the three checksum families
(SHA-512, BLAKE3-256, and the quantum-resistant SHAKE256), the GPG
signature lane when the release carries signatures, and the smoke
check that compares the binary's own `Build:` label against the
tarball's name.

### ...contribute

[CONTRIBUTING.md](../CONTRIBUTING.md) — build prerequisites, the
six-crate structure, the gate discipline. [CLA.md](../CLA.md) — the
dual-license contribution grant (one `Signed-off-by:` line accepts
it). [RULES.md](RULES.md) — the non-negotiables in their shortest
form.

### ...report a security issue

[SECURITY.md](../SECURITY.md) — the private reporting route, the
scope (including the input-data discipline: no key content persists,
no network in the audio path), and what is deliberately out of scope.

### ...license zylaxion commercially

[COMMERCIAL_LICENSE.md](../COMMERCIAL_LICENSE.md) — the dual-license
contract, tiers, payment, verification. zylaxion is GPL-3.0-only for
open use; the commercial lane exists for the uses copyleft cannot
honestly cover.

### ...maintain the project

[MAINTENANCE.md](MAINTENANCE.md) — the maintainer lane: the version
ladder (`scripts/dev/version-to.sh`), the release pipeline, the
gate-keepers, and the honest ledgers (the 600-line-law debt list).
[KERNEL_COMPATIBILITY.md](KERNEL_COMPATIBILITY.md) — the kernel floor
and the per-distro bpffs layouts the tap meets, each entry resolved
with its evidence.

### ...understand the design

[PHILOSOPHY.md](PHILOSOPHY.md) — why the boring and stealth.
[EBPF_ONLY_VS_DAEMON.md](EBPF_ONLY_VS_DAEMON.md) — the honest ledger
of the eBPF-only bet: what the daemon deletion wins, what it costs,
and who the trade serves.
[BRANDING.md](BRANDING.md) — the palette contract (brand purple
`#A855F7`, the CLI color tiers). [QA.md](QA.md) — verified answers,
one question one honest answer. [GLOSSARY.md](GLOSSARY.md) — the
vocabulary. The historical audit evidence (`audit-v6.0.0*.md`,
`release-readiness-v6.0.0.md`, `archive/`) is the frozen record —
never rewritten; future audit rounds append new files.

## The frozen-record class

`CHANGELOG.md` (root) and the v6.0.0-era files are history: the
gate-keepers exclude them from the emoji/language/header-demand lanes
by name, because history is never rewritten to satisfy a linter. New
facts get new entries, not edited ones.

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
