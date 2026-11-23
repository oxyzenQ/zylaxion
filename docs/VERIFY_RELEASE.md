<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
# Verifying Release Artifacts

Every release ships **three** checksum files per archive, covering
classical + post-quantum algorithms — and, when the owner's signing
key is configured for the release lane, a GPG detached ASCII-armored
signature (`.asc`) per archive. Verify at least one checksum before
trusting a downloaded binary; verify the signature when it is
present.

## Artifacts (night-improve-12c naming)

The tarball name carries the platform id the binary itself reports
(`zylaxion -V` prints the same label on its `Build:` line):

- `zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz` — x86-64-v3 (AVX2-era
  CPUs), dynamic glibc. The safe default: every CPU that runs v4 runs
  v3.
- `zylaxion-vX.Y.Z-linux-amd64-v4-gnu.tar.gz` — x86-64-v4
  (AVX-512-era CPUs), dynamic glibc.

## Checksum files

| File | Algorithm | Family | Quantum-safe? |
|------|-----------|--------|---------------|
| `*.sha512sum` | SHA-512 | SHA-2 | 256-bit (borderline) |
| `*.b3sum` | BLAKE3-256 | BLAKE3 | 256-bit |
| `*.shake256` | SHAKE256 | SHA-3 XOF (NIST PQ) | 256-bit |

## How to verify

All three commands print `<filename>: OK` on success (or `FAILED` on
mismatch). No manual hash comparison needed.

```bash
# Classical (universal, every Linux has this)
sha512sum -c zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.sha512sum

# Quantum-resistant — BLAKE3 (the official b3sum CLI)
b3sum -c zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.b3sum

# Quantum-resistant — SHAKE256 (NIST PQ standard, via Python)
# openssl's -shake256 default output length varies by version/distro;
# Python hashlib.shake_256 is consistent (64 bytes = 128 hex chars)
PKG=zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz
COMPUTED=$(python3 -c "import hashlib; print(hashlib.shake_256(open('$PKG','rb').read()).hexdigest(64))")
EXPECTED=$(awk '{print $1}' "$PKG.shake256")
[ "$COMPUTED" = "$EXPECTED" ] && echo "$PKG: OK" || echo "FAILED"

# GPG detached signature (when the release carries .asc files)
gpg --keyserver keyserver.ubuntu.com --recv-keys F5324E0967F104D58CE025F347A50AEF4B65AAC2
gpg --verify zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.asc \
  zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz
# Two fingerprints, two roles - check both out of band:
#   the PRIMARY key (the import and trust anchor, the keyserver's
#   keyblock certificate):
#     F532 4E09 67F1 04D5 8CE0  25F3 47A5 0AEF 4B65 AAC2
#   the SIGNING SUBKEY the release signature is made with - the
#   fingerprint `gpg --verify` itself prints ("using EDDSA key ..."):
#     3EE4 A49C 7863 0A16 E176  DB2B 3C9E B25B F040 7781
```

The expected verdict is `Good signature from "rezky_nightky (oxyzenQ)
<oxyzenQ@proton.me>"` (aka `Rezky Cahya Sahputra (cosmic dragon)
<130107241+oxyzenQ@users.noreply.github.com>`) — the primary contact
UID answers first, and a `WARNING: This key is not certified with a
trusted signature` notice is normal for first-time imports as long as
the fingerprint matches. The signature itself is made by the
primary's active signing subkey, so the verify output names the
subkey: `using EDDSA key
3EE4A49C78630A16E176DB2B3C9EB25BF0407781` — a verifier who greps the
output for the primary fingerprint alone sees the subkey instead and
calls a good signature a mismatch; expect both lines, each in its
role. The primary fingerprint is the anchor that outlives subkey
rotations; the keyserver always carries the live UID and subkey set —
trust it over this page after a rotation.

Swap the platform label (`-v4-gnu`) for the artifact you actually
downloaded — the label set lives in the release page's table.

## Smoke check after extraction

Run the binary's self-report and compare it against the tarball
name's platform id:

