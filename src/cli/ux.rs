// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

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
//! subcommand, and its one hidden verb — the `_play` player — is
//! documented on the help reference, not a rescue candidate; the ONE
//! subcommand-position rescue is the help word itself — `zylaxion
//! help` tips the reference surface it means, because the help
//! subcommand is disabled and the word would otherwise land
//! tip-less), and no shadowed-suggestion rescue (the
//! subcommand flag pools are two flags wide — a shadowing typo cannot
//! clear the 0.7 bar against them). The flag-pool fallback walks argv
//! to the failing subcommand so a subcommand-level caps typo
//! (`zylaxion preset --PRESET`) rescues from the pool the user is
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
/// names whose appearance anywhere — root or subcommand position, any
/// letter case — must tip the exact top-level spelling instead of the
/// fuzzy rescue. `--help` is the single-tier help surface (root only,
/// night-improve-15); `--check-update` is deliberately root-only
/// (`global = false`); `--version` is the intercepted brand report.
/// The lookup lowercases both sides (the rescue is for a typo'd
/// INTENT, and `--HELP` typed at `zylaxion preset` level must reach
/// the same authority `--help` reaches); the fuzzy engine is
/// intent-blind on these (a bare `v` scores
/// jaro_ci("v", "version") = 0.714 and clears the bar), this table is
/// not.
const TOP_LEVEL_FLAG_RESCUES: &[(&str, &str)] = &[
    ("help", "zylaxion --help"),
    ("h", "zylaxion --help"),
    ("version", "zylaxion -V"),
    ("V", "zylaxion -V"),
    ("check-update", "zylaxion --check-update"),
    ("check-updated", "zylaxion --check-update"),
];

/// The top-level spelling a typed flag name must be redirected to,
/// or `None` when it is not a top-level authority. Case-insensitive
/// on the typed side: the table's own spellings stay authoritative.
fn top_level_flag_rescue(typed: &str) -> Option<&'static str> {
    let needle = typed.to_lowercase();
    TOP_LEVEL_FLAG_RESCUES
        .iter()
        .find(|(name, _)| name.to_lowercase() == needle)
        .map(|(_, authority)| *authority)
}

/// Subcommand-word authorities: bare words typed in the subcommand
/// position that name no verb, yet name the help surface itself.
/// `zylaxion help` is the universal first instinct and clap answers
/// "unrecognized subcommand" with no pointer (the help subcommand is
/// disabled — night-improve-15's single-tier law); this one-row table
/// gives that instinct its redirect. The tip rides the same
/// SuggestedArg render as the flag table.
const SUBCOMMAND_WORD_RESCUES: &[(&str, &str)] = &[("help", "zylaxion --help")];

/// The subcommand-position word a typed verb must be redirected to,
/// or `None` when it is not a help-word authority.
fn subcommand_word_rescue(typed: &str) -> Option<&'static str> {
    let needle = typed.trim().to_lowercase();
    SUBCOMMAND_WORD_RESCUES
        .iter()
        .find(|(name, _)| *name == needle)
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

/// Enrich a fatal parse error with the tip clap's case-sensitive
/// engine missed.
///
/// Priority ladder (one tip, never two — the one-tip contract):
/// 1. clap's own tip stands — never duplicated, never replaced (the
///    shadowed-suggestion rescue the reference runs has no honest work
///    on a two-flag pool; see the module doc).
/// 2. the authority tables — a root-only flag typed at subcommand
///    position in any letter case (`zylaxion preset --HELP`) tips the
///    exact top-level spelling, and the help word typed in the
///    subcommand position (`zylaxion help`) tips the reference
///    surface.
/// 3. the case-insensitive Jaro fallback over the FAILING command's
///    flag pool — fires only where clap was silent, so it can never
///    suggest a flag clap would have rejected for the same lowercase
///    input. The pool is scored on the BUILT command: the derive-built
///    command has not injected clap's automatic `--version` flag until
///    `build()` runs, and scoring the unbuilt pool silently dropped
///    the version surface from every root-level rescue.
fn enrich_unknown_arg_suggestion(
    e: &mut clap::Error,
    cmd: &mut clap::builder::Command,
    argv: &[std::ffi::OsString],
) {
    if e.kind() == clap::error::ErrorKind::InvalidSubcommand {
        if e.get(ContextKind::SuggestedSubcommand).is_some()
            || e.get(ContextKind::SuggestedArg).is_some()
        {
            return; // clap fired its own tip — the last word
        }
        let typed = match e.get(ContextKind::InvalidSubcommand) {
            Some(ContextValue::String(s)) => s.clone(),
            _ => return,
        };
        if let Some(authority) = subcommand_word_rescue(&typed) {
            e.insert(
                ContextKind::SuggestedArg,
                ContextValue::String(authority.to_string()),
            );
        }
        return;
    }
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
    cmd.build();
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
    cmd.build();
    if e.kind() == clap::error::ErrorKind::DisplayVersion {
        let rendered = e.format(&mut cmd).render();
        crate::output::print_stderr_render(&rendered);
        std::process::exit(0);
    }
    enrich_unknown_arg_suggestion(&mut e, &mut cmd, &env_argv());
    let rendered = e.format(&mut cmd).render();
    crate::output::print_stderr_render(&rendered);
    // The clap render ends with a bare newline; the append closes it
    // into clap's canonical footer spacing ("\n\nFor more information…\n").
    crate::output::emit(&format!("\n{}", help_footer()));
    std::process::exit(2);
}

/// The process argv minus `argv[0]` — the walk the pool resolver scores
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
    cmd.build();
    enrich_unknown_arg_suggestion(&mut err, &mut cmd, &owned);
    let rendered = err.format(&mut cmd).render().to_string();
    format!("{rendered}\n{}\n", help_footer())
}

#[cfg(test)]
#[path = "../../test/cli/ux_tests.rs"]
mod tests;
