// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The single help authority for `zylaxion -h` / `--help`
//! (night-improve-15, the zelynic NIGHT-improve-3 single-tier lineage:
//! clap's auto help and its `help` subcommand are disabled — this
//! curated reference is the only help surface, and subcommand-level
//! `--help` is an unknown-argument error, never a second help wall).
//!
//! The integration-pin discipline is the zelynic one: every subcommand
//! the CLI exposes must appear in the reference, and the group
//! headings must hold, so adding a command or reshuffling a group
//! without updating this module fails the suite (the tests below).
//!
//! Tier law (the zelynic help contract, one palette — see
//! [`crate::cli::styles`] and BRANDING.md): the banner and every
//! section heading ride brand purple bold; every command synopsis and
//! every flag spelling rides calm grey (grammar, not content); every
//! runnable example rides status green (the "this is what you type"
//! tier); prose stays default. Under a pipe or `NO_COLOR` everything
//! degrades to byte-exact plain text (the stdout capability gate).

use crate::output::{BRAND_OPEN, GREY_OPEN, OK_OPEN, RESET};

/// Render the end-to-end reference: usage, commands grouped by role,
/// examples, global flags, and the no-subcommand contract.
///
/// Pure function of the color decision so tests can pin the plain
/// text byte-for-byte ([`print_reference`] computes the gate from the
/// real stdout capability).
pub(crate) fn render_reference(color: bool) -> String {
    let mut out = String::new();
    let mut push = |line: String| {
        out.push_str(&line);
        out.push('\n');
    };

    // One span painter so a tier can never drift per line.
    fn span(open: &str, text: &str, on: bool) -> String {
        if on {
            format!("{open}{text}{RESET}")
        } else {
            text.to_string()
        }
    }
    let brand = |t: &str| span(BRAND_OPEN, t, color);
    let grey = |t: &str| span(GREY_OPEN, t, color);
    let ok = |t: &str| span(OK_OPEN, t, color);

    push(brand(
        "━━━ zylaxion — Real-time Mechanical Keyboard Acoustic Synthesizer ━━━",
    ));
    push(String::new());
    push("Turn every keystroke into a spatially-accurate click sound through".into());
    push("your speakers — kernel evdev in, low-latency PipeWire audio out.".into());
    push(String::new());

    push(brand("Run (the eBPF control plane, night-improve-23):"));
    push(String::new());
    push(grey("  sudo zylaxion attach"));
    push("      Plant the kernel-side tap + pinned control plane (once per boot).".into());
    push("      The ONE privileged surface; every other verb stays unprivileged.".into());
    push(ok("      sudo zylaxion attach"));
    push(String::new());
    push(grey("  zylaxion on [--preset <NAME>]"));
    push("      Turn the sound on — spawns the stateless player, no daemon.".into());
    push("      # hear it right now".into());
    push(ok("      zylaxion on"));
    push("      # a specific preset from the compiled registry".into());
    push(ok("      zylaxion on --preset cherryMX"));
    push(String::new());
    push(grey("  zylaxion preset --name <NAME>"));
    push("      Switch the acoustic preset live while playing.".into());
    push(ok("      zylaxion preset --name tpt"));
    push(String::new());
    push(grey("  zylaxion volume --percent <0-100>"));
    push("      Retune the master volume live (100 = the compiled default).".into());
    push(ok("      zylaxion volume --percent 60"));
    push(String::new());
    push(grey("  zylaxion off"));
    push("      Turn the sound off — the player self-exits, zero processes remain.".into());
    push(ok("      zylaxion off"));
    push(String::new());
    push(grey("  sudo zylaxion detach"));
    push("      Unpin everything — nothing of zylaxion remains in the kernel.".into());
    push(String::new());
    push(grey("  zylaxion stats"));
    push("      Kernel-side typing telemetry: per-key counters, zero userspace cost.".into());
    push(String::new());
    push(grey("  zylaxion status"));
    push("      The one-screen truth: kernel verdict, tap residency, player state.".into());
    push(String::new());

    push(brand("Diagnostics:"));
    push(String::new());
    push(grey("  zylaxion doctor"));
    push("      System health report: build facts, kernel/eBPF verdict, audio.".into());
    push("      # the first stop when audio is silent".into());
    push(ok("      zylaxion doctor"));
    push(String::new());

    push(brand("Discover:"));
    push(String::new());
    push(grey("  zylaxion list-presets"));
    push("      List the acoustic presets compiled into this binary.".into());
    push(String::new());
    push(grey("  zylaxion list-backends"));
    push("      List available audio backends.".into());
    push(String::new());
    push(grey("  zylaxion --live"));
    push("      Refresh the status overview every 2 seconds (Ctrl+C to exit).".into());
    push("      # the overview, self-refreshing".into());
    push(ok("      zylaxion --live"));
    push(String::new());

    push(brand("Global flags:"));
    push(String::new());
    for (spelling, desc) in [
        (
            "-V, --version",
            "Print complete version and build information",
        ),
        (
            "    --check-update",
            "Check the latest upstream GitHub release",
        ),
        ("-h, --help", "Print this reference"),
    ] {
        let pad = 19usize.saturating_sub(spelling.len()).max(1);
        push(format!("  {}{}{}", grey(spelling), " ".repeat(pad), desc));
    }
    push(String::new());
    push("Without a subcommand: a quick status overview.".into());
    push(String::new());
    push("License: GPL-3.0-only | https://github.com/oxyzenQ/zylaxion".into());
    out
}

/// Print the reference broken-pipe-safe (`zylaxion -h | head` must
/// truncate, never panic).
pub(crate) fn print_reference() {
    let text = render_reference(crate::output::stdout_color_enabled());
    for line in text.lines() {
        crate::output::print_stdout_line(line);
    }
}

#[cfg(test)]
#[path = "../../test/commands/help_tests.rs"]
mod tests;
