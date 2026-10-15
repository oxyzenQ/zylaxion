// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Kernel capability probe — the eBPF floor verdict (night-improve-23;
//! night-improve-25 retools the input-tap verdict for the
//! fentry → kprobe chain).
//!
//! night-improve-23 is eBPF-only: the input tap is a passive
//! kernel-side probe on the exported `input_event()` function, the
//! state store is pinned maps on bpffs, and the daemon (with its
//! libinput seat capture) is deleted. That makes the kernel the
//! hardware floor: below the owner-mandated minimum there is no
//! fallback lane and no silent downgrade — the tool says exactly why
//! it cannot run. The owner set the floor at 5.15 (the LTS every
//! supported distro already ships); 5.15 carries everything the chain
//! needs (fentry: 5.5+, the kprobe PMU: 4.17+, BPF ring buffer: 5.8+,
//! bpffs pinning: ancient).
//!
//! One probe, two consumers:
//! - the doctor reports every fact with the canonical verdicts
//!   ("verified supported" / "verified unsupported") and the input-tap
//!   verdict verbatim,
//! - the attach path hard-gates on the same report — the probe is
//!   committed once, both surfaces read it, the verdicts cannot drift.
//!
//! The input-tap verdict (night-improve-25) pre-verifies the attach
//! chain from /sys facts, in chain order, recording the FIRST step
//! that is verifiably available:
//!
//! 1. fentry: `input_event` present in /sys/kernel/btf/vmlinux.
//! 2. kprobe: `input_event` absent from the kprobe blacklist.
//! 3. kprobe: `input_handle_event` absent from the same blacklist.
//!
//! The BTF check is a full-token byte search (the BTF string section
//! is NUL-separated, so `\\0input_event\\0` is an exact-name match); it
//! cannot distinguish a FUNC entry from a same-named struct, so the
//! verdict is a preflight, not proof — the attach attempt stays the
//! authoritative verdict (a probe must never promise what a load
//! would refuse). The blacklist check reads tracefs (then its debugfs
//! mirror); when neither is readable the kprobe steps read as
//! unverified, never as available.
//!
//! The pure decision core (release parsing, lockdown parsing, the
//! verdict mapping, the input-tap verdict) is separated from the
//! filesystem probes so the test tree pins it without touching the
//! host.

/// The owner-mandated minimum kernel: 5.15 LTS.
pub const MIN_KERNEL: (u32, u32) = (5, 15);

/// Everything the two consumers need, in one structure.
#[derive(Debug, Clone)]
pub struct KernelReport {
    /// The raw release string (`/proc/sys/kernel/osrelease`), empty
    /// when unreadable.
    pub release: String,
    /// `(major, minor)` parsed from `release`; `None` when
    /// unparsable.
    pub parsed: Option<(u32, u32)>,
    /// `/sys/kernel/btf/vmlinux` exists and carries the `input_event`
    /// token. Optional for the chain (the kprobe steps need no BTF)
    /// — reported for completeness, never fatal.
    pub btf: bool,
    /// A `bpf` filesystem is mounted at `/sys/fs/bpf` — the pinned
    /// state store lives there. Required.
    pub bpffs: bool,
    /// The input-tap verdict — one of the four canonical
    /// night-improve-25 strings ([`input_tap_verdict`]), pre-verifying
    /// the attach chain from /sys facts.
    pub input_tap: String,
    /// `/sys/kernel/security/lockdown` was readable.
    pub lockdown_readable: bool,
    /// The active lockdown mode is `confidentiality` — the kernel
    /// withdraws BPF entirely. Required-false.
    pub lockdown_blocks_bpf: bool,
    /// The raw `kernel.unprivileged_bpf_disabled` value (0 allowed,
    /// 1 disabled-but-recoverable, 2 disabled — see
    /// [`Self::unpriv_bpf_boot_locked`] for what the 2 really means;
    /// any other non-zero value reads as disabled). `0` also when the
    /// sysctl file is absent — no gate file, no gate.
    pub unpriv_bpf_disabled: u8,
    /// The boot-parameter lock behind a raw value of 2
    /// (night-dinner-3): the cmdline carries
    /// `bpf.unprivileged_bpf_disabled=2`, the one shape the kernel
    /// plants as unflippable for this boot. A 2 WITHOUT it is the
    /// distro's built-in default posture and root can write 0 over
    /// it (witnessed live on Debian 13 / 6.12, where the old
    /// "locked, cannot be re-enabled this boot" claim was refuted by
    /// the very next sysctl). `false` when the value is not 2 or the
    /// cmdline is unreadable.
    pub unpriv_bpf_boot_locked: bool,
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

