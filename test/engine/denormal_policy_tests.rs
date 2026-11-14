// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The denormal policy law's tests (night-depth-mariana-1): the pin is
//! read back from the hardware control register (trust-but-verify),
//! the pin is idempotent, the pin corrects a hostile ambient state
//! (the whole reason the law exists), and the law has teeth — after
//! the pin, a product that lands inside the f32 denormal band
//! provably flushes to zero on the calling thread. These tests run on
//! whatever thread cargo's harness gives them; the pin is thread-local
//! by definition, so each test pins its own thread — exactly the
//! posture the render lane gets from `VoicePool` construction.
//!
//! Under Miri the pin is a documented no-op (the UB interpreter does
//! not execute inline asm), so the hardware-dependent tests guard
//! themselves with `cfg(not(miri))` — Miri audits the pure arithmetic
//! families, the control register is hardware.

use crate::engine::denormals::{pin_render_lane, render_lane_policy_active};

/// The pin is visible in the hardware control register.
#[cfg(not(miri))]
#[test]
fn pin_is_read_back_from_the_control_register() {
    pin_render_lane();
    assert!(
        render_lane_policy_active(),
        "the denormal policy law must leave FTZ set (x86-64) or FZ set (AArch64) on the pinning thread"
    );
}

/// The pin is idempotent — pinning twice is one policy, not a delta.
#[test]
fn pin_is_idempotent() {
    pin_render_lane();
    pin_render_lane();
    assert!(
        render_lane_policy_active(),
        "a second pin must not disturb the policy"
    );
}

/// The law corrects a hostile ambient state: a thread carrying the
/// full-IEEE posture (flush bits clear — the state a foreign library
/// could leave behind) is brought to the pinned policy by the pin.
/// Without the law, such a host would silently re-baseline every
/// render hash on that machine; with it, every machine renders the
/// same bytes.
#[cfg(all(target_arch = "x86_64", not(miri)))]
#[test]
fn pin_corrects_a_full_ieee_host() {
    // Clear the FTZ bit — the hostile posture (ldmxcsr takes a memory
    // operand; the test holds its own copy of the asm contract).
    let mut current: u32 = 0;
    unsafe {
        core::arch::asm!(
            "stmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &mut current,
            options(nostack, preserves_flags)
        );
    }
    let hostile: u32 = current & !0x8000;
    unsafe {
        core::arch::asm!(
            "ldmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &hostile,
            options(nostack, preserves_flags)
        );
    }
    assert!(
        !render_lane_policy_active(),
        "precondition: the thread now carries the hostile full-IEEE posture"
    );
    pin_render_lane();
    assert!(
        render_lane_policy_active(),
        "the pin must correct the hostile ambient state to the pinned policy"
    );
}

/// The teeth: a product that lands inside the f32 denormal band
/// (1.0e-20 × 1.0e-20 = 1.0e-40 — between the smallest normal,
/// ~1.18e-38, and the smallest subnormal, ~1.4e-45) flushes to zero
/// under the law. `black_box` keeps the compiler from folding the
/// operands at compile time — the multiply must run under the
/// thread's real control register. This is the exact posture the
/// voice tail walks (the ZACTRIX_AUDIT dust finding) and the exact
/// arithmetic every post-law render hash was measured in. (A product
/// below ~1.4e-45 underflows to zero under full IEEE too — that is
/// arithmetic, not policy — so the probe is aimed at the band the
/// policy actually governs.)
#[cfg(not(miri))]
#[test]
fn denormal_output_flushes_under_the_law() {
    pin_render_lane();
    let dust = std::hint::black_box(1.0e-20f32) * std::hint::black_box(1.0e-20f32);
    assert_eq!(
        dust, 0.0,
        "a denormal product must flush to zero under the pinned law (FTZ on x86-64, FZ on AArch64)"
    );
}

/// The pool construction funnel carries the law: constructing a
/// `VoicePool` pins the calling thread — the audit harness and the
/// player both render on the constructing thread, so this is the
/// contract the whole byte-identity estate rides.
#[cfg(not(miri))]
#[test]
fn pool_construction_pins_the_render_lane() {
    let _pool = crate::engine::VoicePool::new();
    assert!(
        render_lane_policy_active(),
        "VoicePool construction must pin the calling thread's floating-point policy"
    );
}
