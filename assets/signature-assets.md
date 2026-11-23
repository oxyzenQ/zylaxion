<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
# asset signature checksums

night-dragon-maintenance-X1 (2026-11-02): every image under `assets/`
(the original logo plate, the status-verbose capture, and the three
donation qr codes) carries a recorded BLAKE3 digest, so a future swap,
duplicate, or re-encoded fake of the logo is detectable against the
committed bytes. The digests were produced with the canonical `b3sum`
CLI and cross-verified against an independent BLAKE3 implementation
before landing. The sibling cosmostrix signature table agrees where
the trees share bytes: `qr/bitcoin.png` and `qr/solana.png` hash
identically in both repositories, which pins the two ledgers to each
other.

night-netrunner-Z1 (2026-11-23) extends the signed set past the
images to the GPG trust anchors: `gpg/maintainer-
47A50AEF4B65AAC2.asc` and `gpg/pinned-maintainer-fpr.txt` — the two
files the D5 commit-signature lane reads as its pin (build/gpg.rs
verifies, the runtime policy in src/ebpf/attest/ reports). A swapped
or re-keyed key file now fails `b3sum -c` by name BEFORE it can
vouch for anything, closing the one tamper surface the lane's own
docs left to "a visible diff": the diff is also a checksum mismatch.
The two anchors hash identically to the zelynic estate's signature
table (the same maintainer key pins both projects), so the two
ledgers cross-pin each other's trust roots too.

Tool: `cargo install b3sum` (the BLAKE3 team's canonical CLI).

Regenerate and compare against the table below — any mismatch means
the file on disk no longer matches the signed asset:

```bash
cd assets
b3sum $(find . -name '*.png' -o -path './gpg/*' | sort)
```

| file | bytes | blake3 |
|------|-------|--------|
| gpg/maintainer-47A50AEF4B65AAC2.asc | 4,918 | `639b5d94603e14fa9022cad8e26efff7711bdaedd92f08723ffa96c0b2d90d77` |
| gpg/pinned-maintainer-fpr.txt | 40 | `dc6108d809afdbbb41756c37ef3ce3eec4b674ed1c8c225678c0103c1e4cfa3b` |
| qr/bitcoin.png | 575 | `09ec09fcdc4815e23373cb266bb120aa4e1af140fb9d598e117d05478a791b47` |
| qr/ethereum.png | 591 | `4eb520de0fe258d2c82ba07ccba8d9e48bafc63904d022a83cc42b59d1ed2ac7` |
| qr/solana.png | 595 | `bca56b0a454daf06ff2d57fe401719553e28733e73479de2652a0740a9e87d1a` |
| zylaxion-demo-status-verbose.png | 323,054 | `cbb15746d0bf6af948d8daee2a7cd38f65db31518378f571c40aaa9efd9715eb` |
| zylaxion-original-logo-2026.png | 1,347,996 | `31081091ede857e08a78ba2fe9e923fac71bbb52e1c08679c53719f195286837` |

The status-verbose capture is pinned on the same terms as the logo:
it is the README's demo face, so a re-encoded or substituted capture
is detectable against the committed bytes too.

When an asset legitimately changes, refresh the matching row in the
same commit that replaces the file. The toolkit commands that
produced these assets live in [tools.md](tools.md).

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