    /// The pure posture law: does this sysctl value allow the plain
    /// user's bpf() calls at all? Only `0` does — `1` and `2` both
    /// gate every non-bpf_capable caller; root is never gated (the
    /// gate keys on capability, not on the sysctl).
    pub fn unpriv_bpf_allowed(&self) -> bool {
        self.unpriv_bpf_disabled == 0
    }

    /// The doctor's posture line — a pure function of the raw value
    /// and the boot-param fact. The value-2 arm splits on the lock:
    /// a boot-parameter 2 is unflippable this boot; any other 2 is
    /// the distro's default and root may write 0 over it. Every arm
    /// carries the substring the E2E engine pins ("unprivileged
    /// bpf") — do not reword it away.
    pub fn unpriv_bpf_verdict(&self) -> String {
        match self.unpriv_bpf_disabled {
            0 => "unprivileged bpf: allowed — the control-plane verbs run as a plain user"
                .to_string(),
            1 => "unprivileged bpf: disabled (1) — the verbs cannot run as a plain user; \
                  root may re-enable it this boot: sysctl -w kernel.unprivileged_bpf_disabled=0"
                .to_string(),
            2 if self.unpriv_bpf_boot_locked => {
                "unprivileged bpf: locked (2) — the boot parameter \
                  bpf.unprivileged_bpf_disabled=2 planted this lock and it cannot be re-enabled \
                  this boot; the control-plane verbs cannot run as a plain user"
                    .to_string()
            }
            2 => "unprivileged bpf: disabled (2) — the distro's built-in default, no \
                  boot-parameter lock on the cmdline; root may re-enable it this boot: \
                  sudo sysctl -w kernel.unprivileged_bpf_disabled=0"
                .to_string(),
            other => format!(
                "unprivileged bpf: disabled ({other}) — the kernel refuses unprivileged bpf()"
            ),
        }
    }

    /// The kernel-policy gate for every surface that must talk to the
    /// pinned control plane (night-badas-4 round 5): when the kernel
    /// gates unprivileged bpf() and the caller is not root, no map
    /// read or write can succeed — the honest answer is the classified
    /// refusal, not the misleading "not attached" shape an EPERM
    /// surfaces as. The message carries the substring the E2E battery
    /// pins ("unprivileged bpf") — do not reword it without the
    /// engine's sm_audio.py.
    pub fn unpriv_map_write_refusal(&self, euid_is_root: bool) -> Option<String> {
        if euid_is_root || self.unpriv_bpf_allowed() {
            return None;
        }
        Some(format!(
            "{}\n  tip: the pins stay readable after `sudo zylaxion attach`, but the \
             control-plane map ops need the kernel's unprivileged bpf gate open — \
             see `zylaxion doctor`",
            self.unpriv_bpf_verdict()
        ))
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
/// parse; a missing minor reads as 0; a present-but-unparsable
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

// ── The input-tap verdict (night-improve-25) ──────────────────────────────

/// The BTF-side availability of the fentry lane (step 1 of the chain).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtfStatus {
    /// `/sys/kernel/btf/vmlinux` exists and carries the `input_event`
    /// token — the fentry lane's preflight passes.
    Available,
    /// The BTF file is absent (a kernel built without
    /// `CONFIG_DEBUG_INFO_BTF`).
    Absent,
    /// The BTF file exists but cannot be read.
    Unreadable,
    /// The BTF is readable but carries no `input_event` token.
    MissingSymbol,
}

/// One symbol's standing on the kernel's kprobe blacklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlacklistStatus {
    /// The blacklist is readable and the symbol is not on it — the
    /// kprobe step's preflight passes.
    NotListed,
    /// The symbol is explicitly blacklisted: the kernel refuses to
    /// probe it.
    Listed,
    /// The blacklist could not be read (tracefs restricted for this
    /// session); the step is UNVERIFIED, never available.
    Unreadable,
}

