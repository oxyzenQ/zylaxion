// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Centralised error/warning formatting and the semantic color tiers.
//!
//! All user-facing error and warning messages go through these helpers
//! to guarantee a consistent prefix and one palette:
//!
//!   - errors:   `zylaxion: error: <message>` — bold red label, red body
//!   - warnings: `zylaxion: warning: <message>` — bold yellow label, yellow body
//!
//! This matches the GNU coding-standard convention used by `gcc`,
//! `clang`, `make`, etc., and makes it trivial for users to `grep`
//! zylaxion's output for errors.
//!
//! # The palette (night-improve-6, the family contract)
//!
//! The zelynic owner color contract, shared across the toolchain so
//! clap-rendered and application-rendered output look like one tool:
//!
//!   - brand purple `#A855F7` — section headers (the clap surface paints
//!     its `Usage:`/`Options:` headings with the same value)
//!   - error red `#FF5A5A` — the labeled error lane (softer than pure
//!     alarm red, kept readable on black)
//!   - warning yellow `#FFEB3C` — the warning lane
//!   - calm grey `#8B8B8B` — the subordinate tier: passive suggestions
//!     (`tip:`, `hint:`, `fix:`, possible-value lists) inside an error
//!     block explain instead of alarming; the same value the clap
//!     surface paints its suggestion candidates with
//!   - status green `#50FA7B` — affirmative verdicts and runnable tips
//!     (a suggestion that quotes a command to type is an action, not
//!     more of the red block it lives in)
//!
//! # Capability gate
//!
//! One gate, no depth ladder (the contract `crate::cli::styles` documents for
//! the clap surface): color renders only when stderr is a terminal,
//! `NO_COLOR` is unset and `CLICOLOR != 0`. Truecolor escapes ride the
//! same approximation rule as clap's probe — a 16-color terminal
//! approximates the RGB triplets itself. Every emitter is
//! broken-pipe-safe: a failed stderr write is dropped, never a panic
//! (the zelynic `eprintln_safe` semantic).

use std::io::{IsTerminal, Write};

/// Error red `#FF5A5A` — bold label prefix, red body.
const ERROR_OPEN: &str = "\x1b[38;2;255;90;90m";
/// Error red, bold — the label token itself.
const ERROR_BOLD_OPEN: &str = "\x1b[1;38;2;255;90;90m";
/// Warning yellow `#FFEB3C` — bold label prefix, yellow body.
const WARN_OPEN: &str = "\x1b[38;2;255;235;60m";
/// Warning yellow, bold — the label token itself.
const WARN_BOLD_OPEN: &str = "\x1b[1;38;2;255;235;60m";
/// Brand purple `#A855F7`, bold — section headers.
pub(crate) const BRAND_OPEN: &str = "\x1b[1;38;2;168;85;247m";
/// Calm grey `#8B8B8B` — the passive-suggestion tier.
pub(crate) const GREY_OPEN: &str = "\x1b[38;2;139;139;139m";
/// Status green `#50FA7B` — affirmative verdicts and runnable tips.
pub(crate) const OK_OPEN: &str = "\x1b[38;2;80;250;123m";
/// Brand purple `#A855F7`, regular weight — the version-report header
/// weight (the cosmostrix `version_report()` parity; the bold variant
/// above stays the section-banner weight).
const BRAND_REGULAR_OPEN: &str = "\x1b[38;2;168;85;247m";
pub(crate) const RESET: &str = "\x1b[0m";

