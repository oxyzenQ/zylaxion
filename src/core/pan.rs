// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The stereo pan table (night-improve-23: split out of core/mod.rs
//! per the 600-LOC no-mercy cap — a pure, self-contained mapping with
//! no dependencies on the orchestrator's state).

/// Map a Linux evdev scancode to a stereo pan position.
///
/// Standard QWERTY keyboards have roughly 15 columns.  Scancodes are
/// not contiguous across rows, so we use a simple heuristic: remap the
/// scancode into a 0–1 range based on common key columns, then shift
/// to [-1, 1].
///
/// | Region (scancodes)          | Position |
/// |----------------------------|----------|
/// | 1–14 (Escape, F1–F10…)     | far left → center-left |
/// | 16–27 (QWERTY row left)     | left → center |
/// | 30–41, 43–53 (main rows)   | full spread |
/// | 57–69 (right cluster)      | center-right → right |
///
/// The mapping is deliberately simple — it just needs to produce
/// audible stereo separation, not a physically accurate model.
pub(super) fn scancode_to_pan(scancode: u32) -> f32 {
    // Approximate column index: most alphanumeric keys are in the
    // range 2–69.  Normalise to 0..1 then shift to -1..1.
    let column = match scancode {
        // Row 1: Escape + F-keys (far left to center-left)
        1 => 0.0,
        // Row 1–2: Escape + F-keys + number row (scancodes 2–14)
        2..=14 => (scancode - 2) as f32 / 12.0,
        // Row 3: QWERTY row
        16..=27 => (scancode - 16) as f32 / 11.0,
        // Row 4: ASDF row
        30..=41 => (scancode - 30) as f32 / 11.0,
        // Row 5: ZXCV row
        42..=53 => (scancode - 42) as f32 / 11.0,
        // Row 6: bottom row (Shift=42.., Ctrl=29/97, Alt=56/100, Space=57)
        57 => 0.5, // Space — center
        // v10.2.0 (dragonzen audit B16): navigation cluster
        // (PrintScreen, ScrollLock, Pause, Insert, Home, PgUp, Delete,
        // End, PgDn, arrows) — scancodes 70–89. These physically sit
        // right-of-center on standard layouts, so pan them center-
        // right (0.6). Previously fell to the default 0.5 (center),
        // which made arrow keys sound like they're coming from the
        // middle of the keyboard — wrong.
        //
        // Note: scancodes 86 (Numpad-), 87 (F11), 88 (F12), 89 (Numpad=)
        // overlap with the numpad/F-key cluster below — they get the
        // nav-cluster pan (0.6) which is fine since they're also
        // right-of-center.
        70..=89 => 0.6,
        // Right-side numpad cluster — scancodes 90-100. The numpad
        // physically sits at the far right of standard layouts.
        90..=100 => 0.7,
        // Everything else: center
        _ => 0.5,
    };

    // Map [0, 1] → [-1, 1] with slight bias toward center for a
    // more natural stereo image.
    (column - 0.5) * 2.0
}
