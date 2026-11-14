// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The denormal policy law (night-depth-mariana-1): one pinned
//! floating-point environment for the render lane — flush-to-zero on
//! the outputs, written by us, never inherited from the machine.
//!
//! # Why the law exists
//!
//! The byte-identity contract ("same input → same sound, every time")
//! after night-depth-hardening-1 rides a tail that is pure IEEE-754
//! mul/add — bit-exact on any conforming core under one given control
//! word. Exactly one environment variable was left unpinned: the
//! floating-point control state. The SSE control register (x86-64)
//! and FPCR (AArch64) both ship a flush-to-zero policy that the OS
//! leaves at its default (denormals fully supported), but nothing
//! guarantees the default survives: any library on the link line, any
//! host, any distribution may run the render thread with FTZ already
//! set — and the render would silently diverge from every published
//! hash on that machine. This module removes that variable by writing
//! the policy ourselves, once per render-lane thread, at
//! [`VoicePool`](crate::engine::VoicePool) construction — the single
//! funnel every renderer (the player, the audit harness, the test
//! tree) passes through.
//!
//! # Why the pinned word is FTZ-only, and why it is cross-arch
//! symmetric
//!
//! Both directions pin the environment; the measured answer picked
//! this one:
//!
//! 1. **The denormal dust path is real work.** The ZACTRIX_AUDIT
//!    finding — the click/spring integrator states end below the
//!    smallest normal f32 — means the voice tail walks the subnormal
//!    band, and every SSE op there pays a microcode assist. Measured
//!    on the A/B machine (the footprint state-b driver, 60 s, 5
//!    keys/s): 71 → 38 CPU ticks between the t=5 s and t=60 s
//!    samples (~1.29% → ~0.69% of one core, a 46% relative
//!    reduction). The idle lane is untouched.
//! 2. **The law's delta is dust-confined on BOTH shipped
//!    architectures, measured.** The delta instrument ran on x86-64
//!    and on the arm64 CI runner: 12 of 24 presets byte-identical
//!    under the law on each, every first divergent sample at -313.1
//!    dB relative to the stream peak — 253 dB below the -60 dB
//!    measurand floor. 0 gate failures on both sides. x86-64 pins
//!    FTZ (MXCSR bit 15, flush denormal results); AArch64 pins FZ
//!    (FPCR bit 24, flush denormal outputs for scalar and vector
//!    f32/f64) — the same output-flush contract in each control
//!    word.
//! 3. **DAZ (input flush) is measured byte-irrelevant and therefore
//!    not part of the law.** Rendering with FTZ+DAZ produced hashes
//!    identical to FTZ-only on every preset × rate (the rates audit,
//!    DIFF_COUNT 0) — no rendered byte depends on input flushing.
//!    The law pins the bit the bytes obey: FTZ. The word stays
//!    symmetric with AArch64's FZ, whose output-flush semantics are
//!    the exact counterpart.
//! 4. **The one-time byte re-baseline is quantified.** Under the law
//!    every preset × rate render hash changes against the pre-law
//!    ambient posture where the dust band is touched — and the delta
//!    instrument (`--denormal-delta`) proves the first divergent
//!    sample of every preset sits at or below the -60 dB measurand
//!    floor: voice lifetime, ring decay, and tail endpoint all print
//!    identical. The audible envelope is untouched; the new hashes
//!    are the estate's re-baselined bytes, and the parity lane rides
//!    them.
//!
//! # What the law does not do
//!
//! It does not touch rounding mode (the hardware default round-to-
//! nearest-even stays), it does not run per sample (one MXCSR/FPCR
//! write per pool construction — nothing in the hot path), and it
//! does not touch the audio callback thread (it only drains the ring
//! buffer; the render math lives on the thread that constructed the
//! pool). AArch64's FEAT_AFP controls (FIZ/AH) are deliberately not
//! written — the legacy FZ semantics are the symmetric contract, and
//! the AFP bits are reserved on cores without the feature.
//! Architectures other than the two shipped lanes are a documented
//! no-op.
//!
//! # Under Miri
//!
//! The UB interpreter does not execute inline asm — the pin and the
//! read-back compile to a documented no-op under `cfg(miri)`, and the
//! policy tests that need real hardware guard themselves the same
//! way. Miri audits the pure arithmetic families; the control
//! register is hardware, not UB.

