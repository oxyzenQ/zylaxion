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
//! The Pro mode section (night-improve-25, the zelynic
//! NIGHT-improve-45 lineage) extends the same law to the hidden
//! estate: one computed description column, flag spellings grey,
//! environment rows default prose (an environment variable is not a
//! flag), examples green.

use crate::output::{BRAND_OPEN, GREY_OPEN, OK_OPEN, RESET};

/// Render the end-to-end reference: usage, commands grouped by role,
/// examples, global flags, the Pro mode estate (short aliases, the
/// environment law, the hidden verb), and the no-subcommand contract.
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
    push("your speakers — the eBPF tap in, low-latency ALSA audio out.".into());
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
    push(grey("  zylaxion preset --switch <NAME>"));
    push("      Switch the acoustic preset live while playing.".into());
    push(ok("      zylaxion preset --switch topre"));
    push(String::new());
    push(grey("  zylaxion volume --percent <0-200>"));
    push(
        "      Retune the master volume live (100 = the compiled default; 200 = the \
         safety cap; --force-this overrides up to 10000)."
            .into(),
    );
    push(ok("      zylaxion volume --percent 60"));
    push(String::new());
    push(grey("  zylaxion off"));
    push("      Turn the sound off — the player self-exits, zero processes remain.".into());
    push(ok("      zylaxion off"));
    push(String::new());
    push(grey("  sudo zylaxion detach"));
    push("      Unpin everything — nothing of zylaxion remains in the kernel.".into());
    push(String::new());
    push(grey("  zylaxion status [--verbose]"));
    push("      The one-screen truth: kernel verdict, tap residency, player state.".into());
    push("      # the deep read appends the kernel-side typing telemetry".into());
    push(ok("      zylaxion status --verbose"));
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

    // night-improve-25 (the zelynic NIGHT-improve-45 lineage): the
    // Pro mode estate — every hidden and advanced spelling documented
    // COMPLETELY on the one reference surface, grouped by the lane
    // that owns it. Geometry (the hunt-35 law, adapted): ONE
    // description column across every group and its continuations,
    // COMPUTED from the estate's longest spelling so it cannot drift
    // per row. Tier law: flag spellings and the verb synopsis ride
    // grey (grammar); the environment rows ride default prose — an
    // environment variable is not a flag (the hunt-39 boundary);
    // runnable examples ride green, and every example spelling is new
    // to the surface (the no-duplicate law — the Run section owns the
    // long spellings, Pro mode owns the short ones).
    let aliases: &[(&str, &str)] = &[
        (
            "-s, --switch <NAME>",
            "preset's live switch, the short spelling",
        ),
        (
            "-p, --percent <0-200>",
            "volume's live retune, the short spelling",
        ),
        ("-v, --verbose", "status's deep read, the short spelling"),
    ];
    let environment: &[(&str, &str)] = &[
        (
            "ZYLAXION_PRESET=<name>",
            "the persistent preset `zylaxion on` starts from",
        ),
        (
            "ZYLAXION_VOLUME=<gain>",
            "the persistent output gain, linear — 5.5 laptop\nspeakers, 1.5 headphones; a different scale\nthan the percent lane, capped at 10.0",
        ),
        ("NO_COLOR=1", "plain bytes on every surface, always"),
        (
            "RUST_LOG=<filter>",
            "the log lane: info on attach and the\nplayer, warn on the rest",
        ),
    ];
    let hidden_verbs: &[(&str, &str)] = &[(
        "zylaxion _play",
        "the internal stateless player behind the\non verb; it owns no flags of its own —\nrun it in the foreground for the full\nerror when it dies at startup",
    )];
    let pro_col = aliases
        .iter()
        .chain(environment)
        .chain(hidden_verbs)
        .map(|(spelling, _)| spelling.len())
        .max()
        .unwrap_or(0)
        + 2;
    // A plain fn over `push` (the span precedent — no closure
    // captures, so the section's direct pushes interleave freely).
    fn pro_row(
        push: &mut dyn FnMut(String),
        pro_col: usize,
        painted: &str,
        spelling: &str,
        desc: &str,
    ) {
        let pad = pro_col.saturating_sub(spelling.len()).max(1);
        let mut lines = desc.split('\n');
        let first = lines.next().unwrap_or("");
        push(format!("    {painted}{}{}", " ".repeat(pad), first));
        let cont = " ".repeat(4 + pro_col);
        for line in lines {
            push(format!("{cont}{line}"));
        }
    }
    push(brand("Pro mode:"));
    push(String::new());
    push("  The hidden and advanced vocabulary, complete — grouped by".into());
    push("  the lane that owns it.".into());
    push(String::new());
    push("  Short aliases:".into());
    push(String::new());
    for (spelling, desc) in aliases {
        pro_row(&mut push, pro_col, &grey(spelling), spelling, desc);
    }
    push(String::new());
    push("      # the short spellings in action".into());
    push(ok("      zylaxion preset -s topre"));
    push(ok("      zylaxion volume -p 60"));
    push(ok("      zylaxion status -v"));
    push(String::new());
    push("  Environment (the no-config law):".into());
    push(String::new());
    for (spelling, desc) in environment {
        pro_row(&mut push, pro_col, spelling, spelling, desc);
    }
    push(String::new());
    push("      # the headphones gain, every start".into());
    push(ok("      ZYLAXION_VOLUME=1.5 zylaxion on"));
    push(String::new());
    push("  The hidden verb:".into());
    push(String::new());
    for (spelling, desc) in hidden_verbs {
        pro_row(&mut push, pro_col, &grey(spelling), spelling, desc);
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
