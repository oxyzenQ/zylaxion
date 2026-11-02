<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# zylaxion Commercial License

zylaxion is dual-licensed:

- **AGPL-3.0-only** for open-source use (see [LICENSE](LICENSE)).
- **Commercial License** for proprietary and commercial use (this document).

If your use of zylaxion can satisfy the copyleft obligations of the
AGPL-3.0-only — including releasing your modifications, integrations, and
the corresponding source code to anyone you distribute to — you do not
need to pay anything. The AGPL path is complete, free, and first-class;
it is not a trial. The Commercial License exists for the uses AGPL
cannot honestly cover: keeping your modifications proprietary,
embedding zylaxion in a closed product, or redistributing it under
different terms.

---

## 1. Who Needs a Commercial License

You need a Commercial License if any of the following describe your use:

- **Production use at a company** — running zylaxion on commercial
  workstations, kiosks, demo units, or developer fleets, where the AGPL's
  copyleft obligations (open-sourcing your modifications and
  integrations) cannot or will not be met.
- **Product embedding** — shipping zylaxion (or a derivative of its
  synthesis engine) inside a closed hardware or software product,
  appliance, or image.
- **SaaS and content offerings** — providing a hosted or packaged
  experience whose value depends on zylaxion, where you would otherwise
  be obligated to release your service's zylaxion-related source code.
- **Redistribution under different terms** — distributing zylaxion (or a
  derivative) under terms other than AGPL-3.0-only.
- **Any use that cannot meet AGPL-3.0 copyleft obligations** — when in
  doubt about whether your use complies, that doubt is the signal to
  either study [LICENSE](LICENSE) carefully or buy the license that
  removes the question.

## 2. Who Does NOT Need One

The following uses are fully covered by AGPL-3.0-only and cost nothing:

- **Personal use** — keyboard sound on your own machines.
- **Hobby projects** — experiments, home labs, self-hosted setups,
  stream setups you do not sell.
- **Non-commercial research** — academic study, benchmarking,
  measurement, and publication of results.
- **Education** — teaching, coursework, and learning.
- **Open-source contributions back to upstream** — forking, fixing,
  extending, and opening PRs against this repository. The
  [trademark policy](TRADEMARK.md) governs the Marks, not this
  license; contribution forks keep the name unchanged and are welcome
  without permission or payment.

## 3. Pricing