/// Read the x86-64 SSE control word. `stmxcsr` takes a memory
/// operand, so the pointer rides a general register; the asm! default
/// (no `nomem` option) keeps the memory effect declared, so the
/// compiler cannot cache or reorder the value across the barrier. The
/// `_mm_getcsr` intrinsic is deprecated upstream with exactly this
/// inline-asm shape named as the replacement.
#[cfg(all(target_arch = "x86_64", not(miri)))]
#[inline]
fn read_mxcsr() -> u32 {
    let mut mxcsr: u32 = 0;
    unsafe {
        core::arch::asm!(
            "stmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &mut mxcsr,
            options(nostack, preserves_flags)
        );
    }
    mxcsr
}

/// Write the x86-64 SSE control word (`ldmxcsr`, memory operand —
/// same barrier discipline as [`read_mxcsr`]).
#[cfg(all(target_arch = "x86_64", not(miri)))]
#[inline]
fn write_mxcsr(bits: u32) {
    unsafe {
        core::arch::asm!(
            "ldmxcsr [{mxcsr_ptr}]",
            mxcsr_ptr = in(reg) &bits,
            options(nostack, preserves_flags)
        );
    }
}

/// The x86-64 flush bit the law pins: bit 15 = FTZ (flush-to-zero on
/// denormal results). DAZ (bit 6, input flush) is measured
/// byte-irrelevant and deliberately excluded — see the module doc.
/// The AArch64 counterpart is one bit — FPCR bit 24 (FZ) — the
/// output-flush contract in a single word.
#[cfg(all(target_arch = "x86_64", not(miri)))]
const X86_LAW_BITS: u32 = 0x8000;

/// Pin the render lane's floating-point policy on the calling thread.
///
/// Idempotent (the bit is OR'd in), one control-register write, and
/// thread-local by hardware definition. The pin *sets* the flush bit
/// on whatever state the host or a library left behind — a host that
/// ships full-IEEE posture renders the same bytes as one that ships
/// flush posture, because the law overwrites both; see the module doc
/// for the measured why.
#[inline]
pub(crate) fn pin_render_lane() {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        // Read-modify-write preserves every other field (rounding
        // mode, exception masks and trap enables — all owner
        // defaults).
        write_mxcsr(read_mxcsr() | X86_LAW_BITS);
    }
    #[cfg(all(target_arch = "aarch64", not(miri)))]
    {
        // FPCR bit 24 = FZ — flush-to-zero for denormalized outputs,
        // scalar and vector f32/f64. Read-modify-write preserves
        // every other field (rounding mode, exception trap enables —
        // all owner defaults).
        let mut fpcr: u64;
        unsafe {
            core::arch::asm!("mrs {0}, fpcr", out(reg) fpcr);
            fpcr |= 1 << 24;
            core::arch::asm!("msr fpcr, {0}", in(reg) fpcr);
        }
    }
    // Every other architecture — and Miri, which does not execute
    // inline asm — is a documented no-op: Miri audits the pure
    // arithmetic families, the control register is hardware.
    #[cfg(any(miri, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
    {
        // Nothing to pin — see the module doc.
    }
}

/// Read back the pinned bit (the audit's trust-but-verify surface).
///
/// Returns `true` when the calling thread's control register carries
/// the law's policy: flush-to-zero ON, the exact arithmetic posture
/// every post-law render hash was measured in. Under Miri and on a
/// no-op architecture the answer is vacuously `true` — those surfaces
/// do not carry the render's bytes.
#[cfg(test)]
#[inline]
pub(crate) fn render_lane_policy_active() -> bool {
    #[cfg(all(target_arch = "x86_64", not(miri)))]
    {
        read_mxcsr() & X86_LAW_BITS == X86_LAW_BITS
    }
    #[cfg(all(target_arch = "aarch64", not(miri)))]
    {
        let fpcr: u64;
        unsafe {
            core::arch::asm!("mrs {0}, fpcr", out(reg) fpcr);
        }
        fpcr & (1 << 24) != 0
    }
    #[cfg(any(miri, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
    {
        true
    }
}
