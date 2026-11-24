<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
# The night-frontier-Z17 concurrency census (2026-11-24)

One question, answered by measurement, not by appetite: does this
tree need THE INTERLEAVING THEOREM — the missing for-all tier the
zelynic lineage named (Z6's own confession: "the SMP hammers are
sampled schedules") — an exhaustive, solver-checked proof that NO
interleaving of the concurrency protocol can reach a corrupt state?
The zelynic candidate is real there because zelynic owns a
multi-step, multi-actor protocol over packed words: a CAS draw
racing a refill racing a debt pay racing a generation belt on the
SAME bucket, where two interleaved half-protocols genuinely can
manufacture a state neither actor intended. An interleaving
theorem is the only honest instrument for that class.

This census asked whether this estate owns ANY surface of that
class. The method: every cross-thread and cross-CPU share in the
tree enumerated, classified by shape, and cross-checked against the
instrument that already owns its verification. Head commit at census
time: `45066f3` (the netrunner-Z2 docs commit; all twelve lanes
green, the E2E re-dispatch run 240 included).

## The census, surface by surface

The enumeration (grep over `src/`, `ebpf/src/`, `build/` for every
atomic, RMW, and unsafe share, then each site read in full):

1. **The audio bridge — `src/audio/mod.rs`, the SPSC ring.** The
   render thread (producer) and the cpal audio callback (consumer)
   share ONE `HeapRb<[f32; 2]>` (16384 frames), split into
   producer/consumer halves — the vendor `ringbuf` crate's own SPSC
   discipline, NOT a hand-rolled protocol. No atomics are written
   by this tree's code on that path at all; the ordering law (the
   consumer's stream is an order-preserving subsequence of the
   producer's; produced == accepted + refused) is the vendor's
   single-owner-per-half invariant, and the tree's OWN pins hold it
   under deliberate pressure: `test/audio/ring_lanes_tests.rs`
   (night-frontier-Z1, the A2 remainder) — FIFO integrity under
   50x oversubscription (50 000 frames against a 1 024-frame
   capacity), the accounting law (every accepted frame drains),
   and the drop contract (a full ring refuses the NEWEST frame
   intact and accepts again after a drain). Verified green in this
   census's own run: 2/2 passed, 0.01 s.
2. **The control-plane retune — the volume cell.** The watcher
   thread writes `Arc<AtomicU32>` (f32 bits through the
   `volume_bits` clamp law); the render loop reads it once per
   `process_sample`. Single RMW-width cell, single writer, no
   multi-step protocol — the interleaving space collapses to "the
   render loop sees either the old bits or the new bits", and the
   VALUE domain of both is already proven for-all by the Kani
   harness `volume_bits_clamps_every_volume` (twenty-harness lane,
   `kani.yml`). Contended shape pinned live:
   `test/engine/live_lanes_tests.rs`
   (night-hardening-2) — 4 threads x 5 000 retunes over 20 000
   rendered samples, the hard clamp and NaN silence never break.
   Verified green in this census's own run: 2/2 passed (both
   suites' four assertions).
3. **The model hot-swap — `ArcSwap`.** The watcher swaps the
   `Arc`-held acoustic model; the render loop's `load()` guard
   keeps a snapshot alive per batch. Vendor primitive (epoch-based
   reclamation, no lock), single swapper, and the same live pin
   (2 threads x 250 swaps under render, `live_lanes_tests.rs`)
   holds the render net. The vendor's internal protocol is not
   this tree's code — a model-checker there would be checking the
   vendor, not the product (the kani.yml loom-stance sentence, this
   estate's own recorded law).
4. **The event channel — crossbeam-channel.** MPMC channel
   primitives (the unbounded `Sender`/`Receiver` pair and the
   `select!` over `[events, stop]`), vendor-owned end to end. The
   tree's law (the parked wait, the stop lane) is pinned by
   `test/core/` and the E2E battery; no hand-rolled protocol
   exists to theorem.
5. **The kernel-side counters — `ebpf/src/main.rs`.** The stats map
   is `Array<AtomicU64>` and the programs perform exactly two
   RMWs, both `fetch_add(1, Relaxed)` (press/byte counters) —
   monotone accumulation on independent cells; there is no
   read-modify-write DECISION anywhere in the kernel object, no
   packed word, no generation belt. An interleaving of two
   `fetch_add`s on the same cell yields either order of the same
   sum — the commutative law the hardware already guarantees; the
   interleaving space contains exactly one reachable value.
6. **The signal/wake lane — `session/signals.rs`, `wake.rs`.**
   `AtomicBool`/`AtomicI32` flat flag cells (GRACEFUL_STOP,
   WAKE_FD, WAKE_WRITE_FD_RAW) written once in setup or from the
   signal handler's async-safe context (the raw `libc::write` wake
   byte). Single-word flat stores — the interleaving space is
   value-timing only, and the shutdown contract is pinned by the
   CLI E2E lane.
7. **The per-instance PRNG state — `mechanical/mod.rs`.**
   `keystroke_counter`, `last_trigger_us`, `last_interval_us` are
   per-instance `AtomicU64` cells owned by the ONE engine instance
   (held behind the ArcSwap; the v5.0.1 lesson that moved them off
   globals). The fetch_add counter is monotone; the two `swap`
   timestamps are single-writer (the render thread itself) — the
   exact class the interleaving theorem exists to rule out (a torn
   two-word protocol) does not exist here because the words are
   never read-modify-written in a multi-step dance.

**The finding, stated plainly:** zero `compare_exchange`, zero CAS
retry loops, zero packed-word multi-actor protocols anywhere in
this tree — the shape that makes an interleaving theorem the only
honest instrument does not exist here. Every cross-thread share is
either (a) a vendor primitive whose internal protocol is the
vendor's own surface, (b) a single-RMW-width cell whose VALUE law
is already a standing Kani theorem, or (c) a flat flag/counter
whose interleaving space is a single commutative sum. For (b), the
for-all tier already covers what interleavings could reach: both
the old bits and the new bits are inside the proven domain — that
is precisely what "for ALL inputs in the domain" means when the
protocol's reachable states are exactly the value domain.

## Why the theorem is a zelynic need, not a zylaxion need

The honest contrast, so the record teaches the right lesson.
zelynic's datapath owes the kernel money arithmetic under a
multi-actor protocol: a token draw, a refill event, and a debt
payback each open the SAME packed word, mutate different fields,
and race each other's generations. The single-thread Kani laws
prove each ACTOR's arithmetic for all states; the interleaving
theorem is what proves the ACTORS cannot corrupt each other's
half-writes. That gap is real there because the state is shared
and mutable in multi-field units.

zylaxion's architecture made the opposite trade when it chose its
bridges: cross-thread state is single-word and single-owner (or
vendor-owned), so every "interleaving" reduces to a value
choice that the for-all tier already subsumes. The census's
verdict is therefore not "the work was skipped" but "the class
of bug the work proves absent cannot be constructed here" —
the same standing the kani.yml header already records for the loom
instrument ("the concurrency surface is std primitives plus the
external crates' own protocols — no hand-rolled lock-free
algorithm to prove — a loom dependency here would model-check the
vendors' internals, not this tree's code: theater, not proof").

## The verdict

**STAND-DOWN, recorded:** the interleaving theorem is NOT
APPLICABLE to this estate. No instrument lands, no dependency
joins, no CI lane is spent — and the census itself is the
deliverable, because an LTS claim ("every concurrency law named,
each with its instrument") is only honest when the refusal to
build one is also on the record with its evidence. The three
tiers this estate's concurrency surfaces actually need are all
standing:

- for-all (value domains): the twenty Kani harnesses, push lane;
- sampled (interleaving-independent invariants under real
  contention): the ring-lane and live-lane pins, fresh-run green
  in this census (4/4 tests, both suites);
- end-to-end: the Supermassive E2E battery, green on run 240.

The day a hand-rolled multi-actor protocol enters the tree (a
packed control word, a two-phase commit, anything where two
threads own different fields of one word), this census is the
document that reopens: the target class is named, the instrument
is designed (zelynic's two-CPU model over the dual-compiled pure
core, the Z12 lemma-decomposition discipline for the wall), and
the retirement door is the recorded precedent. Until then, the
honest claim in the ledger is the one this census lets the estate
make: **the interleaving space of every concurrency surface is
either vendor-owned, subsumed by a standing for-all theorem, or
commutative by construction — and the one instrument that could
check the remaining class found no remaining class to check.**

## Live evidence of the census's own runs

- `cargo test ring_lanes` — 2 passed, 0 failed (FIFO under
  pressure, drop contract), 0.01 s, pinned 1.98.1 toolchain.
- `cargo test live_lanes` — 2 passed, 0 failed (volume-cell
  retune and ArcSwap model swap under 20 k concurrent render
  samples), 0.01 s.
- Enumeration commands: the atomic/RMW grep over `src/`, `ebpf/src/`,
  `build/` (the seven surfaces above are the complete result set —
  nothing was filtered, the vendor crates' own internals aside).

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
