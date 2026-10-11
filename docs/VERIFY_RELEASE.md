<!-- SPDX-License-Identifier: GPL-3.0-only -->
<!-- Copyright (C) 2026 rezky_nightky (oxyzenQ) -->

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
| `*.b2sum` | BLAKE2b-512 | BLAKE2 | 256-bit |
| `*.shake256` | SHAKE256 | SHA-3 XOF (NIST PQ) | 256-bit |

## How to verify

All three commands print `<filename>: OK` on success (or `FAILED` on
mismatch). No manual hash comparison needed.

```bash
# Classical (universal, every Linux has this)
sha512sum -c zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.sha512sum

# Quantum-resistant — BLAKE2b (fastest, in coreutils)
b2sum -c zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.b2sum

# Quantum-resistant — SHAKE256 (NIST PQ standard, via Python)
# openssl's -shake256 default output length varies by version/distro;
# Python hashlib.shake_256 is consistent (64 bytes = 128 hex chars)
PKG=zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz
COMPUTED=$(python3 -c "import hashlib; print(hashlib.shake_256(open('$PKG','rb').read()).hexdigest(64))")
EXPECTED=$(awk '{print $1}' "$PKG.shake256")
[ "$COMPUTED" = "$EXPECTED" ] && echo "$PKG: OK" || echo "FAILED"

# GPG detached signature (when the release carries .asc files)
gpg --verify zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.asc \
  zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz
# Import the owner's public key once, from the repository's release
# page or a keyserver, and check the fingerprint out of band.
```

Swap the platform label (`-v4-gnu`) for the artifact you actually
downloaded — the label set lives in the release page's table.

## Smoke check after extraction

Run the binary's self-report and compare it against the tarball
name's platform id:

```bash
./zylaxion -V
# Build: linux-amd64-v3-gnu (<short-sha>)   <- must match the label
# Build-time: M/D/YYYY HH:MM (UTC)          <- Hinnant-stamped UTC
```

## Why three algorithms

Defense in depth across three independent hash families
(SHA-2, BLAKE2, SHA-3 XOF). A future cryptanalytic break of any
single family does not invalidate verification via the other two.

## Verification tools required

- `sha512sum` — GNU coreutils (preinstalled on every Linux)
- `b2sum` — GNU coreutils ≥ 8.x (preinstalled on modern distros)
- `python3` — for the SHAKE256 lane (hashlib is stdlib)
- `gpg` — GnuPG 2.x (only for the signature lane)

All four tools ship with Arch Linux, Debian, Ubuntu, Fedora, Alpine,
and macOS by default — no extra install needed.
