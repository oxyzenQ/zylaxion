// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The attach error taxonomy (night-improve-25) — the classified
//! vocabulary the attach chain speaks.
//!
//! Every way `sudo zylaxion attach` can fail has exactly one variant
//! here, and every variant renders a complete human sentence the
//! doctor can print verbatim (no bare strings, no collapsed causes —
//! an attach that cannot explain itself is a failed attach). The
//! chain classifies its raw syscall and loader failures through the
//! pure constructors at the bottom of this module; the tests feed
//! each synthetic failure into the classifier and assert the exact
//! variant, the same discipline the night-badas-1 privilege gate
//! added for its polarity.
//!
//! The variant set is closed on purpose (the owner's spec): nine
//! ways, no more. A failure that does not fit one of them is
//! [`AttachError::Unknown`] with its detail — honest about being
//! unclassified, never pretending to a cause it cannot prove.
//!
//! The marker lane: a failed attach plants a one-entry array map on
//! bpffs (see `plant_error_marker` in [`crate::ebpf::bpf_syscall`])
//! holding [`AttachError::code`] — the stable wire format the doctor
//! reads back with [`AttachError::from_code`] to name the last
//! failure. The codes are pinned by tests; they are a wire format,
//! never renumbered.

use std::fmt;

/// The classified way an attach failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachError {
    /// The fentry lane (step 1 of the chain) is unavailable: no
    /// vmlinux BTF, an unreadable BTF, or `input_event` absent from
    /// it. The chain falls back to the kprobe lanes; this surfaces
    /// as the FINAL error only when they fail too (or when the gate
    /// refuses before the chain starts).
    FentryUnavailable {
        /// Why the fentry lane cannot run on this host.
        reason: String,
    },
    /// The target symbol is on the kernel's kprobe blacklist
    /// (/sys/kernel/debug/kprobes/blacklist) — the kernel refuses to
    /// probe it. `input_event` is an `EXPORT_SYMBOL` plain-text
    /// function and is NOT blacklisted on mainline kernels; seeing
    /// this means a hardened or patched kernel.
    KprobeBlacklisted,
    /// The kernel exposes no kprobe-able `input_event` /
    /// `input_handle_event` symbol (perf_event_open answered ENOENT)
    /// and the fentry lane was unavailable — the input core cannot be
    /// tapped on this kernel.
    SymbolMissing,
    /// The BPF verifier rejected the tap program (program load
    /// failed with a verifier log). The detail carries the log.
    VerifierRejected {
        /// The kernel verifier's own words.
        detail: String,
    },
    /// A BPF operation was refused for lack of privilege (EPERM /
    /// EACCES) — attach must run as root, and lockdown 'integrity'
    /// still allows BPF; check both.
    PermissionDenied,
    /// bpffs is not mounted at /sys/fs/bpf — the pinned state store
    /// has nowhere to live.
    BpffsUnavailable,
    /// Pinning to bpffs failed at the named path. The attach never
    /// leaves a partial pin set, so everything was cleaned before
    /// this error returned.
    PinFailed {
        /// The bpffs path that refused the pin.
        path: String,
    },
    /// Kernel lockdown 'confidentiality' is active — the kernel
    /// withdraws BPF entirely.
    LockdownActive,
    /// The failure fits none of the classified ways; the detail
    /// carries the raw story.
    Unknown {
        /// The unclassified failure's own words.
        detail: String,
    },
}

