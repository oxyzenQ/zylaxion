<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
# zylaxion Maintenance Handbook

The maintainer lane: the commands and contracts that move versions,
cut releases, and keep the gates green. Everything here is the
Owner's jurisdiction; contributors read this to understand why the
gates exist, not to run the levers.

## The version ladder

The single sanctioned tool is `scripts/dev/version-to.sh`:

```bash
./scripts/dev/version-to.sh v80.2.0 --tagline "growth naturally" --commit
```

What it does (all lanes verified in-situ):

1. Validates the version shape (stable semver and the pre-release
   family `vX.Y.Z-rc.N` are both accepted).
2. Rewrites the package version — the root manifest (night-improve-21:
   one package) and every own-version sibling manifest the scan
   discovers, wherever it sits (night-improve-9 generalized; today
   `ebpf/`, `fuzz/`, `proofs/`, and `scripts/audit/harness/`, each
   carrying its own `version = ` line). night-cafe-mclaren-1: discovery
   is dynamic — a manifest added tomorrow (a new test crate, a
   `cosmostrix-*` lane) rides the next bump with zero script changes —
   and segment-scoped: any `/target/`, `/vendor/` or `/.git/` path
   component is excluded at any depth, so cargo-package extractions,
   vendored upstream manifests, and `.git` trees are never rewritten as
   if they were lanes.
3. Syncs `Cargo.lock` through `cargo update --workspace --offline`,
   then verifies every member's lock entry actually moved (third-
   party entries untouched). Each touched sibling's own detached lock
   resyncs the same way (night-improve-27).
4. Writes the `[Released]` ledger entry into CHANGELOG.md — honestly:
   an absent file, an already-recorded version, or a missing
   `[Unreleased]` anchor is a warning and a no-op, never a fabricated
   entry.
5. Carries the release detail (night-ledger-1, placement fixed by
   night-ledger-3): the notes the cycle accumulated under
   `[Unreleased]` move verbatim into the version's own section —
   `## [vX.Y.Z] — tagline — date` — spliced directly under the
   `[Released]` ledger and above the running section, so everything
   released sits above `[Unreleased]` and a shipped change is never
   read from the unreleased zone (nine releases piled up there before
   this: v80.0.3 through v80.2.1 shipped with no section of their
   own). The date comes from the system clock in UTC; the tagline is
   optional and rides the release commit subject when given. An
   ambiguous ledger (more than one `[Released]` or `[Unreleased]`
   heading) refuses the carry; an empty running section is the
   expected steady state after a clean carry, not a failure.
