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
//! [`crate::cli_styles`] and BRANDING.md): the banner and every
//! section heading ride brand purple bold; every command synopsis and
//! every flag spelling rides calm grey (grammar, not content); every
//! runnable example rides status green (the "this is what you type"
//! tier); prose stays default. Under a pipe or `NO_COLOR` everything
//! degrades to byte-exact plain text (the stdout capability gate).

use crate::error_format::{BRAND_OPEN, GREY_OPEN, OK_OPEN, RESET};

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

    push(brand("Run:"));
    push(String::new());
    push(grey("  zylaxion start [--preset <NAME>]"));
    push("      Run in the foreground (Ctrl+C to quit).".into());
    push("      # hear it right now".into());
    push(ok("      zylaxion start"));
    push("      # a specific preset from config.toml".into());
    push(ok("      zylaxion start --preset cherry"));
    push(String::new());
    push(grey("  zylaxion daemon [--preset <NAME>] [--foreground]"));
    push("      Run as a background daemon, controlled via stop / status.".into());
    push("      --foreground stays attached for systemd user units.".into());
    push("      # background daemon, stopped with: zylaxion stop".into());
    push(ok("      zylaxion daemon"));
    push("      # the systemd user-unit shape".into());
    push(ok("      zylaxion daemon --foreground"));
    push(String::new());
    push(grey("  zylaxion stop"));
    push("      Stop a running daemon.".into());
    push(String::new());
    push(grey("  zylaxion status"));
    push("      Show daemon status.".into());
    push(String::new());

    push(brand("Diagnostics:"));
    push(String::new());
    push(grey("  zylaxion doctor"));
    push("      System health report: build facts, input group, runtime dir, audio.".into());
    push("      # the first stop when audio is silent".into());
    push(ok("      zylaxion doctor"));
    push(String::new());
    push(grey("  zylaxion testconf [FILE]"));
    push("      Validate config.toml syntax and parameter ranges.".into());
    push("      # a specific file instead of the search path".into());
    push(ok("      zylaxion testconf my.conf"));
    push(String::new());

    push(brand("Discover:"));
    push(String::new());
    push(grey("  zylaxion list-presets"));
    push("      List available acoustic presets from config.toml.".into());
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
    let text = render_reference(crate::error_format::stdout_color_enabled());
    for line in text.lines() {
        crate::error_format::print_stdout_line(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The integration-pin law (the zelynic help contract): every
    /// subcommand the CLI exposes must appear in the reference, and
    /// the group headings must hold — a new command or a reshuffle
    /// without updating this module fails the suite.
    #[test]
    fn reference_carries_every_subcommand_and_group() {
        let text = render_reference(false);
        for command in [
            "start",
            "daemon",
            "stop",
            "status",
            "doctor",
            "testconf",
            "list-presets",
            "list-backends",
        ] {
            assert!(
                text.contains(&format!("zylaxion {command}")),
                "reference must teach the command: {command}"
            );
        }
        for heading in ["Run:", "Diagnostics:", "Discover:", "Global flags:"] {
            assert!(
                text.contains(heading),
                "reference must carry the group heading: {heading}"
            );
        }
    }

    /// The tier law under color: headings purple, grammar grey,
    /// examples green; under no color: zero escape bytes anywhere
    /// (the pipe/`NO_COLOR` plain contract).
    #[test]
    fn tiers_carry_and_plain_mode_is_byte_clean() {
        let plain = render_reference(false);
        assert!(
            !plain.contains('\x1b'),
            "plain mode must carry zero escape bytes"
        );
        let painted = render_reference(true);
        for (needle, why) in [
            (BRAND_OPEN, "headings must ride purple"),
            (GREY_OPEN, "synopses must ride grey"),
            (OK_OPEN, "examples must ride green"),
        ] {
            assert!(painted.contains(needle), "{why}");
        }
        // Every open span is closed: the reset count matches the sum
        // of the three tier opens.
        let opens = painted.matches(BRAND_OPEN).count()
            + painted.matches(GREY_OPEN).count()
            + painted.matches(OK_OPEN).count();
        assert_eq!(
            painted.matches(RESET).count(),
            opens,
            "every span must close"
        );
    }

    /// The no-subcommand contract and the license footer close the
    /// reference — the same facts the old clap `after_help` carried,
    /// now on the one curated surface.
    #[test]
    fn reference_closes_with_contract_and_license() {
        let text = render_reference(false);
        assert!(
            text.contains("Without a subcommand: a quick status overview."),
            "the no-subcommand contract must be taught"
        );
        assert!(
            text.contains("License: GPL-3.0-only | https://github.com/oxyzenQ/zylaxion"),
            "the license footer must close the reference"
        );
    }
}
