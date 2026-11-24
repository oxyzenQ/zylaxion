// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

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
//! The classified set is closed on purpose (the owner's spec): nine
//! ways, no more. A failure that does not fit one of them is honest
//! about being unclassified, never pretending to a cause it cannot
//! prove — free-form failures through [`AttachError::Unknown`] with
//! their detail, and the classifier fallthroughs through
//! [`AttachError::UnknownErrno`], which carries the raw errno
//! STRUCTURED and renders its strerror text only when the sentence
//! prints (night-depth-hardening-4: the eager `format!` over a
//! symbolic errno was the verifier's SAT death valley — the deep
//! proof lane measured it). Both ride the same wire code 9, so an
//! old marker file keeps reading as the same failure.
//!
//! The marker lane: a failed attach plants a one-entry array map on
//! bpffs (see `plant_error_marker` in [`crate::ebpf::bpf_syscall`])
//! holding [`AttachError::code`] — the stable wire format the doctor
//! reads back with [`AttachError::from_code`] to name the last
//! failure. The codes are pinned by tests; they are a wire format,
//! never renumbered.

use std::fmt;

/// The attach lane whose raw errno reached a classifier's honest
/// fallthrough — the context the lazy render speaks with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrnoLane {
    /// perf_event_open registering a kprobe on the named symbol.
    KprobeOpen {
        /// The symbol the registration asked for.
        symbol: String,
    },
    /// The BTF tracing link syscall (the fentry lane's link step).
    FentryLink,
    /// BPF_LINK_CREATE on the perf-event-backed link (the kprobe
    /// lane's link step).
    LinkCreate,
}

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
    /// The embedded tap object failed the supply-chain anchor gate
    /// (night-netrunner-Z1): its blake3 does not recompute to the
    /// build-time stamp, or the stamp is absent (an out-of-tree
    /// compile that bypassed build.rs). The loader refuses the load —
    /// the ring-0 payload runs only on the strength of bytes the
    /// build staged, never on unverified bytes. The detail carries
    /// the verdict's own words (both digests on drift, the absence on
    /// a missing stamp).
    ObjectUnsealed {
        /// The anchor verdict's own words (the same detail the sig
        /// report's D4 row renders).
        detail: String,
    },
    /// The failure fits none of the classified ways; the detail
    /// carries the raw story.
    Unknown {
        /// The unclassified failure's own words.
        detail: String,
    },
    /// The classifier's honest fallthrough: the raw errno did not fit
    /// the lane's classified table. The errno rides STRUCTURED and the
    /// strerror text renders only when the sentence prints (night-
    /// depth-hardening-4): under the verifier's whole-i32 proofs the
    /// constructors stay string-machinery-free — the eager `format!`
    /// over a symbolic errno was the SAT death valley the deep proof
    /// lane measured — and the doctor's sentence loses nothing, the
    /// render produces the same bytes the eager form did (pinned by
    /// tests).
    UnknownErrno {
        /// The lane whose classified table the errno did not fit.
        lane: ErrnoLane,
        /// The raw errno the syscall answered with.
        errno: i32,
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
            AttachError::ObjectUnsealed { detail } => format!(
                "the tap object failed the supply-chain anchor gate: {} — the loader \
                 refuses to load an unsealed object into the kernel (run `zylaxion sig` \
                 for the full depth-matrix verdict)",
                detail.trim_end()
            ),
            AttachError::Unknown { detail } => {
                // Command-neutral verb (night-total-lts-2): the detach
                // lane's classified races render through this variant
                // too, and "the attach failed" printed under a detach
                // run was its own dirty bug. The context is which
                // command the user ran; the sentence stays honest.
                format!(
                    "the operation failed for an unclassified reason: {}.",
                    detail.trim_end()
                )
            }
            AttachError::UnknownErrno { lane, errno } => {
                // The lazy render (night-depth-hardening-4): the errno
                // text resolves here, on the concrete print path, never
                // inside the constructors the whole-i32 proofs drive.
                // The bytes match the eager form this lane replaced,
                // pinned by tests.
                let text = errno_text(*errno);
                let detail = match lane {
                    ErrnoLane::KprobeOpen { symbol } => format!(
                        "perf_event_open on kprobe:{symbol} failed with errno {errno} ({text})"
                    ),
                    ErrnoLane::FentryLink => format!(
                        "the fentry tracing link syscall failed with errno {errno} ({text})"
                    ),
                    ErrnoLane::LinkCreate => {
                        format!("BPF_LINK_CREATE failed with errno {errno} ({text})")
                    }
                };
                format!("the operation failed for an unclassified reason: {detail}.")
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
            AttachError::ObjectUnsealed { .. } => 10,
            AttachError::Unknown { .. } => 9,
            // The structured errno fallthrough shares Unknown's wire
            // code: the marker band stays the pinned 1..=10 space and
            // an old marker file keeps reading as the same honest
            // unclassified failure.
            AttachError::UnknownErrno { .. } => 9,
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
            // The night-netrunner-Z1 variant takes the next free
            // code: 9 stays Unknown (the wire-format law — an old
            // marker's code must keep reading as the same failure).
            10 => AttachError::ObjectUnsealed {
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
            other => AttachError::UnknownErrno {
                lane: ErrnoLane::KprobeOpen {
                    symbol: symbol.to_string(),
                },
                errno: other,
            },
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
            other => AttachError::UnknownErrno {
                lane: ErrnoLane::FentryLink,
                errno: other,
            },
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
            other => AttachError::UnknownErrno {
                lane: ErrnoLane::LinkCreate,
                errno: other,
            },
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
            ProgramError::LoadError {
                verifier_log,
                io_error,
                ..
            } => {
                // night-depth-hardening-1: the verifier log's wording,
                // line count, and state-dump depth differ per kernel
                // version (5.15 vs 6.x print different context for the
                // same rejection), so the user-visible verdict keeps
                // only the tail — the last lines carry the actual
                // rejection verdict — and the full log rides the info
                // journal. The tail bound keeps the live output shape
                // stable across kernels while the classification (the
                // code-4 marker) stays untouched.
                let full = verifier_log.to_string();
                // The raw errno rides the journal: a PRE-WALK rejection
                // (the summary says zero instructions processed) never
                // prints a reason line — the errno is the only signal
                // the kernel gives (night-frontier-Z2, the notice-relay
                // load named it).
                log::info!(
                    "BPF verifier rejected the tap program (errno {}), full log:\n{full}",
                    io_error.raw_os_error().unwrap_or(0)
                );
                AttachError::VerifierRejected {
                    detail: format!(
                        "errno {} — {}",
                        io_error.raw_os_error().unwrap_or(0),
                        verifier_tail(&full, 3)
                    ),
                }
            }
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

/// The verifier log tail (night-depth-hardening-1): the last `keep`
/// non-empty lines, each trimmed and capped at 120 characters, joined
/// with " | ". Verifier logs end with the verdict line and often open
/// with hundreds of state-dump lines whose wording and ordering are
/// kernel-version-specific; the tail is the load-bearing, bounded half
/// of the log — same input, same output, on every kernel shape.
/// `pub(crate)`: the Kani lane proves the tail's bound law (night-
/// frontier-Z6) — at most `keep` lines, each capped, over any input.
///
/// night-depth-hardening-4 reshaped the body as an assume-guarantee
/// decomposition with a byte-identical contract (the unit tree pins
/// the exact render): the deep lane measured the former single-body
/// form — iterator-adapter chain first, plain loops with preallocated
/// staging after — running the verifier out of memory either way, and
/// the probe ladder isolated the cause. One SAT instance carrying BOTH
/// a loop over a heap `Vec` (a symbolic trip count, unrolled to the
/// unwind bound) AND heap `String` growth multiplies the memory model
/// into a bomb; the stages prove in their OWN instances —
/// [`keep_last_lines`] over the line-count axis,
/// [`cap_and_join`] at the cap's exact boundary — and the
/// composition here (slice, call) is pinned byte-exact by the unit
/// tree. No behavior moved: same lines, same filter, same cap, same
/// separator, same empty-log verdict. The decomposition was necessary
/// but not sufficient — the second measurement round found a bomb
/// inside each stage's own instance (the cap arm's per-char String
/// collect on concrete input; the keep harness's wide symbolic
/// selector) and reshaped both; the per-stage doc notes carry those
/// records.
pub(crate) fn verifier_tail(log_text: &str, keep: usize) -> String {
    cap_and_join(&keep_last_lines(log_text, keep))
}

/// The tail's stage one: every non-empty trimmed line of `log_text`,
/// cut to the last `keep` of them. `pub(crate)`: the Kani lane proves
/// the keep bound over this stage's whole (bounded) line-count domain
/// — the harness rides a symbolic selector over concrete logs, whose
/// trip counts stay small enough for the verifier's memory ceiling
/// (the decomposition note on [`verifier_tail`] carries the
/// measurement). The line walk is a manual `\n` scan, not
/// `str::lines()`: the lines() iterator machinery is a measured
/// memory bomb under the verifier (an 85-second out-of-memory on a
/// seven-byte FULLY CONCRETE probe — the pointer model pays for the
/// iterator's internal searcher whatever the input), while `trim()`
/// and the explicit loop probe in seconds. Equivalence holds by the
/// body's own contract: every kept line is trimmed, and `trim()`
/// strips the `\r` that CRLF line endings leave inside a `\n`-cut
/// segment — so cutting on `\n` alone and trimming yields the SAME
/// kept lines `lines()` yields (the unit tree pins the exact render
/// across CRLF, blank-line, and trailing-segment shapes).
pub(crate) fn keep_last_lines(log_text: &str, keep: usize) -> Vec<&str> {
    let bytes = log_text.as_bytes();
    let mut lines: Vec<&str> = Vec::with_capacity(16);
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let trimmed = log_text[start..index].trim();
            if !trimmed.is_empty() {
                lines.push(trimmed);
            }
            start = index + 1;
        }
        index += 1;
    }
    // The trailing segment after the last newline (empty when the
    // log ends with one): the same cut-and-trim arm as the loop body.
    let trimmed = log_text[start..].trim();
    if !trimmed.is_empty() {
        lines.push(trimmed);
    }
    let drop = lines.len().saturating_sub(keep);
    lines[drop..].to_vec()
}

/// The tail's stage-two cap as production binds it: 120 characters
/// plus the ellipsis. A named constant, not a magic number at the cut
/// site — the binding is data the unit tree pins (the 200-character
/// probe), while the cut mechanism itself is cap-generic and rides
/// [`cap_and_join_at`] where the proof lane can quantify it
/// (the harness doc in `kani_laws.rs` carries the measurement story).
const VERIFIER_LINE_CAP: usize = 120;

/// The tail's stage two: each line capped at 120 characters plus the
/// ellipsis, joined with " | ". `pub(crate)`: the Kani lane proves the
/// cap law over concrete array probes at the exact boundary (a
/// 121-character line cuts to 120 plus the ellipsis; shorter lines
/// ride through whole, joined). The cap arm's cut rides a char-
/// boundary slice, not a per-char collect: the slice is the measured
/// shape — the collect's String growth ground CBMC past six minutes
/// on the concrete probe alone (the note inside the body carries the
/// record).
pub(crate) fn cap_and_join(lines: &[&str]) -> String {
    cap_and_join_at(lines, VERIFIER_LINE_CAP)
}

/// The cap-and-join mechanism, parameterized over the cut bound: the
/// line over `cap` characters cuts to its `cap`-th char boundary plus
/// the ellipsis, shorter lines ride through whole, and adjacent lines
/// join with the separator. Production binds `cap` to
/// [`VERIFIER_LINE_CAP`] (120) at [`cap_and_join`]; the binding
/// is pinned by the unit tree's 200-character probe, and the
/// mechanism is what the proof lane quantifies — at a small cap the
/// same law verifies in seconds where the 121-character probe ground
/// the verifier out of memory (the pointer model walks every byte of
/// the probe through the heap objects regardless of the input's
/// concreteness; a small cap carries the identical branch structure
/// over a fraction of the instance — the tree's own laws-not-details
/// idiom, the classifier table's shape applied to the cut).
pub(crate) fn cap_and_join_at(lines: &[&str], cap: usize) -> String {
    let mut tail = String::with_capacity(64);
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            tail.push_str(" | ");
        }
        // The cut point in one scan: the byte offset of the
        // (cap+1)-th char, when one exists. Slicing there yields
        // exactly the first `cap` chars — byte-identical to the
        // former `chars().take(120).collect()` (the unit tree pins
        // the render) without the per-char String growth. That
        // growth was the second measured memory bomb on the proof
        // lane: a 120-iteration collect rides ~six reallocations
        // through the verifier's heap model, and CBMC ground past
        // the six-minute mark on the concrete 121-character probe
        // alone; the boundary scan plus one bounded push_str carries
        // the same law in a fraction of the instance
        // (night-depth-hardening-4, the second measurement).
        if let Some((offset, _)) = line.char_indices().nth(cap) {
            tail.push_str(&line[..offset]);
            tail.push_str("...");
        } else {
            tail.push_str(line);
        }
    }
    tail
}

/// errno -> the strerror text (the marker doctor line never sees this;
/// the live attach log does). A tiny local table instead of
/// `strerror_r` FFI — the errno set that can reach it is small and
/// known, and std-only discipline holds. `pub(crate)`: the Kani lane
/// proves the table's totality over the whole i32 errno domain
/// (night-frontier-Z6).
pub(crate) fn errno_text(errno: i32) -> &'static str {
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