/// The input-tap verdict — a pure function of the probe facts, in
/// chain order, recording the FIRST step that is verifiably
/// available. The four canonical shapes (the doctor renders this
/// verbatim):
///
/// 1. `"input tap: fentry:input_event (BTF-available)"`
/// 2. `"input tap: kprobe:input_event (fentry unavailable: <reason>)"`
/// 3. `"input tap: kprobe:input_handle_event (input_event unavailable: <reason>)"`
/// 4. `"input tap: UNAVAILABLE (<classified reason>)"`
///
/// A step counts as available only when VERIFIED (BTF token found;
/// blacklist read and clean) — an unreadable blacklist never reads as
/// available, and the unavailable fallbacks carry their reasons so
/// the verdict explains itself. The attach attempt remains the
/// authoritative verdict; this is the preflight the doctor prints.
pub fn input_tap_verdict(btf: BtfStatus, primary: BlacklistStatus, alt: BlacklistStatus) -> String {
    match btf {
        BtfStatus::Available => "input tap: fentry:input_event (BTF-available)".to_string(),
        other => {
            let fentry_reason = match other {
                BtfStatus::Absent => "no vmlinux BTF (CONFIG_DEBUG_INFO_BTF=n)".to_string(),
                BtfStatus::Unreadable => "vmlinux BTF unreadable".to_string(),
                BtfStatus::MissingSymbol => "input_event not present in vmlinux BTF".to_string(),
                BtfStatus::Available => unreachable!("matched above"),
            };
            if primary == BlacklistStatus::NotListed {
                return format!(
                    "input tap: kprobe:input_event (fentry unavailable: {fentry_reason})"
                );
            }
            if alt == BlacklistStatus::NotListed {
                let primary_reason = match primary {
                    BlacklistStatus::Listed => "input_event is on the kprobe blacklist",
                    BlacklistStatus::Unreadable => "kprobe blacklist unreadable",
                    BlacklistStatus::NotListed => unreachable!("matched above"),
                };
                return format!(
                    "input tap: kprobe:input_handle_event (input_event unavailable: \
                     {primary_reason})"
                );
            }
            // The chain is exhausted or unverifiable from this session —
            // the compound reason names every step's standing, and when
            // anything is merely unverifiable the verdict says so.
            let mut reasons = vec![format!("fentry unavailable: {fentry_reason}")];
            for (symbol, status) in [("input_event", primary), ("input_handle_event", alt)] {
                reasons.push(match status {
                    BlacklistStatus::Listed => format!("{symbol} is kprobe-blacklisted"),
                    BlacklistStatus::Unreadable => {
                        format!("{symbol} unverified (kprobe blacklist unreadable)")
                    }
                    BlacklistStatus::NotListed => unreachable!("matched above"),
                });
            }
            format!("input tap: UNAVAILABLE ({})", reasons.join("; "))
        }
    }
}