```bash
./zylaxion -V
# Build: pro-linux-amd64-v3-gnu (<short-sha>) <- must match the label
# Build-time: YYYY-MM-DD HH:MMZ               <- Hinnant-stamped UTC (ISO 8601)
# Signature: Pure eBPF builtin — Official Build by rezky_nightky (oxyzenQ)
# Depth-matrix: ... status=verified ...       <- the provenance seal, green on a TTY
./zylaxion sig
# verdict: SIGNATURE: AUTHENTIC (6/6 depths verified)  <- exit 2 on any tamper
```

The ownership claim is embedded MASKED (night-signature-advanced-1):
the artifact carries no greppable identity, so a rebrand that strips
the wording still materializes the original owner. The Depth-matrix
line reports `status=verified` only when the masked layers, their
fingerprints, the identity binding, and the blake3 anchor all
recompute — `status=tampered` (or a nonzero `sig` exit) is the honest
signal that the artifact was patched.

The fifth depth (`tap-object`, night-depth-hardening-Z1 D2) goes one
circle tighter: the embedded eBPF object — the one component that
executes at ring 0 — re-hashes against the digest the build stamped
over the staged object (`ZYLAXION_TAP_OBJECT_B3`). A binary whose
embedded object was patched after the build reports AUTHENTIC identity
layers and still exits 2 with the `D4 tap-object` row FAIL naming both
digests. CI proves this end to end: the `Dragon Guard - CI` check job
runs `scripts/audit/tamper-sig-probe.sh`, which patches one byte of a
real built binary and demands the refusal by name.

The sixth depth (`commit-signature`, night-netrunner-Z1 — the zelynic
night-solid-defense-2 shape) closes the channel gap the artifact's
own bytes cannot: the commit the binary claims is GPG-signed by the
PINNED maintainer key, verified at build time against the repo's
tracked public key (`assets/gpg/`, primary
`F5324E0967F104D58CE025F347A50AEF4B65AAC2` — the same key the
signature lane above imports) in an isolated throwaway keyring. The
row reports the BUILD's own verdict honestly: a tree on an unsigned
HEAD reads FAIL "no GPG signature" (5/6, exit 2 — the provenance
chain stops at an unbound commit), a HEAD signed by the pinned key
reads AUTHENTIC 6/6, and a foreign signer is named by fingerprint
(the MITM shape). With the maintainer key verified out of band (the
signature lane above) and releases cut from signed commits and tags,
the chain from key to tag to commit to binary closes — a binary
swapped on the wire can no longer replay its claims unanswered. The
trust anchors themselves carry recorded BLAKE3 digests in
`assets/signature.b3sum` (`b3sum -c assets/signature.b3sum` from the
repo root), so a swapped or re-keyed key file fails by name before
it can vouch for anything.

