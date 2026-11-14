// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The denormal policy law (night-depth-mariana-1): one pinned
//! floating-point environment for the render lane — flush-to-zero
//! mode, written by us, never inherited from the machine.
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
//! host, any distribution may run the render thread with FTZ/DAZ
//! already set — and the render would silently diverge from every
//! published hash on that machine. This module removes that variable
//! by writing the policy ourselves, once per render-lane thread, at
//! [`VoicePool`] construction — the single funnel every renderer (the
//! player, the audit harness, the test tree) passes through.
//!
//! # Why the pinned word is flush-to-zero, not full IEEE
//!
//! Both directions pin the environment; the measured answer picked
//! this one:
//!
//! 1. **The denormal dust path is real work.** The ZACTRIX_AUDIT
//!    finding — the click/spring integrator states end below the
//!    smallest normal f32 — means the voice tail walks the subnormal
//!    band, and every SSE op there pays a microcode assist. Measured
//!    on the A/B machine (the footprint state-b driver, 60 s, 5
//!    keys/s): 71 → 39 CPU ticks, 1.183% → 0.65% of one core — a 45%
//!    relative reduction, with the idle lane (state-c) untouched.
//! 2. **The policy is cross-arch symmetric by specification.** x86-64
//!    pins FTZ (MXCSR bit 15, flush denormal results) + DAZ (bit 6,
//!    treat denormal inputs as zero); AArch64 pins FZ (FPCR bit 24),
//!    which the architecture defines as flushing denormalized input
//!    operands AND output results for scalar and vector f32 — exactly
//!    the DAZ+FTZ pair. One policy, two words, same arithmetic.
//! 3. **The one-time byte re-baseline is quantified.** Under the law
//!    every preset × rate render hash changes against the pre-law
//!    ambient posture — and the delta instrument
//!    (`zactrix-rates`' sibling, `--denormal-delta`) proves the first
//!    divergent sample of every preset sits at or below the -60 dB
//!    measurand floor: voice lifetime, ring decay, and tail endpoint
//!    all print identical. The audible envelope is untouched; the new
//!    hashes are the estate's re-baselined bytes, and the cross-arch
//!    parity lane rides them.
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

/// Read the x86-64 SSE control word. `stmxcsr` takes a memory
/// operand, so the pointer rides a general register; the asm! default
/// (no `nomem` option) keeps the memory effect declared, so the
/// compiler cannot cache or reorder the value across the barrier. The
/// `_mm_getcsr` intrinsic is deprecated upstream with exactly this
/// inline-asm shape named as the replacement.
#[cfg(target_arch = "x86_64")]
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
#[cfg(target_arch = "x86_64")]
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

/// The x86-64 flush bits the law pins: bit 15 = FTZ (flush-to-zero on
/// denormal results), bit 6 = DAZ (denormals-are-zero on inputs).
/// The AArch64 counterpart is one bit — FPCR bit 24 (FZ) — which the
/// architecture defines as flushing denormalized inputs AND outputs,
/// the DAZ+FTZ pair in one word.
const X86_FLUSH_BITS: u32 = 0x8040;

/// Pin the render lane's floating-point policy on the calling thread.
///
/// Idempotent (the bits are OR'd in), one control-register write, and
/// thread-local by hardware definition. The pin *sets* the flush bits
/// on whatever state the host or a library left behind — a host that
/// ships full-IEEE posture renders the same bytes as one that ships
/// flush posture, because the law overwrites both; see the module doc
/// for the measured why.
#[inline]
pub(crate) fn pin_render_lane() {
    #[cfg(target_arch = "x86_64")]
    {
        // Read-modify-write preserves every other field (rounding
        // mode, exception masks and trap enables — all owner
        // defaults).
        write_mxcsr(read_mxcsr() | X86_FLUSH_BITS);
    }
    #[cfg(target_arch = "aarch64")]
    {
        // FPCR bit 24 = FZ — flush-to-zero for denormalized inputs
        // and outputs, scalar and vector f32/f64. Read-modify-write
        // preserves every other field (rounding mode, exception trap
        // enables — all owner defaults).
        let mut fpcr: u64;
        unsafe {
            core::arch::asm!("mrs {0}, fpcr", out(reg) fpcr);
            fpcr |= 1 << 24;
            core::arch::asm!("msr fpcr, {0}", in(reg) fpcr);
        }
    }
    // Every other architecture: the documented no-op. zylaxion ships
    // exactly two lanes (x86-64, AArch64); a third lane must come with
    // its own policy word here before it can claim byte-identity.
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        // Nothing to pin — see the module doc.
    }
}

/// Read back the pinned bits (the audit's trust-but-verify surface).
///
/// Returns `true` when the calling thread's control register carries
/// the law's policy: flush-to-zero ON, the exact arithmetic posture
/// every post-law render hash was measured in. On a no-op
/// architecture the answer is vacuously `true` — the same
/// architecture cannot claim byte-identity anyway.
#[cfg(test)]
#[inline]
pub(crate) fn render_lane_policy_active() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        read_mxcsr() & X86_FLUSH_BITS == X86_FLUSH_BITS
    }
    #[cfg(target_arch = "aarch64")]
    {
        let fpcr: u64;
        unsafe {
            core::arch::asm!("mrs {0}, fpcr", out(reg) fpcr);
        }
        fpcr & (1 << 24) != 0
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        true
    }
}
