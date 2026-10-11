// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI UX contract — the single authority for fatal clap error
//! rendering (the zelynic `cli/ux.rs` lineage, ported in
//! night-improve-19 in the shape zylaxion's surface needs).
//!
//! Exit-code contract: clap usage errors exit 2; runtime failures
//! exit 1 through the application's own [`crate::output::error`] lane;
//! the version surface exits 0. Every fatal CLI path renders through
//! this module so all layers end identically — with exactly one
//! canonical `For more information, try '--help'.` footer. clap cannot
//! render that footer itself (the footer key comes from an
//! ArgAction::Help argument; zylaxion's `--help` is a parsed field per
//! the single-tier help law, night-improve-15), so the bridge appends
//! the canonical wording itself — exactly once, never duplicated.
//!
//! Rendering: clap errors are re-rendered with the command's brand
//! styles (purple headers/usage, red error label, grey valid tips —
//! see [`crate::cli::styles`]) and routed through the same capability
//! gate as every other surface: plain bytes when the stderr gate is
//! off, clap's ANSI bytes otherwise — one color law. The closest-match
//! machinery lives in [`crate::cli::suggestion`].
//!
//! Honest deltas from the zelynic reference (documented, not silent):
//! no escape-hatch probe (zylaxion's subcommands carry no positionals,
//! so clap never injects the "to pass as a value" tip), no removed-
//! subcommand or hidden-vocabulary rescues (zylaxion renamed no
//! subcommand and hides none), and no shadowed-suggestion rescue (the
//! subcommand flag pools are two flags wide — a shadowing typo cannot
//! clear the 0.7 bar against them). The flag-pool fallback walks argv
//! to the failing subcommand so a subcommand-level caps typo
//! (`zylaxion start --PRESET`) rescues from the pool the user is
//! actually in — the root-only pool the reference scores from would
//! miss it.

use clap::CommandFactory;
use clap::error::{ContextKind, ContextValue};

use super::suggestion::closest_long_flag_ci;

/// Canonical clap help-footer wording — appended by the bridge because
/// clap's formatter skips it without an ArgAction::Help argument.
fn help_footer() -> &'static str {
    "For more information, try '--help'."
}

/// Top-level-authority flags (the zelynic NIGHT-boost-12 lineage):
/// names whose subcommand-position appearance must tip the top-level
/// spelling instead of the fuzzy rescue. `--help` is the single-tier
/// help surface (root only, night-improve-15); `--check-update` and
/// `--live` are deliberately root-only (`global = false`); `--version`
/// is the intercepted brand report. The fuzzy engine is intent-blind
/// on these (`jaro_ci("V", "live")` clears the bar); this table is
/// not.
const TOP_LEVEL_FLAG_RESCUES: &[(&str, &str)] = &[
    ("help", "zylaxion --help"),
    ("h", "zylaxion --help"),
    ("version", "zylaxion -V"),
    ("V", "zylaxion -V"),
    ("check-update", "zylaxion --check-update"),
    ("check-updated", "zylaxion --check-update"),
    ("live", "zylaxion --live"),
];

/// The top-level spelling a typed flag name must be redirected to,
/// or `None` when it is not a top-level authority.
fn top_level_flag_rescue(typed: &str) -> Option<&'static str> {
    TOP_LEVEL_FLAG_RESCUES
        .iter()
        .find(|(name, _)| *name == typed)
        .map(|(_, authority)| *authority)
}

/// The long-flag pool of the command argv actually addresses: the root
/// pool by default, the named subcommand's pool once argv names one
/// (zylaxion has one subcommand level, no nesting).
fn failing_command_pool<'a>(
    cmd: &'a clap::builder::Command,
    argv: &[std::ffi::OsString],
) -> Vec<&'a str> {
    let mut level = cmd;
    for tok in argv {
        let tok = tok.to_string_lossy().to_string();
        if tok.starts_with('-') {
            break; // flags never name a subcommand
        }
        match level.find_subcommand(&tok) {
            Some(sub) => level = sub,
            None => break,
        }
    }
    level
        .get_arguments()
        .filter(|arg| !arg.is_hide_set())
        .filter_map(|arg| arg.get_long())
        .collect()
}