night-netrunner-Z1 binds the same verdict at the loader: attach
re-hashes the embedded object against the build-time stamp BEFORE
anything else it does and refuses the load on drift or a missing
stamp (`AttachError::ObjectUnsealed`, marker code 10) — until this
gate existed, a patched binary failed `sig` but attached anyway.
Every successful attach then appends one record to the local
hash-chained attestation log (`/var/lib/zylaxion/attestation.log`:
the loaded object's digest, the commit, the commit-signature state,
the chain step, the timestamp — each record's blake3 folding the
previous record's digest); `sig`'s `attest-log` field line replays
the chain and names the first break by line number. The log is a
local audit trail, not a public witness network — its module docs
(`src/ebpf/attest/log.rs`) state the limits plainly.

### What the fp= token hashes (night-pro-1)

`fp=` on the Depth-matrix line is the FNV-1a 64-bit digest of the
CLEAR manifest bytes only — the identity claim text, never the
artifact bytes (artifact identity is the D3 anchor's seat). The exact
byte stream hashed is the full Signature-line manifest, `Pure eBPF
builtin — Official Build by rezky_nightky (oxyzenQ)` (63 UTF-8
bytes), digest `247d2835ac8bde6a` — night-ux-orion-3 dropped the
manifest's trailing year (the holder renders without a claim year),
while zelynic still carries the retired year form (68 bytes,
`a60dfb6a39745f34`) until its own remake lands, so the two `fp=`
values differ by design. zejtron's manifest is the identity
clause only (46 bytes, `9174e0ce08feccba` — the family voice is a
separate claim const there) and cosmostrix's carries the "Cosmic
Dragon" voice (64 bytes, `fe0d7239f86e7c1b`) — differently framed
claims hash differently by design. The authors layer carried the
estate-wide retired address form (57 bytes, `c97b13f816035d55`)
until the night-remake-1 proton migration: zylaxion now carries the
proton address (43 bytes, `66771e5f273b9cae`), and the sibling
artifacts keep the retired layer until their own remake lands.

## Verifying an older release (pre-BLAKE3 archives)

Releases before the blake3 checksum upgrade ship `*.b2sum` files
(BLAKE2b-512) instead of `*.b3sum`. Those remain fully verifiable:
`b2sum` is GNU coreutils and preinstalled everywhere.

```bash
# Legacy lane — archives published before BLAKE3
b2sum -c zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.b2sum
```

## Why three algorithms

Defense in depth across three independent hash families
(SHA-2, BLAKE3, SHA-3 XOF). A future cryptanalytic break of any
single family does not invalidate verification via the other two.

## Process identity: the reproducible release chain (night-depth-hardening-Z1)

The checksums above establish **artifact identity** — these exact bytes
are the bytes the release published — and the D3 anchor
(`scripts/dev/verify-reproducible-build.sh`) proves the build self-reports
honestly. night-depth-hardening-Z1 upgrades the claim to **process
identity** (D4): the `Dragon Guard - Reproducible Release Chain` lane
(`.github/workflows/reproducible.yml`) builds the static musl v3 shape on
TWO INDEPENDENT HOSTS from ONE pinned recipe
(`scripts/release/repro-build.sh`) — the pinned toolchain
(`rust-toolchain.toml`), `SOURCE_DATE_EPOCH` pinned to the commit
timestamp, redacted path prefixes, `--locked` deps, no shared build
cache — and the verify job `cmp`s the two binaries. Byte-identical means
the hash the release publishes is the hash anyone can rebuild from
source, which retires the "modified build without a trace" class
permanently.

Reproduce it locally (the same recipe the CI hosts run):

```bash
# one build, the CI host shape (prints the sha256)
./scripts/release/repro-build.sh

# the local process proof: two cold builds on this host, cmp'd
./scripts/release/repro-build.sh --cold-two-pass
```

Honesty notes, recorded not hidden: the musl v3 shape is the
process-identity INSTRUMENT — it is not a shipped artifact, and the
release contract stays glibc-only (night-dragon-maintenance-T1, zero new
surface). The recipe compiles alsa-lib from a sha256-pinned source
tarball with musl-gcc into a private prefix and asserts the
pkg-config verdict before linking, so the glibc-weld trap the T1 removal
documented is avoided by construction. The static binary's `-V` and
stamp surfaces run anywhere; its ALSA runtime stays partial by nature
(libasound dlopens its PCM plugin tree), which is exactly why the lane
proves identity, not audio.

**Measured (the chain live):** commit `42c0cb4`, the two CI hosts
produced byte-identical binaries with byte-identical `-V` stamps —
shared sha256
`69cca2cf403688a07553e3c70da77b5fb248664ca205a02a8d604a3801d56d51` —
and the lane has re-passed on every push since. The same recipe's
local two-cold-pass verdict is recorded in
`docs/audit-night-depth-hardening-Z1.md`.

## Verification tools required

- `sha512sum` — GNU coreutils (preinstalled on every Linux)
- `b3sum` — the official BLAKE3 CLI (`cargo install --locked b3sum`;
  some distros package it as `blake3`, e.g. Arch's `blake3`)
- `python3` — for the SHAKE256 lane (hashlib is stdlib)
- `gpg` — GnuPG 2.x (only for the signature lane)

Three of the four tools ship with Arch Linux, Debian, Ubuntu, Fedora,
Alpine, and macOS by default; `b3sum` is one install away on any
machine with Rust (this project's audience by definition).

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
