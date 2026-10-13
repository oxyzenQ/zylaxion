// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The userspace eBPF surface (night-improve-23, the daemonless
//! architecture; night-improve-25 restores the approved attach chain).
//!
//! Five concerns, one folder:
//!
//! - [`args`] — the input-event argument shim: the four arguments of
//!   the kernel's exported `input_event()` both attach programs read
//!   (single-sourced with the ebpf crate via `#[path]`).
//! - [`attach_error`] — the classified attach failure taxonomy plus
//!   the marker wire format (the code a failed attach leaves on
//!   bpffs for the doctor to read back).
//! - [`embedded`] — the tap object's aligned embedding (the zelynic
//!   NIGHT-hunt-30 lesson: aya's ELF parser needs 8-byte-aligned bytes,
//!   and `include_bytes!` placement is linker luck — the AlignedElf
//!   wrapper makes alignment a property of the type).
//! - [`pin`] — the bpffs pin paths: the single source of truth for
//!   everything the tap leaves at /sys/fs/bpf/zylaxion.
//! - [`loader`] — attach (walk the fentry → kprobe chain, init, pin:
//!   the privileged one-shot that plants the kernel-side state store).
//! - [`teardown`] — detach (unpin everything through the classified
//!   night-improve-26 cleanup: pins, failure marker, the pin directory
//!   itself) and the doctor's read side of the failure marker.
//!
//! The module is feature-gated (`ebpf`, default on). The dormant lane
//! (`--no-default-features`) compiles the same verb surface with the
//! honest refusal: a binary built without the loader says so instead of
//! pretending.

#[cfg(feature = "ebpf")]
pub mod args;
#[cfg(feature = "ebpf")]
pub mod attach_error;
#[cfg(feature = "ebpf")]
pub mod bpf_syscall;
#[cfg(feature = "ebpf")]
pub mod embedded;
#[cfg(feature = "ebpf")]
pub mod loader;
#[cfg(feature = "ebpf")]
pub mod pin;
#[cfg(feature = "ebpf")]
pub mod teardown;

/// The verb surface both lanes expose: the same call signatures, so the
/// commands layer cannot tell them apart.
#[cfg(feature = "ebpf")]
pub use loader::{attach, detach};

/// The dormant lane: the honest refusal (the verbs only — the dormant
/// [`dormant::AttachError`] is never named outside the inference of
/// these two calls).
#[cfg(not(feature = "ebpf"))]
pub use dormant::{attach, detach};

/// The dormant lane: the same call signatures, the honest refusal.
#[cfg(not(feature = "ebpf"))]
pub mod dormant {
    /// The refusal's classification: nothing in this binary can reach
    /// the kernel, so the taxonomy's honest bucket is Unknown with the
    /// rebuild instruction as its detail. (The full nine-variant
    /// taxonomy lives in the ebpf-gated [`crate::ebpf::attach_error`]
    /// — it classifies aya and raw-syscall failures the dormant build
    /// has no code for, so it cannot compile here.)
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum AttachError {
        /// The one dormant-lane refusal (see [`DORMANT_REFUSAL`]).
        Unknown {
            /// The refusal text.
            detail: String,
        },
    }

    impl std::fmt::Display for AttachError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let AttachError::Unknown { detail } = self;
            f.write_str(detail)
        }
    }

    /// The refusal text both dormant verbs return.
    pub const DORMANT_REFUSAL: &str = "this binary was built without eBPF support \
         (--no-default-features) — rebuild with default features to attach the tap";

    /// Dormant attach: refuse.
    pub fn attach() -> Result<String, AttachError> {
        Err(AttachError::Unknown {
            detail: DORMANT_REFUSAL.to_string(),
        })
    }

    /// Dormant detach: refuse (nothing this binary could have pinned).
    pub fn detach() -> Result<String, AttachError> {
        Err(AttachError::Unknown {
            detail: DORMANT_REFUSAL.to_string(),
        })
    }
}