impl fmt::Display for AttachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            AttachError::FentryUnavailable { reason } => format!(
                "the fentry lane is unavailable on this kernel ({reason}) — the attach chain \
                 needs the kprobe lanes or nothing."
            ),
            AttachError::KprobeBlacklisted => "the kernel's kprobe blacklist blocks the input \
                core symbols (/sys/kernel/debug/kprobes/blacklist) — this kernel refuses to \
                probe input_event."
                .to_string(),
            AttachError::SymbolMissing => "the kernel exposes no kprobe-able \
                input_event/input_handle_event symbol and fentry was unavailable — the input \
                core cannot be tapped on this kernel."
                .to_string(),
            AttachError::VerifierRejected { detail } => {
                // Verifier logs end with whatever the kernel last said
                // (often a bare newline); the render owns the sentence.
                format!(
                    "the BPF verifier rejected the tap program: {}.",
                    detail.trim_end()
                )
            }
            AttachError::PermissionDenied => "a BPF operation was refused for lack of \
                privilege — attach must run under sudo, and kernel lockdown 'integrity' \
                still allows BPF (only 'confidentiality' withdraws it)."
                .to_string(),
            AttachError::BpffsUnavailable => "bpffs is not mounted at /sys/fs/bpf — the \
                pinned state store has nowhere to live (fix: sudo mount -t bpf bpf \
                /sys/fs/bpf, normally automatic)."
                .to_string(),
            AttachError::PinFailed { path } => format!(
                "pinning to bpffs failed at {path} — the attach cleaned every pin it had \
                 planted; nothing of zylaxion remains half-attached."
            ),
            AttachError::LockdownActive => "kernel lockdown 'confidentiality' is active — \
                the kernel withdraws BPF; choose a lockdown mode that allows it (integrity \
                or none)."
                .to_string(),
            AttachError::Unknown { detail } => {
                format!(
                    "the attach failed for an unclassified reason: {}.",
                    detail.trim_end()
                )
            }
        };
        f.write_str(&s)
    }
}

impl AttachError {
    /// The marker-map code — the stable wire format a failed attach
    /// leaves for the doctor to read back ([`Self::from_code`] is its
    /// inverse). 0 is reserved for "no failure recorded"; the codes
    /// are pinned by tests and must never be renumbered (a renumbered
    /// code would make an old marker read as a different failure).
    pub fn code(&self) -> u32 {
        match self {
            AttachError::FentryUnavailable { .. } => 1,
            AttachError::KprobeBlacklisted => 2,
            AttachError::SymbolMissing => 3,
            AttachError::VerifierRejected { .. } => 4,
            AttachError::PermissionDenied => 5,
            AttachError::BpffsUnavailable => 6,
            AttachError::PinFailed { .. } => 7,
            AttachError::LockdownActive => 8,
            AttachError::Unknown { .. } => 9,
        }
    }