/// The full-token BTF search: does `/sys/kernel/btf/vmlinux` carry the
/// symbol as a complete NUL-delimited string? The BTF string section
/// begins with a NUL and separates names with NULs, so
/// `b"\0<symbol>\0"` is an exact-name match — `xinput_event` and
/// `input_event2` both miss.
pub fn btf_contains_symbol(btf_bytes: &[u8], symbol: &str) -> bool {
    let mut needle = Vec::with_capacity(symbol.len() + 2);
    needle.push(0u8);
    needle.extend_from_slice(symbol.as_bytes());
    needle.push(0u8);
    btf_bytes.windows(needle.len()).any(|w| w == needle)
}

/// Parse one kprobe blacklist rendering for one symbol. The blacklist
/// lines read `<address> <symbol>[\t<module>]` (the format
/// /sys/kernel/tracing/kprobes/blacklist documents); the caller owns
/// the read and hands the content in.
pub fn parse_blacklist(content: &str, symbol: &str) -> BlacklistStatus {
    let listed = content.lines().any(|line| {
        let mut fields = line.split_whitespace();
        let _address = fields.next();
        fields.next() == Some(symbol)
    });
    if listed {
        BlacklistStatus::Listed
    } else {
        BlacklistStatus::NotListed
    }
}

/// The pure unprivileged-bpf law: parse the sysctl file's contents
/// into the raw posture value. Empty or malformed content reads as 0
/// (allowed) — a probe must never invent a lock it cannot witness,
/// and the gate's own bounds keep the real values at 0, 1 or 2.
pub fn parse_unpriv_bpf_disabled(raw: &str) -> u8 {
    raw.trim().parse::<u8>().unwrap_or(0)
}

/// The pure boot-param lock law (night-dinner-3): does the kernel
/// command line carry the one parameter shape that plants a value of
/// 2 the sysctl cannot undo this boot? An exact whole-token match —
/// `bpf.unprivileged_bpf_disabled=1` or a stray prefix must not read
/// as the lock. The runtime-default 2 (the Debian 13 shape the
/// doctor used to mislabel "locked") carries no such token, which is
/// exactly what the live session proved: the write of 0 took.
pub fn unpriv_bpf_locked_by_boot_param(cmdline: &str) -> bool {
    cmdline
        .split_whitespace()
        .any(|token| token == "bpf.unprivileged_bpf_disabled=2")
}

// ── bpffs reachability (night-dinner-3) ─────────────────────────────

/// The pure traversal law: does this directory's mode grant the
/// identity the execute bit that stat()-ing through it needs? The
/// identity selects ONE triad — owner, then group, then other — the
/// kernel's own decision order, with no fall-through (an owner denied
/// by the owner triad does not borrow the other triad's bit).
pub fn mode_grants_x(mode: u32, owner: u32, group: u32, uid: u32, gid: u32) -> bool {
    let shift = if uid == owner {
        6
    } else if gid == group {
        3
    } else {
        0
    };
    (mode >> shift) & 0o1 == 0o1
}

/// The bpffs reach verdict for one identity (night-dinner-3): can
/// that user stat() through the pin path? The mount root and the pin
/// directory each need the x bit; the pins themselves open under
/// their own file perms (attach hands them to the invoking user), so
/// only the two directories gate the walk.
#[derive(Debug, Clone, Copy)]
pub struct BpffsReach {
    /// `/sys/fs/bpf` grants this identity the traversal bit (or the
    /// identity is root, whom the capability law bypasses the bits
    /// for).
    pub root_traversable: bool,
    /// The mount root's permission bits, octal-rendered in the fix
    /// lines (`0` when the root is unstatable — the doctor's own
    /// bpffs-mounted check names that case first).
    pub root_mode: u32,
    /// The pin directory exists (a tap's state store is planted —
    /// though under a blocked walk even this fact is beyond the
    /// caller's sight, so the doctor renders the root verdict only).
    pub pin_dir_present: bool,
    /// `/sys/fs/bpf/zylaxion` grants the traversal bit — meaningful
    /// only when [`Self::pin_dir_present`].
    pub pin_dir_traversable: bool,
}

