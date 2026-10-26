<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion Maintenance Handbook

The maintainer lane: the commands and contracts that move versions,
cut releases, and keep the gates green. Everything here is the
Owner's jurisdiction; contributors read this to understand why the
gates exist, not to run the levers.

## The version ladder

The single sanctioned tool is `scripts/dev/version-to.sh`:

```bash
./scripts/dev/version-to.sh v11.1.0
```

What it does (all lanes verified in-situ):

1. Validates the version shape (stable semver and the pre-release
   family `vX.Y.Z-rc.N` are both accepted).
2. Rewrites the package version — the root manifest (night-improve-21:
   one package; the sibling-manifest scanner stays honest for the day
   a second manifest appears) — in one pass.
3. Syncs `Cargo.lock` through `cargo update --workspace --offline`,
   then verifies every member's lock entry actually moved (third-
   party entries untouched).
4. Writes the `[Released]` ledger entry into CHANGELOG.md — honestly:
   an absent file, an already-recorded version, or a missing
   `[Unreleased]` anchor is a warning and a no-op, never a fabricated
   entry.

The rule around it: versions move ONLY through this tool, by the
Owner, and a release tag is only ever pushed by the Owner. A tag
pushed while `Cargo.toml` still names the previous version fails the
release pipeline's version-contract check before any build cost.

## The release lane

`Dragon Guard - Release` fires on `v*` tags (plus the
workflow_dispatch recovery lane — re-run on the tag ref, nothing
re-pushed). The pipeline: validate the channel and the version
contract, build the x86-64-v3/v4 gnu legs with `--locked` and the
`ZYLAXION_BUILD` platform stamp, verify each artifact (v3 by
execution, v4 by byte-grep — the AVX-512 runner rule), package the
flat tarball, three checksums, GPG signatures when the key secret is
configured, then the inventory tripwire and the publish. The full
contract, including the musl dormant lane's evidence, lives in the
workflow header (`.github/workflows/release.yml`).

Before pushing a tag, run locally:

```bash
cargo pro-linux-amd64-v3-gnu && ./target/x86_64-unknown-linux-gnu/pro-linux-amd64-v3-gnu/zylaxion -V
# Build: must read local-linux-amd64-v3-gnu — the same label chain
# the release artifact reports.
./scripts/gates/check-release-parity.sh
```

## The gate-keepers

`./scripts/gate-keepers.sh` before every push; CI re-runs the same
sections. The sections and their home gates:

| # | Section | Gate |
|---|---------|------|
| 1 | File permissions (644/755, shebang parity) | `scripts/gates/check-permissions.sh` |
| 2 | SPDX headers | `scripts/gates/check-headers.sh` |
| 3 | Rust LOC cap (600) | `scripts/gates/check-loc.sh` |
| 4 | Scripts LOC cap (1000) | `scripts/gates/check-scripts-loc.sh` |
| 5 | Language discipline | `scripts/gates/check-language.sh` |
| 6 | Emoji sweep | inline in `gate-keepers.sh` |
| 7 | Actions pins health | `scripts/gates/check-actions-pins.sh` |
| 8 | Release parity | `scripts/gates/check-release-parity.sh` |
| 9 | Name-case rule (lowercase law, nothing excluded) | `scripts/gates/check-name-case.py` |

A new gate joins this table and the orchestrator in the same commit —
a gate that is not wired is a wish, not a gate.

## The honest ledgers

Two ledgers the maintainer owns and shrinks over time:

1. **The LOC debt list — paid.** The six pre-gate files that carried
   `// LOC_EXEMPT:` markers were split in night-improve-18 and the
   marker lane is retired for Rust: over the cap means split, the
   same hour (a pure move — new module files, byte-identical
   behavior, gates green before and after). `scripts/gates/check-loc.sh`
   also enforces the layout law: src roots hold only their entry
   file, tests live under the package's test/ tree.
2. **The actions-pin estate** — the SHA-pinned third-party actions.
   `check-actions-pins.sh` self-heals drift in `auto` mode and warns
   otherwise; majors are NEVER auto-healed — a major move is a
   maintainer decision, taken once, deliberately.

## Dependency hygiene

The tree carries no time crate and no regex crate (the
night-improve-13 contract) — keep it that way. When a dependency
wants to join, it answers one question in the PR body: which job,
currently done by std or an existing dependency, cannot be done
without it. Feature-flag off what a dependency pulls but zylaxion
never uses (the env_logger `default-features = false` shape is the
precedent).

## The tell-once law (the docs' dedup rule)

Every fact, story, and claim lives in ONE canonical home: the kernel
floor story in KERNEL_COMPATIBILITY.md, the licensing tiers and prices
in COMMERCIAL_LICENSE.md, the law book in RULES.md, the threat model
in ZACTRIX_SECURITY.md. Every other doc that needs the fact either
links the home or carries a summary row that cannot drift silently —
never a second full copy. A duplicated table is a drift waiting to
happen; the pricing table that once lived in the README's licensing
section had already drifted from its canonical twin when this rule
landed, which is the whole argument for the rule in one example.

The sanctioned exceptions, by law rather than by accident:

- the per-file disclaimer block — injected, identical by
  construction, and the injector gate holds its placement;
- the doc-header block (owner/repository/license) — structural
  boilerplate every document carries;
- the CHANGELOG and the frozen audit records — they QUOTE the
  canonical docs as historical evidence; they never retell.

When a doc must repeat a number, it repeats it with its home named,
so the next reader knows which copy wins. The audit that landed this
rule (night-dragon-maintenance-5) walked every 12-word narrative
shingle shared by two living docs and classified each pair; the
remaining shared text is lawful reference (summary rows that name
their home, threat-model table rows, quoted command recipes, the
historical records above).

## Secrets the pipeline expects

| Secret | Used by | Absent behavior |
|--------|---------|-----------------|
| `CRATES_IO_TOKEN` | crates.io publish job (`CARGO_REGISTRY_TOKEN` mapping) | the publish fails fast on the presence check |
| `GPG_PRIVATE_KEY` | release build job | releases ship checksums-only |
| `GPG_PASSPHRASE` | release build job | key imported without a passphrase |

Neither blocks a release. The signing lane is the upgrade, not the
dependency. `CRATES_IO_TOKEN` is the one hard requirement on its own
lane: the tag-driven publish job refuses to run without it (the
secret maps onto cargo's `CARGO_REGISTRY_TOKEN` inside the workflow —
the repository secret is always named `CRATES_IO_TOKEN`, the same
one-name law the sibling zejtron estate keeps, so no workflow ever
grows a second spelling of the same token). The `GITHUB_TOKEN` the
workflows read is GitHub's built-in `${{ github.token }}` — it is
never a repository secret to configure and never appears under
another name.

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