6. Leaves the tree auditable (night-cafe-mclaren-1):
   `scripts/gates/check-version-parity.sh` — gate-keepers section 11,
   the read-only CI arm — asserts between bumps that every own-version
   sibling manifest equals the root version, every sibling lock equals
   its manifest, and the root lock equals the root package; it imports
   the bump's own scan functions verbatim, so the gate and the bump can
   never disagree about what a lane is. The bump lane itself is
   regression-tested by `scripts/dev/test-version-to-changelog.sh`
   (the ledger and carry battery) and
   `scripts/dev/test-version-to-sibling-scan.sh` (the discovery
   battery: any lane the repo authors rides the bump, generated trees
   stay invisible) — gate-keepers sections 12-13.

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
configured, then the inventory tripwire, the DRAFT release, and the
un-draft publish. night-dragon-maintenance-6: the release is created
as a draft and only un-drafted after every asset has uploaded —
GitHub's immutable-release feature locks assets at publish time, so
publish-on-creation would reject the tail of a multi-asset upload
(the mature estate's v80.0.1 incident). A red pipeline leaves a
draft behind, never a half-published release. The full contract
lives in the workflow header (`.github/workflows/release.yml`).

Before pushing a tag, run locally:

```bash
cargo pro-linux-amd64-v3-gnu && ./target/x86_64-unknown-linux-gnu/pro-linux-amd64-v3-gnu/zylaxion -V
# Build: must read local-pro-linux-amd64-v3-gnu — the same label chain
# the release artifact reports (its local- marker is the only delta).
./scripts/gates/check-release-parity.sh
```

## The owner's machine audit

The owner runbook is `scripts/audit/owner-forensic-audit.sh`
(DEPTHBORE-1): five unprivileged tests against the real desktop
machine — idle CPU, active CPU, memory footprint, SIGKILL recovery,
and clean signal stop — run as the user who runs `zylaxion on`, with
the tap already attached. It prints raw measurements next to every
verdict and runs on the owner's machine, never in CI.

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
| 10 | Documentation disclaimer (placement check; normalize is the explicit repair command) | `scripts/gates/inject-disclaimer.sh --check` |

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

The weekly lane (`Dragon Guard - Dependency Maintenance`) refreshes
EVERY tracked lock, not just the root one (night-zen-1): the repo
carries four lock surfaces (root, fuzz/, ebpf/,
scripts/audit/harness/), and a bare `cargo update` at the root left
the sibling locks drifting. The lane runs
`scripts/dev/update-deps.sh`, which auto-detects each tracked
manifest via `git ls-files` and updates in place. When the ebpf tree
moves (its lock in practice), the same script regenerates the
prebuilt lane through its one writer
(`scripts/dev/regenerate-ebpf-prebuilt.sh`) — the rebuilt object and
rewritten manifest ride the same maintenance commit, so
`check-prebuilt-parity.sh` never trips on a stale pin. The lane
already bootstraps the ebpf prerequisites before the update step,
which is what the regen path requires.

The same lane carries the Sunday security sweep (AXP1-ZnQ —
Architect's eXecution Protocol 1, born night-solid-defense-1 — the
owner's supply-chain sentinel,
`scripts/dev/security-sweep.py`), the watcher that turns the weekly
run into a security verdict the sleeping owner can trust. It audits
every tracked lockfile with `cargo audit --format json` twice — once
before the update (the HEAD tree snapshot) and once after — and
classifies the diff into the two scenarios that matter: an advisory an
in-range bump clears is reported in the job summary as an urgent
auto-update and the run stays green (the tree was exposed before the
sweep, it is clean now — without the lane it would have carried the
advisory for a week — and the update commit body carries the
sentinel's note); a finding nothing in the current requirement set can
reach keeps the run RED after the update commit lands — the summary
says CALL THE OWNER and names the reason per finding (a patched
release exists but no requirement reaches it, so the manifest must
move deliberately, or no patched release is published at all, so the
dependency wants replacing). Identity (night-solid-defense-1): the
verdict speaks under the owner's sentinel name — every banner,
GitHub annotation, and summary signs as AXP1-ZnQ (Architect's
eXecution Protocol 1), so a red Sunday
names its watcher (the zejtron shape, ported whole). One deliberate
divergence from that reference: zejtron's verdict gates the commit
lane and ships nothing while red; this tree lands the fixable bumps
first and raises the red after — the owner's explicit contract here.
The bar is the canonical lane's bar: remaining unmaintained, unsound
and yanked warnings fail the run the same way `--deny warnings` fails
them in `audit.yml`, a scanner that cannot run is a red verdict (never
a silent green), and the final verdict step is the job's only exit-1
point so fixable bumps always land before the red is raised. The
canonical push lane (`audit.yml`) and the local gatekeeper
(`scripts/build.sh check-all`) scan the same five-lock surface, so no
lane can certify a tree a sibling lock is poisoning. The scan-only
dispatch (`scan_only`, the zejtron night-supply-1 red button) runs the
whole verdict on the committed tree any day of the week and ships
nothing — no update, no checks, no commit.

The same lane also carries the address refresh (AXP1-ZnQ —
Architect's eXecution Protocol 1, night-learning-into-darkness-1 —
the protocol's front door is `docs/AXP1-ZnQ.md`): after `cargo test`
and before the commit step, `scripts/dev/axp1znq_refresh.py` re-probes
the whole `repos-for-ci/` catalog concurrently — an address survives
only when it answers AND serves a fresh index (the InRelease `Date:`
header is the freshness witness against `--max-age-days`, default 14)
— and writes the survivors back in their authored order, the dead and
stale left as dated tombstones the owner revives by deleting one
line. The write law is bounded and guarded: the 100-line cap prunes
the oldest tombstones first, an alive pool under the 10-address floor
(`--min-lines`) warns by name, and the never-empty guard holds a
zero-alive catalog untouched — that warning names CALL THE OWNER, the
one critical the refresher escalates. The refresher never blocks the
ship lane (it exits 0 with its warnings recorded); when the catalog
moved, its note rides the weekly commit body next to the sweep's
note, so the one commit carries deps AND addresses. The apt fallback
was the protocol's first consumer; every CI lane now walks the
resolver too (17 fallback sites over 9 workflows, the port landed
estate-wide the same night) — no repoint target is a hardcoded server
anymore, and a catalog with nothing alive fails the lane loud.

## The root-owned pycache trap (the zelynic incident, documented)

Anatomy of the incident (observed on the zelynic sibling, recorded
here so the failure shape can never repeat on this tree): Python
scripts that run under `sudo` import project-local modules, and
CPython answers every import by writing
`__pycache__/<module>.cpython-*.pyc` beside the source — as the
invoking user. Under sudo that user is root, so the cache files are
owned by root while the rest of the tree stays user-owned. The next
plain-user `rm -rf` of the checkout then dies mid-flight with
`Permission denied` on exactly those cache files, and the half-deleted
tree looks corrupt for no functional reason.

The prevention now lives at the source, on every entry point that
can reach a root invocation:

- `scripts/supermassive/supermassive-test.py` sets
  `sys.dont_write_bytecode = True` before its first project-local
  import and exports `PYTHONDONTWRITEBYTECODE=1` for every python
  child it spawns;
- `scripts/supermassive/sm_static.py` (also runnable directly) sets
  the same flag before importing `sm_report`;
- `scripts/supermassive/supermassive-test.sh` (the wrapper the CI
  leg drives) exports `PYTHONDONTWRITEBYTECODE=1` before the exec.

With bytecode suppression on, a root-run supermassive leg leaves no
cache file behind, so no user-owned tree ever again carries a
root-owned file. The gate scripts that invoke python (`python3 -c`
one-liners, heredoc feeds) import no project module and never had
the exposure. Manual escape hatches, should a foreign script ever
reintroduce the shape: run python with `python -B` (or the env var
above), and clean an already-poisoned tree once with
`sudo rm -rf scripts/**/__pycache__` (or
`sudo chown -R "$(id -u):$(id -g)" <path>`) before the plain-user
removal.

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
- the README's pricing display mirror (night-dragon-maintenance-X1,
  owner call): the Commercial Licensing section carries the tier
  table verbatim from COMMERCIAL_LICENSE.md section 3, its home
  named above the table; a price move refreshes both copies in the
  same commit — the sync rule is what keeps the mirror from
  re-creating the drift that landed this rule.

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
