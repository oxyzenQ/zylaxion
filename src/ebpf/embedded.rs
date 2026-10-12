// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Aligned embedding of the eBPF tap object (night-improve-23, the
//! zelynic NIGHT-hunt-30 lesson carried over verbatim).
//!
//! The embedded ELF is parsed at load time by aya, which hands the bytes
//! straight to the `object` crate — and `object`'s Pod casts read ELF64
//! structures straight out of the buffer, which requires the buffer's
//! ADDRESS to be 8-byte aligned. `include_bytes!` produces an align-1
//! static, so whether the bytes land 8-aligned is decided by the linker's
//! .rodata packing — per binary, per host, per build. On the zelynic
//! lineage's owner host every build of 2026-09-20..21 placed the object
//! at an unaligned address, and the failure surfaced as
//! `error parsing BPF object: error parsing ELF data` — with a HEALTHY
//! artifact. The fix is this module's [`AlignedElf`]: the embedded object
//! rides inside an 8-aligned wrapper, so the linker's packing decisions
//! can no longer reach aya's parser.

/// The dedup-defeating guard value of [`AlignedElf`].
///
/// A fixed magic ("ZYLF-23\0" — this hunt's zylaxion stamp) whose only
/// job is to make every `AlignedElf` allocation's bytes differ from a raw
/// `include_bytes!` of the same object. rustc's const interner dedups
/// immutable allocations by content, ignoring alignment — the zelynic
/// probe verified a wrapper static and a raw include of the SAME file
/// folding onto one address. Without the guard, a future raw include of
/// the staged object anywhere in the tree could fold the aligned
/// allocation onto an align-1 placement and silently reintroduce this
/// bug.
const ALIGNED_ELF_GUARD: u64 = u64::from_le_bytes(*b"ZYLF-23\0");

/// An embedded eBPF ELF whose bytes are 8-byte aligned by construction.
///
/// Two hardening details, both load-bearing (verified against rustc
/// 1.98.1 in the zelynic release probe: guard + object bytes are emitted
/// as ONE contiguous allocation at an 8-aligned offset):
///
/// - `#[repr(C, align(8))]` makes every allocation of this type
///   8-aligned by contract — symbol alignment is honored by every
///   linker, so this is a property of the type, not of the layout luck.
///   The C layout pins `bytes` to offset 8 (after the u64 guard), so the
///   data pointer stays 8-aligned.
/// - the `_guard` field makes the allocation's content unique (see
///   [`ALIGNED_ELF_GUARD`]), so the const interner can never fold it
///   into an align-1 placement.
///
/// Embed with `&AlignedElf::new(*include_bytes!(path)).bytes` — the
/// projection serves a `&[u8]` pointing INTO this aligned allocation.
#[repr(C, align(8))]
pub struct AlignedElf<const N: usize> {
    _guard: u64,
    /// The ELF container bytes, at offset 8 of the allocation.
    pub bytes: [u8; N],
}

impl<const N: usize> AlignedElf<N> {
    /// Wrap one embedded ELF for aligned embedding.
    pub const fn new(bytes: [u8; N]) -> Self {
        Self {
            _guard: ALIGNED_ELF_GUARD,
            bytes,
        }
    }
}

/// Tripwire: the misalignment of an embedded-object slice, as
/// `Some(ptr % 8)`.
///
/// Structurally impossible while the object rides inside
/// [`AlignedElf`] — kept as the pure detector behind the load-path
/// preflight, so any future regression (a new unaligned embed path, a
/// wrapper refactor that loses the alignment) turns back into a one-line
/// diagnosis instead of a hunt through a healthy artifact.
pub fn misalignment(bytes: &[u8]) -> Option<usize> {
    let m = bytes.as_ptr() as usize % 8;
    if m == 0 { None } else { Some(m) }
}

/// The load-path preflight built on [`misalignment`]. Returns the full
/// error message when `bytes` sit at an address the `object` crate's
/// ELF64 parser cannot read from — `None` when the load may proceed.
pub fn alignment_violation(bytes: &[u8], name: &str) -> Option<String> {
    misalignment(bytes).map(|m| {
        format!(
            "the embedded {name} BPF object sits {m} bytes past an 8-byte boundary \
             ({:p}) — aya's ELF parser must read 8-byte-aligned bytes, and the \
             AlignedElf embedding is supposed to guarantee exactly that. This is a \
             build regression, not an environment problem: rebuild from source, and \
             report it if it persists",
            bytes.as_ptr()
        )
    })
}

#[cfg(test)]
#[path = "../../test/ebpf/embedded_tests.rs"]
mod tests;
