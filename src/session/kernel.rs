// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Kernel capability probe — the eBPF floor verdict (night-improve-23).
//!
//! night-improve-23 is eBPF-only: the input tap is a BPF tracepoint
//! program, the state store is pinned maps on bpffs, and the daemon
//! (with its libinput seat capture) is deleted. That makes the kernel
//! the hardware floor: below the owner-mandated minimum there is no
//! fallback lane and no silent downgrade — the tool says exactly why
//! it cannot run. The owner set the floor at 5.15 (the LTS every
//! supported distro already ships); 5.15 carries everything the tap
//! needs (tracepoint `input:input_event`: 5.2+, BPF ring buffer:
//! 5.8+, bpffs pinning: ancient).
//!
//! One probe, two consumers:
//! - the doctor reports every fact with the canonical verdicts
//!   ("verified supported" / "verified unsupported"),
//! - the attach path hard-gates on the same report — the probe is
//!   committed once, both surfaces read it, the verdicts cannot
//!   drift.
//!
//! The pure decision core (release parsing, lockdown parsing, the
//! verdict mapping) is separated from the filesystem probes so the
//! test tree pins it without touching the host.

/// The owner-mandated minimum kernel: 5.15 LTS.
pub const MIN_KERNEL: (u32, u32) = (5, 15);

/// Everything the two consumers need, in one structure.
#[derive(Debug, Clone)]
pub struct KernelReport {
    /// The raw release string (`/proc/sys/kernel/osrelease`), empty
    /// when unreadable.
    pub release: String,
    /// `(major, minor)` parsed from `release`; `None` when
    /// unparseable.
    pub parsed: Option<(u32, u32)>,
    /// `/sys/kernel/btf/vmlinux` exists. Optional for the tracepoint
    /// tap (no CO-RE introspection needed) — reported for
    /// completeness, never fatal.
    pub btf: bool,
    /// A `bpf` filesystem is mounted at `/sys/fs/bpf` — the pinned
    /// state store lives there. Required.
    pub bpffs: bool,
    /// The `input:input_event` tracepoint directory is visible under
    /// tracefs. Visibility is session-dependent (tracefs may be
    /// restricted for unprivileged readers) — informational; the
    /// attach attempt is the authoritative verdict.
    pub tracepoint_visible: bool,
    /// `/sys/kernel/security/lockdown` was readable.
    pub lockdown_readable: bool,
    /// The active lockdown mode is `confidentiality` — the kernel
    /// withdraws BPF entirely. Required-false.
    pub lockdown_blocks_bpf: bool,
}

impl KernelReport {
    /// The floor verdict: the parsed release meets the 5.15 minimum.
    pub fn supported(&self) -> bool {
        is_supported(self.parsed)
    }

    /// The canonical verdict string the doctor and the attach
    /// refusal both print — a pure function of the probe facts.
    pub fn verdict(&self) -> &'static str {
        if self.supported() {
            "verified supported"
        } else {
            "verified unsupported"
        }
    }
}

/// The pure floor law: `(major, minor)` meets the 5.15 minimum.
pub fn is_supported(parsed: Option<(u32, u32)>) -> bool {
    match parsed {
        Some((major, minor)) => {
            major > MIN_KERNEL.0 || (major == MIN_KERNEL.0 && minor >= MIN_KERNEL.1)
        }
        None => false,
    }
}

/// Parse a kernel release string into `(major, minor)`.
///
/// Handles the real-world shapes: "6.8.0-45-generic",
/// "5.15.167.4-microsoft-standard-WSL2", "6.12.1". The major must
/// parse; a missing minor reads as 0; a present-but-unparseable
/// minor is a hard `None` (never guess a version the kernel did not
/// state).
pub fn parse_kernel_release(release: &str) -> Option<(u32, u32)> {
    let mut parts = release.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = match parts.next() {
        Some(m) => m.parse::<u32>().ok()?,
        None => 0,
    };
    Some((major, minor))
}

/// The pure lockdown law: does the active mode block BPF?
///
/// `/sys/kernel/security/lockdown` renders as a whitespace-separated
/// mode list with the ACTIVE mode in square brackets, e.g.
/// `none [integrity] confidentiality`. Only `confidentiality`
/// withdraws BPF (`integrity` allows it).
pub fn lockdown_blocks(lockdown: &str) -> bool {
    lockdown
        .split_whitespace()
        .find_map(|token| token.strip_prefix('[').and_then(|t| t.strip_suffix(']')))
        .map(|active| active == "confidentiality")
        .unwrap_or(false)
}

/// Probe the host kernel (the filesystem-reading half). Every field
/// degrades to its honest default when the backing file is absent —
/// a probe must never panic on a hardened kernel.
pub fn probe() -> KernelReport {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let parsed = parse_kernel_release(&release);
    let btf = std::path::Path::new("/sys/kernel/btf/vmlinux").exists();
    let bpffs = bpffs_mounted();
    let tracepoint_visible = std::path::Path::new("/sys/kernel/tracing/events/input/input_event")
        .exists()
        || std::path::Path::new("/sys/kernel/debug/tracing/events/input/input_event").exists();
    let lockdown = std::fs::read_to_string("/sys/kernel/security/lockdown").ok();
    let lockdown_readable = lockdown.is_some();
    let lockdown_blocks_bpf = lockdown.map(|s| lockdown_blocks(&s)).unwrap_or(false);
    KernelReport {
        release,
        parsed,
        btf,
        bpffs,
        tracepoint_visible,
        lockdown_readable,
        lockdown_blocks_bpf,
    }
}

/// Scan `/proc/mounts` for a `bpf` filesystem mounted at
/// `/sys/fs/bpf` (no statfs FFI — the same std-only discipline the
/// rest of the session folder keeps).
fn bpffs_mounted() -> bool {
    std::fs::read_to_string("/proc/mounts")
        .map(|content| {
            content.lines().any(|line| {
                // Fields: device mount-point filesystem-type options ...
                let mut fields = line.split_whitespace();
                let _device = fields.next();
                let mount = fields.next();
                let fstype = fields.next();
                mount == Some("/sys/fs/bpf") && fstype == Some("bpf")
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "../../test/session/kernel_tests.rs"]
mod tests;
