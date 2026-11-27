<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
# The night-cafe-mclaren-1 version-adaptivity audit (2026-11-27)

One task, one question: is `scripts/dev/version-to.sh` genuinely
adaptive — does every Cargo.toml the repo authors, anywhere in the
tree, follow the main version on every bump, today and for every lane
added tomorrow — or does the adaptivity stop where the script's own
list stops? The owner's framing: bump once (say `v100.0.0-dragon.1`)
and every manifest that carries a version follows, wherever it sits, a
McLaren design where the machine, not the operator, keeps the parts in
step. This audit answers for the whole bump surface at commit
`ae03236` (v80.5.7). Honest verdicts only; every finding below was
reproduced by command, not inferred from reading alone.

## Method

The audit read the full bump flow (758 lines: the scan, the rewrite,
the lock resyncs at two speeds, the ledger, the README probe, the
prebuilt-lane refresh, the recovery door), then traced every version
surface in the tree (`grep -rn` for the live version across the repo),
then verified the CI arms: which gates run in which workflow, what the
prebuilt parity gate holds, and what no gate holds. Every suspicion
was reproduced against the real tree before it became a finding.

## Findings

1. **The adaptive core exists and is dynamic — the owner's promise
   already held at HEAD.** `sibling_scan()` runs `find` over the whole
   tree and honors every manifest carrying its own `version = ` line.
   No lane is named in the script; a `cosmostrix-*` manifest created
   tomorrow rides the next bump with zero script changes. Verified by
   the scan output at HEAD: `ebpf/`, `proofs/`, `fuzz/`, and
   `scripts/audit/harness/` all discovered, none hardcoded.

2. **BUG (reproduced): generated and third-party manifests were
   discovered as lanes.** The scan excluded only the root `./target/`
   and `./.git/` prefixes. Two planted manifests were discovered by the
   shipped scan and would have been force-rewritten to the release
   version on the next bump:
   `scripts/audit/harness/target/package/zylaxion-harness-80.5.7/Cargo.toml`
   (the `cargo package` extraction shape, valid at any lane with a
   nested `target/`) and `vendor/fake-dep/Cargo.toml` (the
   `cargo vendor` shape — a rewrite there ships a checksum failure, not
   a lane). Fixed in `85851b3`: the exclusion is a path-SEGMENT law
   (`*/target/*`, `*/vendor/*`, `*/.git/*`), so it holds at any depth.
   Proof: the same planted manifests are absent from the scan after the
   fix, the four real lanes still list.

3. **The header's sibling list was stale.** The script's own header
   claimed "today that is ebpf/, fuzz/, and scripts/audit/harness/" —
   `proofs/` was discovered and bumped by the scan but absent from the
   doc. Fixed in the same commit (the exact stale-doc class the
   disclaimer warns about).

4. **No CI arm audited the parity contract between bumps.**
   `check-prebuilt-parity.sh` holds the ebpf lane's tree pin
   (blake3 + version mirror), but a future lane drifting from the root
   — or a lane lock going stale — could build, test, and ship with no
   gate refusing. Fixed in `9e5143f`: `scripts/gates/check-version-parity.sh`,
   a read-only gate that imports `sibling_scan()` and `current_version()`
   VERBATIM from the shipped bump script (two implementations of one
   discovery law is how a gate learns to lie) and asserts root == every
   own-version sibling manifest, every sibling lock == its manifest,
   root lock == root package. Wired as gate-keepers section 11.
   Proof: exit 0 on the clean tree (four lanes, four lane locks, root
   lock, all 80.5.7); exit 1 on a planted
   `lanes/cosmostrix-abc/Cargo.toml` at `99.9.9`, naming the manifest
   and the heal.

5. **The changelog battery was red on the shipped tree, unwired to
   CI.** `test-version-to-changelog.sh` ran 18 passed / 6 failed at
   clean HEAD: the F1 fixture builder appended its seed at end of file,
   an assumption that only holds while `## [Unreleased]` is the last
   section — the live ledger carries the Carried record and Frozen
   archive sections below it, so the seed landed in the archive zone
   and the carry (correctly) refused. The failures were the fixture's,
   not the tool's: F7/F10 passed on the same extracted functions, the
   live ledger spacer shape is legal, and the next real bump's ledger
   line and carry are healthy. Fixed in `e398576` (the seed splices
   directly under the anchored heading): 24 passed / 0 failed.

6. **The discovery lane had no test of its own.** Fixed in `dce990f`:
   `scripts/dev/test-version-to-sibling-scan.sh` extracts the scan
   verbatim and proves ten fixtures (11 checks): the owner's exact
   `cosmostrix-abc` example discovered, depth irrelevant, generated /
   vendored / `.git` / `version.workspace = true` manifests invisible,
   the bump rewrite heals every discovered lane, a stale lane cannot
   survive a bump, the planted generated manifest survives untouched,
   the scan deterministic. Both batteries now ride the wholesale run
   (gate-keepers sections 12-13) — CI teeth for the whole bump lane.

## What was not changed, on purpose

- No version bump: versions move only through the sanctioned tool, by
  the Owner. Every file in this audit stays at 80.5.7.
- No CHANGELOG restructuring: the historical sections below
  `## [Unreleased]` (Carried record, Frozen notes, the pre-ledger-3
  version sections) are records, not drift; the carry and the heal
  handle the shape. The battery fixture was made shape-tolerant
  instead.
- No behavior change to the rewrite sed, the lock resync, or the
  ledger writer: the audit found no defect there; the ones it found
  live in discovery, CI coverage, and the test fixture.

## Verdict

The owner's adaptivity promise is now real end to end: dynamic
discovery (any lane, anywhere, any name), generated trees excluded at
any depth, a CI gate that fails the push when a version surface drifts,
and two batteries wired into the wholesale run so the lane cannot
silently regress. Zero stale: the header list, the maintenance
handbook, and this audit name the same four lanes the scan discovers.

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
