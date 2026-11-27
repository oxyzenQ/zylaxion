# SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
# SPDX-License-Identifier: AGPL-3.0-only OR Commercial

# AXP1-ZnQ (Architect's eXecution Protocol 1)

The maintenance bot's one protocol: no CI lane marries a hardcoded
server again. The owner named the bottleneck after the v80.5.8 week —
an apt lane died on a runner-image repo answering 403 "no longer
signed", and the fallback lane's repoint target was itself a hardcoded
address that a bad hour could sink. The answer is a tested address
pool the bot curates, not a hardcoded default the tree prays for.

Identity: **AXP1-ZnQ** stands for **Architect's eXecution Protocol 1**
— the expansion rides every mention of the name across the docs, the
workflows and the scripts, by the owner's standing law (the name was
adopted by night-solid-defense-1 for the Sunday sentinel; this pass
adds the protocol's second half, the address catalog, on
night-learning-into-darkness-1). The existing mentions — the sweep's
banners, the maintenance workflow's comments — keep their sentinel
voice; this document is the protocol's front door.

## The catalog (repos-for-ci/*.txt)

`repos-for-ci/` at the repo root holds one `.txt` per address family
(`apt-mirrors.txt` ships today). The law of every catalog file:

- one candidate address per line, the whole trimmed line IS the
  address (no trailing comments — a comment occupies its own line,
  starting with `#`);
- `https://archive.ubuntu.com/ubuntu` — an HTTP(S) address, probed
  with a real GET, TLS verified with the runner's own CA store (a
  cert failure is DOWN, never ignored);
- `dns.example.com:53` — a host:port address, probed with a socket
  connect (the DNS-resolver catalog rides this kind when its lane
  adopts the resolver);
- the authored order is the owner's trust ranking — canonical
  archive first, the runner-native azure mirror second, then the
  wide mirrors. It is never a latency leaderboard; see the write law;
- bounded: 10 to 100 active lines per file, the owner's min and max
  — both halves enforced (the refresher prunes tombstones at the
  100-line file cap and warns when the alive pool drops under
  `--min-lines`, default 10, `0` disables);
- the shipped seed was probe-verified (12 of 15 candidates answered
  HTTP 200 on `dists/noble/InRelease` at authoring time;
  `security.ubuntu.com` is excluded on purpose — it serves the
  security pocket, not the whole archive, and the apt fallback lane
  installs from the full pool).

## The two commands

Both live in `scripts/dev/`, stdlib-only Python (urllib, socket,
concurrent.futures — no third-party dependency joins the maintenance
surface, the LTS law), and share one probe core
(`scripts/dev/axp1znq_probe.py`) so a mirror the resolver picks and a
mirror the refresher keeps are measured the same way.

**The resolver — `axp1znq_resolve.py`** — is the failover half, the
consumer CI lanes call:

```
python3 scripts/dev/axp1znq_resolve.py \
  --file repos-for-ci/apt-mirrors.txt \
  --probe-path dists/noble/InRelease \
  --first
```

It walks the catalog top to bottom, probes each address, narrates
every verdict to stderr (line number, reason, milliseconds — the CI
log keeps the audit trail), prints the first ALIVE address to stdout
and exits 0. Line 1 down, the lane moves on; line 5 answering is the
one the lane uses — the owner's scenario, verbatim. A catalog with no
live address is exit 1: a lane must never silently fall back to a
dead default. Freshness is reported in the narration but never blocks
the pick — a minutes-old index still serves the lane; curation is the
refresher's job. Report mode (no `--first`) prints the whole table or
`--json` for machines.

**The refresher — `axp1znq_refresh.py`** — is the catalog half, the
Sunday pass the maintenance lane runs:

```
python3 scripts/dev/axp1znq_refresh.py \
  --root repos-for-ci \
  --probe-path dists/noble/InRelease \
  --note-file "${ZYLAXION_SWEEP_DIR}/axp1-address-note.txt"
```

It probes every catalog concurrently, keeps an address only when it
answers AND serves a fresh index (the InRelease file's own `Date:`
header is the freshness witness; older than `--max-age-days`, default
14, is STALE — the owner asked for repos that are updated AND connect
stably, and alive-but-frozen is a lie the catalog must not keep), and
writes the survivors back. The write law:

- survivors keep their authored order — the catalog's shape is the
  owner's trust ranking, never a weekly reshuffle a noisy runner
  would churn;
- a dead or stale address becomes a dated tombstone in place
  (`# down 2026-11-27 (unreachable ...) - https://...`), readable
  history the owner can revive by deleting one line;
- the 100-line cap prunes the OLDEST tombstones first; an active
  address is never pruned;
- the never-empty guard: zero live addresses means the file is NOT
  rewritten (a flaky runner probe must never erase the owner's pool)
  — the lane warns (`::warning::` in CI) and the note names CALL THE
  OWNER, the one critical the refresher escalates;
- idempotent: unchanged catalog, unchanged file — no write, no
  commit-note, no churn;
- `--dry-run` reports the verdicts and writes nothing.

## The wiring (the Sunday cadence)

The maintenance lane (`Dragon Guard - Dependency Maintenance`, cron
`0 3 * * 0` — Sunday 03:00 UTC = 10:00 WIB) runs the refresher as the
step after `cargo test` and before `Commit and push`, gated on
`scan_only != true` like every ship step. The owner's sequencing law:
the address refresh rides AFTER the dependency update finishes, and
the commit step's `git add -A` picks both up, so the one weekly commit
carries deps AND addresses — with the refresher's note and the sweep's
note both landing in the commit body. The refresher never blocks the
ship lane: it exits 0 with warnings recorded; the red stays the
verdict step's (AXP1-ZnQ — Architect's eXecution Protocol 1 — and its
own exit-1 law).

The first wired consumer is the maintenance lane's own apt fallback:
its repoint target is no longer a hardcoded server — the resolver
hands the lane the first catalog address that actually answers, and a
catalog with nothing alive fails the lane loud. The other lanes adopt
incrementally (the E2E micro-VM guest floors and the DNS guard steps
are candidates; a lane adopts by swapping its hardcoded address for a
resolver call — no new machinery needed, that is the point of the
pool).

## The honest edges

- Probe verdicts are network-relative: the authoring sandbox verified
  11 of the 12 shipped lines, and the first runner pass has since
  judged the last one — on 2026-11-27 the maintenance run found
  mirrors.kernel.org dark from the runner network and the catalog now
  carries its dated tombstone (11 active lines, above the
  10-address floor). A runner-region difference is exactly what the
  weekly curation exists to reconcile.
- The fresh-index probe path is `dists/noble/InRelease` — the noble
  LTS the runner fleet runs. When the fleet moves to the next LTS,
  the probe path moves with it (one flag, both commands).
- `--first` worst case on a dead-first catalog is bounded by
  attempts x timeout per address (both are flags); an HTTP probe
  rides the whole-request timeout alone (urllib's one-socket law),
  the connect timeout binds only the host:port kind, and on proxied
  networks the total timeout is what bites.

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
