// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The userspace eBPF surface (night-improve-23, the daemonless
//! architecture).
//!
//! Three concerns, one folder:
//!
//! - [`embedded`] — the tap object's aligned embedding (the zelynic
//!   NIGHT-hunt-30 lesson: aya's ELF parser needs 8-byte-aligned bytes,
//!   and `include_bytes!` placement is linker luck — the AlignedElf
//!   wrapper makes alignment a property of the type).
//! - [`pin`] — the bpffs pin paths: the single source of truth for
//!   everything the tap leaves at /sys/fs/bpf/zylaxion.
//! - [`loader`] — attach (load, init, pin: the privileged one-shot that
//!   plants the kernel-side state store) and detach (unpin everything:
//!   after it, NOTHING of zylaxion exists in the kernel).
//!
//! The module is feature-gated (`ebpf`, default on). The dormant lane
//! (`--no-default-features`) compiles the same verb surface with the
//! honest refusal: a binary built without the loader says so instead of
//! pretending.

#[cfg(feature = "ebpf")]
pub mod bpf_syscall;
#[cfg(feature = "ebpf")]
pub mod embedded;
#[cfg(feature = "ebpf")]
pub mod loader;
#[cfg(feature = "ebpf")]
pub mod pin;

/// The verb surface both lanes expose: the same call signatures, so the
/// commands layer cannot tell them apart.
#[cfg(feature = "ebpf")]
pub use loader::{attach, detach};

/// The dormant lane: the honest refusal.
#[cfg(not(feature = "ebpf"))]
pub use dormant::{attach, detach};

/// The dormant lane: the same call signatures, the honest refusal.
#[cfg(not(feature = "ebpf"))]
pub mod dormant {
    /// The refusal text both dormant verbs return.
    pub const DORMANT_REFUSAL: &str = "this binary was built without eBPF support \
         (--no-default-features) — rebuild with default features to attach the tap";

    /// Dormant attach: refuse.
    pub fn attach() -> Result<String, String> {
        Err(DORMANT_REFUSAL.to_string())
    }

    /// Dormant detach: refuse (nothing this binary could have pinned).
    pub fn detach() -> Result<String, String> {
        Err(DORMANT_REFUSAL.to_string())
    }
}