Prices are pegged to USD, payable in cryptocurrency (see
[Payment](#7-payment) below). Tiers are self-declared by the buyer in
good faith — pick the tier that honestly matches your situation; nobody
audits you, but misrepresenting revenue to underpay is a breach of the
license terms.

| Tier                 | Price                                                           | Target                                                                   |
| -------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------ |
| Personal             | Free (AGPL-3.0)                                                  | Hobby, personal, non-commercial, open-source contributions               |
| Individual           | $99/year                                                        | Solo devs, freelancers, revenue under $100K/year                         |
| Professional         | $999/year                                                       | Teams up to 5 seats, revenue under $500K/year, priority support with SLA |
| Business             | $2,199/year                                                     | SMB, revenue $100K–$10M/year                                             |
| Enterprise Internal  | $19,999/year                                                    | Enterprise >$10M/year, internal use only, NO redistribution              |
| OEM / Redistribution | $79,999/year OR $129,999 one-time + $25,999.80/year maintenance | Embedding or reselling in a shipped product; redistribution rights       |

**Professional vs Individual.** Individual ($99/year) is single-seat
for solo developers and freelancers. Professional ($999/year) covers
teams up to 5 seats at revenue under $500K, with priority support
and a response SLA. The architecture licensed is identical;
Professional adds seats and support, not patterns.

**Enterprise Internal vs OEM / Redistribution.** Enterprise Internal
($19,999/year) is internal use only — run zylaxion on any number of
hosts inside the enterprise, build internal tooling around it, but
do not embed or resell it in a distributed product. OEM /
Redistribution ($79,999/year, or $129,999 one-time + 20%/year
maintenance) grants redistribution rights: embed the eBPF input tap,
the daemonless control plane, the Zactrix Engine, and the zero-sample
DSP discipline inside a shipped product.

**OEM pricing note:** the OEM tier offers two equivalent paths —
annual ($79,999/year) for buyers who prefer opex, and perpetual
($129,999 one-time + $25,999.80/year maintenance, i.e. 20%) for
buyers who prefer capex and want to cap long-term risk. Total cost
over multi-year horizons:

- 1 year: annual = $79,999; perpetual = $155,998.80
- 3 years: annual = $239,997; perpetual = $207,998.40
- 5 years: annual = $399,995; perpetual = $259,998
- 10 years: annual = $799,990; perpetual = $389,997

The perpetual option is not stackable with the multi-year discount.
Grandfathering applies to whichever path the buyer purchased.

**Why $79,999 and not $29,999.** zylaxion is the only tool in the
Linux ecosystem that ships eBPF input-event capture wired directly
to a procedural DSP engine. Not "one of a few" — one of one. Every
other approach (userspace daemon, evdev grabber, LD_PRELOAD, sample
player) violates at least one of the four constraints the tool
exists to satisfy: zero daemon, no device grab, hotplug-safe by
construction, zero config file. The architecture is not redundant;
it is the minimum sufficient shape for the constraint set. The
$79,999 anchor reflects that uniqueness, calibrated below the
zelynic and cosmostrix OEM anchors ($149,999) — whose categories
have competitors — and modestly above zejtron's ($99,999) — whose
category has Sysdig, Plixer, and dozens of CLIs. A first mover in
a category that does not yet exist prices the category, not the
LOC.

Notes:

- **Revenue thresholds** refer to your total annual revenue (company or
  individual, whichever applies), not your zylaxion-specific revenue.
- **Multi-year discount** (owner discretion): 20% off a 2-year term,
  30% off a 3-year term.
- **The ladder climbs with the work.** zylaxion's value increases
  year by year; the owner re-prices the business end of
  the ladder as the project compounds. Grandfathering is the standing
  rule: buyers who paid before a price move are grandfathered at the
  price they bought, for the term they bought. The business end moves
  with the work: Business $2,199/year and Company $29,999/year were
  the owner's pricing call for zylaxion (night-improve-17) — Personal
  stays free and Individual holds at $99/year. Existing Company-tier
  customers at $29,999 keep their purchased price for their purchased
  term — the Company tier is renamed to OEM / Redistribution with
  identical rights, so nothing changes for existing buyers, and the
  earlier $9,199 cohort is grandfathered the same way. New licenses
  follow the current table ($79,999/year or $129,999 perpetual +
  20%/year).
- **Redistribution rights** are granted at the OEM / Redistribution
  tier only. Trademark licensing is separate — see
  [TRADEMARK.md](TRADEMARK.md).
- No free tier above Personal is granted without explicit owner
  approval.

### Unauthorized Use

zylaxion's code is open to read, learn from, and build on — under
AGPL-3.0-only's honest terms, or under a paid Commercial License.
What is NOT open is taking it: stealing the code, forking it
illegally, stripping or ignoring the license, or shipping it
inside a closed-source commercial product — whether done by a
company or by an individual — without contacting the owner
(rezky_nightky / oxyzenQ,
[with.rezky@gmail.com](mailto:with.rezky@gmail.com)) and paying
for the license first. That is not a loophole, a gray area, or a
clever business move. It is theft of one person's years of work,
a breach of AGPL-3.0-only and of this Commercial License, and —
in the owner's own words — the mark of the worst kind of human:
a rotten one. If closed source is what you need, the OEM /
Redistribution tier exists precisely for that; buy it. If you cannot
or will not pay, the AGPL path is complete and free — open-source what
you build. There is no third path that leaves you clean.

## 4. What You're Actually Licensing

The OEM / Redistribution tier licenses the technology stack, not just
the keyboard sound binary. The five assets below are what that license
actually covers — each one is verifiable from this tree, not from a
pitch:

1. **The eBPF input tap architecture** — passive kernel-side capture
   with zero device grab, hotplug-safe and suspend-safe.
2. **The daemonless control plane** — pinned BPF maps as state store,
   attach-once, unprivileged-verb pattern.
3. **The Zactrix Engine** — time-variant spectral envelope synthesis,
   LTS-frozen with six proofs
   ([docs/ZACTRIX_LTS.md](docs/ZACTRIX_LTS.md)) and full security
   audit ([docs/ZACTRIX_SECURITY.md](docs/ZACTRIX_SECURITY.md)).
4. **Zero-sample DSP architecture** — no audio decoder, no wavetable,
   no sample asset (the ELF `DT_NEEDED` sweep proves it).
5. **The micro-randomization and sample-rate-honest laws** — the
   `c^(R/S)` renormalization and per-keystroke jitter algorithms.

If you want to adopt, adapt, or integrate any of these into your own
product (closed-source, proprietary, or SaaS), the OEM /
Redistribution tier gives you the license. If you want to build from
source under AGPL-3.0-only, the Personal tier is free. This is
technology licensing, not a keyboard-sound subscription. The
$79,999/year OEM / Redistribution tier is priced against the cost of
replicating the architecture in-house (an estimated 6-12 months of
R&D), not against consumer keyboard sound products.

The OEM / Redistribution tier is not priced for a keyboard sound
binary. It licenses the embeddable DSP architecture — the eBPF input
tap, the daemonless control plane, the Zactrix Engine (LTS-frozen
with six proofs), the zero-sample DSP discipline, and the
micro-randomization law — for adaptation into embedded devices, game
engines, accessibility tools, and hardware keyboards. See "Use Cases
Beyond Keyboard Sound" (section 5 below, mirrored in the README).
The OEM / Redistribution price reflects the architectural surface,
not the proof-of-concept application.

## 5. Use Cases Beyond Keyboard Sound

The same stack serves problems beyond the desktop keyboard. The tap,
the control plane, the engine, and the zero-sample law are separable
assets, and each use case below adapts one or more of them:

- **Accessibility tools** — auditory feedback for visually impaired
  users, adaptive input devices.
- **Remote desktop / VDI** — input capture without daemon overhead.
- **Embedded devices** — zero-sample audio feedback for kiosks, ATMs,
  medical devices with storage constraints.
- **Game engines** — input sound module without audio assets.
- **Security / observability tools** — kernel-side input capture for
  auditing, parental control, or compliance.
- **Hardware keyboards** — built-in synthesis in firmware.

Commercial licenses cover adaptation into any of these contexts —
Enterprise Internal for use inside the enterprise, OEM /
Redistribution for embedding in a shipped product. The architecture
is the asset; the keyboard sound is the proof of concept.

## 6. What Each Tier Grants

Every paid tier — Individual, Professional, Business, Enterprise
Internal, and OEM / Redistribution — grants, for the license term:

- **Use without AGPL copyleft** — the right to use, modify, and integrate
  zylaxion in your products and infrastructure without the obligation to
  open-source your modifications.
- **Priority support** — your licensing and integration questions are
  answered ahead of the free queue.
- **Internal modification rights** — private forks and patches for your
  own use, with no disclosure requirement.

The **Professional** tier adds the response SLA the table names, on
top of the queue priority every paid tier holds.

The **Enterprise Internal** tier additionally covers enterprise-scale
internal use: run zylaxion on any number of hosts inside the
enterprise and build internal tooling around it — with no
redistribution rights.

The **OEM / Redistribution** tier additionally grants:

- **Redistribution rights** — the right to distribute zylaxion (or your
  derivative of it) as part of your own offering, under your own terms,
  to your customers. This is the only tier that carries it.
- Rebranding and redistribution of the Marks still requires separate
  trademark permission — see [TRADEMARK.md](TRADEMARK.md).

## 7. Payment

Payment is **USD-pegged, paid in crypto**: send the cryptocurrency
equivalent of the USD price at purchase time. The exchange rate is
taken from CoinGecko or CoinMarketCap at the moment of the transaction
agreement. These are owner-verified receive addresses — the same
addresses that passed offline cryptographic verification described in
the [zelynic README](https://github.com/oxyzenQ/zelynic#crypto-donations)
(EIP-55 mixed-case checksum for Ethereum, bech32m witness-v1 for
Taproot, base58-to-32-byte ed25519 key for Solana).

**Always double-check the address on screen before sending.** Network
mismatches — for example, sending USDT-ERC20 to a Solana address, or
BTC to a non-Taproot address — permanently lose funds.

### Solana (SOL / USDT-SPL)

```text
88umzS7abaToaGQVgTVXt5SnuvcjTw2jPSM6Ha2JYmXM
```

![Solana payment address QR code](assets/qr/solana.png)

### Ethereum (ETH / USDT-ERC20 / USDC-ERC20)

```text
0x1bCbA21c07B5636a942De27AA7Ee8283cEDb4C3D
```

![Ethereum payment address QR code](assets/qr/ethereum.png)

### Bitcoin (BTC, Taproot P2TR)

```text
bc1p88nqysn4p8u9zxwz2pyxs5pl77wllcrk6ca2r2l3ryr3863hxkys5vdkze
```

![Bitcoin payment address QR code](assets/qr/bitcoin.png)

These addresses double as the project's voluntary donation addresses;
commercial payments are distinguished by the licensing inquiry that
accompanies them, not by a different address.

## 8. Verification Process

1. **Send a licensing inquiry** to [with.rezky@gmail.com](mailto:with.rezky@gmail.com)
   (or open a GitHub issue at [@oxyzenQ](https://github.com/oxyzenQ))
   naming your chosen tier and term.
2. **Pay** the USD-pegged amount in crypto to the address above for
   your chosen network.
3. **Submit the transaction hash** by replying to the inquiry thread.
4. **Owner verifies the transaction on-chain** — amount, address, and
   confirmations.
5. **License PDF issued within 48 hours** of verification, sent to
   your inquiry email.

The license PDF names the licensee, the tier, the term, and the grant
scope. Keep it; it is the proof of your commercial rights.

## 9. Contact

- **Licensing email**: [with.rezky@gmail.com](mailto:with.rezky@gmail.com)
- **GitHub**: [@oxyzenQ](https://github.com/oxyzenQ)

For trademark matters, see [TRADEMARK.md](TRADEMARK.md) — commercial
licensing and trademark licensing are separate.

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