/// Whether stderr may carry color: a TTY, `NO_COLOR` unset, and
/// `CLICOLOR != 0` (checked once per call — these paths are cold).
/// Exposed crate-wide so the clap error bridge ([`crate::cli::ux`])
/// routes its rendered bytes through the SAME gate — one color law.
pub(crate) fn color_enabled() -> bool {
    if !std::io::stderr().is_terminal() {
        return false;
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    match std::env::var("CLICOLOR") {
        Ok(v) => v != "0",
        Err(_) => true,
    }
}

/// Broken-pipe-safe stderr write: a failed write is dropped, never a
/// panic (the zelynic `eprintln_safe` semantic).
pub(crate) fn emit(text: &str) {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(text.as_bytes());
    let _ = err.write_all(b"\n");
}

/// A raw clap-rendered error block on stderr, through the SAME
/// capability gate as every other surface: plain bytes when the gate
/// is off, clap's ANSI bytes otherwise. Broken-pipe-safe; no newline
/// added — the caller owns the block's exact tail (the canonical
/// footer spacing).
pub(crate) fn print_stderr_render(rendered: &clap::builder::StyledStr) {
    let text = if color_enabled() {
        rendered.ansi().to_string()
    } else {
        rendered.to_string()
    };
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(text.as_bytes());
}

/// Print an error message to stderr in the standard format.
///
/// Format: `zylaxion: error: <message>\n` — bold red label, red body.
/// Suggestion lines inside a multi-line body (`tip:`, `hint:`, `fix:`,
/// possible-value lists, did-you-mean) ride the calm-grey tier; a tip
/// quoting a runnable command (`'zylaxion ...`) rides the status-green
/// tier.
#[inline]
pub fn error(msg: impl AsRef<str>) {
    emit_labeled("error", ERROR_BOLD_OPEN, ERROR_OPEN, msg.as_ref());
}

/// Print a warning message to stderr in the standard format.
///
/// Format: `zylaxion: warning: <message>\n` — bold yellow label,
/// yellow body, the same suggestion-line classification as errors.
#[inline]
pub fn warning(msg: impl AsRef<str>) {
    emit_labeled("warning", WARN_BOLD_OPEN, WARN_OPEN, msg.as_ref());
}

/// The labeled lane renderer. Line 0 carries the GNU label; the body
/// is wrapped per line — suggestion lines demote to the calm tier,
/// runnable tips promote to green, everything else rides the lane
/// color. Mono mode reproduces the exact plain prefix byte-for-byte.
fn emit_labeled(label: &str, label_open: &str, body_open: &str, msg: &str) {
    if !color_enabled() {
        emit(&format!("zylaxion: {label}: {msg}"));
        return;
    }
    let mut out = String::with_capacity(msg.len() + 64);
    for (i, line) in msg.lines().enumerate() {
        if i == 0 {
            out.push_str(&format!(
                "{label_open}zylaxion: {label}:{RESET} {body_open}{line}{RESET}"
            ));
        } else {
            out.push_str(&wrap_body_line(line, body_open));
        }
        out.push('\n');
    }
    out.pop(); // no trailing newline — emit adds it
    emit(&out);
}

/// Classify and color one continuation line of a labeled block.
fn wrap_body_line(line: &str, body_open: &str) -> String {
    let t = line.trim_start();
    let is_suggestion = t.starts_with("tip:")
        || t.starts_with("hint:")
        || t.starts_with("fix:")
        || t.starts_with("[possible values")
        || t.starts_with("(possible values")
        || t.starts_with("did you mean");
    let is_runnable = (t.contains("'zylaxion ") || t.contains("'cargo ")) && is_suggestion;
    if is_runnable {
        format!("{OK_OPEN}{line}{RESET}")
    } else if is_suggestion {
        format!("{GREY_OPEN}{line}{RESET}")
    } else {
        format!("{body_open}{line}{RESET}")
    }
}

// ── verdict and tier helpers (the doctor/overview surface) ──────────────
//
// The lane law (night-hunter-4): these helpers decorate STDOUT report
// lines — the doctor's verdicts, the overview's posture, the preset
// listing's marker — so they gate on the STDOUT capability probe, not
// the stderr one. The two gates disagree exactly when one stream is a
// terminal and the other is not (`zylaxion list-presets | grep topre`
// on an interactive shell): the old stderr gate leaked ANSI bytes into
// the pipe; the stdout gate keeps pipes byte-identical plain. The
// stderr lane (error/warning/emit_labeled) keeps its own probe — one
// gate per stream, the same rule both probe bodies implement.

/// An affirmative verdict line (the caller supplies the word —
/// `PASS: ...`) — status green.
pub fn ok_line(text: &str) -> String {
    if !stdout_color_enabled() {
        return text.to_string();
    }
    format!("{OK_OPEN}{text}{RESET}")
}

/// A failed verdict line (the caller supplies the word — `FAIL: ...`)
/// — error red.
pub fn fail_line(text: &str) -> String {
    if !stdout_color_enabled() {
        return text.to_string();
    }
    format!("{ERROR_OPEN}{text}{RESET}")
}

/// A caution verdict line (night-hunter-4) — warning yellow. A finding
/// that is not a failure: the stale player pid in the map, an absent
/// optional capability. Same lane law as [`ok_line`] — stdout-gated.
pub fn warn_line(text: &str) -> String {
    if !stdout_color_enabled() {
        return text.to_string();
    }
    format!("{WARN_OPEN}{text}{RESET}")
}

/// The report banner: the family's one title-line shape, the sibling
/// zejtron's banner law — `━━━ <title> ━━━` in brand purple bold.
/// Every report surface opens with this exact frame (the doctor, the
/// overview, the update check, the catalogs, every future report), so
/// the family reads as one family — one line, one purple, one frame.
pub fn section(text: &str) -> String {
    if !stdout_color_enabled() {
        return format!("━━━ {text} ━━━");
    }
    format!("{BRAND_OPEN}━━━ {text} ━━━{RESET}")
}

/// One key/value report row — the zelynic geometry, byte-identical to
/// the zelynic doctor reference: two-space indent, the key (colon
/// included) left-padded to 11 columns, one space, then the value, so
/// every report surface's values align at the same column. The value
/// arrives pre-decorated ([`ok_line`]/[`fail_line`]/[`tip_line`]/
/// [`warn_line`] or plain); mono mode is byte-identical plain.
pub fn kv(key: &str, value: &str) -> String {
    format!("  {key:<11} {value}")
}

/// A report-level warning row (the zelynic shape): `  ! text`, warning
/// yellow on a color-capable stdout — the row the report's trailing
/// `Warnings:` block carries. Mono mode is byte-identical plain.
pub fn warn_bullet(text: &str) -> String {
    format!("  {}", warn_line(&format!("! {text}")))
}

/// A passive suggestion line (`fix: ...`) — calm grey.
pub fn tip_line(text: &str) -> String {
    if !stdout_color_enabled() {
        return text.to_string();
    }
    format!("{GREY_OPEN}{text}{RESET}")
}

/// An affirmative inline marker (` <- active`) — status green.
pub fn ok_marker(text: &str) -> String {
    ok_line(text)
}

// ── the stdout report lane (night-improve-13, the zelynic
//    print_version_report lineage) ────────────────────────────────────
//
// The version report writes to STDOUT (it is the command's product,
// not a diagnostic). Same palette, same capability rule, one scope
// change: the gate probes stdout instead of stderr.

/// Whether stdout may carry color: a TTY, `NO_COLOR` unset, and
/// `CLICOLOR != 0` (the exact stderr rule, stdout-scoped). Exposed
/// crate-wide so curated stdout surfaces (the help reference) gate
/// their spans on the same rule.
pub(crate) fn stdout_color_enabled() -> bool {
    if !std::io::stdout().is_terminal() {
        return false;
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    match std::env::var("CLICOLOR") {
        Ok(v) => v != "0",
        Err(_) => true,
    }
}

/// Broken-pipe-safe stdout write + newline: a failed write is dropped,
/// never a panic (the zelynic `println_safe` semantic — `zylaxion -V |
/// head -1` must not kill the process with SIGPIPE-by-panic).
pub fn print_stdout_line(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.write_all(b"\n");
}

/// A single-line brand span — brand purple `#A855F7`, regular weight
/// (the cosmostrix `version_report()` parity, night-dragon-maintenance-T1):
/// the version report's header lines each ride their OWN wrap, never one
/// wrap around a joined multi-line header — a multi-line span paints
/// every line after the first with it (the zelynic
/// night-dragon-maintenance-8 field report), so the helper refuses one
/// in debug builds. Plain text when stdout is not a color-capable
/// terminal.
pub fn brand(text: &str) -> String {
    debug_assert!(
        !text.contains('\n'),
        "color helpers wrap single lines only — a multi-line msg paints \
         every line after the first with it; split the lines and wrap each"
    );
    if !stdout_color_enabled() {
        return text.to_string();
    }
    format!("{BRAND_REGULAR_OPEN}{text}{RESET}")
}

#[cfg(test)]
#[path = "../../test/output/output_tests.rs"]
mod tests;
