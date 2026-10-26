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
//! The module is unconditional since night-dragon-maintenance-0 (the
//! zejtron unkillable-ebpf law): there is no feature gate and no build
//! flag can produce a dormant binary — every zylaxion build carries the
//! full tap surface. On a git checkout the loader embeds the
//! source-built object (the nested nightly build build.rs drives); on
//! a registry tarball it embeds the maintainer-built object staged
//! from ebpf-prebuilt/ (manifest-verified) — either way the install
//! needs no nightly toolchain on the target machine.

pub mod args;
pub mod attach_error;
pub mod bpf_syscall;
pub mod embedded;
pub mod loader;
pub mod pin;
pub mod teardown;

/// The verb surface: the same call signatures the commands layer has
/// always consumed.
pub use loader::{attach, detach};