/// Stat both directories and apply [`mode_grants_x`] for `(uid,
/// gid)`. Root (uid 0) bypasses the bits — the kernel's capability
/// law, not the mode's. A missing or unstatable mount root reads as
/// non-traversable with mode 0: this probe must never lie about a
/// mount it cannot see.
pub fn bpffs_reach(uid: u32, gid: u32) -> BpffsReach {
    use std::os::unix::fs::MetadataExt;
    let (root_mode, root_traversable) = match std::fs::metadata("/sys/fs/bpf") {
        Ok(m) => {
            let mode = m.mode();
            (
                mode & 0o7777,
                uid == 0 || mode_grants_x(mode, m.uid(), m.gid(), uid, gid),
            )
        }
        Err(_) => (0, false),
    };
    match std::fs::metadata("/sys/fs/bpf/zylaxion") {
        Ok(m) => {
            let mode = m.mode();
            BpffsReach {
                root_traversable,
                root_mode,
                pin_dir_present: true,
                pin_dir_traversable: uid == 0 || mode_grants_x(mode, m.uid(), m.gid(), uid, gid),
            }
        }
        Err(_) => BpffsReach {
            root_traversable,
            root_mode,
            pin_dir_present: false,
            pin_dir_traversable: false,
        },
    }
}

/// Probe the host kernel (the filesystem-reading half). Every field
/// degrades to its honest default when the backing file is absent —
/// a probe must never panic on a hardened kernel.
pub fn probe() -> KernelReport {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let parsed = parse_kernel_release(&release);
    let btf_status = probe_btf_status();
    let btf = btf_status == BtfStatus::Available;
    let bpffs = bpffs_mounted();
    let input_tap = input_tap_verdict(
        btf_status,
        probe_blacklist("input_event"),
        probe_blacklist("input_handle_event"),
    );
    let lockdown = std::fs::read_to_string("/sys/kernel/security/lockdown").ok();
    let lockdown_readable = lockdown.is_some();
    let lockdown_blocks_bpf = lockdown.map(|s| lockdown_blocks(&s)).unwrap_or(false);
    let unpriv_bpf_disabled = std::fs::read_to_string("/proc/sys/kernel/unprivileged_bpf_disabled")
        .map(|s| parse_unpriv_bpf_disabled(&s))
        .unwrap_or(0);
    let cmdline = std::fs::read_to_string("/proc/cmdline").unwrap_or_default();
    KernelReport {
        release,
        parsed,
        btf,
        bpffs,
        input_tap,
        lockdown_readable,
        lockdown_blocks_bpf,
        unpriv_bpf_disabled,
        unpriv_bpf_boot_locked: unpriv_bpf_locked_by_boot_param(&cmdline),
    }
}

/// The BTF-side fact: absent, unreadable, missing the symbol, or
/// available.
fn probe_btf_status() -> BtfStatus {
    let path = std::path::Path::new("/sys/kernel/btf/vmlinux");
    if !path.exists() {
        return BtfStatus::Absent;
    }
    match std::fs::read(path) {
        Ok(bytes) => {
            if btf_contains_symbol(&bytes, "input_event") {
                BtfStatus::Available
            } else {
                BtfStatus::MissingSymbol
            }
        }
        Err(_) => BtfStatus::Unreadable,
    }
}

/// The blacklist-side fact for one symbol: tracefs first, then the
/// debugfs mirror (the same order the attach-era probes used); every
/// path unreadable is honestly Unreadable.
fn probe_blacklist(symbol: &str) -> BlacklistStatus {
    for path in [
        "/sys/kernel/tracing/kprobes/blacklist",
        "/sys/kernel/debug/tracing/kprobes/blacklist",
    ] {
        if let Ok(content) = std::fs::read_to_string(path) {
            return parse_blacklist(&content, symbol);
        }
    }
    BlacklistStatus::Unreadable
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