    /// Reconstruct the variant from a marker-map code (the doctor's
    /// read side of the marker lane). Detail-carrying variants come
    /// back with their canonical detail — the marker stores the
    /// classification, not the per-run detail; `sudo zylaxion attach`
    /// prints the full story live.
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            1 => AttachError::FentryUnavailable {
                reason: "see the attach run's output".to_string(),
            },
            2 => AttachError::KprobeBlacklisted,
            3 => AttachError::SymbolMissing,
            4 => AttachError::VerifierRejected {
                detail: "see the attach run's output".to_string(),
            },
            5 => AttachError::PermissionDenied,
            6 => AttachError::BpffsUnavailable,
            7 => AttachError::PinFailed {
                path: "see the attach run's output".to_string(),
            },
            8 => AttachError::LockdownActive,
            9 => AttachError::Unknown {
                detail: "see the attach run's output".to_string(),
            },
            _ => return None,
        })
    }

    /// The kprobe registration verdict: what `perf_event_open`'s errno
    /// means for a kprobe registration on one symbol. The errno values
    /// are the kernel's own classifications (kernel/trace/trace_kprobe.c
    /// and the perf core): ENOENT is a symbol kallsyms cannot resolve,
    /// EINVAL is the registration refusing the probe (blacklisted or
    /// otherwise not probe-able), EPERM/EACCES is privilege.
    pub fn from_kprobe_open(symbol: &str, errno: i32) -> Self {
        match errno {
            libc::EPERM | libc::EACCES => AttachError::PermissionDenied,
            libc::ENOENT => AttachError::SymbolMissing,
            libc::EINVAL => AttachError::KprobeBlacklisted,
            other => {
                let text = errno_text(other);
                AttachError::Unknown {
                    detail: format!(
                        "perf_event_open on kprobe:{symbol} failed with errno {other} ({text})"
                    ),
                }
            }
        }
    }

    /// The fentry link verdict: what the tracing link syscall's errno
    /// means. EPERM is privilege (or lockdown withdrawing BPF — the
    /// gate catches the named lockdown case first); EINVAL is the
    /// kernel refusing the tracing link for a program whose BTF
    /// attachment did not hold; anything else is honestly unknown.
    pub fn from_fentry_link(errno: i32) -> Self {
        match errno {
            libc::EPERM | libc::EACCES => AttachError::PermissionDenied,
            libc::ENOENT => AttachError::SymbolMissing,
            libc::EINVAL => AttachError::FentryUnavailable {
                reason: "the kernel refused the BTF tracing link".to_string(),
            },
            other => {
                let text = errno_text(other);
                AttachError::Unknown {
                    detail: format!(
                        "the fentry tracing link syscall failed with errno {other} ({text})"
                    ),
                }
            }
        }
    }

    /// The perf-link verdict: what BPF_LINK_CREATE's errno means for
    /// the kprobe lane's perf-event-backed link.
    pub fn from_link_create(errno: i32) -> Self {
        match errno {
            libc::EPERM | libc::EACCES => AttachError::PermissionDenied,
            libc::EINVAL => AttachError::Unknown {
                detail: "BPF_LINK_CREATE rejected the perf-event link (the kernel may not \
                         support bpf_perf_link — 5.10+ required, inside the 5.15 floor)"
                    .to_string(),
            },
            other => {
                let text = errno_text(other);
                AttachError::Unknown {
                    detail: format!("BPF_LINK_CREATE failed with errno {other} ({text})"),
                }
            }
        }
    }

    /// The program-load verdict (the load stage both lanes pass
    /// through aya): a BTF resolution failure is a fentry-lane fact
    /// (the kprobe lane never resolves BTF), a verifier log is a
    /// verifier rejection, and a privilege errno classifies through
    /// the same law as the raw syscall lanes.
    pub fn from_program_load(e: aya::programs::ProgramError) -> Self {
        use aya::programs::ProgramError;
        match e {
            ProgramError::Btf(err) => AttachError::FentryUnavailable {
                reason: format!("vmlinux BTF lookup failed: {err}"),
            },
            ProgramError::LoadError { verifier_log, .. } => AttachError::VerifierRejected {
                detail: verifier_log.to_string(),
            },
            ProgramError::SyscallError(sc) if is_permission(&sc.io_error) => {
                AttachError::PermissionDenied
            }
            other => AttachError::Unknown {
                detail: format!("{other}"),
            },
        }
    }
}

/// The privilege law shared by every classifier: EPERM and EACCES are
/// privilege refusals, whatever lane reported them.
fn is_permission(e: &std::io::Error) -> bool {
    matches!(e.raw_os_error(), Some(libc::EPERM) | Some(libc::EACCES))
}

/// errno -> the strerror text (the marker doctor line never sees this;
/// the live attach log does). A tiny local table instead of
/// `strerror_r` FFI — the errno set that can reach it is small and
/// known, and std-only discipline holds.
fn errno_text(errno: i32) -> &'static str {
    match errno {
        libc::EPERM => "operation not permitted",
        libc::EACCES => "permission denied",
        libc::ENOENT => "no such file or directory",
        libc::EINVAL => "invalid argument",
        libc::ENOMEM => "out of memory",
        libc::EBUSY => "device or resource busy",
        libc::E2BIG => "argument list too long",
        libc::ENOSPC => "no space left on device",
        _ => "unrecognized errno",
    }
}

#[cfg(test)]
#[path = "../../test/ebpf/attach_error_tests.rs"]
mod tests;
