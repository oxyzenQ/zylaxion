<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
# The night-cafe-mclaren-5 package-size audit (2026-11-28)

One task, one question: does the publish pipeline own the size axis —
does anything, anywhere, measure the registry tarball against the
crates.io upload cap before an upload rides on it — or does a tarball
that crossed the cap still sail through every gate and die at the
registry's own endpoint with a 413? The owner's framing: the gate
count had reached nineteen and none of them measured bytes.

## The cap and the law

The crates.io upload endpoint states its own limit in its rejection:

```
status 413 Payload Too Large: max upload size is: 10485760
```

The gate's law is strictly smaller-than against that number
(`scripts/gates/check-package-size.sh`, constant `CAP = 10485760`):
a tarball at exactly the cap fails locally instead of gambling that
the server accepts an exact boundary. If crates.io ever changes the
limit, the constant follows the server's message — the gate tracks
the server's truth, never a private number.

## What the registry proves (measured 2026-11-28)

Every version the registry serves was queried live
(`https://crates.io/api/v1/crates/zylaxion/<version>`, the
`version.crate_size` field, all `yanked=false`):

| Version          | Created (UTC)          | Bytes    |
|------------------|------------------------|----------|
| 80.5.7-dragon.1  | 2026-11-26 06:48:44    | 701869   |
| 80.5.7-dragon.2  | 2026-11-26 08:08:09    | 702388   |
| 80.5.8           | 2026-11-27 21:02:34    | 718154   |

Every published version rides roughly 700-718 KB against the
10485760-byte cap. dragon.1 and dragon.2 are both live, about one
hour and twenty minutes apart, at essentially the same size — the
registry carries no oversized version.

Honesty note, recorded because this round inherited claims from an
earlier session that the live evidence does not support: no commit
mentioning "413" exists in the repository history (the commit search
returns nothing), a previously cited fix SHA (`6617cef4`) resolves to
no commit on the remote, and the registry holds no ~11.25 MiB
version. Those incident details — the exact upload timestamp, the
1m36s failure window, the oversized dragon.2 tarball — are
unverifiable from every source available today. What is verifiable
and what this round shipped on: the cap is real (the server states
it), the allowlist is real (Cargo.toml, night-dinner-11, include-mode
fail-closed), and the size axis had no detector. The detector is now
the deliverable; the incident narrative it memorializes is kept only
where evidence supports it.

## The tarball today (measured, locally)

`cargo package --locked --no-verify --allow-dirty` on the current
allowlist tree produces `zylaxion-80.5.8.crate`:

- 718256 bytes — 6% of the cap, 9767504 bytes of headroom;
- 173 members, zero `webp`/`png`/`gif` members (the allowlist carries
  exactly one `assets/` entry, the GPG fingerprint the build
  include_str!-es at compile time);
- the registry-facing documentation set aboard (`README.md`,
  `CHANGELOG.md`, `LICENSE`, `NOTICE`, and the rest of the compliance
  list in Cargo.toml's include lane);
- the largest member is `CHANGELOG.md` at 227755 bytes, then
  `docs/INNOVATIONS.md` at 82816 bytes — text, not binaries. If the
  tarball ever creeps toward the cap, the ledger itself is the first
  suspect, and the gate's failure diagnostics name exactly that.

## The gate, wired at all three surfaces

The MAINTENANCE law — a new gate joins the table and the orchestrator
in the same commit, unwired is a wish — held:

1. `scripts/gates/check-package-size.sh` — the gate itself; runs
   `cargo package --locked --no-verify --allow-dirty`, stats the
   `.crate` artifact, fails with the ten largest members listed when
   the count reaches the cap;
2. `scripts/gate-keepers.sh` section 14 — the wholesale pre-push run
   and its CI mirror execute the gate on every push;
3. `.github/workflows/crates-io.yml` — a pre-upload step runs the
   gate in the publish job, after the tag/version check and before
   `cargo publish`, so a too-large tarball fails the job instead of
   dying as a 413 at the registry.

`--no-verify` is deliberate: the size is the file walk's verdict, not
a build's, and the compile lanes (ci.yml, the publish verification
build) own compilation. `--allow-dirty` lets the gate run pre-push
locally where the tree is by definition not clean; on a CI checkout
it is a no-op.

## The registry render finding (verified against the live page)

This round inherited a claim that relative asset references in the
README (`<img src="assets/...">`) render as permanent 404s on
crates.io and therefore must be rewritten to absolute raw URLs. The
live registry says otherwise: the readme served for 80.5.8 carries

```
src="https://github.com/oxyzenQ/zylaxion/raw/HEAD/assets/zylaxion-original-logo-2026.png"
src="https://github.com/oxyzenQ/zylaxion/raw/HEAD/assets/zylaxion-demo-status-verbose.png"
```

— crates.io rewrote the README's relative references against the
repository (the `blob/HEAD` / `raw/HEAD` shapes) at render time. The
repository README needs no rewrite, and a README whose images were
converted to absolute URLs earlier in the cycle carried no lasting
regression when later rounds restored the relative form. Reproduce:
`curl -L -H "User-Agent: <contact>" https://crates.io/api/v1/crates/zylaxion/80.5.8/readme | grep -oE 'src="[^"]*"'`.

## Verification commands

Every number in this document reproduces with:

```bash
cargo package --locked --no-verify --allow-dirty
stat -c '%s' target/package/zylaxion-*.crate
tar -tzvf target/package/zylaxion-*.crate | awk '{print $3, $NF}' | sort -rn | head
bash scripts/gates/check-package-size.sh
```

and the registry rows with the version endpoint above.

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
