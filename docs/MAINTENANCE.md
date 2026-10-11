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
2. Rewrites the workspace version — the root manifest AND every
   member crate that carries its own version line — in one pass.
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

A new gate joins this table and the orchestrator in the same commit —
a gate that is not wired is a wish, not a gate.

## The honest ledgers

Two ledgers the maintainer owns and shrinks over time:

1. **The LOC debt list** — the files carrying `// LOC_EXEMPT:`
   markers (see `scripts/gates/check-loc.sh`'s output for the live
   list). The endgame is zero markers. A split is a pure move: new
   module files, byte-identical behavior, gates green before and
   after. Schedule it like any version bump — deliberately, not
   under pressure.
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

## Secrets the pipeline expects

| Secret | Used by | Absent behavior |
|--------|---------|-----------------|
| `GPG_PRIVATE_KEY` | release build job | releases ship checksums-only |
| `GPG_PASSPHRASE` | release build job | key imported without a passphrase |

Neither blocks a release. The signing lane is the upgrade, not the
dependency.
