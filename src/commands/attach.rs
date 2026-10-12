// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The `attach` and `detach` subcommands (night-improve-23;
//! night-improve-25: the classified error surface).
//!
//! `attach` is the ONE privileged surface in the whole architecture: it
//! walks the fentry → kprobe attach chain, loads the tap, and pins the
//! control plane to bpffs — then exits. Everything else zylaxion does
//! afterwards (player, preset switches, status, telemetry) reads and
//! writes the pinned maps as the unprivileged user the pins were
//! handed to. Failures arrive classified ([`crate::ebpf::attach_error::AttachError`]
//! on the ebpf lane) and print their complete sentences; a failed
//! attach leaves nothing half-attached — only the failure marker the
//! doctor reads.
//!
//! `detach` unpins everything — after it, NOTHING of zylaxion exists in
//! the kernel. It stays reachable unprivileged: unlink permission on the
//! pin files decides, which is exactly the ownership the attach handed
//! out.

use std::process;

/// Handle `zylaxion attach` (run under sudo).
pub fn cmd_attach() {
    // The privilege verdict (night-improve-16's audit table, the
    // night-improve-23 REQUIRE row): attach is the one surface that
    // must run as root.
    //
    // THE POSITIVE-POLARITY RULE (the night-badas-1 lesson, pinned):
    // requirement gates take the POSITIVE euid fact — the same shape
    // the sibling gates in player.rs and update.rs use. This call site
    // once passed the NEGATED fact, and the double negation inverted
    // the whole gate: root was refused ("attach must run as root"
    // under sudo) while non-root sailed through. The fact flows in as
    // it is read; the requirement function owns every negation. The
    // test tree pins the polarity both ways.
    if let Some(refusal) =
        crate::session::privilege::attach_requirement(crate::session::privilege::euid_is_root())
    {
        crate::output::error(refusal);
        process::exit(1);
    }

    init_logger();

    match crate::ebpf::attach() {
        Ok(summary) => {
            println!("{}", summary);
        }
        Err(e) => {
            crate::output::error(e.to_string());
            process::exit(1);
        }
    }
}

/// Handle `zylaxion detach`.
pub fn cmd_detach() {
    init_logger();

    match crate::ebpf::detach() {
        Ok(summary) => {
            println!("{}", summary);
        }
        Err(e) => {
            crate::output::error(e.to_string());
            process::exit(1);
        }
    }
}

/// Shared logger wiring (the night-improve-13 lane): one builder, the
/// hand-rolled UTC stamp, no time crate. The loader logs the attach
/// chain's per-step verdicts at info (the fentry refusal, the kprobe
/// fallbacks); the summary is the user-facing half of the same story.
fn init_logger() {
    use std::io::Write as _;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {:<5} {}] {}",
                crate::info::clock::iso_utc_millis(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
}
