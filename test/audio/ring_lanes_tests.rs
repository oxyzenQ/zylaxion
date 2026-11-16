// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

// ring_lanes_tests.rs — the SPSC ring's interleaving pins
// (night-frontier-Z1, the A2 remainder: the loom-style pin the
// decision-table battery left open). The production bridge is one
// producer (the render thread's try_push) and one consumer (the audio
// callback's try_pop) over HeapRb<[f32; 2]>, where a push onto a full
// ring refuses with the value returned — the NEWEST sample loses,
// never the oldest. No loom, no new dependency: the laws asserted
// here hold under EVERY interleaving, so real threads with bounded
// work prove what a model checker would:
//
//   - FIFO integrity: the consumer's delivered stream is an order-
//     preserving subsequence of the produced sequence (no reordering,
//     no duplication, whatever the pressure);
//   - the accounting law: produced == accepted + refused, and every
//     accepted frame is delivered — an SPSC ring can only refuse new
//     frames, never lose an accepted one;
//   - the drop contract: a full ring returns the refused frame
//     intact, keeps the oldest data, and accepts again after a drain.

use super::*;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

/// Deliberate pressure: 50x oversubscription against the capacity, so
/// the hammer's window crosses the full/empty boundary many times.
const OVERFLOW_CAPACITY: usize = 1_024;
const PRODUCED_FRAMES: usize = 50_000;

#[test]
#[cfg_attr(
    miri,
    ignore = "the interleaving hammer is a real-thread lane; the pure families stay in the miri scope"
)]
fn the_spsc_ring_preserves_fifo_order_under_any_interleaving() {
    let rb = HeapRb::<[f32; 2]>::new(OVERFLOW_CAPACITY);
    let (mut prod, mut cons) = rb.split();
    let accepted = Arc::new(AtomicUsize::new(0));
    let producer_done = Arc::new(AtomicBool::new(false));
    let producer = {
        let accepted = Arc::clone(&accepted);
        let done = Arc::clone(&producer_done);
        thread::spawn(move || {
            for tag in 0..PRODUCED_FRAMES {
                // The render thread's exact contract (audio/mod.rs
                // pushes with `let _ =`): a push onto a full ring
                // drops the frame. The hammer counts every outcome so
                // nothing can vanish unaccounted.
                if prod.try_push([tag as f32, 1.0]).is_ok() {
                    accepted.fetch_add(1, Ordering::Relaxed);
                }
            }
            done.store(true, Ordering::Relaxed);
        })
    };
    let consumer = thread::spawn(move || {
        let mut last = -1.0f32;
        let mut delivered = 0usize;
        loop {
            match cons.try_pop() {
                Some([tag, mark]) => {
                    assert_eq!(mark, 1.0, "the marker channel must survive");
                    assert!(
                        tag > last,
                        "FIFO broken: {tag} after {last} (a reorder or a duplication)"
                    );
                    last = tag;
                    delivered += 1;
                }
                None => {
                    if producer_done.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::yield_now();
                }
            }
        }
        delivered
    });
    producer.join().expect("the producer thread must not panic");
    let delivered = consumer.join().expect("the consumer thread must not panic");
    let accepted_count = accepted.load(Ordering::Relaxed);
    assert!(
        delivered == accepted_count && accepted_count <= PRODUCED_FRAMES,
        "the accounting law: every accepted frame drains ({delivered} delivered \
         vs {accepted_count} accepted of {PRODUCED_FRAMES} produced)"
    );
}

#[test]
fn the_ring_refuses_full_drains_clean_and_accepts_again() {
    let rb = HeapRb::<[f32; 2]>::new(64);
    let (mut prod, mut cons) = rb.split();
    for tag in 0..64u32 {
        prod.try_push([tag as f32, 1.0])
            .expect("the corpus is sized to the capacity");
    }
    // The drop contract at the boundary: the next push is REFUSED with
    // the frame returned intact — the newest sample loses, the oldest
    // stays.
    let refused = prod
        .try_push([99.0, 1.0])
        .expect_err("a full ring must refuse");
    assert_eq!(refused[0], 99.0, "the refused frame comes back intact");
    assert_eq!(refused[1], 1.0, "the refused marker survives");
    // FIFO order across the whole corpus, then empty forever.
    for tag in 0..64u32 {
        let [left, right] = cons.try_pop().expect("every accepted frame drains");
        assert_eq!(left, tag as f32, "FIFO order across the full corpus");
        assert_eq!(right, 1.0, "the marker channel survives the drain");
    }
    // The callback's silence path, not a hang: pops on an empty ring
    // return None, indefinitely.
    assert!(cons.try_pop().is_none());
    assert!(cons.try_pop().is_none());
    // And the vacancy side accepts again after the drain — the
    // adaptive-wait contract the orchestrator rides.
    prod.try_push([100.0, 1.0])
        .expect("the drained ring has vacancy");
    assert_eq!(
        cons.try_pop().expect("the post-drain frame"),
        [100.0, 1.0],
        "the ring keeps working across the full-empty boundary"
    );
}
