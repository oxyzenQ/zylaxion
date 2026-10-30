// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Clap brand styling — the one place clap's rendered surfaces (help
//! text, usage lines, and the errors clap prints itself) pick up the
//! zylaxion palette.
//!
//! Purple brand identity: section headings (`Usage:`, `Commands:`,
//! `Options:`) render in bold truecolor purple #A855F7 — the zelynic
//! branding format the owner mandates for the product family, so every
//! purple element across the toolchain uses the exact same RGB value.
//!
//! Grey grammar law (night-improve-2, the owner's eye-strain call):
//! flags and subcommand names are typeable GRAMMAR, not content — so
//! every flag spelling (`--verbose`, `-v`, `--preset <NAME>`) and every
//! command synopsis in the `Commands:` list rides the calm grey tier
//! #8B8B8B, dimmer than the descriptions it annotates. Grey says
//! "context, not content". The same grey paints clap's suggestion
//! candidates (`valid`): a did-you-mean tip is grammar too.
//!
//! Style harmony: clap's stock styles leave error labels plain red and
//! paint nothing else with intent. These entries align the clap
//! surface with the product's semantic palette — error red #FF5A5A
//! (softer than pure alarm red), invalid-choice yellow #FFEB3C — so
//! clap-rendered and application-rendered output look like one tool.
//!
//! Depth contract: zylaxion has no --color-mode ladder (clap's auto
//! probe governs: TTY renders, pipes strip). On a 16-color terminal the
//! terminal itself approximates the RGB triplets; the brand purple and
//! the calm grey both survive that approximation. A forced 16-color
//! rung would require a color-mode surface this CLI does not offer —
//! if that surface ever lands, mirror zelynic's clap_styles_16 here.

use clap::builder::Styles;
use clap::builder::styling::{Color, Effects, RgbColor, Style};

/// Brand purple: #A855F7 (168, 85, 247) — the family branding color.
/// `pub(crate)`: the one-palette sync pin (night-dragon-lts-4 audit)
/// reads it against the output module's escape constants, so the two
/// spellings of the same contract can never drift silently.
pub(crate) const BRAND_PURPLE: RgbColor = RgbColor(168, 85, 247);

/// Calm grey: #8B8B8B (139, 139, 139) — the grammar tier. Dimmer than
/// the data it annotates; no eye strain. Rides the same sync pin as
/// [`BRAND_PURPLE`].
pub(crate) const CALM_GREY: RgbColor = RgbColor(139, 139, 139);

/// Error red: #FF5A5A (255, 90, 90) — the labeled error lane. Rides
/// the same sync pin as [`BRAND_PURPLE`].
pub(crate) const ERROR_RED: RgbColor = RgbColor(255, 90, 90);

/// Invalid yellow: #FFEB3C (255, 235, 60) — the typed-mistake tier.
/// Rides the same sync pin as [`BRAND_PURPLE`].
pub(crate) const INVALID_YELLOW: RgbColor = RgbColor(255, 235, 60);

/// The clap brand styles for every rendered surface of this CLI.
#[must_use]
pub fn styles() -> Styles {
    Styles::styled()
        .header(
            Style::new()
                .effects(Effects::BOLD)
                .fg_color(Some(Color::Rgb(BRAND_PURPLE))),
        )
        .usage(
            Style::new()
                .effects(Effects::BOLD)
                .fg_color(Some(Color::Rgb(BRAND_PURPLE))),
        )
        .literal(Style::new().fg_color(Some(Color::Rgb(CALM_GREY))))
        .placeholder(Style::new())
        .error(
            Style::new()
                .effects(Effects::BOLD)
                .fg_color(Some(Color::Rgb(ERROR_RED))),
        )
        .valid(Style::new().fg_color(Some(Color::Rgb(CALM_GREY))))
        .invalid(Style::new().fg_color(Some(Color::Rgb(INVALID_YELLOW))))
}