/// Enrich an UnknownArgument error with the tip clap's case-sensitive
/// engine missed.
///
/// Priority ladder (one tip, never two — the one-tip contract):
/// 1. clap's own tip stands — never duplicated, never replaced (the
///    shadowed-suggestion rescue the reference runs has no honest work
///    on a two-flag pool; see the module doc).
/// 2. the top-level authority table — a root-only flag typed at
///    subcommand position (`zylaxion start --help`) tips the exact
///    top-level spelling.
/// 3. the case-insensitive Jaro fallback over the FAILING command's
///    flag pool — fires only where clap was silent, so it can never
///    suggest a flag clap would have rejected for the same lowercase
///    input.
fn enrich_unknown_arg_suggestion(
    e: &mut clap::Error,
    cmd: &clap::builder::Command,
    argv: &[std::ffi::OsString],
) {
    if e.kind() != clap::error::ErrorKind::UnknownArgument {
        return;
    }
    if e.get(ContextKind::SuggestedArg).is_some() {
        return; // clap fired its own tip — the last word
    }
    let typed = match e.get(ContextKind::InvalidArg) {
        Some(ContextValue::String(s)) => s.trim_start_matches('-').to_string(),
        _ => return,
    };
    if typed.is_empty() {
        return;
    }
    if let Some(authority) = top_level_flag_rescue(&typed) {
        e.insert(
            ContextKind::SuggestedArg,
            ContextValue::String(authority.to_string()),
        );
        return;
    }
    let candidates = failing_command_pool(cmd, argv);
    if let Some(best) = closest_long_flag_ci(&typed, &candidates) {
        e.insert(
            ContextKind::SuggestedArg,
            ContextValue::String(format!("--{best}")),
        );
    }
}

/// Render a clap parse error in the canonical zylaxion shape and exit.
///
/// Every error kind renders through the brand styles and the shared
/// capability gate (plain when the stderr gate is off, clap's ANSI
/// bytes otherwise), broken-pipe-safe. Usage errors end with exactly
/// one canonical footer and exit 2; the version surface keeps clap's
/// exit 0 (its body is the shared version report, it carries no
/// footer). Broken pipe is swallowed, matching clap's own exit()
/// behavior.
pub fn exit_clap_error(e: clap::Error) -> ! {
    let mut e = e;
    let mut cmd = super::Cli::command();
    if e.kind() == clap::error::ErrorKind::DisplayVersion {
        let rendered = e.format(&mut cmd).render();
        crate::output::print_stderr_render(&rendered);
        std::process::exit(0);
    }
    enrich_unknown_arg_suggestion(&mut e, &cmd, &env_argv());
    let rendered = e.format(&mut cmd).render();
    crate::output::print_stderr_render(&rendered);
    // The clap render ends with a bare newline; the append closes it
    // into clap's canonical footer spacing ("\n\nFor more information…\n").
    crate::output::emit(&format!("\n{}", help_footer()));
    std::process::exit(2);
}

/// The process argv minus argv[0] — the walk the pool resolver scores
/// (production entry; the test harness passes synthetic argv instead).
fn env_argv() -> Vec<std::ffi::OsString> {
    std::env::args_os().skip(1).collect()
}

// The bridge pins live in this module (zylaxion's LOC lane is the
// 600-line cap — one theme, one file, the zelynic test/ split is the
// debt-lane shape for when these outgrow it). render_via_bridge
// mirrors exit_clap_error minus the process::exit so render-level
// contracts are testable: the plain-text clap render (which ends with
// a bare newline) followed by the bridge's manual footer append —
// reproducing clap's canonical "\n<footer>\n" tail. The test process
// runs in the Mono lane (stderr captured), so pins hold byte-for-byte
// in every lane: text identical, only the escape bytes differ.
#[cfg(test)]
fn render_via_bridge(argv: &[&str]) -> String {
    use clap::Parser as _;
    // match, not expect_err: Cli carries no Debug derive (the parse
    // surface never needs one), and the harness must not demand it.
    let mut err = match super::Cli::try_parse_from(argv) {
        Err(e) => e,
        Ok(_) => panic!("argv must fail to parse: {argv:?}"),
    };
    // argv[0] stripped: production scores env_argv() (skip(1)); the
    // harness holds the same contract with synthetic argv.
    let owned: Vec<std::ffi::OsString> =
        argv.iter().skip(1).map(std::ffi::OsString::from).collect();
    let mut cmd = super::Cli::command();
    enrich_unknown_arg_suggestion(&mut err, &cmd, &owned);
    let rendered = err.format(&mut cmd).render().to_string();
    format!("{rendered}\n{}\n", help_footer())
}

#[cfg(test)]
#[path = "../../test/cli/ux_tests.rs"]
mod tests;
